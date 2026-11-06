use std::collections::{HashMap, HashSet};
use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use libloading::os::windows::Library;

const STEAM_API_DLL: &str = "steam_api64.dll";
const MAX_CONTROLLERS: usize = 16;
const MAX_ORIGINS: usize = 8;

pub type InputHandle = u64;
pub const ALL_CONTROLLERS: InputHandle = u64::MAX;
type Iface = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct DigitalActionData {
    pub state: bool,
    pub active: bool,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct AnalogActionData {
    pub mode: i32,
    pub x: f32,
    pub y: f32,
    pub active: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TriggerCommand {
    pub mode: i32,
    _pad: [u8; 4],
    pub data: [u8; 12],
}

impl TriggerCommand {
    pub fn new(mode: i32, params: &[u8]) -> Self {
        let mut data = [0; 12];
        data[..params.len()].copy_from_slice(params);
        TriggerCommand { mode, _pad: [0; 4], data }
    }
}

#[repr(C)]
struct TriggerEffectParam {
    mask: u8,
    _pad: [u8; 7],
    commands: [TriggerCommand; 2],
}

const _: () = assert!(std::mem::size_of::<TriggerEffectParam>() == 48);

pub const LEFT: usize = 0;
pub const RIGHT: usize = 1;
pub const LEFT_TRIGGER: usize = 2;
pub const RIGHT_TRIGGER: usize = 3;

#[derive(Clone, Copy, Default)]
struct Motor {
    level: u16,
    until: Option<Instant>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum HandleKind {
    ActionSet,
    Digital,
    Analog,
}

unsafe fn sym<T: Copy>(lib: &Library, name: &str) -> Result<T, String> {
    lib.get::<T>(name)
        .map(|s| *s)
        .map_err(|e| format!("missing {name} in {STEAM_API_DLL}: {e}"))
}

macro_rules! steam_fns {
    ($($field:ident = $name:literal: fn($($arg:ty),*) $(-> $ret:ty)?;)*) => {
        struct Fns {
            $($field: unsafe extern "C" fn(Iface $(, $arg)*) $(-> $ret)?,)*
        }

        impl Fns {
            unsafe fn load(lib: &Library) -> Result<Self, String> {
                Ok(Fns { $($field: sym(lib, concat!("SteamAPI_ISteamInput_", $name))?,)* })
            }
        }
    };
}

steam_fns! {
    init = "Init": fn(bool) -> bool;
    shutdown = "Shutdown": fn() -> bool;
    run_frame = "RunFrame": fn(bool);
    connected_controllers = "GetConnectedControllers": fn(*mut InputHandle) -> i32;
    input_type = "GetInputTypeForHandle": fn(InputHandle) -> i32;
    trigger_vibration = "TriggerVibration": fn(InputHandle, u16, u16);
    trigger_vibration_extended = "TriggerVibrationExtended": fn(InputHandle, u16, u16, u16, u16);
    simple_haptic_event = "TriggerSimpleHapticEvent": fn(InputHandle, i32, u8, i8, u8, i8);
    dualsense_trigger_effect = "SetDualSenseTriggerEffect": fn(InputHandle, *const TriggerEffectParam);
    set_led_color = "SetLEDColor": fn(InputHandle, u8, u8, u8, u32);
    show_binding_panel = "ShowBindingPanel": fn(InputHandle) -> bool;
    set_manifest_path = "SetInputActionManifestFilePath": fn(*const c_char) -> bool;
    action_set_handle = "GetActionSetHandle": fn(*const c_char) -> u64;
    digital_action_handle = "GetDigitalActionHandle": fn(*const c_char) -> u64;
    analog_action_handle = "GetAnalogActionHandle": fn(*const c_char) -> u64;
    activate_action_set = "ActivateActionSet": fn(InputHandle, u64);
    digital_action_data = "GetDigitalActionData": fn(InputHandle, u64) -> DigitalActionData;
    analog_action_data = "GetAnalogActionData": fn(InputHandle, u64) -> AnalogActionData;
    digital_action_origins = "GetDigitalActionOrigins": fn(InputHandle, u64, u64, *mut i32) -> i32;
    analog_action_origins = "GetAnalogActionOrigins": fn(InputHandle, u64, u64, *mut i32) -> i32;
    glyph_png = "GetGlyphPNGForActionOrigin": fn(i32, i32, u32) -> *const c_char;
    origin_name = "GetStringForActionOrigin": fn(i32) -> *const c_char;
}

pub struct SteamInput {
    _lib: Library,
    iface: Iface,
    fns: Fns,
    handles: HashMap<(HandleKind, String), u64>,
    motors: HashMap<InputHandle, [Motor; 4]>,
    trigger_motors_on: HashSet<InputHandle>,
    trigger_effects_until: HashMap<(InputHandle, usize), Instant>,
}

unsafe impl Send for SteamInput {}

static STEAM_INPUT: Mutex<Option<SteamInput>> = Mutex::new(None);

unsafe fn owned_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
    (!s.is_empty()).then_some(s)
}

impl SteamInput {
    unsafe fn open() -> Result<Self, String> {
        let lib = Library::open_already_loaded(STEAM_API_DLL)
            .map_err(|e| format!("failed to open {STEAM_API_DLL}: {e}"))?;
        let accessor: unsafe extern "C" fn() -> Iface = sym(&lib, "SteamAPI_SteamInput_v006")?;
        let fns = Fns::load(&lib)?;

        let iface = accessor();
        if iface.is_null() {
            return Err("SteamInput006 interface is unavailable".into());
        }
        if !(fns.init)(iface, true) {
            return Err("ISteamInput::Init failed".into());
        }

        Ok(SteamInput { _lib: lib, iface, fns, handles: HashMap::new(), motors: HashMap::new(), trigger_motors_on: HashSet::new(), trigger_effects_until: HashMap::new() })
    }

    pub fn run_frame(&mut self) {
        let now = Instant::now();
        let expired = |until: Option<Instant>| until.is_some_and(|t| t <= now);

        let handles: Vec<_> = self.motors.iter().filter(|(_, m)| m.iter().any(|m| expired(m.until))).map(|(&h, _)| h).collect();
        for handle in handles {
            if let Some(motors) = self.motors.get_mut(&handle) {
                motors.iter_mut().filter(|m| expired(m.until)).for_each(|m| *m = Motor::default());
            }
            self.send_motors(handle);
        }

        let effects: Vec<_> = self.trigger_effects_until.iter().filter(|(_, &t)| t <= now).map(|(&k, _)| k).collect();
        for (handle, trigger) in effects {
            self.set_trigger_effect(handle, trigger, TriggerCommand::default(), None);
        }

        unsafe { (self.fns.run_frame)(self.iface, true) }
    }

    pub fn connected_controllers(&self) -> Vec<InputHandle> {
        let mut handles = [0; MAX_CONTROLLERS];
        let count = unsafe { (self.fns.connected_controllers)(self.iface, handles.as_mut_ptr()) };
        handles[..count.clamp(0, MAX_CONTROLLERS as i32) as usize].to_vec()
    }

    pub fn input_type(&self, handle: InputHandle) -> i32 {
        unsafe { (self.fns.input_type)(self.iface, handle) }
    }

    pub fn set_motors(&mut self, handle: InputHandle, levels: [Option<u16>; 4], seconds: Option<f64>) {
        let until = seconds.filter(|s| *s > 0.0).map(|s| Instant::now() + Duration::from_secs_f64(s));
        let motors = self.motors.entry(handle).or_default();
        for (motor, level) in motors.iter_mut().zip(levels) {
            if let Some(level) = level {
                *motor = Motor { level, until: until.filter(|_| level > 0) };
            }
        }
        self.send_motors(handle);
    }

    fn send_motors(&mut self, handle: InputHandle) {
        let levels = self.motors.get(&handle).copied().unwrap_or_default().map(|m| m.level);
        let [l, r, lt, rt] = levels;
        let triggers_on = lt > 0 || rt > 0;
        let use_extended = triggers_on || self.trigger_motors_on.contains(&handle);
        match triggers_on {
            true => self.trigger_motors_on.insert(handle),
            false => self.trigger_motors_on.remove(&handle),
        };
        unsafe {
            match use_extended {
                true => (self.fns.trigger_vibration_extended)(self.iface, handle, l, r, lt, rt),
                false => (self.fns.trigger_vibration)(self.iface, handle, l, r),
            }
        }
        if levels == [0; 4] {
            self.motors.remove(&handle);
        }
    }

    pub fn simple_haptic_event(&self, handle: InputHandle, location: i32, intensity: u8, gain_db: i8) {
        unsafe { (self.fns.simple_haptic_event)(self.iface, handle, location, intensity, gain_db, 0, 0) }
    }

    pub fn set_trigger_effect(&mut self, handle: InputHandle, trigger: usize, command: TriggerCommand, seconds: Option<f64>) {
        match seconds.filter(|s| *s > 0.0 && command.mode != 0) {
            Some(s) => self.trigger_effects_until.insert((handle, trigger), Instant::now() + Duration::from_secs_f64(s)),
            None => self.trigger_effects_until.remove(&(handle, trigger)),
        };
        let mut param = TriggerEffectParam { mask: 1 << trigger, _pad: [0; 7], commands: [TriggerCommand::default(); 2] };
        param.commands[trigger] = command;
        unsafe { (self.fns.dualsense_trigger_effect)(self.iface, handle, &param) }
    }

    pub fn set_led_color(&self, handle: InputHandle, r: u8, g: u8, b: u8, restore_default: bool) {
        unsafe { (self.fns.set_led_color)(self.iface, handle, r, g, b, restore_default as u32) }
    }

    pub fn show_binding_panel(&self, handle: InputHandle) -> bool {
        unsafe { (self.fns.show_binding_panel)(self.iface, handle) }
    }

    pub fn set_manifest_path(&mut self, path: &str) -> bool {
        let Ok(path) = CString::new(path) else { return false };
        self.handles.clear();
        unsafe { (self.fns.set_manifest_path)(self.iface, path.as_ptr()) }
    }

    fn handle(&mut self, kind: HandleKind, name: &str) -> Option<u64> {
        let key = (kind, name.to_owned());
        if let Some(&handle) = self.handles.get(&key) {
            return Some(handle);
        }
        let lookup = match kind {
            HandleKind::ActionSet => self.fns.action_set_handle,
            HandleKind::Digital => self.fns.digital_action_handle,
            HandleKind::Analog => self.fns.analog_action_handle,
        };
        let cname = CString::new(name).ok()?;
        let handle = unsafe { lookup(self.iface, cname.as_ptr()) };
        if handle == 0 {
            return None;
        }
        self.handles.insert(key, handle);
        Some(handle)
    }

    pub fn activate_action_set(&mut self, controller: InputHandle, set: &str) -> bool {
        let Some(set) = self.handle(HandleKind::ActionSet, set) else { return false };
        unsafe { (self.fns.activate_action_set)(self.iface, controller, set) };
        true
    }

    pub fn digital_action(&mut self, controller: InputHandle, action: &str) -> DigitalActionData {
        match self.handle(HandleKind::Digital, action) {
            Some(action) => unsafe { (self.fns.digital_action_data)(self.iface, controller, action) },
            None => DigitalActionData::default(),
        }
    }

    pub fn analog_action(&mut self, controller: InputHandle, action: &str) -> AnalogActionData {
        match self.handle(HandleKind::Analog, action) {
            Some(action) => unsafe { (self.fns.analog_action_data)(self.iface, controller, action) },
            None => AnalogActionData::default(),
        }
    }

    pub fn action_origins(&mut self, controller: InputHandle, set: &str, action: &str, analog: bool) -> Vec<i32> {
        let (kind, lookup) = match analog {
            true => (HandleKind::Analog, self.fns.analog_action_origins),
            false => (HandleKind::Digital, self.fns.digital_action_origins),
        };
        let (Some(set), Some(action)) = (self.handle(HandleKind::ActionSet, set), self.handle(kind, action)) else {
            return Vec::new();
        };
        let mut origins = [0i32; MAX_ORIGINS];
        let count = unsafe { lookup(self.iface, controller, set, action, origins.as_mut_ptr()) };
        origins[..count.clamp(0, MAX_ORIGINS as i32) as usize].to_vec()
    }

    pub fn glyph_png(&self, origin: i32, size: i32, flags: u32) -> Option<String> {
        unsafe { owned_string((self.fns.glyph_png)(self.iface, origin, size, flags)) }
    }

    pub fn origin_name(&self, origin: i32) -> Option<String> {
        unsafe { owned_string((self.fns.origin_name)(self.iface, origin)) }
    }
}

impl Drop for SteamInput {
    fn drop(&mut self) {
        for &handle in self.motors.keys() {
            unsafe { (self.fns.trigger_vibration_extended)(self.iface, handle, 0, 0, 0, 0) };
        }
        let off = TriggerEffectParam { mask: 0b11, _pad: [0; 7], commands: [TriggerCommand::default(); 2] };
        for &(handle, _) in self.trigger_effects_until.keys() {
            unsafe { (self.fns.dualsense_trigger_effect)(self.iface, handle, &off) };
        }
        unsafe { (self.fns.shutdown)(self.iface) };
    }
}

pub fn input_type_name(kind: i32) -> &'static str {
    match kind {
        1 => "SteamController",
        2 => "XBox360Controller",
        3 => "XBoxOneController",
        4 => "GenericGamepad",
        5 => "PS4Controller",
        6 => "AppleMFiController",
        7 => "AndroidController",
        8 => "SwitchJoyConPair",
        9 => "SwitchJoyConSingle",
        10 => "SwitchProController",
        11 => "MobileTouch",
        12 => "PS3Controller",
        13 => "PS5Controller",
        14 => "SteamDeckController",
        _ => "Unknown",
    }
}

pub fn load() -> Result<(), String> {
    let api = unsafe { SteamInput::open()? };
    *STEAM_INPUT.lock().unwrap() = Some(api);
    Ok(())
}

pub fn unload() {
    STEAM_INPUT.lock().unwrap().take();
}

pub fn with<R>(f: impl FnOnce(&mut SteamInput) -> R) -> Option<R> {
    STEAM_INPUT.lock().unwrap().as_mut().map(f)
}
