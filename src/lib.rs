use gmod::lua::{LuaFunction, LuaString, State};
use gmod::*;

mod actions;
mod events;
mod glyphs;
mod haptics;
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

unsafe fn opt_seconds(lua: State, arg: i32) -> Option<f64> {
    (!lua.is_none_or_nil(arg)).then(|| lua.check_number(arg))
}

unsafe fn opt_side(lua: State, arg: i32) -> haptics::Side {
    if lua.is_none_or_nil(arg) {
        return haptics::Side::Both;
    }
    let side = haptics::Side::parse(&lua.check_string(arg));
    match side {
        Some(side) => side,
        None => lua.error(format!("bad argument #{arg}: expected 'left', 'right' or 'both'")),
    }
}

unsafe fn check_u16(lua: State, arg: i32) -> u16 {
    lua.check_integer(arg).clamp(0, u16::MAX as _) as u16
}

unsafe fn check_u8(lua: State, arg: i32) -> u8 {
    lua.check_integer(arg).clamp(0, u8::MAX as _) as u8
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
    events::set_actions(actions::button_actions());
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
    let events = steam_api::with(|api| {
        api.run_frame();
        events::poll(api)
    });
    for event in events.unwrap_or_default() {
        fire(lua, event);
    }
    0
}

unsafe fn fire(lua: State, event: events::Event) {
    use events::Event::*;
    lua.get_global(lua_string!("hook"));
    lua.get_field(-1, lua_string!("Run"));
    let (name, handle) = match &event {
        Connected(h, _) => ("SteamInputControllerConnected", h),
        Disconnected(h) => ("SteamInputControllerDisconnected", h),
        Pressed(h, ..) => ("SteamInputActionPressed", h),
        Released(h, ..) => ("SteamInputActionReleased", h),
    };
    lua.push_string(name);
    lua.push_string(&handle.to_string());
    let nargs = match &event {
        Connected(_, kind) => {
            lua.push_string(kind);
            3
        }
        Disconnected(_) => 2,
        Pressed(_, set, action) | Released(_, set, action) => {
            lua.push_string(set);
            lua.push_string(action);
            4
        }
    };
    lua.pcall_ignore(nargs, 0);
    lua.pop();
}

unsafe fn register(lua: State, name: String, parent: Option<String>, table: i32, func: &str) -> i32 {
    lua.check_table(table);
    let result = register::parse_action_set(lua, table, &name).and_then(|mut set| {
        set.parent = parent;
        actions::register(&name, set)
    });
    if let Err(e) = result {
        lua.error(format!("[steaminput] {func}('{name}'): {e}"));
    }
    0
}

#[lua_function]
unsafe fn register_action_set(lua: State) -> i32 {
    let name = lua.check_string(1).into_owned();
    register(lua, name, None, 2, "RegisterActionSet")
}

