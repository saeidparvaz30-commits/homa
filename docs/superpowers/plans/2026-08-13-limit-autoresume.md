# Homa v3 Limit Auto Resume Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resizable always-on-top roster with minimize and click-to-focus, plus automatic detection of usage-limited sessions and automatic resume of mid-task sessions when the limit resets.

**Architecture:** A new `limit.rs` module reads transcript tails and turns synthetic limit messages into a `Limited` agent status with a computed reset instant. A new `terminal.rs` module maps a session pid to its terminal HWND by walking process ancestry and matching window titles against the transcript's `aiTitle`. A new `inject.rs` module focuses that HWND and types `continue` + Enter. The poll loop stamps Limited state each tick and fires injection once per limit event when the reset passes.

**Tech Stack:** Rust (Tauri 2, serde, sysinfo, windows-sys), React 18 + TypeScript + Tailwind, vitest, cargo test.

**Spec:** `docs/superpowers/specs/2026-08-13-limit-autoresume-design.md`

## Global Constraints

- One new Rust dependency is approved: `windows-sys 0.59` (already in Tauri's tree). No other new deps, Rust or npm.
- Homa never writes into `~/.claude`. Injection writes into terminal windows, never files.
- Windows only. `cargo` is not on PATH: prepend `$env:USERPROFILE\.cargo\bin`.
- Rust tests from `src-tauri/` with `cargo test`; frontend tests from repo root with `npm test`.
- No em dashes anywhere.
- Commit after every task.

---

### Task 1: Settings for overlay size and the auto resume switch

**Files:**
- Modify: `src-tauri/src/settings.rs`

**Interfaces:**
- Produces: `Settings { sound_enabled, sound_on_idle, overlay_x, overlay_y, overlay_w: Option<f64>, overlay_h: Option<f64>, auto_resume_enabled: bool }` with `auto_resume_enabled` defaulting to `true` for missing keys.

- [ ] **Step 1: Write the failing tests** (append inside `mod tests`)

```rust
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
```

- [ ] **Step 2: Run to verify failure** — `cargo test settings`, expect compile FAIL: no field `auto_resume_enabled`.

- [ ] **Step 3: Implement** — extend the struct:

```rust
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
```

and add `overlay_w: None, overlay_h: None, auto_resume_enabled: true,` to `Default`.

- [ ] **Step 4: Run** — `cargo test settings`, expect PASS (4 tests).
- [ ] **Step 5: Commit** — `git commit -m "feat: settings for overlay size and auto resume switch"`

---

### Task 2: Resizable overlay with top bar, minimize, and tray restore

**Files:**
- Modify: `src-tauri/tauri.conf.json` (overlay window object)
- Modify: `src-tauri/src/overlay.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/components/OverlayRoster.tsx`
- Modify: `src/components/OverlayRoster.test.tsx`

**Interfaces:**
- Consumes: Task 1's `overlay_w/overlay_h`.
- Produces: Tauri command `hide_overlay()`; `overlay::remember_size(w: f64, h: f64)`; tray menu id `show-overlay`. Frontend no longer calls `setSize`.

- [ ] **Step 1: Write the failing tests** — in `OverlayRoster.test.tsx` append:

```tsx
test("top bar shows a minimize button that hides the overlay", () => {
  vi.mocked(invoke).mockClear();
  setAgents([]);
  render(<OverlayRoster />);
  fireEvent.click(screen.getByLabelText("minimize"));
  expect(invoke).toHaveBeenCalledWith("hide_overlay");
});
```

- [ ] **Step 2: Run** — `npm test`, expect FAIL: unable to find label "minimize".

- [ ] **Step 3: Implement**

`tauri.conf.json` overlay object: `"width": 320, "height": 280, "resizable": true` (keep every other key).

`OverlayRoster.tsx`: delete the `useEffect`/`lastH`/`ROW_H`/`CHROME_H`/`MAX_H`/`WIDTH` sizing block and the `getCurrentWindow`/`LogicalSize` imports. Replace the drag strip with a top bar as first child of the container:

```tsx
      <div className="flex h-6 w-full items-center">
        <div data-tauri-drag-region className="h-full flex-1 cursor-grab" />
        <button
          aria-label="minimize"
          onClick={() => invoke("hide_overlay").catch(() => {})}
          className="px-2 text-neutral-400 hover:text-neutral-100"
        >
          &#8211;
        </button>
      </div>
```

`overlay.rs`: extend restore and add size persistence:

```rust
use tauri::PhysicalSize;

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

pub fn remember_size(w: f64, h: f64) {
    let mut s = Settings::load();
    if s.overlay_w == Some(w) && s.overlay_h == Some(h) {
        return;
    }
    s.overlay_w = Some(w);
    s.overlay_h = Some(h);
    s.save();
}
```

`main.rs`: add the command and handle Resized; add the tray item.

```rust
#[tauri::command]
fn hide_overlay(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.hide();
    }
}
```

Register `hide_overlay` in `generate_handler!`. In the overlay `on_window_event` closure add:

```rust
                    if let WindowEvent::Resized(size) = e {
                        overlay::remember_size(size.width as f64, size.height as f64);
                    }
```

Tray: add `let show_overlay = MenuItem::with_id(app, "show-overlay", "Show overlay", true, None::<&str>)?;`, include it in `Menu::with_items(app, &[&show, &show_overlay, &quit])?`, and in `on_menu_event` add:

```rust
                    "show-overlay" => {
                        if let Some(w) = app.get_webview_window("overlay") {
                            let _ = w.show();
                        }
                    }
```

- [ ] **Step 4: Run** — `npm test` PASS; `cargo build` from `src-tauri/` compiles.
- [ ] **Step 5: Commit** — `git commit -m "feat: resizable overlay with top bar, minimize, and tray restore"`

---

### Task 3: Limit detection module

**Files:**
- Create: `src-tauri/src/limit.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod limit;` alphabetically)

