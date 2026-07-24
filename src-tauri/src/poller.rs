use std::path::{Path, PathBuf};
use sysinfo::System;

use crate::{
    derive::repo_from_cwd, enrich::enrich_from_path, liveness::pid_alive, mapper::map_status,
    model::AgentState, session::parse_session,
};

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
