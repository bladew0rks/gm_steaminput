use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::actions::garrysmod_dir;

static CACHE: Mutex<Option<HashMap<(i32, i32, u32), String>>> = Mutex::new(None);

fn wine_path(path: &str) -> PathBuf {
    if path.starts_with('/') {
        PathBuf::from(format!("Z:{}", path.replace('/', "\\")))
    } else {
        PathBuf::from(path)
    }
}

pub fn material_path(origin: i32, size: i32, flags: u32, source: impl FnOnce() -> Option<String>) -> Option<String> {
    let key = (origin, size, flags);
    if let Some(cached) = CACHE.lock().unwrap().get_or_insert_with(HashMap::new).get(&key) {
        return Some(cached.clone());
    }

    let result = (|| {
        let source = wine_path(&source()?);
        let dir = garrysmod_dir().ok()?.join("data").join("steaminput").join("glyphs");
        std::fs::create_dir_all(&dir).ok()?;
        let name = format!("{origin}_{size}_{flags}.png");
        std::fs::copy(&source, dir.join(&name)).ok()?;
        Some(format!("../data/steaminput/glyphs/{name}"))
    })();

    if result.is_some() {
        CACHE.lock().unwrap().get_or_insert_with(HashMap::new).insert(key, result.clone().unwrap());
    }
    result
}