**Interfaces:**
- Produces:
  - `pub enum LimitKind { Session { reset_h: u32, reset_m: u32 }, Credit, Login }`
  - `pub struct LimitEvent { pub kind: LimitKind }`
  - `pub fn detect(lines: &[String]) -> Option<LimitEvent>`
  - `pub fn parse_reset(text: &str) -> Option<(u32, u32)>`
  - `pub fn resets_at_ms(now_ms: i64, local_secs_since_midnight: i64, h: u32, m: u32) -> i64`
  - `pub fn local_secs_since_midnight() -> i64` (uses `std::process` free Win32 later; for now derive from `chrono`-free formula below)
  - `pub fn last_ai_title(lines: &[String]) -> Option<String>`
  - `pub fn read_tail(path: &std::path::Path, max_bytes: u64) -> Vec<String>`

- [ ] **Step 1: Write the failing tests** — create `limit.rs` with the test module:

```rust
use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;

    fn synth(text: &str) -> String {
        format!(
            r#"{{"type":"assistant","message":{{"model":"<synthetic>","role":"assistant","content":[{{"type":"text","text":"{text}"}}]}}}}"#
        )
    }
    fn real_assistant() -> String {
        r#"{"type":"assistant","message":{"model":"claude-opus-4-8","role":"assistant","content":[{"type":"text","text":"done"}]}}"#.into()
    }
    fn user_line() -> String {
        r#"{"type":"user","message":{"role":"user","content":"go on"}}"#.into()
    }

    #[test]
    fn detects_session_limit_with_reset_time() {
        let lines = vec![real_assistant(), synth("You've hit your session limit \u{b7} resets 12:40am (Europe/Oslo)")];
        match detect(&lines) {
            Some(LimitEvent { kind: LimitKind::Session { reset_h: 0, reset_m: 40 } }) => {}
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn newer_activity_makes_the_limit_stale() {
        let lines = vec![synth("You've hit your session limit \u{b7} resets 4:00pm (Europe/Oslo)"), user_line()];
        assert!(detect(&lines).is_none());
        let lines2 = vec![synth("You've hit your session limit \u{b7} resets 4:00pm (Europe/Oslo)"), real_assistant()];
        assert!(detect(&lines2).is_none());
    }

    #[test]
    fn no_response_requested_synthetic_is_ignored_not_activity() {
        let lines = vec![synth("You've hit your session limit \u{b7} resets 4:00pm (Europe/Oslo)"), synth("No response requested.")];
        assert!(detect(&lines).is_some());
    }

    #[test]
    fn credit_and_login_variants_detected_without_reset() {
        assert!(matches!(detect(&[synth("Credit balance is too low")]), Some(LimitEvent { kind: LimitKind::Credit })));
        assert!(matches!(detect(&[synth("Login expired \u{b7} Please run /login")]), Some(LimitEvent { kind: LimitKind::Login })));
    }

    #[test]
    fn parse_reset_handles_am_pm_and_no_minutes() {
        assert_eq!(parse_reset("resets 12:40am (Europe/Oslo)"), Some((0, 40)));
        assert_eq!(parse_reset("resets 4:05pm (Europe/Oslo)"), Some((16, 5)));
        assert_eq!(parse_reset("resets 3pm"), Some((15, 0)));
        assert_eq!(parse_reset("resets 12pm"), Some((12, 0)));
        assert_eq!(parse_reset("no time here"), None);
    }

    #[test]
    fn resets_at_rolls_forward_to_next_occurrence() {
        // Local time 23:00:00; reset 12:40am -> 1h40m ahead.
        let now_ms = 1_000_000_000;
        let secs = 23 * 3600;
        assert_eq!(resets_at_ms(now_ms, secs, 0, 40), now_ms + (100 * 60) * 1000);
        // Local 01:00, reset 12:40am -> tomorrow.
        let secs2 = 3600;
        assert_eq!(resets_at_ms(now_ms, secs2, 0, 40), now_ms + ((24 * 3600 - 3600) + 40 * 60) as i64 * 1000);
    }

    #[test]
    fn last_ai_title_wins() {
        let lines = vec![
            r#"{"type":"ai-title","aiTitle":"old title"}"#.to_string(),
            real_assistant(),
            r#"{"type":"ai-title","aiTitle":"new title"}"#.to_string(),
        ];
        assert_eq!(last_ai_title(&lines).as_deref(), Some("new title"));
    }

    #[test]
    fn read_tail_returns_last_lines_of_large_file() {
        let p = std::env::temp_dir().join("homa-limit-tail-test.jsonl");
        let big: String = (0..5000).map(|i| format!("line{i}\n")).collect();
        std::fs::write(&p, big).unwrap();
        let lines = read_tail(&p, 4096);
        assert!(lines.len() > 1);
        assert_eq!(lines.last().unwrap(), "line4999");
        assert!(read_tail(Path::new("C:\\does\\not\\exist.jsonl"), 4096).is_empty());
    }
}
```

