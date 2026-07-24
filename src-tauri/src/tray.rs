use crate::aggregate::TraySummary;
use crate::model::AgentStatus;
use std::path::PathBuf;
use tauri::image::Image;
use tauri::{AppHandle, Manager};

fn icon_name(top: AgentStatus) -> &'static str {
    match top {
        AgentStatus::Waiting => "tray-waiting.png",
        AgentStatus::Idle => "tray-idle.png",
        AgentStatus::Working => "tray-calm.png",
        AgentStatus::Ended => "tray-ended.png",
    }
}

fn icon_path(app: &AppHandle, name: &str) -> PathBuf {
    // Bundled: resources/icons. Dev: source-tree icons via CARGO_MANIFEST_DIR.
    if let Ok(dir) = app.path().resource_dir() {
        let p = dir.join("icons").join(name);
        if p.exists() {
            return p;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("icons").join(name)
}

pub fn apply(app: &AppHandle, summary: &TraySummary) {
    let Some(tray) = app.tray_by_id("homa-tray") else {
        return;
    };
    if let Ok(img) = Image::from_path(icon_path(app, icon_name(summary.top))) {
        let _ = tray.set_icon(Some(img));
    }
    let tip = format!(
        "Homa  waiting {}  idle {}  working {}",
        summary.waiting, summary.idle, summary.working
    );
    let _ = tray.set_tooltip(Some(&tip));
}
