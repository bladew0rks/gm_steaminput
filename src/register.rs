use std::collections::BTreeMap;

use gmod::lua::{LuaString, State};
use gmod::lua_string;

use crate::actions::{Action, ActionSet, AnalogAction, AnalogMode};

const LUA_TNIL: i32 = 0;
const LUA_TSTRING: i32 = 4;
const LUA_TTABLE: i32 = 5;

unsafe fn field_string(lua: State, table: i32, key: LuaString, name: &str) -> Result<Option<String>, String> {
    lua.get_field(table, key);
    let result = match lua.lua_type(-1) {
        LUA_TNIL => Ok(None),
        LUA_TSTRING => Ok(lua.get_string(-1).map(|s| s.into_owned())),
        _ => Err(format!("'{name}' must be a string")),
    };
    lua.pop();
    result
}

unsafe fn defaults(lua: State, table: i32, context: &str) -> Result<Vec<String>, String> {
    lua.get_field(table, lua_string!("default"));
    let result = match lua.lua_type(-1) {
        LUA_TNIL => Ok(Vec::new()),
        LUA_TSTRING => Ok(vec![lua.get_string(-1).unwrap_or_default().into_owned()]),
        LUA_TTABLE => {
            let list = lua.get_top();
            let mut out = Vec::new();
            for i in 1..=lua.len(list) {
                lua.raw_geti(list, i);
                if lua.lua_type(-1) != LUA_TSTRING {
                    lua.pop();
                    lua.pop();
                    return Err(format!("{context}: 'default' entries must be strings"));
                }
                out.push(lua.get_string(-1).unwrap_or_default().into_owned());
                lua.pop();
            }
            Ok(out)
        }
        _ => Err(format!("{context}: 'default' must be a string or a list of strings")),
    };
    lua.pop();
    result
}

struct Entry {
    title: String,
    mode: Option<String>,
    defaults: Vec<String>,
}

unsafe fn entries(lua: State, set: i32, key: LuaString, section: &str) -> Result<BTreeMap<String, Entry>, String> {
    let mut out = BTreeMap::new();
    lua.get_field(set, key);
    match lua.lua_type(-1) {
        LUA_TNIL => {
            lua.pop();
            return Ok(out);
        }
        LUA_TTABLE => {}
        _ => {
            lua.pop();
            return Err(format!("'{section}' must be a table"));
        }
    }

    let table = lua.get_top();
    lua.push_nil();
    while lua.next(table) != 0 {
        if lua.lua_type(-2) != LUA_TSTRING {
            lua.pop_n(3);
            return Err(format!("'{section}' keys must be action names"));
        }
        let name = lua.get_string(-2).unwrap_or_default().into_owned();
        let context = format!("{section}.{name}");
        let value = lua.get_top();

        let entry = match lua.lua_type(value) {
            LUA_TSTRING => Ok(Entry {
                title: lua.get_string(value).unwrap_or_default().into_owned(),
                mode: None,
                defaults: Vec::new(),
            }),
            LUA_TTABLE => (|| {
                Ok(Entry {
                    title: field_string(lua, value, lua_string!("title"), &format!("{context}.title"))?
                        .unwrap_or_else(|| name.clone()),
                    mode: field_string(lua, value, lua_string!("mode"), &format!("{context}.mode"))?,
                    defaults: defaults(lua, value, &context)?,
                })
            })(),
            _ => Err(format!("{context} must be a title string or a table")),
        };

        match entry {
            Ok(entry) => {
                out.insert(name, entry);
                lua.pop();
            }
            Err(e) => {
                lua.pop_n(3);
                return Err(e);
            }
        }
    }
    lua.pop();
    Ok(out)
}

pub unsafe fn parse_action_set(lua: State, table: i32) -> Result<ActionSet, String> {
    let mut set = ActionSet {
        title: field_string(lua, table, lua_string!("title"), "title")?.unwrap_or_default(),
        ..Default::default()
    };

    for (name, e) in entries(lua, table, lua_string!("buttons"), "buttons")? {
        set.buttons.insert(name, Action { title: e.title, defaults: e.defaults });
    }
    for (name, e) in entries(lua, table, lua_string!("triggers"), "triggers")? {
        set.triggers.insert(name, Action { title: e.title, defaults: e.defaults });
    }
    for (name, e) in entries(lua, table, lua_string!("analog"), "analog")? {
        let mode_name = e.mode.as_deref().unwrap_or("joystick_move");
        let mode = AnalogMode::parse(mode_name).ok_or_else(|| {
            format!("analog.{name}.mode must be 'joystick_move' or 'absolute_mouse', got '{mode_name}'")
        })?;
        set.analog.insert(name, AnalogAction { title: e.title, mode, defaults: e.defaults });
    }
    Ok(set)
}