- [ ] **Step 2: Run** — `cargo test limit`, expect compile FAIL (missing types).

- [ ] **Step 3: Implement** above the test module:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum LimitKind {
    Session { reset_h: u32, reset_m: u32 },
    Credit,
    Login,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LimitEvent {
    pub kind: LimitKind,
}

/// Scans newest-first. A synthetic limit message is fresh only if no real
/// user or assistant turn was written after it; otherwise the user already
/// moved past it and it must not retrigger anything.
pub fn detect(lines: &[String]) -> Option<LimitEvent> {
    for line in lines.iter().rev() {
        let v: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let t = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
        if t != "assistant" && t != "user" {
            continue;
        }
        let synthetic =
            v["message"].get("model").and_then(|m| m.as_str()) == Some("<synthetic>");
        if !synthetic {
            return None; // real activity newer than any limit line
        }
        let text = v["message"]["content"]
            .as_array()
            .and_then(|c| c.first())
            .and_then(|b| b.get("text"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if text.contains("limit") && text.contains("resets") {
            let (reset_h, reset_m) = parse_reset(text)?;
            return Some(LimitEvent { kind: LimitKind::Session { reset_h, reset_m } });
        }
        if text.contains("Credit balance") {
            return Some(LimitEvent { kind: LimitKind::Credit });
        }
        if text.contains("Login expired") {
            return Some(LimitEvent { kind: LimitKind::Login });
        }
        // Other synthetic lines ("No response requested.") are neither
        // activity nor limits: keep scanning older lines.
    }
    None
}

/// Parses "resets 12:40am" / "resets 3pm" into 24h (hour, minute).
pub fn parse_reset(text: &str) -> Option<(u32, u32)> {
    let idx = text.find("resets ")?;
    let rest = &text[idx + 7..];
    let token: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ':' || *c == 'a' || *c == 'p' || *c == 'm')
        .collect();
    let lower = token.to_ascii_lowercase();
    let pm = lower.ends_with("pm");
    let am = lower.ends_with("am");
    if !pm && !am {
        return None;
    }
    let hm = &lower[..lower.len() - 2];
    let (h_str, m_str) = match hm.split_once(':') {
        Some((h, m)) => (h, m),
        None => (hm, "0"),
    };
    let mut h: u32 = h_str.parse().ok()?;
    let m: u32 = m_str.parse().ok()?;
    if h == 12 {
        h = 0;
    }
    if pm {
        h += 12;
    }
    if h > 23 || m > 59 {
        return None;
    }
    Some((h, m))
}

/// Pure next-occurrence arithmetic: the reset is a local wall clock time,
/// so the delta to it is computed in local seconds and applied to epoch now.
pub fn resets_at_ms(now_ms: i64, local_secs_since_midnight: i64, h: u32, m: u32) -> i64 {
    let target = (h as i64) * 3600 + (m as i64) * 60;
    let mut delta = target - local_secs_since_midnight;
    if delta <= 0 {
        delta += 24 * 3600;
    }
    now_ms + delta * 1000
}

pub fn last_ai_title(lines: &[String]) -> Option<String> {
    for line in lines.iter().rev() {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            if v.get("type").and_then(|x| x.as_str()) == Some("ai-title") {
                return v.get("aiTitle").and_then(|x| x.as_str()).map(String::from);
            }
        }
    }
    None
}

/// Reads at most `max_bytes` from the end of the file, split into whole lines
/// (a partial first line is dropped). Transcripts grow to many MB; the poll
/// loop touches them every 2s, so it must never read the whole file.
pub fn read_tail(path: &Path, max_bytes: u64) -> Vec<String> {
    let Ok(mut f) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(max_bytes);
    if f.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }
    let mut buf = String::new();
    if f.read_to_string(&mut buf).is_err() {
        return Vec::new();
    }
    let mut lines: Vec<String> = buf.lines().map(String::from).collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    lines
}
```

Add `pub mod limit;` to `lib.rs`.

- [ ] **Step 4: Run** — `cargo test limit`, expect PASS (8 tests).
- [ ] **Step 5: Commit** — `git commit -m "feat: limit detection from transcript synthetic messages"`

---

### Task 4: Limited status in the model, aggregate, tray, notify

**Files:**
- Modify: `src-tauri/src/model.rs`
- Modify: `src-tauri/src/aggregate.rs`
- Modify: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/notify.rs`
- Modify: `src-tauri/src/alias.rs`, `src-tauri/src/reap.rs`, `src-tauri/src/poller.rs` (test fixture literals gain the three new fields)

