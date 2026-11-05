use std::sync::Mutex;

use libloading::os::windows::Library;

const STEAM_API_DLL: &str = "steam_api64.dll";

static STEAM_API: Mutex<Option<Library>> = Mutex::new(None);

pub fn load() -> Result<(), String> {
    let lib = Library::open_already_loaded(STEAM_API_DLL)
        .map_err(|e| format!("failed to open {STEAM_API_DLL}: {e}"))?;
    *STEAM_API.lock().unwrap() = Some(lib);
    Ok(())
}

pub fn unload() {
    STEAM_API.lock().unwrap().take();
}

pub fn is_loaded() -> bool {
    STEAM_API.lock().unwrap().is_some()
}
