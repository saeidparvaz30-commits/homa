use crate::aggregate::TraySummary;
use crate::model::AgentStatus;
use tauri::{AppHandle, Manager};

pub fn drive(app: &AppHandle, summary: &TraySummary) {
    let Some(w) = app.get_webview_window("overlay") else {
        return;
    };
    let attention = matches!(summary.top, AgentStatus::Waiting | AgentStatus::Idle);
    if attention {
        let _ = w.show();
    } else {
        let _ = w.hide();
    }
}