**Interfaces:**
- Produces: `AgentStatus::Limited` (priority 3; Waiting becomes 4), `AgentState { limited_until: Option<i64>, was_busy_at_limit: bool, resume_fired: bool }`, `TraySummary.limited`.

- [ ] **Step 1: Write the failing tests**

`model.rs` tests:

```rust
    #[test]
    fn limited_sits_between_waiting_and_idle() {
        assert!(AgentStatus::Waiting.priority() > AgentStatus::Limited.priority());
        assert!(AgentStatus::Limited.priority() > AgentStatus::Idle.priority());
    }
```

`aggregate.rs` tests:

```rust
    #[test]
    fn limited_counts_and_can_top_the_summary() {
        let s = summarize(&[a("1", AgentStatus::Working), a("2", AgentStatus::Limited)]);
        assert_eq!(s.top, AgentStatus::Limited);
        assert_eq!(s.limited, 1);
    }

    #[test]
    fn transition_into_limited_fires() {
        let prev = vec![a("1", AgentStatus::Working)];
        let next = vec![a("1", AgentStatus::Limited)];
        let t = transitions(&prev, &next);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].to, AgentStatus::Limited);
    }
```

- [ ] **Step 2: Run** — `cargo test`, expect compile FAIL: no variant `Limited`.

- [ ] **Step 3: Implement**

`model.rs`: add `Limited` to the enum; priorities: Waiting 4, Limited 3, Idle 2, Working 1, Ended 0. Append to `AgentState` after `ended_at`:

```rust
    /// Epoch ms when the usage limit lifts; None for credit/login limits.
    pub limited_until: Option<i64>,
    pub was_busy_at_limit: bool,
    pub resume_fired: bool,
```

`aggregate.rs`: add `pub limited: usize` to `TraySummary`, count it in the loop (`AgentStatus::Limited => limited += 1,`), and widen the transitions gate to `AgentStatus::Waiting | AgentStatus::Idle | AgentStatus::Limited`.

`tray.rs`: `AgentStatus::Limited => "tray-idle.png",` (amber: stalled, not urgent) and extend the tooltip: `"Homa  waiting {}  limited {}  idle {}  working {}"`.

`notify.rs`: add `AgentStatus::Limited => format!("{} hit its usage limit", t.name),` to the body match (no sound for Limited; the sound condition already only covers Waiting/Idle).

Every `AgentState { ... }` literal in tests across `alias.rs`, `reap.rs`, `aggregate.rs`, `poller.rs` gains `limited_until: None, was_busy_at_limit: false, resume_fired: false,`.

- [ ] **Step 4: Run** — `cargo test`, expect PASS everywhere.
- [ ] **Step 5: Commit** — `git commit -m "feat: Limited agent status with tray count and toast"`

---

### Task 5: Poller stamps Limited state and selects sessions due for resume

**Files:**
- Modify: `src-tauri/src/poller.rs`

**Interfaces:**
- Consumes: `limit::{detect, read_tail, resets_at_ms, LimitEvent, LimitKind}`, model fields from Task 4.
- Produces:
  - `pub fn apply_limits(prev: &[AgentState], next: &mut [AgentState], events: &[(String, Option<LimitEvent>)], now_ms: i64, local_secs: i64)`
  - `pub fn due_for_resume(agents: &[AgentState], now_ms: i64, enabled: bool) -> Vec<usize>`
  - `pub fn transcript_path(cwd: &str, session_id: &str) -> PathBuf` becomes `pub`.
  - Loop body: reads tails, applies limits, marks `resume_fired` after firing (firing itself lands in Task 7).

- [ ] **Step 1: Write the failing tests** (append inside poller `mod tests`)

```rust
    use crate::limit::{LimitEvent, LimitKind};
    use crate::model::{AgentState, AgentStatus};

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
        Some(LimitEvent { kind: LimitKind::Session { reset_h: 1, reset_m: 0 } })
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
        apply_limits(&prev, &mut next, &[("s1".into(), session_event())], 99_000, 12 * 3600);
        assert_eq!(next[0].limited_until, Some(5_000), "must not restamp");
        assert!(next[0].was_busy_at_limit && next[0].resume_fired);
    }

    #[test]
    fn no_event_leaves_agent_untouched_and_ended_stays_ended() {
        let prev = vec![ag("s1", AgentStatus::Working)];
        let mut next = vec![ag("s1", AgentStatus::Working), ag("s2", AgentStatus::Ended)];
        apply_limits(&prev, &mut next, &[("s1".into(), None), ("s2".into(), session_event())], 1, 0);
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
```

- [ ] **Step 2: Run** — `cargo test poller`, expect compile FAIL: `apply_limits` not found.

- [ ] **Step 3: Implement** in `poller.rs`:

```rust
use crate::limit::{self, LimitEvent, LimitKind};

pub fn apply_limits(
    prev: &[AgentState],
    next: &mut [AgentState],
    events: &[(String, Option<LimitEvent>)],
    now_ms: i64,
    local_secs: i64,
) {
    for a in next.iter_mut() {
        if a.status == crate::model::AgentStatus::Ended {
            continue;
        }
        let ev = events
            .iter()
            .find(|(sid, _)| *sid == a.session_id)
            .and_then(|(_, e)| e.as_ref());
        let Some(ev) = ev else { continue };
        let carried = prev.iter().find(|p| p.session_id == a.session_id);
        a.status = crate::model::AgentStatus::Limited;
        match carried {
            Some(p) if p.status == crate::model::AgentStatus::Limited => {
                a.limited_until = p.limited_until;
                a.was_busy_at_limit = p.was_busy_at_limit;
                a.resume_fired = p.resume_fired;
            }
            _ => {
                a.was_busy_at_limit = carried
                    .map(|p| p.status == crate::model::AgentStatus::Working)
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
            a.status == crate::model::AgentStatus::Limited
                && a.was_busy_at_limit
                && !a.resume_fired
                && a.limited_until.map(|t| now_ms >= t).unwrap_or(false)
        })
        .map(|(i, _)| i)
        .collect()
}
```

Make `transcript_path` pub. In the loop body, after `let mut next = scan_once(...)` and before `reap`:

```rust
            let events: Vec<(String, Option<LimitEvent>)> = next
                .iter()
                .filter(|a| a.status != crate::model::AgentStatus::Ended)
                .map(|a| {
                    let tail = limit::read_tail(&transcript_path(&a.cwd, &a.session_id), 65_536);
                    (a.session_id.clone(), limit::detect(&tail))
                })
                .collect();
            apply_limits(&prev, &mut next, &events, crate::reap::now_ms(), limit::local_secs_since_midnight());
```

`local_secs_since_midnight` lands in Task 6 with windows-sys; until then add to `limit.rs` a placeholder-free portable version using epoch math against the timezone offset obtained from `std::time` is impossible, so implement it NOW in `limit.rs` via `GetLocalTime` behind `#[cfg(windows)]` after Task 6 adds the dependency. For this task, add the dependency early instead: move the Cargo.toml edit here if the build requires it.

- [ ] **Step 4: Run** — `cargo test`, expect PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat: poller stamps Limited state and selects due resumes"`

---

### Task 6: windows-sys dependency, local clock, terminal window resolution, click to focus

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/limit.rs` (add `local_secs_since_midnight`)
- Create: `src-tauri/src/terminal.rs`
- Modify: `src-tauri/src/lib.rs`, `src-tauri/src/main.rs`

**Interfaces:**
- Produces:
  - `pub struct Win { pub hwnd: isize, pub pid: u32, pub title: String }`
  - `pub fn ancestor_pids(pid: u32) -> Vec<u32>` (self included, innermost first)
  - `pub fn list_windows() -> Vec<Win>`
  - `pub fn pick_window(wins: &[Win], ancestors: &[u32], ai_title: Option<&str>) -> Option<isize>` (pure)
  - `pub fn resolve_hwnd(pid: u32, ai_title: Option<&str>) -> Option<isize>`
  - `pub fn focus_hwnd(hwnd: isize)`
  - Tauri command `focus_session(pid: u32, cwd: String, session_id: String)`

- [ ] **Step 1: Cargo.toml** — add:

```toml
windows-sys = { version = "0.59", features = [
  "Win32_Foundation",
  "Win32_UI_WindowsAndMessaging",
  "Win32_UI_Input_KeyboardAndMouse",
  "Win32_System_SystemInformation",
] }
```

- [ ] **Step 2: Write the failing tests** — in `terminal.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn w(hwnd: isize, pid: u32, title: &str) -> Win {
        Win { hwnd, pid, title: title.into() }
    }

    #[test]
    fn title_match_on_ancestor_window_wins() {
        let wins = vec![
            w(1, 100, "\u{25d0} other session"),
            w(2, 100, "\u{25d0} my task title"),
            w(3, 999, "my task title"), // right title, not an ancestor
        ];
        assert_eq!(pick_window(&wins, &[50, 100], Some("my task title")), Some(2));
    }

    #[test]
    fn falls_back_to_innermost_ancestor_window_without_title_match() {
        let wins = vec![w(9, 200, "whatever"), w(8, 100, "shell")];
        assert_eq!(pick_window(&wins, &[100, 200], None), Some(8));
        assert_eq!(pick_window(&wins, &[300, 200], Some("nope")), Some(9));
    }

    #[test]
    fn no_candidates_yields_none() {
        assert_eq!(pick_window(&[], &[1, 2], Some("x")), None);
    }

    #[test]
    fn own_process_ancestry_is_nonempty_and_starts_with_self() {
        let pids = ancestor_pids(std::process::id());
        assert_eq!(pids.first().copied(), Some(std::process::id()));
        assert!(pids.len() >= 2, "a test process always has a parent");
    }

    #[test]
    fn local_secs_since_midnight_is_in_range() {
        let s = crate::limit::local_secs_since_midnight();
        assert!((0..86_400).contains(&s));
    }
}
```

- [ ] **Step 3: Run** — `cargo test terminal`, expect compile FAIL.

- [ ] **Step 4: Implement**

`limit.rs` append:

```rust
#[cfg(windows)]
pub fn local_secs_since_midnight() -> i64 {
    use windows_sys::Win32::System::SystemInformation::{GetLocalTime, SYSTEMTIME};
    unsafe {
        let mut st: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut st);
        (st.wHour as i64) * 3600 + (st.wMinute as i64) * 60 + st.wSecond as i64
    }
}
```

`terminal.rs`:

```rust
use sysinfo::System;

