use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;

const MAX_ANALOG_ACTIONS: usize = 16;
const MAX_DIGITAL_ACTIONS: usize = 128;

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
pub enum AnalogMode {
    JoystickMove,
    AbsoluteMouse,
}

impl AnalogMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "joystick_move" => Some(Self::JoystickMove),
            "absolute_mouse" => Some(Self::AbsoluteMouse),
            _ => None,
        }
    }

    fn input_mode(self) -> &'static str {
        match self {
            Self::JoystickMove => "joystick_move",
            Self::AbsoluteMouse => "absolute_mouse",
        }
    }

    fn group_mode(self) -> &'static str {
        match self {
            Self::JoystickMove => "joystick_move",
            Self::AbsoluteMouse => "joystick_mouse",
        }
    }
}

pub struct Action {
    pub title: String,
    pub defaults: Vec<String>,
}

pub struct AnalogAction {
    pub title: String,
    pub mode: AnalogMode,
    pub defaults: Vec<String>,
}

#[derive(Default)]
pub struct ActionSet {
    pub title: String,
    pub buttons: BTreeMap<String, Action>,
    pub analog: BTreeMap<String, AnalogAction>,
    pub triggers: BTreeMap<String, Action>,
}

struct Registry {
    sets: BTreeMap<String, ActionSet>,
    dirty: bool,
}

static REGISTRY: Mutex<Registry> = Mutex::new(Registry {
    sets: BTreeMap::new(),
    dirty: false,
});

struct DigitalSource {
    group: &'static str,
    mode: &'static str,
    input: &'static str,
}

fn digital_source(name: &str) -> Option<DigitalSource> {
    let (group, mode, input) = match name {
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
    };
    Some(DigitalSource { group, mode, input })
}

fn stick_source(name: &str) -> Option<&'static str> {
    match name {
        "joystick" | "left_stick" => Some("joystick"),
        "right_joystick" | "right_stick" => Some("right_joystick"),
        _ => None,
    }
}

fn trigger_source(name: &str) -> Option<&'static str> {
    match name {
        "lt" | "left_trigger" => Some("left_trigger"),
        "rt" | "right_trigger" => Some("right_trigger"),
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
        return Err(format!("action set name '{name}' may only contain letters, digits and underscores"));
    }

    let mut analog_sources = BTreeMap::new();
    let mut check_analog = |action: &str, source: &'static str| match analog_sources.insert(source, action.to_owned()) {
        Some(other) => Err(format!("'{action}' and '{other}' both default to '{source}'")),
        None => Ok(()),
    };

    for (action, def) in &set.buttons {
        if !valid_identifier(action) {
            return Err(format!("button action name '{action}' is invalid"));
        }
        for d in &def.defaults {
            digital_source(d).ok_or_else(|| format!("button '{action}': unknown default '{d}'"))?;
        }
    }
    for (action, def) in &set.analog {
        if !valid_identifier(action) {
            return Err(format!("analog action name '{action}' is invalid"));
        }
        for d in &def.defaults {
            let source = stick_source(d).ok_or_else(|| format!("analog '{action}': unknown default '{d}'"))?;
            check_analog(action, source)?;
        }
    }
    for (action, def) in &set.triggers {
        if !valid_identifier(action) {
            return Err(format!("trigger action name '{action}' is invalid"));
        }
        for d in &def.defaults {
            let source = trigger_source(d).ok_or_else(|| format!("trigger '{action}': unknown default '{d}'"))?;
            check_analog(action, source)?;
        }
    }

    let names: Vec<&String> = set.buttons.keys().chain(set.analog.keys()).chain(set.triggers.keys()).collect();
    for (i, a) in names.iter().enumerate() {
        if names[i + 1..].contains(a) {
            return Err(format!("action '{a}' is defined more than once"));
        }
    }
    Ok(())
}

pub fn register(name: &str, set: ActionSet) -> Result<(), String> {
    validate(name, &set)?;
    let mut reg = REGISTRY.lock().unwrap();

    let others = reg.sets.iter().filter(|(n, _)| n.as_str() != name).map(|(_, s)| s);
    let (mut analog, mut digital) = (set.analog.len() + set.triggers.len(), set.buttons.len());
    for s in others {
        analog += s.analog.len() + s.triggers.len();
        digital += s.buttons.len();
    }
    if analog > MAX_ANALOG_ACTIONS {
        return Err(format!("too many analog actions across all sets ({analog} > {MAX_ANALOG_ACTIONS})"));
    }
    if digital > MAX_DIGITAL_ACTIONS {
        return Err(format!("too many button actions across all sets ({digital} > {MAX_DIGITAL_ACTIONS})"));
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
    fn new() -> Self {
        Vdf { out: String::new(), depth: 0 }
    }

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
}

#[derive(Default)]
struct Group {
    mode: &'static str,
    inputs: BTreeMap<&'static str, Vec<String>>,
    gameaction: Option<String>,
}

fn build_groups(set_name: &str, set: &ActionSet) -> BTreeMap<&'static str, Group> {
    let mut groups: BTreeMap<&'static str, Group> = BTreeMap::new();

    for (action, def) in &set.analog {
        for d in &def.defaults {
            let source = stick_source(d).unwrap();
            let g = groups.entry(source).or_default();
            g.mode = def.mode.group_mode();
            g.gameaction = Some(steam_action_name(set_name, action));
        }
    }
    for (action, def) in &set.triggers {
        for d in &def.defaults {
            let source = trigger_source(d).unwrap();
            let g = groups.entry(source).or_default();
            g.mode = "trigger";
            g.gameaction = Some(steam_action_name(set_name, action));
        }
    }
    for (action, def) in &set.buttons {
        for d in &def.defaults {
            let src = digital_source(d).unwrap();
            let g = groups.entry(src.group).or_default();
            if g.mode.is_empty() {
                g.mode = src.mode;
            }
            g.inputs
                .entry(src.input)
                .or_default()
                .push(format!("game_action {set_name} {}, , ", steam_action_name(set_name, action)));
        }
    }
    groups
}

