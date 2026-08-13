use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sysinfo::System;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager};

use crate::aggregate::{summarize, transitions};
use crate::alias::{self, Aliases};
use crate::limit::{self, LimitEvent, LimitKind};
use crate::model::AgentStatus;
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

pub fn transcript_path(cwd: &str, session_id: &str) -> PathBuf {
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
            limited_until: None,
            was_busy_at_limit: false,
            resume_fired: false,
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

/// Marks sessions with a fresh transcript limit event as Limited. The first
/// sighting stamps the reset instant and whether the session was mid task;
/// later polls carry those stamps so nothing restamps or refires.
pub fn apply_limits(
    prev: &[AgentState],
    next: &mut [AgentState],
    events: &[(String, Option<LimitEvent>)],
    now_ms: i64,
    local_secs: i64,
) {
    for a in next.iter_mut() {
        if a.status == AgentStatus::Ended {
            continue;
        }
        let ev = events
            .iter()
            .find(|(sid, _)| *sid == a.session_id)
            .and_then(|(_, e)| e.as_ref());
        let Some(ev) = ev else { continue };
        let carried = prev.iter().find(|p| p.session_id == a.session_id);
        a.status = AgentStatus::Limited;
        match carried {
            Some(p) if p.status == AgentStatus::Limited => {
                a.limited_until = p.limited_until;
                a.was_busy_at_limit = p.was_busy_at_limit;
                a.resume_fired = p.resume_fired;
            }
            _ => {
                a.was_busy_at_limit = carried
                    .map(|p| p.status == AgentStatus::Working)
                    .unwrap_or(false);
                a.limited_until = match ev.kind {
                    LimitKind::Session { reset_h, reset_m } => {
                        Some(limit::resets_at_ms(now_ms, local_secs, reset_h, reset_m))
                    }
                    LimitKind::Credit | LimitKind::Login => None,
                };
            }
        }
    }
}

pub fn due_for_resume(agents: &[AgentState], now_ms: i64, enabled: bool) -> Vec<usize> {
    if !enabled {
        return Vec::new();
    }
    agents
        .iter()
        .enumerate()
        .filter(|(_, a)| {
            a.status == AgentStatus::Limited
                && a.was_busy_at_limit
                && !a.resume_fired
                && a.limited_until.map(|t| now_ms >= t).unwrap_or(false)
        })
        .map(|(i, _)| i)
        .collect()
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
            let events: Vec<(String, Option<LimitEvent>)> = next
                .iter()
                .filter(|a| a.status != AgentStatus::Ended)
                .map(|a| {
                    let tail =
                        limit::read_tail(&transcript_path(&a.cwd, &a.session_id), 65_536);
                    (a.session_id.clone(), limit::detect(&tail))
                })
                .collect();
            apply_limits(
                &prev,
                &mut next,
                &events,
                crate::reap::now_ms(),
                limit::local_secs_since_midnight(),
            );
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

    fn ag(sid: &str, status: AgentStatus) -> AgentState {
        AgentState {
            pid: 1,
            session_id: sid.into(),
            name: sid.into(),
            cwd: "c".into(),
            repo: "r".into(),
            branch: None,
            status,
            raw_status: "x".into(),
            started_at: 0,
            status_updated_at: 0,
            model: None,
            context_pct: None,
            last_activity: None,
            ended_at: None,
            limited_until: None,
            was_busy_at_limit: false,
            resume_fired: false,
        }
    }

    fn session_event() -> Option<LimitEvent> {
        Some(LimitEvent {
            kind: LimitKind::Session { reset_h: 1, reset_m: 0 },
        })
    }

    #[test]
    fn fresh_limit_on_working_session_stamps_limited_and_was_busy() {
        let prev = vec![ag("s1", AgentStatus::Working)];
        let mut next = vec![ag("s1", AgentStatus::Idle)];
        // now: local midnight, reset 01:00 -> one hour ahead
        apply_limits(&prev, &mut next, &[("s1".into(), session_event())], 10_000, 0);
        assert_eq!(next[0].status, AgentStatus::Limited);
        assert!(next[0].was_busy_at_limit);
        assert_eq!(next[0].limited_until, Some(10_000 + 3600 * 1000));
    }

    #[test]
    fn limit_on_idle_session_is_limited_but_not_mid_task() {
        let prev = vec![ag("s1", AgentStatus::Idle)];
        let mut next = vec![ag("s1", AgentStatus::Idle)];
        apply_limits(&prev, &mut next, &[("s1".into(), session_event())], 10_000, 0);
        assert_eq!(next[0].status, AgentStatus::Limited);
        assert!(!next[0].was_busy_at_limit);
    }

    #[test]
    fn carried_limit_keeps_first_stamp_and_flags() {
        let mut p = ag("s1", AgentStatus::Limited);
        p.limited_until = Some(5_000);
        p.was_busy_at_limit = true;
        p.resume_fired = true;
        let prev = vec![p];
        let mut next = vec![ag("s1", AgentStatus::Idle)];
        apply_limits(
            &prev,
            &mut next,
            &[("s1".into(), session_event())],
            99_000,
            12 * 3600,
        );
        assert_eq!(next[0].limited_until, Some(5_000), "must not restamp");
        assert!(next[0].was_busy_at_limit && next[0].resume_fired);
    }

    #[test]
    fn no_event_leaves_agent_untouched_and_ended_stays_ended() {
        let prev = vec![ag("s1", AgentStatus::Working)];
        let mut next = vec![ag("s1", AgentStatus::Working), ag("s2", AgentStatus::Ended)];
        apply_limits(
            &prev,
            &mut next,
            &[("s1".into(), None), ("s2".into(), session_event())],
            1,
            0,
        );
        assert_eq!(next[0].status, AgentStatus::Working);
        assert_eq!(next[1].status, AgentStatus::Ended);
    }

    #[test]
    fn credit_limit_has_no_reset_instant() {
        let prev = vec![ag("s1", AgentStatus::Working)];
        let mut next = vec![ag("s1", AgentStatus::Idle)];
        let ev = Some(LimitEvent { kind: LimitKind::Credit });
        apply_limits(&prev, &mut next, &[("s1".into(), ev)], 1, 0);
        assert_eq!(next[0].status, AgentStatus::Limited);
        assert_eq!(next[0].limited_until, None);
    }

    #[test]
    fn due_for_resume_selects_only_ripe_mid_task_unfired_sessions() {
        let mut ripe = ag("ripe", AgentStatus::Limited);
        ripe.limited_until = Some(1_000);
        ripe.was_busy_at_limit = true;
        let mut early = ag("early", AgentStatus::Limited);
        early.limited_until = Some(99_000);
        early.was_busy_at_limit = true;
        let mut idle_at_limit = ag("idle", AgentStatus::Limited);
        idle_at_limit.limited_until = Some(1_000);
        let mut fired = ag("fired", AgentStatus::Limited);
        fired.limited_until = Some(1_000);
        fired.was_busy_at_limit = true;
        fired.resume_fired = true;
        let agents = vec![ripe, early, idle_at_limit, fired];
        assert_eq!(due_for_resume(&agents, 50_000, true), vec![0]);
        assert!(due_for_resume(&agents, 50_000, false).is_empty());
    }
}
