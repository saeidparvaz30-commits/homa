use crate::settings::Settings;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};

/// The overlay is a permanent dashboard, not an alert: it is shown once at
/// startup and hidden only by its own minimize button. Attention is carried
/// by dot colour, the tray icon, toasts, and sound.
pub fn restore_and_show(app: &AppHandle) {
    let Some(w) = app.get_webview_window("overlay") else {
        return;
    };
    let s = Settings::load();
    if let (Some(x), Some(y)) = (s.overlay_x, s.overlay_y) {
        let _ = w.set_position(PhysicalPosition::new(x as i32, y as i32));
    }
    if let (Some(wd), Some(ht)) = (s.overlay_w, s.overlay_h) {
        let _ = w.set_size(PhysicalSize::new(wd as u32, ht as u32));
    }
    let _ = w.show();
}

pub fn remember_position(x: f64, y: f64) {
    let mut s = Settings::load();
    if s.overlay_x == Some(x) && s.overlay_y == Some(y) {
        return;
    }
    s.overlay_x = Some(x);
    s.overlay_y = Some(y);
    s.save();
}

pub fn remember_size(w: f64, h: f64) {
    let mut s = Settings::load();
    if s.overlay_w == Some(w) && s.overlay_h == Some(h) {
        return;
    }
    s.overlay_w = Some(w);
    s.overlay_h = Some(h);
    s.save();
}
