use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;

const MAX_ANALOG_ACTIONS: usize = 16;
const MAX_BUTTON_ACTIONS: usize = 128;

const CONTROLLER_TYPES: &[&str] = &[
    "controller_xbox360",
    "controller_xboxone",
    "controller_xboxelite",
    "controller_ps3",
    "controller_ps4",
    "controller_ps5",
    "controller_ps5_edge",
    "controller_switch_pro",
    "controller_switch2_pro",
    "controller_switch_joycon_pair",
    "controller_neptune",
    "controller_steamcontroller_gordon",
    "controller_generic",
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Button,
    Trigger,
    Stick,
    Mouse,
}

pub struct Action {
    pub title: String,
    pub kind: Kind,
    pub defaults: Vec<String>,
}

#[derive(Default)]
pub struct ActionSet {
    pub title: String,
    pub actions: BTreeMap<String, Action>,
}

struct Registry {
    sets: BTreeMap<String, ActionSet>,
    dirty: bool,
}

static REGISTRY: Mutex<Registry> = Mutex::new(Registry { sets: BTreeMap::new(), dirty: false });

fn button_source(name: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match name {
        "a" | "button_a" => ("button_diamond", "four_buttons", "button_a"),
        "b" | "button_b" => ("button_diamond", "four_buttons", "button_b"),
        "x" | "button_x" => ("button_diamond", "four_buttons", "button_x"),
        "y" | "button_y" => ("button_diamond", "four_buttons", "button_y"),
        "dpad_up" | "dpad_north" => ("dpad", "dpad", "dpad_north"),
        "dpad_down" | "dpad_south" => ("dpad", "dpad", "dpad_south"),
        "dpad_left" | "dpad_west" => ("dpad", "dpad", "dpad_west"),
        "dpad_right" | "dpad_east" => ("dpad", "dpad", "dpad_east"),
        "lb" | "left_bumper" => ("switch", "switches", "left_bumper"),
        "rb" | "right_bumper" => ("switch", "switches", "right_bumper"),
        "start" | "button_escape" => ("switch", "switches", "button_escape"),
        "back" | "select" | "button_menu" => ("switch", "switches", "button_menu"),
        "button_back_left" => ("switch", "switches", "button_back_left"),
        "button_back_right" => ("switch", "switches", "button_back_right"),
        "button_back_left_upper" => ("switch", "switches", "button_back_left_upper"),
        "button_back_right_upper" => ("switch", "switches", "button_back_right_upper"),
        "lt" | "left_trigger" => ("left_trigger", "trigger", "click"),
        "rt" | "right_trigger" => ("right_trigger", "trigger", "click"),
        "ls" | "left_stick_click" => ("joystick", "joystick_move", "click"),
        "rs" | "right_stick_click" => ("right_joystick", "joystick_move", "click"),
        _ => return None,
    })
}

fn analog_source(kind: Kind, name: &str) -> Option<(&'static str, &'static str)> {
    match (kind, name) {
        (Kind::Trigger, "lt" | "left_trigger") => Some(("left_trigger", "trigger")),
        (Kind::Trigger, "rt" | "right_trigger") => Some(("right_trigger", "trigger")),
        (Kind::Stick, "joystick" | "left_stick") => Some(("joystick", "joystick_move")),
        (Kind::Stick, "right_joystick" | "right_stick") => Some(("right_joystick", "joystick_move")),
        (Kind::Mouse, "joystick" | "left_stick") => Some(("joystick", "joystick_mouse")),
        (Kind::Mouse, "right_joystick" | "right_stick") => Some(("right_joystick", "joystick_mouse")),
        _ => None,
    }
}

pub fn steam_action_name(set: &str, action: &str) -> String {
    format!("{set}_{action}")
}

