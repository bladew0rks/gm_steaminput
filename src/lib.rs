use gmod::lua::{LuaFunction, LuaString, State};
use gmod::*;

mod actions;
mod glyphs;
mod register;
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

unsafe fn report_error(lua: State, msg: &str) {
    lua.get_global(lua_string!("ErrorNoHalt"));
    lua.push_string(&format!("[steaminput] {msg}\n"));
    lua.call(1, 0);
}

fn apply_actions() -> Result<(), String> {
    let path = actions::write_files()?;
    match steam_api::with(|api| api.set_manifest_path(&path)) {
        Some(true) => Ok(()),
        Some(false) => Err(format!("Steam rejected the action manifest at {path}")),
        None => Err("Steam Input is not initialized".into()),
    }
}

#[lua_function]
unsafe fn run_frame(lua: State) -> i32 {
    if actions::take_dirty() {
        if let Err(e) = apply_actions() {
            report_error(lua, &format!("failed to apply actions: {e}"));
        }
    }
    steam_api::with(|api| api.run_frame());
    0
}

#[lua_function]
unsafe fn register_action_set(lua: State) -> i32 {
    let name = lua.check_string(1).into_owned();
    lua.check_table(2);
    let result = register::parse_action_set(lua, 2).and_then(|mut set| {
        if set.title.is_empty() {
            set.title = name.clone();
        }
        actions::register(&name, set)
    });
    if let Err(e) = result {
        lua.error(format!("[steaminput] RegisterActionSet('{name}'): {e}"));
    }
    0
}

#[lua_function]
unsafe fn apply_actions_now(lua: State) -> i32 {
    actions::take_dirty();
    match apply_actions() {
        Ok(()) => {
            lua.push_boolean(true);
            1
        }
        Err(e) => {
            lua.push_boolean(false);
            lua.push_string(&e);
            2
        }
    }
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

unsafe fn check_controller(lua: State, arg: i32) -> InputHandle {
    if lua.check_string(arg).trim() == "all" {
        steam_api::ALL_CONTROLLERS
    } else {
        check_handle(lua, arg)
    }
}

#[lua_function]
unsafe fn activate_action_set(lua: State) -> i32 {
    let controller = check_controller(lua, 1);
    let set = lua.check_string(2).into_owned();
    let ok = require_api(lua, |api| api.activate_action_set(controller, &set));
    lua.push_boolean(ok);
    1
}

#[lua_function]
unsafe fn get_digital_action(lua: State) -> i32 {
    let controller = check_handle(lua, 1);
    let action = actions::steam_action_name(&lua.check_string(2), &lua.check_string(3));
    match require_api(lua, |api| api.digital_action(controller, &action)) {
        Some(data) => {
            lua.push_boolean(data.state);
            lua.push_boolean(data.active);
        }
        None => {
            lua.push_boolean(false);
            lua.push_boolean(false);
        }
    }
    2
}

#[lua_function]
unsafe fn get_analog_action(lua: State) -> i32 {
    let controller = check_handle(lua, 1);
    let action = actions::steam_action_name(&lua.check_string(2), &lua.check_string(3));
    match require_api(lua, |api| api.analog_action(controller, &action)) {
        Some(data) => {
            let (x, y, active) = (data.x, data.y, data.active);
            lua.push_number(x as f64);
            lua.push_number(y as f64);
            lua.push_boolean(active);
        }
        None => {
            lua.push_number(0.0);
            lua.push_number(0.0);
            lua.push_boolean(false);
        }
    }
    3
}

unsafe fn push_origins(lua: State, analog: bool) -> i32 {
    let controller = check_handle(lua, 1);
    let set = lua.check_string(2).into_owned();
    let action = actions::steam_action_name(&set, &lua.check_string(3));
    let origins = require_api(lua, |api| api.action_origins(controller, &set, &action, analog));
    lua.create_table(origins.len() as i32, 0);
    for (i, origin) in origins.iter().enumerate() {
        lua.push_integer(*origin as _);
        lua.raw_seti(-2, i as i32 + 1);
    }
    1
}

#[lua_function]
unsafe fn get_digital_action_origins(lua: State) -> i32 {
    push_origins(lua, false)
}

#[lua_function]
unsafe fn get_analog_action_origins(lua: State) -> i32 {
    push_origins(lua, true)
}

#[lua_function]
unsafe fn get_glyph(lua: State) -> i32 {
    let origin = lua.check_integer(1) as i32;
    let size = if lua.is_none_or_nil(2) { 0 } else { lua.check_integer(2).clamp(0, 2) as i32 };
    let flags = if lua.is_none_or_nil(3) { 0 } else { lua.check_integer(3) as u32 };
    let source = require_api(lua, |api| api.glyph_png(origin, size, flags));
    match glyphs::material_path(origin, size, flags, || source) {
        Some(path) => lua.push_string(&path),
        None => lua.push_nil(),
    }
    1
}

#[lua_function]
unsafe fn get_origin_name(lua: State) -> i32 {
    let origin = lua.check_integer(1) as i32;
    match require_api(lua, |api| api.origin_name(origin)) {
        Some(name) => lua.push_string(&name),
        None => lua.push_nil(),
    }
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
    set_function(lua, lua_string!("RegisterActionSet"), register_action_set);
    set_function(lua, lua_string!("GetDigitalActionOrigins"), get_digital_action_origins);
    set_function(lua, lua_string!("GetAnalogActionOrigins"), get_analog_action_origins);
    set_function(lua, lua_string!("GetGlyph"), get_glyph);
    set_function(lua, lua_string!("GetOriginName"), get_origin_name);
    set_function(lua, lua_string!("ApplyActions"), apply_actions_now);
    set_function(lua, lua_string!("ActivateActionSet"), activate_action_set);
    set_function(lua, lua_string!("GetDigitalAction"), get_digital_action);
    set_function(lua, lua_string!("GetAnalogAction"), get_analog_action);
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
