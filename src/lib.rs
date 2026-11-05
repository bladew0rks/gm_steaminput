use gmod::*;

mod steam_api;

#[lua_function]
unsafe fn is_loaded(lua: gmod::lua::State) -> i32 {
    lua.push_boolean(steam_api::is_loaded());
    1
}

#[gmod13_open]
unsafe fn gmod13_open(lua: gmod::lua::State) -> i32 {
    if let Err(e) = steam_api::load() {
        lua.error(format!("[steaminput] {e}"));
    }

    lua.new_table();
    lua.push_string(env!("CARGO_PKG_VERSION"));
    lua.set_field(-2, lua_string!("VERSION"));
    lua.push_function(is_loaded);
    lua.set_field(-2, lua_string!("IsLoaded"));
    lua.set_global(lua_string!("steaminput"));

    lua.get_global(lua_string!("print"));
    lua.push_string(concat!("[steaminput] loaded v", env!("CARGO_PKG_VERSION")));
    lua.call(1, 0);
    0
}

#[gmod13_close]
fn gmod13_close(_lua: gmod::lua::State) -> i32 {
    steam_api::unload();
    0
}
