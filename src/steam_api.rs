use std::ffi::c_void;
use std::sync::Mutex;

use libloading::os::windows::Library;

const STEAM_API_DLL: &str = "steam_api64.dll";
pub const MAX_CONTROLLERS: usize = 16;

pub type InputHandle = u64;
type Iface = *mut c_void;

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