pub struct Win {
    pub hwnd: isize,
    pub pid: u32,
    pub title: String,
}

/// Innermost first: [session pid, shell pid, terminal pid, ...].
pub fn ancestor_pids(pid: u32) -> Vec<u32> {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All);
    let mut out = vec![pid];
    let mut cur = sysinfo::Pid::from_u32(pid);
    for _ in 0..16 {
        let Some(p) = sys.process(cur).and_then(|p| p.parent()) else {
            break;
        };
        out.push(p.as_u32());
        cur = p;
    }
    out
}

pub fn list_windows() -> Vec<Win> {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    };
    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> i32 {
        let out = &mut *(lparam as *mut Vec<Win>);
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let mut buf = [0u16; 512];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        if n > 0 {
            out.push(Win {
                hwnd: hwnd as isize,
                pid,
                title: String::from_utf16_lossy(&buf[..n as usize]),
            });
        }
        1
    }
    let mut out: Vec<Win> = Vec::new();
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::EnumWindows;
        EnumWindows(Some(cb), &mut out as *mut _ as isize);
    }
    out
}

/// Title match beats ancestry depth; among ancestors, innermost first so a
/// conhost window is preferred over an outer launcher.
pub fn pick_window(wins: &[Win], ancestors: &[u32], ai_title: Option<&str>) -> Option<isize> {
    if let Some(t) = ai_title {
        if let Some(w) = wins
            .iter()
            .find(|w| ancestors.contains(&w.pid) && w.title.ends_with(t))
        {
            return Some(w.hwnd);
        }
    }
    for pid in ancestors {
        if let Some(w) = wins.iter().find(|w| w.pid == *pid) {
            return Some(w.hwnd);
        }
    }
    None
}

pub fn resolve_hwnd(pid: u32, ai_title: Option<&str>) -> Option<isize> {
    pick_window(&list_windows(), &ancestor_pids(pid), ai_title)
}

pub fn focus_hwnd(hwnd: isize) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VK_MENU,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };
    unsafe {
        if IsIconic(hwnd as _) != 0 {
            ShowWindow(hwnd as _, SW_RESTORE);
        }
        // A background process may not steal foreground; a synthetic Alt tap
        // marks this thread as input-active so SetForegroundWindow is honored.
        let mut alt: [INPUT; 2] = std::mem::zeroed();
        for (i, flags) in [(0usize, KEYBD_EVENT_FLAGS::default()), (1usize, KEYEVENTF_KEYUP)] {
            alt[i].r#type = INPUT_KEYBOARD;
            alt[i].Anonymous.ki.wVk = VK_MENU;
            alt[i].Anonymous.ki.dwFlags = flags;
        }
        SendInput(2, alt.as_ptr(), std::mem::size_of::<INPUT>() as i32);
        SetForegroundWindow(hwnd as _);
    }
}
```

`lib.rs`: `pub mod terminal;` (alphabetical). `main.rs` command:

```rust
#[tauri::command]
fn focus_session(pid: u32, cwd: String, session_id: String) -> Result<(), String> {
    let tail = homa_lib::limit::read_tail(
        &homa_lib::poller::transcript_path(&cwd, &session_id),
        65_536,
    );
    let title = homa_lib::limit::last_ai_title(&tail);
    match homa_lib::terminal::resolve_hwnd(pid, title.as_deref()) {
        Some(h) => {
            homa_lib::terminal::focus_hwnd(h);
            Ok(())
        }
        None => Err("no terminal window found".into()),
    }
}
```

Register in `generate_handler!`.

- [ ] **Step 5: Run** — `cargo test`, expect PASS; `cargo build` compiles.
- [ ] **Step 6: Commit** — `git commit -m "feat: terminal window resolution and focus_session command"`

---

### Task 7: Injector, resume firing in the loop, tray kill switch

**Files:**
- Create: `src-tauri/src/inject.rs`
- Modify: `src-tauri/src/lib.rs`, `src-tauri/src/poller.rs`, `src-tauri/src/main.rs`

**Interfaces:**
- Consumes: `terminal::{resolve_hwnd, focus_hwnd}`, `poller::due_for_resume`, `Settings.auto_resume_enabled`.
- Produces: `pub fn inject::nudge(hwnd: isize)` (focus, settle, type `continue`, Enter); tray menu id `autoresume`; loop firing with one summary toast.

- [ ] **Step 1: Implement `inject.rs`** (thin Win32 wrapper; no unit tests, covered by the manual checklist):

```rust
use crate::terminal;