fn valid_identifier(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn validate(name: &str, set: &ActionSet) -> Result<(), String> {
    if !valid_identifier(name) {
        return Err(format!("set name '{name}' may only contain letters, digits and underscores"));
    }

    let mut taken = BTreeMap::new();
    for (action, def) in &set.actions {
        if !valid_identifier(action) {
            return Err(format!("action name '{action}' may only contain letters, digits and underscores"));
        }
        for d in &def.defaults {
            if def.kind == Kind::Button {
                button_source(d).ok_or_else(|| format!("'{action}': unknown default '{d}'"))?;
                continue;
            }
            let (group, _) = analog_source(def.kind, d).ok_or_else(|| format!("'{action}': unknown default '{d}'"))?;
            if let Some(other) = taken.insert(group, action) {
                return Err(format!("'{action}' and '{other}' both default to '{group}'"));
            }
        }
    }
    Ok(())
}

pub fn register(name: &str, set: ActionSet) -> Result<(), String> {
    validate(name, &set)?;
    let mut reg = REGISTRY.lock().unwrap();

    let others = reg.sets.iter().filter(|(n, _)| *n != name).flat_map(|(_, s)| s.actions.values());
    let (buttons, analog) = others.chain(set.actions.values()).fold((0, 0), |(b, a), action| match action.kind {
        Kind::Button => (b + 1, a),
        _ => (b, a + 1),
    });
    if analog > MAX_ANALOG_ACTIONS {
        return Err(format!("too many analog actions across all sets (max {MAX_ANALOG_ACTIONS})"));
    }
    if buttons > MAX_BUTTON_ACTIONS {
        return Err(format!("too many button actions across all sets (max {MAX_BUTTON_ACTIONS})"));
    }

    reg.sets.insert(name.to_owned(), set);
    reg.dirty = true;
    Ok(())
}

pub fn take_dirty() -> bool {
    std::mem::take(&mut REGISTRY.lock().unwrap().dirty)
}

struct Vdf {
    out: String,
    depth: usize,
}

impl Vdf {
    fn quote(s: &str) -> String {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    }

    fn kv(&mut self, k: &str, v: &str) {
        let _ = writeln!(self.out, "{}{}\t{}", "\t".repeat(self.depth), Self::quote(k), Self::quote(v));
    }

    fn open(&mut self, k: &str) {
        let indent = "\t".repeat(self.depth);
        let _ = writeln!(self.out, "{indent}{}\n{indent}{{", Self::quote(k));
        self.depth += 1;
    }

    fn close(&mut self) {
        self.depth -= 1;
        let _ = writeln!(self.out, "{}}}", "\t".repeat(self.depth));
    }

    fn block(&mut self, k: &str, f: impl FnOnce(&mut Self)) {
        self.open(k);
        f(self);
        self.close();
    }
}

#[derive(Default)]
struct Group {
    mode: &'static str,
    inputs: BTreeMap<&'static str, Vec<String>>,
    gameaction: Option<String>,
}

fn build_groups(set_name: &str, set: &ActionSet) -> BTreeMap<&'static str, Group> {
    let mut groups: BTreeMap<&'static str, Group> = BTreeMap::new();
    for (action, def) in &set.actions {
        let steam_name = steam_action_name(set_name, action);
        for d in &def.defaults {
            if def.kind == Kind::Button {
                let (source, mode, input) = button_source(d).unwrap();
                let g = groups.entry(source).or_default();
                if g.mode.is_empty() {
                    g.mode = mode;
                }
                g.inputs.entry(input).or_default().push(format!("game_action {set_name} {steam_name}, , "));
            } else {
                let (source, mode) = analog_source(def.kind, d).unwrap();
                let g = groups.entry(source).or_default();
                g.mode = mode;
                g.gameaction = Some(steam_name.clone());
            }
        }
    }
    groups
}

fn remap_for_controller(controller: &str, source: &'static str, mode: &'static str) -> (&'static str, &'static str) {
    match (controller, source, mode) {
        ("controller_steamcontroller_gordon", "right_joystick", "joystick_mouse") => ("right_trackpad", "absolute_mouse"),
        ("controller_steamcontroller_gordon", "right_joystick", _) => ("right_trackpad", mode),
        _ => (source, mode),
    }
}

fn controller_config(controller: &str, sets: &BTreeMap<String, ActionSet>, revision: u32) -> String {
    let mut v = Vdf { out: String::new(), depth: 0 };
    let mut presets = Vec::new();
    let mut next_id = 0u32;

    v.open("controller_mappings");
    v.kv("version", "3");
    v.kv("revision", &revision.to_string());
    v.kv("title", "#Title_SteamInput");
    v.kv("description", "#Description_SteamInput");
    v.kv("creator", "-1");
    v.kv("controller_type", controller);

    for (set_name, set) in sets {
        let mut bindings = Vec::new();
        for (source, group) in build_groups(set_name, set) {
            let (source, mode) = remap_for_controller(controller, source, group.mode);
            let id = next_id;
            next_id += 1;
            bindings.push((id, source));

            v.block("group", |v| {
                v.kv("id", &id.to_string());
                v.kv("mode", mode);
                v.block("inputs", |v| {
                    for (input, binds) in &group.inputs {
                        v.block(input, |v| {
                            v.block("activators", |v| {
                                v.block("Full_Press", |v| {
                                    v.block("bindings", |v| binds.iter().for_each(|b| v.kv("binding", b)));
                                })
                            })
                        });
                    }
                });
                if let Some(action) = &group.gameaction {
                    v.block("settings", |v| {
                        v.kv("virtual_mode", "1");
                        if mode == "joystick_mouse" {
                            v.kv("output_joystick", "2");
                        }
                    });
                    v.block("gameactions", |v| v.kv(set_name, action));
                }
            });
        }
        presets.push((set_name, bindings));
    }

    for (i, (set_name, bindings)) in presets.into_iter().enumerate() {
        v.block("preset", |v| {
            v.kv("id", &i.to_string());
            v.kv("name", set_name);
            v.block("group_source_bindings", |v| {
                for (id, source) in bindings {
                    v.kv(&id.to_string(), &format!("{source} active"));
                }
            });
        });
    }

    v.block("settings", |_| {});
    v.close();
    v.out
}

