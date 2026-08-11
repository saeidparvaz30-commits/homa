use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sysinfo::System;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager};

use crate::aggregate::{summarize, transitions};
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

pub fn scan_once(dir: &Path, enrich: bool) -> Vec<AgentState> {
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
    out.sort_by(|a, b| {
        b.status
            .priority()
            .cmp(&a.status.priority())
            .then(a.name.cmp(&b.name))
    });
    out
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
            let next = scan_once(&dir, window_visible);
            let prev = { shared.lock().unwrap().clone() };
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
                crate::overlay::drive(&app, &summary);
                *shared.lock().unwrap() = next;
            }
            // Drain any queued file events (non-blocking), then wait the reconcile interval.
            while rx.try_recv().is_ok() {}
            std::thread::sleep(Duration::from_millis(2000));
        }
    });
}
