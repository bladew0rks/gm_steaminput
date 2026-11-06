use std::collections::{BTreeMap, BTreeSet};
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
    frame: u64,
    actions: Vec<(String, String)>,
    controllers: Vec<InputHandle>,
    held: BTreeSet<(InputHandle, usize)>,
    changed_at: BTreeMap<(InputHandle, usize), (u64, bool)>,
}

static TRACKER: Mutex<Tracker> = Mutex::new(Tracker {
    frame: 0,
    actions: Vec::new(),
    controllers: Vec::new(),
    held: BTreeSet::new(),
    changed_at: BTreeMap::new(),
});

pub fn set_actions(actions: Vec<(String, String)>) {
    let mut t = TRACKER.lock().unwrap();
    t.actions = actions;
    t.held.clear();
    t.changed_at.clear();
}

pub fn primary() -> Option<InputHandle> {
    TRACKER.lock().unwrap().controllers.first().copied()
}

fn lookup<R>(set: &str, action: &str, f: impl FnOnce(&Tracker, usize) -> R) -> Option<R> {
    let t = TRACKER.lock().unwrap();
    let i = t.actions.iter().position(|(s, a)| s == set && a == action)?;
    Some(f(&t, i))
}

pub fn is_down(handle: InputHandle, set: &str, action: &str) -> bool {
    lookup(set, action, |t, i| t.held.contains(&(handle, i))).unwrap_or(false)
}

pub fn changed_this_frame(handle: InputHandle, set: &str, action: &str, pressed: bool) -> bool {
    lookup(set, action, |t, i| t.changed_at.get(&(handle, i)) == Some(&(t.frame, pressed))).unwrap_or(false)
}

pub fn poll(api: &mut SteamInput) -> Vec<Event> {
    let mut t = TRACKER.lock().unwrap();
    t.frame += 1;
    let Tracker { frame, actions, controllers, held, changed_at } = &mut *t;
    let mut events = Vec::new();
    let current = api.connected_controllers();

    for &h in controllers.iter().filter(|h| !current.contains(h)) {
        for &(_, i) in held.iter().filter(|(held_h, _)| *held_h == h) {
            let (set, action) = &actions[i];
            events.push(Event::Released(h, set.clone(), action.clone()));
        }
        held.retain(|(held_h, _)| *held_h != h);
        changed_at.retain(|(held_h, _), _| *held_h != h);
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
            changed_at.insert((h, i), (*frame, down));
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
