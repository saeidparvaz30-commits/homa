use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sysinfo::System;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager};

use crate::aggregate::{summarize, transitions};
use crate::alias::{self, Aliases};
use crate::{
    derive::repo_from_cwd, enrich::enrich_from_path, liveness::pid_alive, mapper::map_status,
    model::AgentState, session::parse_session,
};

pub type Shared = Arc<Mutex<Vec<AgentState>>>;

pub fn sessions_dir() -> PathBuf {
    if let Ok(p) = std::env::var("HOMA_SESSIONS_DIR") {
        return PathBuf::from(p);
    }
    dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("sessions")
}

fn transcript_path(cwd: &str, session_id: &str) -> PathBuf {
    let slug = cwd.replace(['\\', '/', ':'], "-").replace(' ', "-");
    dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("projects")
        .join(slug)
        .join(format!("{session_id}.jsonl"))
}

pub fn scan_once_with(dir: &Path, enrich: bool, aliases: &Aliases) -> Vec<AgentState> {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All);
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let s = match parse_session(&bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };
        crate::probe::record(&s.status);
        let alive = pid_alive(s.pid, &sys);
        let status = map_status(&s.status, alive);
        let mut st = AgentState {
            pid: s.pid,
            session_id: s.session_id.clone(),
            name: s.name.clone(),
            cwd: s.cwd.clone(),
            repo: repo_from_cwd(&s.cwd),
            branch: None,
            status,
            raw_status: s.status.clone(),
            started_at: s.started_at,
            status_updated_at: s.status_updated_at,
            model: None,
            context_pct: None,
            last_activity: None,
            ended_at: None,
        };
        if enrich && alive {
            let e = enrich_from_path(&transcript_path(&s.cwd, &s.session_id));
            st.model = e.model;
            st.context_pct = e.context_pct;
            st.branch = e.branch;
        }
        out.push(st);
    }
    alias::resolve(&mut out, aliases);
    out.sort_by(|a, b| {
        b.status
            .priority()
            .cmp(&a.status.priority())
            .then(a.name.cmp(&b.name))
    });
    out
}

pub fn scan_once(dir: &Path, enrich: bool) -> Vec<AgentState> {
    scan_once_with(dir, enrich, &alias::load())
}

pub fn start_watching(app: AppHandle, shared: Shared) {
    std::thread::spawn(move || {
        let dir = sessions_dir();
        let _ = std::fs::create_dir_all(&dir);
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher: RecommendedWatcher = match notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        }) {
            Ok(w) => w,
            Err(_) => return,
        };
        let _ = watcher.watch(&dir, RecursiveMode::NonRecursive);
        loop {
            let window_visible = app
                .get_webview_window("main")
                .and_then(|w| w.is_visible().ok())
                .unwrap_or(false);
            let prev = { shared.lock().unwrap().clone() };
            let mut next = scan_once(&dir, window_visible);
            crate::reap::reap(&prev, &mut next, crate::reap::now_ms(), crate::reap::ENDED_TTL_MS);
            if next != prev {
                let summary = summarize(&next);
                let settings = crate::settings::Settings::load();
                for t in transitions(&prev, &next) {
                    let _ = app.emit("agent-transition", &t);
                    crate::notify::notify_transition(&app, &t, &settings);
                }
                let _ = app.emit("agents-updated", &next);
                let _ = app.emit("tray-summary", &summary);
                crate::tray::apply(&app, &summary);
                *shared.lock().unwrap() = next;
            }
            // Drain any queued file events (non-blocking), then wait the reconcile interval.
            while rx.try_recv().is_ok() {}
            std::thread::sleep(Duration::from_millis(2000));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_once_applies_aliases_to_names() {
        let dir = std::env::temp_dir().join("homa-poller-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Use this test process's own pid so the session reads as alive.
        let pid = std::process::id();
        let cwd = "C:\\Homa\\Test\\Folder";
        let json = format!(
            r#"{{"pid":{pid},"sessionId":"abc","cwd":"C:\\Homa\\Test\\Folder","startedAt":1,"name":"agent-folder-61","status":"busy","statusUpdatedAt":1}}"#
        );
        std::fs::write(dir.join(format!("{pid}.json")), json).unwrap();

        let mut aliases = crate::alias::Aliases::new();
        aliases.insert(crate::alias::normalize_key(cwd), "renamed".into());

        let got = scan_once_with(&dir, false, &aliases);
        assert_eq!(got.len(), 1, "expected the fixture session to be picked up");
        assert_eq!(got[0].name, "renamed");
    }

    #[test]
    fn scan_once_without_alias_keeps_claude_name() {
        let dir = std::env::temp_dir().join("homa-poller-test-noalias");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let pid = std::process::id();
        let json = format!(
            r#"{{"pid":{pid},"sessionId":"abc","cwd":"C:\\Homa\\Other","startedAt":1,"name":"agent-other-9","status":"busy","statusUpdatedAt":1}}"#
        );
        std::fs::write(dir.join(format!("{pid}.json")), json).unwrap();

        let got = scan_once_with(&dir, false, &crate::alias::Aliases::new());
        assert_eq!(got[0].name, "agent-other-9");
    }
}
