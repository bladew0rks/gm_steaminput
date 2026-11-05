use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::Mutex;

use libloading::os::windows::Library;

const STEAM_API_DLL: &str = "steam_api64.dll";
pub const MAX_CONTROLLERS: usize = 16;

pub type InputHandle = u64;
pub const ALL_CONTROLLERS: InputHandle = u64::MAX;
const MAX_ORIGINS: usize = 8;
type Iface = *mut c_void;
type NameToHandle = unsafe extern "C" fn(Iface, *const c_char) -> u64;
type GetOrigins = unsafe extern "C" fn(Iface, InputHandle, u64, u64, *mut i32) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DigitalActionData {
    pub state: bool,
    pub active: bool,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct AnalogActionData {
    pub mode: i32,
    pub x: f32,
    pub y: f32,
    pub active: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum HandleKind {
    ActionSet,
    Digital,
    Analog,
}

pub struct SteamInput {
    _lib: Library,
    iface: Iface,
    shutdown: unsafe extern "C" fn(Iface) -> bool,
    run_frame: unsafe extern "C" fn(Iface, bool),
    get_connected_controllers: unsafe extern "C" fn(Iface, *mut InputHandle) -> i32,
    get_input_type_for_handle: unsafe extern "C" fn(Iface, InputHandle) -> i32,
    trigger_vibration: unsafe extern "C" fn(Iface, InputHandle, u16, u16),
    set_led_color: unsafe extern "C" fn(Iface, InputHandle, u8, u8, u8, u32),
    show_binding_panel: unsafe extern "C" fn(Iface, InputHandle) -> bool,
    set_manifest_path: unsafe extern "C" fn(Iface, *const c_char) -> bool,
    get_action_set_handle: NameToHandle,
    activate_action_set: unsafe extern "C" fn(Iface, InputHandle, u64),
    get_digital_action_handle: NameToHandle,
    get_digital_action_data: unsafe extern "C" fn(Iface, InputHandle, u64) -> DigitalActionData,
    get_analog_action_handle: NameToHandle,
    get_analog_action_data: unsafe extern "C" fn(Iface, InputHandle, u64) -> AnalogActionData,
    get_digital_action_origins: GetOrigins,
    get_analog_action_origins: GetOrigins,
    get_glyph_png: unsafe extern "C" fn(Iface, i32, i32, u32) -> *const c_char,
    get_string_for_origin: unsafe extern "C" fn(Iface, i32) -> *const c_char,
    handles: Mutex<HashMap<(HandleKind, String), u64>>,
}

unsafe impl Send for SteamInput {}

static STEAM_INPUT: Mutex<Option<SteamInput>> = Mutex::new(None);

unsafe fn sym<T: Copy>(lib: &Library, name: &str) -> Result<T, String> {
    lib.get::<T>(name)
        .map(|s| *s)
        .map_err(|e| format!("missing {name} in {STEAM_API_DLL}: {e}"))
}

impl SteamInput {
    unsafe fn open() -> Result<Self, String> {
        let lib = Library::open_already_loaded(STEAM_API_DLL)
            .map_err(|e| format!("failed to open {STEAM_API_DLL}: {e}"))?;

        let accessor: unsafe extern "C" fn() -> Iface = sym(&lib, "SteamAPI_SteamInput_v006")?;
        let init: unsafe extern "C" fn(Iface, bool) -> bool = sym(&lib, "SteamAPI_ISteamInput_Init")?;

        let iface = accessor();
        if iface.is_null() {
            return Err("SteamInput006 interface is unavailable".into());
        }

        let api = SteamInput {
            iface,
            shutdown: sym(&lib, "SteamAPI_ISteamInput_Shutdown")?,
            run_frame: sym(&lib, "SteamAPI_ISteamInput_RunFrame")?,
            get_connected_controllers: sym(&lib, "SteamAPI_ISteamInput_GetConnectedControllers")?,
            get_input_type_for_handle: sym(&lib, "SteamAPI_ISteamInput_GetInputTypeForHandle")?,
            trigger_vibration: sym(&lib, "SteamAPI_ISteamInput_TriggerVibration")?,
            set_led_color: sym(&lib, "SteamAPI_ISteamInput_SetLEDColor")?,
            show_binding_panel: sym(&lib, "SteamAPI_ISteamInput_ShowBindingPanel")?,
            set_manifest_path: sym(&lib, "SteamAPI_ISteamInput_SetInputActionManifestFilePath")?,
            get_action_set_handle: sym(&lib, "SteamAPI_ISteamInput_GetActionSetHandle")?,
            activate_action_set: sym(&lib, "SteamAPI_ISteamInput_ActivateActionSet")?,
            get_digital_action_handle: sym(&lib, "SteamAPI_ISteamInput_GetDigitalActionHandle")?,
            get_digital_action_data: sym(&lib, "SteamAPI_ISteamInput_GetDigitalActionData")?,
            get_analog_action_handle: sym(&lib, "SteamAPI_ISteamInput_GetAnalogActionHandle")?,
            get_analog_action_data: sym(&lib, "SteamAPI_ISteamInput_GetAnalogActionData")?,
            get_digital_action_origins: sym(&lib, "SteamAPI_ISteamInput_GetDigitalActionOrigins")?,
            get_analog_action_origins: sym(&lib, "SteamAPI_ISteamInput_GetAnalogActionOrigins")?,
            get_glyph_png: sym(&lib, "SteamAPI_ISteamInput_GetGlyphPNGForActionOrigin")?,
            get_string_for_origin: sym(&lib, "SteamAPI_ISteamInput_GetStringForActionOrigin")?,
            handles: Mutex::new(HashMap::new()),
            _lib: lib,
        };

        if !init(iface, true) {
            return Err("ISteamInput::Init failed".into());
        }

        Ok(api)
    }

    pub fn run_frame(&self) {
        unsafe { (self.run_frame)(self.iface, true) }
    }

    pub fn connected_controllers(&self) -> Vec<InputHandle> {
        let mut handles = [0; MAX_CONTROLLERS];
        let count = unsafe { (self.get_connected_controllers)(self.iface, handles.as_mut_ptr()) };
        handles[..(count.clamp(0, MAX_CONTROLLERS as i32) as usize)].to_vec()
    }

    pub fn input_type(&self, handle: InputHandle) -> i32 {
        unsafe { (self.get_input_type_for_handle)(self.iface, handle) }
    }

    pub fn trigger_vibration(&self, handle: InputHandle, left: u16, right: u16) {
        unsafe { (self.trigger_vibration)(self.iface, handle, left, right) }
    }

    pub fn set_led_color(&self, handle: InputHandle, r: u8, g: u8, b: u8, restore_default: bool) {
        unsafe { (self.set_led_color)(self.iface, handle, r, g, b, restore_default as u32) }
    }

    pub fn show_binding_panel(&self, handle: InputHandle) -> bool {
        unsafe { (self.show_binding_panel)(self.iface, handle) }
    }
}

impl SteamInput {
    pub fn set_manifest_path(&self, path: &str) -> bool {
        let Ok(path) = CString::new(path) else { return false };
        self.handles.lock().unwrap().clear();
        unsafe { (self.set_manifest_path)(self.iface, path.as_ptr()) }
    }

    fn handle(&self, kind: HandleKind, name: &str) -> Option<u64> {
        let key = (kind, name.to_owned());
        if let Some(&handle) = self.handles.lock().unwrap().get(&key) {
            return Some(handle);
        }
        let cname = CString::new(name).ok()?;
        let lookup = match kind {
            HandleKind::ActionSet => self.get_action_set_handle,
            HandleKind::Digital => self.get_digital_action_handle,
            HandleKind::Analog => self.get_analog_action_handle,
        };
        let handle = unsafe { lookup(self.iface, cname.as_ptr()) };
        if handle == 0 {
            return None;
        }
        self.handles.lock().unwrap().insert(key, handle);
        Some(handle)
    }

    pub fn activate_action_set(&self, controller: InputHandle, set: &str) -> bool {
        let Some(set) = self.handle(HandleKind::ActionSet, set) else { return false };
        unsafe { (self.activate_action_set)(self.iface, controller, set) };
        true
    }

    pub fn digital_action(&self, controller: InputHandle, action: &str) -> Option<DigitalActionData> {
        let action = self.handle(HandleKind::Digital, action)?;
        Some(unsafe { (self.get_digital_action_data)(self.iface, controller, action) })
    }

    pub fn analog_action(&self, controller: InputHandle, action: &str) -> Option<AnalogActionData> {
        let action = self.handle(HandleKind::Analog, action)?;
        Some(unsafe { (self.get_analog_action_data)(self.iface, controller, action) })
    }
}

unsafe fn owned_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
    (!s.is_empty()).then_some(s)
}

impl SteamInput {
    pub fn action_origins(&self, controller: InputHandle, set: &str, action: &str, analog: bool) -> Vec<i32> {
        let Some(set) = self.handle(HandleKind::ActionSet, set) else { return Vec::new() };
        let kind = if analog { HandleKind::Analog } else { HandleKind::Digital };
        let Some(action) = self.handle(kind, action) else { return Vec::new() };
        let lookup = if analog { self.get_analog_action_origins } else { self.get_digital_action_origins };
        let mut origins = [0i32; MAX_ORIGINS];
        let count = unsafe { lookup(self.iface, controller, set, action, origins.as_mut_ptr()) };
        origins[..count.clamp(0, MAX_ORIGINS as i32) as usize].to_vec()
    }

    pub fn glyph_png(&self, origin: i32, size: i32, flags: u32) -> Option<String> {
        unsafe { owned_string((self.get_glyph_png)(self.iface, origin, size, flags)) }
    }

    pub fn origin_name(&self, origin: i32) -> Option<String> {
        unsafe { owned_string((self.get_string_for_origin)(self.iface, origin)) }
    }
}

impl Drop for SteamInput {
    fn drop(&mut self) {
        unsafe { (self.shutdown)(self.iface) };
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

pub fn with<R>(f: impl FnOnce(&SteamInput) -> R) -> Option<R> {
    STEAM_INPUT.lock().unwrap().as_ref().map(f)
}
