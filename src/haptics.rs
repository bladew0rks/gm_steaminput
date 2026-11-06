use crate::steam_api::{InputHandle, SteamInput, TriggerCommand, LEFT, LEFT_TRIGGER, RIGHT, RIGHT_TRIGGER};

const STEAM_CONTROLLER: i32 = 1;
const XBOX_ONE: i32 = 3;
const PS5: i32 = 13;
const STEAM_DECK: i32 = 14;

const MODE_OFF: i32 = 0;
const MODE_FEEDBACK: i32 = 1;
const MODE_WEAPON: i32 = 2;
const MODE_VIBRATION: i32 = 3;

const CLICK_SECONDS: f64 = 0.05;

#[derive(Clone, Copy, PartialEq)]
pub enum Side {
    Left,
    Right,
    Both,
}

impl Side {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "left" => Some(Side::Left),
            "right" => Some(Side::Right),
            "both" => Some(Side::Both),
            _ => None,
        }
    }

    fn has_left(self) -> bool {
        self != Side::Right
    }

    fn has_right(self) -> bool {
        self != Side::Left
    }

    pub fn haptic_location(self) -> i32 {
        match self {
            Side::Left => 1,
            Side::Right => 2,
            Side::Both => 3,
        }
    }

    fn motors(self, left: usize, right: usize, level: u16) -> [Option<u16>; 4] {
        let mut levels = [None; 4];
        if self.has_left() {
            levels[left] = Some(level);
        }
        if self.has_right() {
            levels[right] = Some(level);
        }
        levels
    }

    fn triggers(self) -> impl Iterator<Item = usize> {
        [(self.has_left(), 0), (self.has_right(), 1)].into_iter().filter(|(on, _)| *on).map(|(_, t)| t)
    }
}

fn scale(strength: f64, max: f64) -> f64 {
    strength.clamp(0.0, 1.0) * max
}

pub fn trigger_command(mode: &str, a: u8, b: u8, c: u8) -> Option<TriggerCommand> {
    Some(match mode {
        "off" => TriggerCommand::default(),
        "feedback" => TriggerCommand::new(MODE_FEEDBACK, &[a.min(9), b.min(8)]),
        "weapon" => TriggerCommand::new(MODE_WEAPON, &[a.clamp(2, 7), b.clamp(a.clamp(2, 7) + 1, 8), c.min(8)]),
        "vibration" => TriggerCommand::new(MODE_VIBRATION, &[a.min(9), b.min(8), c]),
        _ => return None,
    })
}

pub fn set_trigger_effect(api: &mut SteamInput, handle: InputHandle, side: Side, command: TriggerCommand, seconds: Option<f64>) {
    for trigger in side.triggers() {
        api.set_trigger_effect(handle, trigger, command, seconds);
    }
}

pub fn haptic(api: &mut SteamInput, handle: InputHandle, effect: &str, strength: f64, seconds: Option<f64>, side: Side) -> Result<&'static str, String> {
    let kind = api.input_type(handle);
    let level = scale(strength, u16::MAX as f64) as u16;
    let rumble = |api: &mut SteamInput, seconds| {
        api.set_motors(handle, side.motors(LEFT, RIGHT, level), seconds);
        "rumble"
    };

    Ok(match (effect, kind) {
        ("rumble", _) => rumble(api, seconds),

        ("trigger", XBOX_ONE) => {
            api.set_motors(handle, side.motors(LEFT_TRIGGER, RIGHT_TRIGGER, level), seconds);
            "impulse_trigger"
        }
        ("trigger", PS5) => {
            let command = match level {
                0 => TriggerCommand::new(MODE_OFF, &[]),
                _ => TriggerCommand::new(MODE_VIBRATION, &[0, scale(strength, 8.0).ceil() as u8, 40]),
            };
            set_trigger_effect(api, handle, side, command, seconds);
            "adaptive_trigger"
        }
        ("trigger", _) => rumble(api, seconds),

        ("click", STEAM_CONTROLLER | STEAM_DECK) => {
            api.simple_haptic_event(handle, side.haptic_location(), scale(strength, 255.0) as u8, 0);
            "haptic"
        }
        ("click", _) => rumble(api, Some(seconds.unwrap_or(CLICK_SECONDS))),

        ("resistance", PS5) => {
            let command = match level {
                0 => TriggerCommand::new(MODE_OFF, &[]),
                _ => TriggerCommand::new(MODE_FEEDBACK, &[0, scale(strength, 8.0).ceil() as u8]),
            };
            set_trigger_effect(api, handle, side, command, seconds);
            "adaptive_trigger"
        }
        ("resistance", _) => "none",

        _ => return Err(format!("unknown effect '{effect}', expected rumble, trigger, click or resistance")),
    })
}