fn manifest(sets: &BTreeMap<String, ActionSet>) -> String {
    let mut v = Vdf { out: String::new(), depth: 0 };
    let mut loc = vec![
        ("Title_SteamInput".to_owned(), "gm_steaminput".to_owned()),
        ("Description_SteamInput".to_owned(), "Default layout generated by gm_steaminput".to_owned()),
    ];

    v.open("Action Manifest");
    v.block("configurations", |v| {
        for controller in CONTROLLER_TYPES {
            v.block(controller, |v| v.block("0", |v| v.kv("path", &format!("{controller}.vdf"))));
        }
    });

    v.block("actions", |v| {
        for (set_name, set) in sets {
            let set_token = format!("Set_{set_name}");
            loc.push((set_token.clone(), set.title.clone()));

            v.block(set_name, |v| {
                v.kv("title", &format!("#{set_token}"));
                v.kv("legacy_set", "0");

                let mut section = |v: &mut Vdf, name: &str, kinds: &[Kind]| {
                    v.block(name, |v| {
                        for (action, def) in set.actions.iter().filter(|(_, a)| kinds.contains(&a.kind)) {
                            let steam_name = steam_action_name(set_name, action);
                            let token = format!("Action_{steam_name}");
                            loc.push((token.clone(), def.title.clone()));
                            match def.kind {
                                Kind::Stick | Kind::Mouse => v.block(&steam_name, |v| {
                                    v.kv("title", &format!("#{token}"));
                                    v.kv("input_mode", if def.kind == Kind::Mouse { "absolute_mouse" } else { "joystick_move" });
                                }),
                                _ => v.kv(&steam_name, &format!("#{token}")),
                            }
                        }
                    });
                };
                section(v, "StickPadGyro", &[Kind::Stick, Kind::Mouse]);
                section(v, "AnalogTrigger", &[Kind::Trigger]);
                section(v, "Button", &[Kind::Button]);
            });
        }
    });

    v.block("localization", |v| v.block("english", |v| loc.iter().for_each(|(k, val)| v.kv(k, val))));
    v.close();
    v.out
}

fn fnv1a(s: &str) -> u32 {
    s.bytes().fold(0x811c9dc5u32, |h, b| (h ^ b as u32).wrapping_mul(0x01000193))
}

pub fn garrysmod_dir() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().into_iter().flat_map(|cwd| [cwd.join("garrysmod"), cwd]);
    let exe = std::env::current_exe().into_iter().flat_map(|exe| {
        exe.ancestors().skip(1).take(4).map(|d| d.join("garrysmod")).collect::<Vec<_>>()
    });
    cwd.chain(exe)
        .find(|c| c.join("gameinfo.txt").is_file())
        .ok_or_else(|| "could not locate the garrysmod directory".to_owned())
}

pub fn write_files() -> Result<String, String> {
    let reg = REGISTRY.lock().unwrap();
    if reg.sets.is_empty() {
        return Err("no action sets registered".into());
    }

    let dir = garrysmod_dir()?.join("data").join("steaminput");
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create {}: {e}", dir.display()))?;
    let write = |name: &str, contents: String| {
        let path = dir.join(name);
        std::fs::write(&path, contents).map_err(|e| format!("failed to write {}: {e}", path.display()))?;
        Ok::<_, String>(path)
    };

    let manifest = manifest(&reg.sets);
    let revision = fnv1a(&manifest) & 0x7fff_ffff;
    for controller in CONTROLLER_TYPES {
        write(&format!("{controller}.vdf"), controller_config(controller, &reg.sets, revision))?;
    }

    let path = write("manifest.vdf", manifest)?;
    path.to_str().map(str::to_owned).ok_or_else(|| "manifest path is not valid UTF-8".into())
}