fn remap_for_controller(controller: &str, source: &'static str, mode: &'static str) -> (&'static str, &'static str) {
    match (controller, source) {
        ("controller_steamcontroller_gordon", "right_joystick") => (
            "right_trackpad",
            if mode == "joystick_mouse" { "absolute_mouse" } else { mode },
        ),
        _ => (source, mode),
    }
}

fn controller_config(controller: &str, sets: &BTreeMap<String, ActionSet>, revision: u32) -> String {
    let mut v = Vdf::new();
    v.open("controller_mappings");
    v.kv("version", "3");
    v.kv("revision", &revision.to_string());
    v.kv("title", "#Title_SteamInput");
    v.kv("description", "#Description_SteamInput");
    v.kv("creator", "-1");
    v.kv("controller_type", controller);

    let mut next_id = 0u32;
    let mut presets = Vec::new();

    for (set_name, set) in sets {
        let mut bindings = Vec::new();
        for (source, group) in build_groups(set_name, set) {
            let (source, mode) = remap_for_controller(controller, source, group.mode);
            let id = next_id;
            next_id += 1;

            v.open("group");
            v.kv("id", &id.to_string());
            v.kv("mode", mode);
            v.open("inputs");
            for (input, binds) in &group.inputs {
                v.open(input);
                v.open("activators");
                v.open("Full_Press");
                v.open("bindings");
                for b in binds {
                    v.kv("binding", b);
                }
                v.close();
                v.close();
                v.close();
                v.close();
            }
            v.close();
            if let Some(action) = &group.gameaction {
                v.open("settings");
                v.kv("virtual_mode", "1");
                if mode == "joystick_mouse" {
                    v.kv("output_joystick", "2");
                }
                v.close();
                v.open("gameactions");
                v.kv(set_name, action);
                v.close();
            }
            v.close();
            bindings.push((id, source));
        }
        presets.push((set_name, bindings));
    }

    for (i, (set_name, bindings)) in presets.into_iter().enumerate() {
        v.open("preset");
        v.kv("id", &i.to_string());
        v.kv("name", set_name);
        v.open("group_source_bindings");
        for (id, source) in bindings {
            v.kv(&id.to_string(), &format!("{source} active"));
        }
        v.close();
        v.close();
    }

    v.open("settings");
    v.close();
    v.close();
    v.out
}

fn manifest(sets: &BTreeMap<String, ActionSet>) -> String {
    let mut v = Vdf::new();
    v.open("Action Manifest");

    v.open("configurations");
    for controller in CONTROLLER_TYPES {
        v.open(controller);
        v.open("0");
        v.kv("path", &format!("{controller}.vdf"));
        v.close();
        v.close();
    }
    v.close();

    let mut loc = vec![
        ("Title_SteamInput".to_owned(), "gm_steaminput".to_owned()),
        ("Description_SteamInput".to_owned(), "Default layout generated by gm_steaminput".to_owned()),
    ];

    v.open("actions");
    for (set_name, set) in sets {
        v.open(set_name);
        let set_token = format!("Set_{set_name}");
        v.kv("title", &format!("#{set_token}"));
        v.kv("legacy_set", "0");
        loc.push((set_token, set.title.clone()));

        let mut token = |action: &str, title: &str| {
            let t = format!("Action_{}", steam_action_name(set_name, action));
            loc.push((t.clone(), title.to_owned()));
            format!("#{t}")
        };

        v.open("StickPadGyro");
        for (action, def) in &set.analog {
            v.open(&steam_action_name(set_name, action));
            v.kv("title", &token(action, &def.title));
            v.kv("input_mode", def.mode.input_mode());
            v.close();
        }
        v.close();

        v.open("AnalogTrigger");
        for (action, def) in &set.triggers {
            v.kv(&steam_action_name(set_name, action), &token(action, &def.title));
        }
        v.close();

        v.open("Button");
        for (action, def) in &set.buttons {
            v.kv(&steam_action_name(set_name, action), &token(action, &def.title));
        }
        v.close();

        v.close();
    }
    v.close();

    v.open("localization");
    v.open("english");
    for (k, val) in &loc {
        v.kv(k, val);
    }
    v.close();
    v.close();

    v.close();
    v.out
}

fn fnv1a(s: &str) -> u32 {
    s.bytes().fold(0x811c9dc5u32, |h, b| (h ^ b as u32).wrapping_mul(0x01000193))
}

pub fn garrysmod_dir() -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("garrysmod"));
        candidates.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        for dir in exe.ancestors().skip(1).take(4) {
            candidates.push(dir.join("garrysmod"));
        }
    }
    candidates
        .into_iter()
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

    let manifest = manifest(&reg.sets);
    let revision = fnv1a(&manifest) & 0x7fff_ffff;

    for controller in CONTROLLER_TYPES {
        let path = dir.join(format!("{controller}.vdf"));
        std::fs::write(&path, controller_config(controller, &reg.sets, revision))
            .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    }

    let path = dir.join("manifest.vdf");
    std::fs::write(&path, manifest).map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    path.to_str().map(str::to_owned).ok_or_else(|| "manifest path is not valid UTF-8".into())
}
