use std::collections::BTreeSet;
use std::sync::Mutex;

use crate::actions::steam_action_name;
use crate::steam_api::{input_type_name, InputHandle, SteamInput};

pub enum Event {
    Connected(InputHandle, &'static str),
    Disconnected(InputHandle),
    Pressed(InputHandle, String, String),
    Released(InputHandle, String, String),
}

struct Tracker {
    actions: Vec<(String, String)>,
    controllers: Vec<InputHandle>,
    held: BTreeSet<(InputHandle, usize)>,
}

static TRACKER: Mutex<Tracker> = Mutex::new(Tracker { actions: Vec::new(), controllers: Vec::new(), held: BTreeSet::new() });

pub fn set_actions(actions: Vec<(String, String)>) {
    let mut t = TRACKER.lock().unwrap();
    t.actions = actions;
    t.held.clear();
}

pub fn poll(api: &mut SteamInput) -> Vec<Event> {
    let mut t = TRACKER.lock().unwrap();
    let Tracker { actions, controllers, held } = &mut *t;
    let mut events = Vec::new();
    let current = api.connected_controllers();

    for &h in controllers.iter().filter(|h| !current.contains(h)) {
        for &(_, i) in held.iter().filter(|(held_h, _)| *held_h == h) {
            let (set, action) = &actions[i];
            events.push(Event::Released(h, set.clone(), action.clone()));
        }
        held.retain(|(held_h, _)| *held_h != h);
        events.push(Event::Disconnected(h));
    }
    for &h in current.iter().filter(|h| !controllers.contains(h)) {
        events.push(Event::Connected(h, input_type_name(api.input_type(h))));
    }
    *controllers = current;

    for &h in controllers.iter() {
        for (i, (set, action)) in actions.iter().enumerate() {
            let down = api.digital_action(h, &steam_action_name(set, action)).state;
            if down == held.contains(&(h, i)) {
                continue;
            }
            if down {
                held.insert((h, i));
                events.push(Event::Pressed(h, set.clone(), action.clone()));
            } else {
                held.remove(&(h, i));
                events.push(Event::Released(h, set.clone(), action.clone()));
            }
        }
    }
    events
}
