use gmod::*;

#[gmod13_open]
unsafe fn gmod13_open(lua: gmod::lua::State) -> i32 {
    // Global `steaminput` table that the Steam Input bindings will hang off of.
    lua.new_table();
    lua.push_string(env!("CARGO_PKG_VERSION"));
    lua.set_field(-2, lua_string!("VERSION"));
    lua.set_global(lua_string!("steaminput"));

    lua.get_global(lua_string!("print"));
    lua.push_string(concat!("[steaminput] loaded v", env!("CARGO_PKG_VERSION")));
    lua.call(1, 0);
    0
}

#[gmod13_close]
fn gmod13_close(_lua: gmod::lua::State) -> i32 {
    0
}
