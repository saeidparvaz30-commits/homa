use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub sound_enabled: bool,
    pub sound_on_idle: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sound_enabled: false,
            sound_on_idle: false,
        }
    }
}

fn path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("homa")
        .join("settings.json")
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn save(&self) {
        let p = path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, s);
        }
    }
}