#[lua_function]
unsafe fn register_action_layer(lua: State) -> i32 {
    let name = lua.check_string(1).into_owned();
    let parent = lua.check_string(2).into_owned();
    register(lua, name, Some(parent), 3, "RegisterActionLayer")
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

unsafe fn opt_handle(lua: State, arg: i32) -> Option<InputHandle> {
    if lua.is_none_or_nil(arg) { events::primary() } else { Some(check_handle(lua, arg)) }
}

unsafe fn check_set_action(lua: State) -> (String, String) {
    (lua.check_string(1).into_owned(), lua.check_string(2).into_owned())
}

#[lua_function]
unsafe fn get_primary_controller(lua: State) -> i32 {
    match events::primary() {
        Some(h) => lua.push_string(&h.to_string()),
        None => lua.push_nil(),
    }
    1
}

#[lua_function]
unsafe fn is_down(lua: State) -> i32 {
    let (set, action) = check_set_action(lua);
    let down = opt_handle(lua, 3).is_some_and(|h| events::is_down(h, &set, &action));
    lua.push_boolean(down);
    1
}

unsafe fn changed(lua: State, pressed: bool) -> i32 {
    let (set, action) = check_set_action(lua);
    let changed = opt_handle(lua, 3).is_some_and(|h| events::changed_this_frame(h, &set, &action, pressed));
    lua.push_boolean(changed);
    1
}

#[lua_function]
unsafe fn was_pressed(lua: State) -> i32 {
    changed(lua, true)
}

#[lua_function]
unsafe fn was_released(lua: State) -> i32 {
    changed(lua, false)
}

#[lua_function]
unsafe fn get_actions(lua: State) -> i32 {
    let set = lua.check_string(1).into_owned();
    let Some(list) = actions::actions_of(&set) else {
        lua.push_nil();
        return 1;
    };
    lua.create_table(0, list.len() as i32);
    for (name, kind) in list {
        lua.push_string(&name);
        lua.push_string(kind.name());
        lua.set_table(-3);
    }
    1
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
    let levels = [Some(check_u16(lua, 2)), Some(check_u16(lua, 3)), None, None];
    let seconds = opt_seconds(lua, 4);
    api(lua, |api| api.set_motors(handle, levels, seconds));
    0
}

#[lua_function]
unsafe fn trigger_vibration_extended(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    let levels = [2, 3, 4, 5].map(|arg| Some(check_u16(lua, arg)));
    let seconds = opt_seconds(lua, 6);
    api(lua, |api| api.set_motors(handle, levels, seconds));
    0
}

#[lua_function]
unsafe fn trigger_haptic_pulse(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    let side = opt_side(lua, 2);
    let intensity = check_u8(lua, 3);
    let gain = if lua.is_none_or_nil(4) { 0 } else { lua.check_integer(4).clamp(-127, 127) as i8 };
    api(lua, |api| api.simple_haptic_event(handle, side.haptic_location(), intensity, gain));
    0
}

#[lua_function]
unsafe fn set_trigger_effect(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    let side = opt_side(lua, 2);
    let mode = lua.check_string(3).into_owned();
    let [a, b, c] = [4, 5, 6].map(|arg| if lua.is_none_or_nil(arg) { 0 } else { check_u8(lua, arg) });
    let seconds = opt_seconds(lua, 7);
    let Some(command) = haptics::trigger_command(&mode, a, b, c) else {
        lua.error(format!("bad argument #3: unknown trigger effect '{mode}', expected off, feedback, weapon or vibration"));
    };
    api(lua, |api| haptics::set_trigger_effect(api, handle, side, command, seconds));
    0
}

#[lua_function]
unsafe fn haptic(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    let effect = lua.check_string(2).into_owned();
    let strength = if lua.is_none_or_nil(3) { 1.0 } else { lua.check_number(3) };
    let seconds = opt_seconds(lua, 4);
    let side = opt_side(lua, 5);
    match api(lua, |api| haptics::haptic(api, handle, &effect, strength, seconds, side)) {
        Ok(method) => lua.push_string(method),
        Err(e) => lua.error(format!("[steaminput] Haptic: {e}")),
    }
    1
}

#[lua_function]
unsafe fn set_led_color(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    let [r, g, b] = [2, 3, 4].map(|arg| check_u8(lua, arg));
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

unsafe fn set_layer(lua: State, active: bool) -> i32 {
    let handle = check_handle(lua, 1);
    let layer = lua.check_string(2).into_owned();
    lua.push_boolean(api(lua, |api| api.set_layer(handle, &layer, active)));
    1
}

#[lua_function]
unsafe fn activate_action_set_layer(lua: State) -> i32 {
    set_layer(lua, true)
}

#[lua_function]
unsafe fn deactivate_action_set_layer(lua: State) -> i32 {
    set_layer(lua, false)
}

#[lua_function]
unsafe fn deactivate_all_action_set_layers(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    api(lua, |api| api.deactivate_all_layers(handle));
    0
}

#[lua_function]
unsafe fn get_active_action_set_layers(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    let names = actions::set_names();
    let layers = api(lua, |api| api.active_layers(handle, &names));
    push_list(lua, &layers, |name| lua.push_string(name))
}

#[lua_function]
unsafe fn get_current_action_set(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    let names = actions::set_names();
    match api(lua, |api| api.current_action_set(handle, &names)) {
        Some(name) => lua.push_string(&name),
        None => lua.push_nil(),
    }
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
unsafe fn stop_analog_action_momentum(lua: State) -> i32 {
    let (handle, _, action) = check_action(lua);
    api(lua, |api| api.stop_analog_momentum(handle, &action));
    0
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

unsafe fn push_vector(lua: State, v: [f32; 3], scale: f64) {
    lua.get_global(lua_string!("Vector"));
    for c in v {
        lua.push_number(c as f64 * scale);
    }
    lua.call(3, 1);
}

#[lua_function]
unsafe fn get_motion_data(lua: State) -> i32 {
    let handle = check_handle(lua, 1);
    let data = api(lua, |api| api.motion_data(handle));
    let int16 = i16::MAX as f64;

    lua.create_table(0, 3);
    lua.create_table(0, 4);
    for (key, value) in [lua_string!("x"), lua_string!("y"), lua_string!("z"), lua_string!("w")].into_iter().zip(data.quat) {
        lua.push_number(value as f64);
        lua.set_field(-2, key);
    }
    lua.set_field(-2, lua_string!("orientation"));
    push_vector(lua, data.accel, 2.0 / int16);
    lua.set_field(-2, lua_string!("accel"));
    push_vector(lua, data.gyro, 2000.0 / int16);
    lua.set_field(-2, lua_string!("gyro"));
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

    let functions: [(LuaString, LuaFunction); 32] = [
        (lua_string!("GetConnectedControllers"), get_connected_controllers),
        (lua_string!("GetPrimaryController"), get_primary_controller),
        (lua_string!("IsDown"), is_down),
        (lua_string!("WasPressed"), was_pressed),
        (lua_string!("WasReleased"), was_released),
        (lua_string!("GetActions"), get_actions),
        (lua_string!("GetInputType"), get_input_type),
        (lua_string!("TriggerVibration"), trigger_vibration),
        (lua_string!("TriggerVibrationExtended"), trigger_vibration_extended),
        (lua_string!("TriggerHapticPulse"), trigger_haptic_pulse),
        (lua_string!("SetTriggerEffect"), set_trigger_effect),
        (lua_string!("Haptic"), haptic),
        (lua_string!("SetLEDColor"), set_led_color),
        (lua_string!("ResetLEDColor"), reset_led_color),
        (lua_string!("ShowBindingPanel"), show_binding_panel),
        (lua_string!("RegisterActionSet"), register_action_set),
        (lua_string!("ApplyActions"), apply_actions_now),
        (lua_string!("ActivateActionSet"), activate_action_set),
        (lua_string!("GetCurrentActionSet"), get_current_action_set),
        (lua_string!("RegisterActionLayer"), register_action_layer),
        (lua_string!("ActivateActionSetLayer"), activate_action_set_layer),
        (lua_string!("DeactivateActionSetLayer"), deactivate_action_set_layer),
        (lua_string!("DeactivateAllActionSetLayers"), deactivate_all_action_set_layers),
        (lua_string!("GetActiveActionSetLayers"), get_active_action_set_layers),
        (lua_string!("GetDigitalAction"), get_digital_action),
        (lua_string!("GetAnalogAction"), get_analog_action),
        (lua_string!("StopAnalogActionMomentum"), stop_analog_action_momentum),
        (lua_string!("GetDigitalActionOrigins"), get_digital_action_origins),
        (lua_string!("GetAnalogActionOrigins"), get_analog_action_origins),
        (lua_string!("GetGlyph"), get_glyph),
        (lua_string!("GetOriginName"), get_origin_name),
        (lua_string!("GetMotionData"), get_motion_data),
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

    match lua.load_buffer(include_bytes!("util.lua"), lua_string!("=steaminput/util.lua")) {
        Ok(()) => {
            lua.pcall_ignore(0, 0);
        }
        Err(e) => lua.error(format!("[steaminput] failed to load util.lua: {e:?}")),
    }

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
