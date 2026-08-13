use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub sound_enabled: bool,
    pub sound_on_idle: bool,
    #[serde(default)]
    pub overlay_x: Option<f64>,
    #[serde(default)]
    pub overlay_y: Option<f64>,
    #[serde(default)]
    pub overlay_w: Option<f64>,
    #[serde(default)]
    pub overlay_h: Option<f64>,
    #[serde(default = "default_true")]
    pub auto_resume_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sound_enabled: false,
            sound_on_idle: false,
            overlay_x: None,
            overlay_y: None,
            overlay_w: None,
            overlay_h: None,
            auto_resume_enabled: true,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_no_overlay_position() {
        let s = Settings::default();
        assert!(s.overlay_x.is_none() && s.overlay_y.is_none());
    }

    #[test]
    fn old_settings_files_without_overlay_keys_still_load() {
        // v1 wrote only the two sound flags. Loading must not fail on them.
        let json = r#"{"sound_enabled":true,"sound_on_idle":false}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert!(s.sound_enabled);
        assert!(s.overlay_x.is_none());
    }

    #[test]
    fn defaults_enable_auto_resume_and_have_no_size() {
        let s = Settings::default();
        assert!(s.auto_resume_enabled);
        assert!(s.overlay_w.is_none() && s.overlay_h.is_none());
    }

    #[test]
    fn v2_settings_files_without_new_keys_still_load_with_auto_resume_on() {
        let json = r#"{"sound_enabled":true,"sound_on_idle":false,"overlay_x":10.0,"overlay_y":20.0}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert!(s.auto_resume_enabled);
        assert!(s.overlay_w.is_none());
    }
}