fn send_unicode_and_enter(text: &str) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VK_RETURN,
    };
    unsafe {
        let mut inputs: Vec<INPUT> = Vec::new();
        for ch in text.encode_utf16() {
            for flags in [KEYEVENTF_UNICODE, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP] {
                let mut i: INPUT = std::mem::zeroed();
                i.r#type = INPUT_KEYBOARD;
                i.Anonymous.ki.wScan = ch;
                i.Anonymous.ki.dwFlags = flags;
                inputs.push(i);
            }
        }
        for flags in [Default::default(), KEYEVENTF_KEYUP] {
            let mut i: INPUT = std::mem::zeroed();
            i.r#type = INPUT_KEYBOARD;
            i.Anonymous.ki.wVk = VK_RETURN;
            i.Anonymous.ki.dwFlags = flags;
            inputs.push(i);
        }
        SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

/// Focus first, give the terminal a beat to accept input, then type.
pub fn nudge(hwnd: isize) {
    terminal::focus_hwnd(hwnd);
    std::thread::sleep(std::time::Duration::from_millis(250));
    send_unicode_and_enter("continue");
}
```

`lib.rs`: `pub mod inject;`.

- [ ] **Step 2: Wire firing into the poll loop** — in `start_watching`, after `apply_limits(...)` and before the `next != prev` comparison:

```rust
            let due = due_for_resume(&next, crate::reap::now_ms(), settings_now.auto_resume_enabled);
            if !due.is_empty() {
                let mut resumed = 0usize;
                for i in due {
                    let a = &mut next[i];
                    a.resume_fired = true;
                    let tail = limit::read_tail(&transcript_path(&a.cwd, &a.session_id), 65_536);
                    let title = limit::last_ai_title(&tail);
                    match crate::terminal::resolve_hwnd(a.pid, title.as_deref()) {
                        Some(h) => {
                            crate::inject::nudge(h);
                            resumed += 1;
                        }
                        None => {
                            let _ = app
                                .notification()
                                .builder()
                                .title("Homa")
                                .body(format!("{} limit reset, no terminal found, resume it yourself", a.name))
                                .show();
                        }
                    }
                }
                if resumed > 0 {
                    let _ = app
                        .notification()
                        .builder()
                        .title("Homa")
                        .body(format!("Resumed {resumed} agent(s) after limit reset"))
                        .show();
                }
            }
```

where `settings_now` is `crate::settings::Settings::load()` hoisted above the transitions block (reuse the existing `settings` load, moving it before this point). Add `use tauri_plugin_notification::NotificationExt;` to poller imports.

- [ ] **Step 3: Tray kill switch** — in `main.rs` setup:

```rust
            let auto_label = if homa_lib::settings::Settings::load().auto_resume_enabled {
                "Auto-resume: on"
            } else {
                "Auto-resume: off"
            };
            let autoresume = MenuItem::with_id(app, "autoresume", auto_label, true, None::<&str>)?;
```

Include `&autoresume` in the menu between `show_overlay` and `quit`. In `on_menu_event`, clone the item into the closure (`let auto_item = autoresume.clone();` before building) and add:

```rust
                    "autoresume" => {
                        let mut s = homa_lib::settings::Settings::load();
                        s.auto_resume_enabled = !s.auto_resume_enabled;
                        s.save();
                        let _ = auto_item.set_text(if s.auto_resume_enabled {
                            "Auto-resume: on"
                        } else {
                            "Auto-resume: off"
                        });
                    }
```

- [ ] **Step 4: Run** — `cargo test` PASS, `cargo build` compiles.
- [ ] **Step 5: Commit** — `git commit -m "feat: auto resume injection with tray kill switch"`

---

### Task 8: Frontend Limited UI and click to focus

**Files:**
- Modify: `src/types.ts`
- Modify: `src/components/OverlayRoster.tsx`
- Modify: `src/components/OverlayRoster.test.tsx`

**Interfaces:**
- Consumes: `focus_session` command (args `{ pid, cwd, sessionId }`), model fields from Task 4 (`limited_until`, `resume_fired`, status `"limited"`).

- [ ] **Step 1: Write the failing tests** (append; `mk` gains `limited_until: null, was_busy_at_limit: false, resume_fired: false` in its base object):

```tsx
test("limited row shows purple dot and reset time", () => {
  const at = new Date(2026, 7, 13, 0, 40).getTime();
  setAgents([mk({ session_id: "a", name: "x", status: "limited", limited_until: at })]);
  render(<OverlayRoster />);
  expect(screen.getByTestId("row-dot")).toHaveAttribute("data-status", "limited");
  expect(screen.getByText("resets 12:40am")).toBeInTheDocument();
});

test("limited row past reset with resume fired says resuming", () => {
  setAgents([mk({ session_id: "a", name: "x", status: "limited", limited_until: 5, resume_fired: true })]);
  render(<OverlayRoster />);
  expect(screen.getByText("resuming")).toBeInTheDocument();
});

test("single click focuses the session terminal after the dblclick window", () => {
  vi.useFakeTimers();
  vi.mocked(invoke).mockClear();
  setAgents([mk({ session_id: "a", name: "homa", pid: 7, cwd: "C:\\Homa" })]);
  render(<OverlayRoster />);
  fireEvent.click(screen.getByTestId("row-name"));
  expect(invoke).not.toHaveBeenCalled();
  vi.advanceTimersByTime(300);
  expect(invoke).toHaveBeenCalledWith("focus_session", { pid: 7, cwd: "C:\\Homa", sessionId: "a" });
  vi.useRealTimers();
});

test("double click renames and cancels the pending focus", () => {
  vi.useFakeTimers();
  vi.mocked(invoke).mockClear();
  setAgents([mk({ session_id: "a", name: "homa", cwd: "C:\\Homa" })]);
  render(<OverlayRoster />);
  fireEvent.click(screen.getByTestId("row-name"));
  fireEvent.doubleClick(screen.getByTestId("row-name"));
  vi.advanceTimersByTime(600);
  expect(invoke).not.toHaveBeenCalledWith("focus_session", expect.anything());
  expect(screen.getByRole("textbox")).toBeInTheDocument();
  vi.useRealTimers();
});
```

- [ ] **Step 2: Run** — `npm test`, expect FAIL.

- [ ] **Step 3: Implement**

`types.ts`: status union gains `"limited"`; interface gains `limited_until: number | null; was_busy_at_limit: boolean; resume_fired: boolean;`.

`OverlayRoster.tsx`:

```tsx
const DOT: Record<AgentStatus, string> = {
  waiting: "bg-red-500",
  limited: "bg-purple-500",
  idle: "bg-amber-400",
  working: "bg-sky-500",
  ended: "bg-neutral-500",
};

const RANK: Record<AgentStatus, number> = { waiting: 4, limited: 3, idle: 2, working: 1, ended: 0 };

const fmtReset = (ms: number) => {
  const d = new Date(ms);
  const h24 = d.getHours();
  const h = h24 % 12 === 0 ? 12 : h24 % 12;
  const m = String(d.getMinutes()).padStart(2, "0");
  return `${h}:${m}${h24 < 12 ? "am" : "pm"}`;
};
```

Inside the component add the click guard:

```tsx
  const clickTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clickFocus = (a: AgentState) => {
    if (editing !== null) return;
    if (clickTimer.current) clearTimeout(clickTimer.current);
    clickTimer.current = setTimeout(() => {
      invoke("focus_session", { pid: a.pid, cwd: a.cwd, sessionId: a.session_id }).catch(() => {});
    }, 250);
  };
```

Row name span gets `onClick={() => clickFocus(a)}`; `onDoubleClick` becomes:

```tsx
              onDoubleClick={() => {
                if (clickTimer.current) clearTimeout(clickTimer.current);
                beginEdit(a.session_id, a.name);
              }}
```

After the name span/input, inside the row div:

```tsx
          {a.status === "limited" && (
            <span className="ml-auto shrink-0 text-xs text-neutral-400">
              {a.resume_fired ? "resuming" : a.limited_until ? `resets ${fmtReset(a.limited_until)}` : "limited"}
            </span>
          )}
```

Import `AgentState` type and `useRef` if not present.

- [ ] **Step 4: Run** — `npm test` PASS (all frontend tests), `npm run build` compiles.
- [ ] **Step 5: Commit** — `git commit -m "feat: limited rows with countdown and click to focus"`

---

### Task 9: README, full verification, manual checklist

**Files:**
- Modify: `README.md`

- [ ] **Step 1: README** — document: resizable overlay with top bar and minimize (restore via tray > Show overlay), click a row to focus that agent's terminal, double click to rename, Limited state (purple, countdown), auto resume behaviour and its tray kill switch, `%APPDATA%\homa\settings.json` keys, and the two limitations (same-window tabs, VS Code terminals get toast fallback).

- [ ] **Step 2: Full suites** — `cargo test` and `npm test` both green.

- [ ] **Step 3: Manual checklist** (dev run; record each result):

- [ ] Overlay starts at remembered position and size; resizing an edge persists across restart
- [ ] Top bar drags the window; minimize button hides it; tray > Show overlay restores it
- [ ] Single click on a row brings that session's terminal to the foreground (test all three live sessions)
- [ ] Double click still renames without focusing the terminal
- [ ] Fake limit rehearsal: point `HOMA_SESSIONS_DIR` at a fixture session whose transcript ends with a synthetic limit line whose reset time is 2 minutes ahead; row turns purple with countdown; at reset the terminal gets `continue` typed and Enter pressed; toast fires; row clears once the transcript grows
- [ ] Tray toggle Auto-resume: off suppresses injection during a second rehearsal
- [ ] Waiting/idle toasts and tray colors unchanged

- [ ] **Step 4: Commit** — `git commit -m "docs: README for v3 limit auto resume"`

---

## Done when

All nine tasks committed, both suites green, and every manual checklist line verified by eye in a running build.
