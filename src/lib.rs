use gmod::lua::{LuaFunction, LuaString, State};
use gmod::*;

mod steam_api;

use steam_api::InputHandle;

unsafe fn check_handle(lua: State, arg: i32) -> InputHandle {
    let parsed = lua.check_string(arg).trim().parse::<InputHandle>().ok();
    match parsed {
        Some(handle) => handle,
        None => lua.error(format!("bad argument #{arg}: expected a controller handle string")),
    }
}

unsafe fn require_api<R>(lua: State, f: impl FnOnce(&steam_api::SteamInput) -> R) -> R {
    match steam_api::with(f) {
        Some(result) => result,
        None => lua.error("[steaminput] Steam Input is not initialized"),
    }
}

unsafe fn check_byte(lua: State, arg: i32) -> u8 {
    lua.check_integer(arg).clamp(0, 255) as u8
}

unsafe fn check_u16(lua: State, arg: i32) -> u16 {
    lua.check_integer(arg).clamp(0, u16::MAX as _) as u16
}

#[lua_function]
unsafe fn is_loaded(lua: State) -> i32 {
    lua.push_boolean(steam_api::with(|_| ()).is_some());
    1
}

#[lua_function]
unsafe fn run_frame(_lua: State) -> i32 {
    steam_api::with(|api| api.run_frame());
    0
}

#[lua_function]
unsafe fn get_connected_controllers(lua: State) -> i32 {
    let handles = require_api(lua, |api| api.connected_controllers());
    lua.create_table(handles.len() as i32, 0);
    for (i, handle) in handles.iter().enumerate() {
        lua.push_string(&handle.to_string());
        lua.raw_seti(-2, i as i32 + 1);
    }
    1
}

#[lua_function]
unsafe fn get_input_type(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    let kind = require_api(lua, |api| api.input_type(handle));
    lua.push_string(steam_api::input_type_name(kind));
    lua.push_integer(kind as _);
    2
}

#[lua_function]
unsafe fn trigger_vibration(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    let left = check_u16(lua, 2);
    let right = check_u16(lua, 3);
    require_api(lua, |api| api.trigger_vibration(handle, left, right));
    0
}

#[lua_function]
unsafe fn set_led_color(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    let (r, g, b) = (check_byte(lua, 2), check_byte(lua, 3), check_byte(lua, 4));
    require_api(lua, |api| api.set_led_color(handle, r, g, b, false));
    0
}

#[lua_function]
unsafe fn reset_led_color(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    require_api(lua, |api| api.set_led_color(handle, 0, 0, 0, true));
    0
}

#[lua_function]
unsafe fn show_binding_panel(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    let shown = require_api(lua, |api| api.show_binding_panel(handle));
    lua.push_boolean(shown);
    1
}

unsafe fn set_function(lua: State, name: LuaString, func: LuaFunction) {
    lua.push_function(func);
    lua.set_field(-2, name);
}

#[gmod13_open]
unsafe fn gmod13_open(lua: State) -> i32 {
    if let Err(e) = steam_api::load() {
        lua.error(format!("[steaminput] {e}"));
    }

    lua.new_table();
    lua.push_string(env!("CARGO_PKG_VERSION"));
    lua.set_field(-2, lua_string!("VERSION"));
    set_function(lua, lua_string!("IsLoaded"), is_loaded);
    set_function(lua, lua_string!("RunFrame"), run_frame);
    set_function(lua, lua_string!("GetConnectedControllers"), get_connected_controllers);
    set_function(lua, lua_string!("GetInputType"), get_input_type);
    set_function(lua, lua_string!("TriggerVibration"), trigger_vibration);
    set_function(lua, lua_string!("SetLEDColor"), set_led_color);
    set_function(lua, lua_string!("ResetLEDColor"), reset_led_color);
    set_function(lua, lua_string!("ShowBindingPanel"), show_binding_panel);
    lua.set_global(lua_string!("steaminput"));

    lua.get_global(lua_string!("hook"));
    lua.get_field(-1, lua_string!("Add"));
    lua.push_string("Think");
    lua.push_string("steaminput.RunFrame");
    lua.push_function(run_frame);
    lua.call(3, 0);
    lua.pop();

    lua.get_global(lua_string!("print"));
    lua.push_string(concat!("[steaminput] loaded v", env!("CARGO_PKG_VERSION")));
    lua.call(1, 0);
    0
}

#[gmod13_close]
fn gmod13_close(_lua: State) -> i32 {
    steam_api::unload();
    0
}
