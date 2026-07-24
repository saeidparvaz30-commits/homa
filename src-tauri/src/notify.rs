use crate::aggregate::Transition;
use crate::model::AgentStatus;
use crate::settings::Settings;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub fn notify_transition(app: &AppHandle, t: &Transition, settings: &Settings) {
    let body = match t.to {
        AgentStatus::Waiting => format!("{} is waiting on you", t.name),
        AgentStatus::Idle => format!("{} is idle, feed it", t.name),
        _ => return,
    };
    let _ = app.notification().builder().title("Homa").body(&body).show();

    let play = settings.sound_enabled
        && (t.to == AgentStatus::Waiting || (t.to == AgentStatus::Idle && settings.sound_on_idle));
    if play {
        // Windows built-in async chime; spawned so it never blocks the loop.
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "[console]::beep(880,180)"])
            .spawn();
    }
}
