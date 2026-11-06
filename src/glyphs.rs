use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::actions::garrysmod_dir;

static CACHE: Mutex<BTreeMap<(i32, i32, u32), String>> = Mutex::new(BTreeMap::new());

fn wine_path(path: &str) -> PathBuf {
    match path.starts_with('/') {
        true => PathBuf::from(format!("Z:{}", path.replace('/', "\\"))),
        false => PathBuf::from(path),
    }
}

pub fn material_path(origin: i32, size: i32, flags: u32, source: Option<String>) -> Option<String> {
    let key = (origin, size, flags);
    if let Some(cached) = CACHE.lock().unwrap().get(&key) {
        return Some(cached.clone());
    }

    let dir = garrysmod_dir().ok()?.join("data").join("steaminput").join("glyphs");
    std::fs::create_dir_all(&dir).ok()?;
    let name = format!("{origin}_{size}_{flags}.png");
    std::fs::copy(wine_path(&source?), dir.join(&name)).ok()?;

    let path = format!("../data/steaminput/glyphs/{name}");
    CACHE.lock().unwrap().insert(key, path.clone());
    Some(path)
}
