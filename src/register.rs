use gmod::lua::{LuaString, State};
use gmod::lua_string;

use crate::actions::{Action, ActionSet, Kind};

const LUA_TNIL: i32 = 0;
const LUA_TSTRING: i32 = 4;
const LUA_TTABLE: i32 = 5;

unsafe fn string_at(lua: State, index: i32) -> String {
    lua.get_string(index).unwrap_or_default().into_owned()
}

unsafe fn field_string(lua: State, table: i32, key: LuaString, context: &str) -> Result<Option<String>, String> {
    lua.get_field(table, key);
    let result = match lua.lua_type(-1) {
        LUA_TNIL => Ok(None),
        LUA_TSTRING => Ok(Some(string_at(lua, -1))),
        _ => Err(format!("{context} must be a string")),
    };
    lua.pop();
    result
}

unsafe fn defaults(lua: State, table: i32, context: &str) -> Result<Vec<String>, String> {
    lua.get_field(table, lua_string!("default"));
    let list = lua.get_top();
    let result = match lua.lua_type(list) {
        LUA_TNIL => Ok(Vec::new()),
        LUA_TSTRING => Ok(vec![string_at(lua, list)]),
        LUA_TTABLE => (1..=lua.len(list))
            .map(|i| {
                lua.raw_geti(list, i);
                let entry = (lua.lua_type(-1) == LUA_TSTRING).then(|| string_at(lua, -1));
                lua.pop();
                entry.ok_or_else(|| format!("{context}.default entries must be strings"))
            })
            .collect(),
        _ => Err(format!("{context}.default must be a string or a list of strings")),
    };
    lua.pop();
    result
}

unsafe fn parse_action(lua: State, value: i32, name: &str, section: &str) -> Result<(Action, Option<String>), String> {
    let context = format!("{section}.{name}");
    match lua.lua_type(value) {
        LUA_TSTRING => Ok((Action { title: string_at(lua, value), kind: Kind::Button, defaults: Vec::new() }, None)),
        LUA_TTABLE => {
            let title = field_string(lua, value, lua_string!("title"), &format!("{context}.title"))?;
            let mode = field_string(lua, value, lua_string!("mode"), &format!("{context}.mode"))?;
            let defaults = defaults(lua, value, &context)?;
            Ok((Action { title: title.unwrap_or_else(|| name.to_owned()), kind: Kind::Button, defaults }, mode))
        }
        _ => Err(format!("{context} must be a title string or a table")),
    }
}

unsafe fn parse_section(lua: State, set: i32, key: LuaString, section: &str, out: &mut ActionSet) -> Result<(), String> {
    lua.get_field(set, key);
    let table = lua.get_top();
    match lua.lua_type(table) {
        LUA_TNIL => {
            lua.pop();
            return Ok(());
        }
        LUA_TTABLE => {}
        _ => {
            lua.pop();
            return Err(format!("'{section}' must be a table"));
        }
    }

    lua.push_nil();
    while lua.next(table) != 0 {
        let result = (|| {
            if lua.lua_type(-2) != LUA_TSTRING {
                return Err(format!("'{section}' keys must be action names"));
            }
            let name = string_at(lua, -2);
            let (mut action, mode) = parse_action(lua, lua.get_top(), &name, section)?;
            action.kind = match (section, mode.as_deref()) {
                ("buttons", _) => Kind::Button,
                ("triggers", _) => Kind::Trigger,
                (_, None | Some("joystick_move")) => Kind::Stick,
                (_, Some("absolute_mouse")) => Kind::Mouse,
                (_, Some(other)) => {
                    return Err(format!("analog.{name}.mode must be 'joystick_move' or 'absolute_mouse', got '{other}'"))
                }
            };
            match out.actions.insert(name.clone(), action) {
                Some(_) => Err(format!("action '{name}' is defined more than once")),
                None => Ok(()),
            }
        })();
        if let Err(e) = result {
            lua.pop_n(3);
            return Err(e);
        }
        lua.pop();
    }
    lua.pop();
    Ok(())
}

pub unsafe fn parse_action_set(lua: State, table: i32, name: &str) -> Result<ActionSet, String> {
    let mut set = ActionSet {
        title: field_string(lua, table, lua_string!("title"), "title")?.unwrap_or_else(|| name.to_owned()),
        ..Default::default()
    };
    parse_section(lua, table, lua_string!("buttons"), "buttons", &mut set)?;
    parse_section(lua, table, lua_string!("triggers"), "triggers", &mut set)?;
    parse_section(lua, table, lua_string!("analog"), "analog", &mut set)?;
    Ok(set)
}
