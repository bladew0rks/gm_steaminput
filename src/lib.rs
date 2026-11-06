use gmod::lua::{LuaFunction, LuaString, State};
use gmod::*;

mod actions;
mod glyphs;
mod register;
mod steam_api;

use steam_api::{InputHandle, SteamInput};

unsafe fn api<R>(lua: State, f: impl FnOnce(&mut SteamInput) -> R) -> R {
    match steam_api::with(f) {
        Some(result) => result,
        None => lua.error("[steaminput] Steam Input is not initialized"),
    }
}

unsafe fn check_handle(lua: State, arg: i32) -> InputHandle {
    let parsed = match lua.check_string(arg).trim() {
        "all" => Some(steam_api::ALL_CONTROLLERS),
        s => s.parse().ok(),
    };
    match parsed {
        Some(handle) => handle,
        None => lua.error(format!("bad argument #{arg}: expected a controller handle")),
    }
}

unsafe fn check_action(lua: State) -> (InputHandle, String, String) {
    let handle = check_handle(lua, 1);
    let set = lua.check_string(2).into_owned();
    let action = actions::steam_action_name(&set, &lua.check_string(3));
    (handle, set, action)
}

unsafe fn opt_integer(lua: State, arg: i32, default: isize) -> isize {
    if lua.is_none_or_nil(arg) { default } else { lua.check_integer(arg) as isize }
}

unsafe fn push_list<T>(lua: State, items: &[T], push: impl Fn(&T)) -> i32 {
    lua.create_table(items.len() as i32, 0);
    for (i, item) in items.iter().enumerate() {
        push(item);
        lua.raw_seti(-2, i as i32 + 1);
    }
    1
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
            lua.get_global(lua_string!("ErrorNoHalt"));
            lua.push_string(&format!("[steaminput] failed to apply actions: {e}\n"));
            lua.call(1, 0);
        }
    }
    steam_api::with(|api| api.run_frame());
    0
}

#[lua_function]
unsafe fn register_action_set(lua: State) -> i32 {
    let name = lua.check_string(1).into_owned();
    lua.check_table(2);
    if let Err(e) = register::parse_action_set(lua, 2, &name).and_then(|set| actions::register(&name, set)) {
        lua.error(format!("[steaminput] RegisterActionSet('{name}'): {e}"));
    }
    0
}

#[lua_function]
unsafe fn apply_actions_now(lua: State) -> i32 {
    actions::take_dirty();
    let result = apply_actions();
    lua.push_boolean(result.is_ok());
    match result {
        Ok(()) => 1,
        Err(e) => {
            lua.push_string(&e);
            2
        }
    }
}

#[lua_function]
unsafe fn get_connected_controllers(lua: State) -> i32 {
    let handles = api(lua, |api| api.connected_controllers());
    push_list(lua, &handles, |h| lua.push_string(&h.to_string()))
}

#[lua_function]
unsafe fn get_input_type(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    let kind = api(lua, |api| api.input_type(handle));
    lua.push_string(steam_api::input_type_name(kind));
    lua.push_integer(kind as _);
    2
}

#[lua_function]
unsafe fn trigger_vibration(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    let left = lua.check_integer(2).clamp(0, u16::MAX as _) as u16;
    let right = lua.check_integer(3).clamp(0, u16::MAX as _) as u16;
    let seconds = (!lua.is_none_or_nil(4)).then(|| lua.check_number(4));
    api(lua, |api| api.trigger_vibration(handle, left, right, seconds));
    0
}

#[lua_function]
unsafe fn set_led_color(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    let [r, g, b] = [2, 3, 4].map(|arg| lua.check_integer(arg).clamp(0, 255) as u8);
    api(lua, |api| api.set_led_color(handle, r, g, b, false));
    0
}

#[lua_function]
unsafe fn reset_led_color(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    api(lua, |api| api.set_led_color(handle, 0, 0, 0, true));
    0
}

#[lua_function]
unsafe fn show_binding_panel(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    lua.push_boolean(api(lua, |api| api.show_binding_panel(handle)));
    1
}

#[lua_function]
unsafe fn activate_action_set(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    let set = lua.check_string(2).into_owned();
    lua.push_boolean(api(lua, |api| api.activate_action_set(handle, &set)));
    1
}

#[lua_function]
unsafe fn get_digital_action(lua: State) -> i32 {
    let (handle, _, action) = check_action(lua);
    let data = api(lua, |api| api.digital_action(handle, &action));
    lua.push_boolean(data.state);
    lua.push_boolean(data.active);
    2
}

#[lua_function]
unsafe fn get_analog_action(lua: State) -> i32 {
    let (handle, _, action) = check_action(lua);
    let data = api(lua, |api| api.analog_action(handle, &action));
    let (x, y, active) = (data.x, data.y, data.active);
    lua.push_number(x as f64);
    lua.push_number(y as f64);
    lua.push_boolean(active);
    3
}

unsafe fn push_origins(lua: State, analog: bool) -> i32 {
    let (handle, set, action) = check_action(lua);
    let origins = api(lua, |api| api.action_origins(handle, &set, &action, analog));
    push_list(lua, &origins, |o| lua.push_integer(*o as _))
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
    let size = opt_integer(lua, 2, 0).clamp(0, 2) as i32;
    let flags = opt_integer(lua, 3, 0) as u32;
    let source = api(lua, |api| api.glyph_png(origin, size, flags));
    match glyphs::material_path(origin, size, flags, source) {
        Some(path) => lua.push_string(&path),
        None => lua.push_nil(),
    }
    1
}

#[lua_function]
unsafe fn get_origin_name(lua: State) -> i32 {
    let origin = lua.check_integer(1) as i32;
    match api(lua, |api| api.origin_name(origin)) {
        Some(name) => lua.push_string(&name),
        None => lua.push_nil(),
    }
    1
}

#[gmod13_open]
unsafe fn gmod13_open(lua: State) -> i32 {
    if let Err(e) = steam_api::load() {
        lua.error(format!("[steaminput] {e}"));
    }

    let functions: [(LuaString, LuaFunction); 15] = [
        (lua_string!("GetConnectedControllers"), get_connected_controllers),
        (lua_string!("GetInputType"), get_input_type),
        (lua_string!("TriggerVibration"), trigger_vibration),
        (lua_string!("SetLEDColor"), set_led_color),
        (lua_string!("ResetLEDColor"), reset_led_color),
        (lua_string!("ShowBindingPanel"), show_binding_panel),
        (lua_string!("RegisterActionSet"), register_action_set),
        (lua_string!("ApplyActions"), apply_actions_now),
        (lua_string!("ActivateActionSet"), activate_action_set),
        (lua_string!("GetDigitalAction"), get_digital_action),
        (lua_string!("GetAnalogAction"), get_analog_action),
        (lua_string!("GetDigitalActionOrigins"), get_digital_action_origins),
        (lua_string!("GetAnalogActionOrigins"), get_analog_action_origins),
        (lua_string!("GetGlyph"), get_glyph),
        (lua_string!("GetOriginName"), get_origin_name),
    ];

    lua.new_table();
    lua.push_string(env!("CARGO_PKG_VERSION"));
    lua.set_field(-2, lua_string!("VERSION"));
    for (name, func) in functions {
        lua.push_function(func);
        lua.set_field(-2, name);
    }
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
