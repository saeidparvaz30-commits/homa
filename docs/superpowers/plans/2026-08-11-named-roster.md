# Homa v2 Named Roster Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn Homa's always on top overlay into a permanently visible list of Claude Code sessions named by the user, and stop shell activity from firing false attention signals.

**Architecture:** A new `alias.rs` module owns a cwd keyed name store in `%APPDATA%\homa\aliases.json`. The poller applies aliases to `AgentState.name` during each scan, so every existing consumer (overlay, main window, toast text, sort order) inherits chosen names with no changes. The overlay window swaps its count pill component for a roster component that renders one row per session, self sizes its window height to the row count, and writes names back through two new Tauri commands.

**Tech Stack:** Rust (Tauri 2, serde, serde_json), React 18 + TypeScript + Tailwind, vitest + @testing-library/react, cargo test.

## Global Constraints

- **No new dependencies**, Rust or npm. Everything here uses crates and packages already in `Cargo.toml` / `package.json`. If a task appears to need a new dep, stop and ask.
- **Homa never writes into `~/.claude`.** It is read only against Claude's own files. All Homa state lives in `%APPDATA%\homa\`.
- **Windows only.** Path handling is case insensitive and separator tolerant.
- **`cargo` is not on PATH by default.** Prepend `$env:USERPROFILE\.cargo\bin` in each PowerShell session before running cargo.
- **Rust tests:** run from `src-tauri/` with `cargo test`. **Frontend tests:** run from the repo root with `npm test`.
- **No em dashes** in any user visible string, comment, commit message, or doc.
- Commit after every task.

## File Structure

**Create**
- `src-tauri/src/alias.rs` — alias store: key normalisation, atomic load/save, set/clear, name resolution, collision suffixing
- `src-tauri/src/reap.rs` — ended session lifecycle: stamping `ended_at`, pruning after TTL
- `src/components/OverlayRoster.tsx` — the overlay's roster UI, including inline rename
- `src/components/OverlayRoster.test.tsx` — its tests

**Modify**
- `src-tauri/src/mapper.rs` — `shell` maps to Working
- `src-tauri/src/model.rs` — add `ended_at` to `AgentState`
- `src-tauri/src/lib.rs` — register the two new modules
- `src-tauri/src/poller.rs` — apply aliases and reaping inside the scan loop
- `src-tauri/src/overlay.rs` — replace show/hide driving with position restore and persist
- `src-tauri/src/settings.rs` — persist overlay x/y
- `src-tauri/src/main.rs` — register `set_alias` / `get_aliases` commands, show the overlay at startup, remember its position
- `src-tauri/tauri.conf.json` — overlay window starts unfocused
- `src/overlay.tsx` — mount `OverlayRoster` instead of `OverlayPill`
- `src/types.ts` — add `ended_at`
- `README.md` — document naming (Task 8)

**Delete**
- `src/components/OverlayPill.tsx` (Task 6)

---

### Task 1: Map `shell` status to Working

The observed status log contains a `shell` value that v1 never saw. It currently falls through the unknown branch to `Idle`, which in Homa's vocabulary means "finished, feed it a task". Every shell out fires a false attention signal.

**Files:**
- Modify: `src-tauri/src/mapper.rs:6-22`
- Test: `src-tauri/src/mapper.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: nothing
- Produces: no signature change. `map_status(raw: &str, pid_alive: bool) -> AgentStatus` behaviour only.

- [ ] **Step 1: Write the failing test**

Add to the `mod tests` block in `src-tauri/src/mapper.rs`:

```rust
    #[test]
    fn shell_is_working_not_idle() {
        assert_eq!(map_status("shell", true), AgentStatus::Working);
        assert_eq!(map_status("SHELL", true), AgentStatus::Working);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run from `src-tauri/`: `cargo test shell_is_working`
Expected: FAIL, `assertion `left == right` failed: left: Idle, right: Working`

- [ ] **Step 3: Write minimal implementation**

In `src-tauri/src/mapper.rs`, replace the `if r == "busy"` block with:

```rust
    if r == "busy" || r == "shell" {
        return AgentStatus::Working;
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run from `src-tauri/`: `cargo test mapper`
Expected: PASS, all five mapper tests green including the existing `unknown_fails_toward_attention`.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/mapper.rs
git commit -m "fix: map shell status to Working instead of Idle"
```

---

### Task 2: Alias store

The persistence layer only. No consumers yet.

**Files:**
- Create: `src-tauri/src/alias.rs`
- Modify: `src-tauri/src/lib.rs:1`
- Test: `src-tauri/src/alias.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: nothing
- Produces:
  - `pub type Aliases = std::collections::BTreeMap<String, String>`
  - `pub fn normalize_key(cwd: &str) -> String`
  - `pub fn aliases_path() -> std::path::PathBuf`
  - `pub fn load_from(p: &std::path::Path) -> Aliases`
  - `pub fn load() -> Aliases`
  - `pub fn save_to(p: &std::path::Path, a: &Aliases) -> std::io::Result<()>`
  - `pub fn set_in(p: &std::path::Path, cwd: &str, name: &str) -> std::io::Result<Aliases>`

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/alias.rs` containing only the test module plus the imports it needs:

```rust
use serde_json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub type Aliases = BTreeMap<String, String>;

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("homa-alias-test-{tag}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn normalize_is_case_and_separator_insensitive() {
        let a = normalize_key("C:\\Users\\Saeid\\Desktop\\Migration Site");
        let b = normalize_key("c:/users/saeid/desktop/migration site");
        assert_eq!(a, b);
    }

    #[test]
    fn normalize_strips_trailing_separators() {
        assert_eq!(normalize_key("c:\\foo\\bar\\"), normalize_key("c:\\foo\\bar"));
    }

    #[test]
    fn missing_file_loads_empty() {
        let p = tmpdir("missing").join("aliases.json");
        assert!(load_from(&p).is_empty());
    }

    #[test]
    fn corrupt_file_loads_empty_and_does_not_panic() {
        let p = tmpdir("corrupt").join("aliases.json");
        std::fs::write(&p, b"{not json").unwrap();
        assert!(load_from(&p).is_empty());
    }

    #[test]
    fn set_then_load_round_trips() {
        let p = tmpdir("roundtrip").join("aliases.json");
        set_in(&p, "C:\\Foo\\Bar", "my project").unwrap();
        let a = load_from(&p);
        assert_eq!(a.get(&normalize_key("c:/foo/bar")).map(String::as_str), Some("my project"));
    }

    #[test]
    fn set_trims_whitespace() {
        let p = tmpdir("trim").join("aliases.json");
        set_in(&p, "C:\\Foo", "  spaced  ").unwrap();
        assert_eq!(load_from(&p).get(&normalize_key("c:\\foo")).map(String::as_str), Some("spaced"));
    }

    #[test]
    fn empty_name_removes_entry_rather_than_storing_blank() {
        let p = tmpdir("clear").join("aliases.json");
        set_in(&p, "C:\\Foo", "temp").unwrap();
        set_in(&p, "C:\\Foo", "   ").unwrap();
        assert!(load_from(&p).is_empty());
    }

    #[test]
    fn save_overwrites_existing_file() {
        let p = tmpdir("overwrite").join("aliases.json");
        set_in(&p, "C:\\Foo", "first").unwrap();
        set_in(&p, "C:\\Foo", "second").unwrap();
        assert_eq!(load_from(&p).get(&normalize_key("c:\\foo")).map(String::as_str), Some("second"));
    }

    #[test]
    fn save_creates_missing_parent_directory() {
        let p = tmpdir("mkparent").join("nested").join("aliases.json");
        set_in(&p, "C:\\Foo", "ok").unwrap();
        assert!(p.exists());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Add `pub mod alias;` as the first line of `src-tauri/src/lib.rs`, then run from `src-tauri/`: `cargo test alias`
Expected: FAIL to compile, `cannot find function `normalize_key` in this scope` and similar for the other functions.

- [ ] **Step 3: Write minimal implementation**

Insert above the `#[cfg(test)]` block in `src-tauri/src/alias.rs`:

```rust
/// Windows paths are case insensitive and reach us spelled inconsistently
/// depending on how the session was launched, so keys are folded before use.
pub fn normalize_key(cwd: &str) -> String {
    let mut s = cwd.trim().replace('/', "\\").to_lowercase();
    while s.len() > 1 && s.ends_with('\\') {
        s.pop();
    }
    s
}

pub fn aliases_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("homa")
        .join("aliases.json")
}

pub fn load_from(p: &Path) -> Aliases {
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn load() -> Aliases {
    load_from(&aliases_path())
}

pub fn save_to(p: &Path, a: &Aliases) -> std::io::Result<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    // Write to a sibling temp file and rename over the target: on Windows
    // rename replaces atomically, so a crash mid write cannot truncate the store.
    let tmp = p.with_extension("json.tmp");
    let body = serde_json::to_string_pretty(a)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, p)
}

pub fn set_in(p: &Path, cwd: &str, name: &str) -> std::io::Result<Aliases> {
    let mut a = load_from(p);
    let key = normalize_key(cwd);
    let n = name.trim();
    if n.is_empty() {
        a.remove(&key);
    } else {
        a.insert(key, n.to_string());
    }
    save_to(p, &a)?;
    Ok(a)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run from `src-tauri/`: `cargo test alias`
Expected: PASS, 9 tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/alias.rs src-tauri/src/lib.rs
git commit -m "feat: cwd-keyed alias store with atomic writes"
```

---

### Task 3: Name resolution and collision suffixing

Turns the raw store into displayed names. Pure function over a slice, so it is testable without a filesystem or a running app.

**Files:**
- Modify: `src-tauri/src/alias.rs` (append)
- Test: `src-tauri/src/alias.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `Aliases`, `normalize_key` from Task 2; `AgentState` from `crate::model`
- Produces:
  - `pub fn last_segment(cwd: &str) -> String`
  - `pub fn resolve(agents: &mut [crate::model::AgentState], aliases: &Aliases)`

- [ ] **Step 1: Write the failing tests**

Append inside the existing `mod tests` block in `src-tauri/src/alias.rs`:

```rust
    use crate::model::{AgentState, AgentStatus};

    fn agent(cwd: &str, claude_name: &str, started_at: i64, sid: &str) -> AgentState {
        AgentState {
            pid: 1,
            session_id: sid.to_string(),
            name: claude_name.to_string(),
            cwd: cwd.to_string(),
            repo: "r".into(),
            branch: None,
            status: AgentStatus::Working,
            raw_status: "busy".into(),
            started_at,
            status_updated_at: 0,
            model: None,
            context_pct: None,
            last_activity: None,
            ended_at: None,
        }
    }

    #[test]
    fn alias_wins_over_claude_name() {
        let mut a = Aliases::new();
        a.insert(normalize_key("C:\\Foo"), "my project".into());
        let mut v = vec![agent("C:\\Foo", "agent-foo-61", 100, "s1")];
        resolve(&mut v, &a);
        assert_eq!(v[0].name, "my project");
    }

    #[test]
    fn falls_back_to_claude_name_when_no_alias() {
        let mut v = vec![agent("C:\\Foo", "agent-foo-61", 100, "s1")];
        resolve(&mut v, &Aliases::new());
        assert_eq!(v[0].name, "agent-foo-61");
    }

    #[test]
    fn falls_back_to_last_path_segment_when_claude_name_blank() {
        let mut v = vec![agent("C:\\Users\\saeid\\Migration Site", "  ", 100, "s1")];
        resolve(&mut v, &Aliases::new());
        assert_eq!(v[0].name, "Migration Site");
    }

    #[test]
    fn two_sessions_in_one_folder_are_suffixed_by_start_order() {
        let mut a = Aliases::new();
        a.insert(normalize_key("C:\\Foo"), "homa".into());
        let mut v = vec![
            agent("C:\\Foo", "agent-foo-62", 200, "s2"),
            agent("c:/foo/", "agent-foo-61", 100, "s1"),
        ];
        resolve(&mut v, &a);
        let older = v.iter().find(|x| x.session_id == "s1").unwrap();
        let newer = v.iter().find(|x| x.session_id == "s2").unwrap();
        assert_eq!(older.name, "homa #1");
        assert_eq!(newer.name, "homa #2");
    }

    #[test]
    fn single_session_gets_no_suffix() {
        let mut a = Aliases::new();
        a.insert(normalize_key("C:\\Foo"), "homa".into());
        let mut v = vec![agent("C:\\Foo", "agent-foo-61", 100, "s1")];
        resolve(&mut v, &a);
        assert_eq!(v[0].name, "homa");
    }

    #[test]
    fn suffix_order_is_stable_when_start_times_tie() {
        let mut a = Aliases::new();
        a.insert(normalize_key("C:\\Foo"), "homa".into());
        let mut v = vec![
            agent("C:\\Foo", "x", 100, "sB"),
            agent("C:\\Foo", "x", 100, "sA"),
        ];
        resolve(&mut v, &a);
        // Ties break on session_id so numbering does not flip between polls.
        assert_eq!(v.iter().find(|x| x.session_id == "sA").unwrap().name, "homa #1");
        assert_eq!(v.iter().find(|x| x.session_id == "sB").unwrap().name, "homa #2");
    }

    #[test]
    fn different_folders_sharing_a_name_are_also_disambiguated() {
        let mut a = Aliases::new();
        a.insert(normalize_key("C:\\Foo"), "work".into());
        a.insert(normalize_key("C:\\Bar"), "work".into());
        let mut v = vec![
            agent("C:\\Foo", "x", 100, "s1"),
            agent("C:\\Bar", "y", 200, "s2"),
        ];
        resolve(&mut v, &a);
        assert_eq!(v[0].name, "work #1");
        assert_eq!(v[1].name, "work #2");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run from `src-tauri/`: `cargo test alias`
Expected: FAIL to compile, `cannot find function `resolve` in this scope`, and `struct `AgentState` has no field named `ended_at``.

- [ ] **Step 3: Write minimal implementation**

First add the field to `src-tauri/src/model.rs`, after `last_activity`:

```rust
    pub last_activity: Option<i64>,
    /// Wall clock ms when this session was first observed dead. Set by `reap`.
    pub ended_at: Option<i64>,
```

Then append to `src-tauri/src/alias.rs`, above the test module:

```rust
use crate::model::AgentState;

pub fn last_segment(cwd: &str) -> String {
    cwd.trim_end_matches(['\\', '/'])
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or("")
        .to_string()
}

/// Rewrites `name` on every agent to the name the user should see, then
/// disambiguates any duplicates. Runs on every scan, so it must be cheap
/// and its output must be stable for identical input.
pub fn resolve(agents: &mut [AgentState], aliases: &Aliases) {
    for a in agents.iter_mut() {
        let alias = aliases
            .get(&normalize_key(&a.cwd))
            .map(String::as_str)
            .filter(|s| !s.trim().is_empty());
        a.name = match alias {
            Some(s) => s.to_string(),
            None if !a.name.trim().is_empty() => a.name.trim().to_string(),
            None => last_segment(&a.cwd),
        };
    }

    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, a) in agents.iter().enumerate() {
        groups.entry(a.name.clone()).or_default().push(i);
    }
    for (_, mut idxs) in groups {
        if idxs.len() < 2 {
            continue;
        }
        idxs.sort_by(|&x, &y| {
            agents[x]
                .started_at
                .cmp(&agents[y].started_at)
                .then_with(|| agents[x].session_id.cmp(&agents[y].session_id))
        });
        for (n, i) in idxs.into_iter().enumerate() {
            agents[i].name = format!("{} #{}", agents[i].name, n + 1);
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run from `src-tauri/`: `cargo test`
Expected: PASS. `alias` tests are 16 total. Other modules still compile because `ended_at` has an explicit value only in `alias.rs` tests; if `poller.rs` fails to build with `missing field ended_at`, add `ended_at: None,` to the `AgentState` literal at `poller.rs:61-75`.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/alias.rs src-tauri/src/model.rs src-tauri/src/poller.rs
git commit -m "feat: resolve display names from aliases with collision suffixes"
```

---

### Task 4: Reap ended sessions after a TTL

Dead sessions should linger briefly so an exit is noticeable, then leave. Claude usually removes its own session file on a clean exit, so this mostly matters after a crash, but without it a stale file would sit in the list forever.

**Files:**
- Create: `src-tauri/src/reap.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/reap.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `AgentState`, `AgentStatus` from `crate::model`; `ended_at` field from Task 3
- Produces:
  - `pub const ENDED_TTL_MS: i64 = 10_000`
  - `pub fn now_ms() -> i64`
  - `pub fn reap(prev: &[AgentState], next: &mut Vec<AgentState>, now: i64, ttl_ms: i64)`

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/reap.rs`:

```rust
use crate::model::{AgentState, AgentStatus};

pub const ENDED_TTL_MS: i64 = 10_000;

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(sid: &str, status: AgentStatus, ended_at: Option<i64>) -> AgentState {
        AgentState {
            pid: 1,
            session_id: sid.to_string(),
            name: sid.to_string(),
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
            ended_at,
        }
    }

    #[test]
    fn stamps_ended_at_on_first_sighting() {
        let prev = vec![agent("s1", AgentStatus::Working, None)];
        let mut next = vec![agent("s1", AgentStatus::Ended, None)];
        reap(&prev, &mut next, 5_000, ENDED_TTL_MS);
        assert_eq!(next[0].ended_at, Some(5_000));
    }

    #[test]
    fn carries_ended_at_forward_across_polls() {
        let prev = vec![agent("s1", AgentStatus::Ended, Some(5_000))];
        let mut next = vec![agent("s1", AgentStatus::Ended, None)];
        reap(&prev, &mut next, 9_000, ENDED_TTL_MS);
        assert_eq!(next[0].ended_at, Some(5_000), "must not restamp each poll");
    }

    #[test]
    fn keeps_ended_session_inside_ttl() {
        let prev = vec![agent("s1", AgentStatus::Ended, Some(5_000))];
        let mut next = vec![agent("s1", AgentStatus::Ended, None)];
        reap(&prev, &mut next, 14_000, ENDED_TTL_MS);
        assert_eq!(next.len(), 1);
    }

    #[test]
    fn drops_ended_session_past_ttl() {
        let prev = vec![agent("s1", AgentStatus::Ended, Some(5_000))];
        let mut next = vec![agent("s1", AgentStatus::Ended, None)];
        reap(&prev, &mut next, 15_001, ENDED_TTL_MS);
        assert!(next.is_empty());
    }

    #[test]
    fn never_drops_a_live_session() {
        let prev = vec![agent("s1", AgentStatus::Working, None)];
        let mut next = vec![agent("s1", AgentStatus::Working, None)];
        reap(&prev, &mut next, 999_999, ENDED_TTL_MS);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].ended_at, None);
    }

    #[test]
    fn clears_ended_at_if_a_session_comes_back_alive() {
        let prev = vec![agent("s1", AgentStatus::Ended, Some(5_000))];
        let mut next = vec![agent("s1", AgentStatus::Working, None)];
        reap(&prev, &mut next, 6_000, ENDED_TTL_MS);
        assert_eq!(next[0].ended_at, None);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Add `pub mod reap;` to `src-tauri/src/lib.rs` (keep the list alphabetical: after `poller`), then run from `src-tauri/`: `cargo test reap`
Expected: FAIL to compile, `cannot find function `reap` in this scope`.

- [ ] **Step 3: Write minimal implementation**

Insert above the `#[cfg(test)]` block in `src-tauri/src/reap.rs`:

```rust
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Stamps the moment a session was first seen dead, carries that stamp across
/// polls, and removes rows that have been dead longer than `ttl_ms`.
pub fn reap(prev: &[AgentState], next: &mut Vec<AgentState>, now: i64, ttl_ms: i64) {
    for a in next.iter_mut() {
        if a.status != AgentStatus::Ended {
            a.ended_at = None;
            continue;
        }
        let carried = prev
            .iter()
            .find(|p| p.session_id == a.session_id)
            .and_then(|p| p.ended_at);
        a.ended_at = Some(carried.unwrap_or(now));
    }
    next.retain(|a| match (a.status, a.ended_at) {
        (AgentStatus::Ended, Some(t)) => now - t <= ttl_ms,
        _ => true,
    });
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run from `src-tauri/`: `cargo test reap`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/reap.rs src-tauri/src/lib.rs
git commit -m "feat: reap ended sessions after a 10s grace period"
```

---

### Task 5: Wire aliases and reaping into the poller, expose commands

Everything built so far is inert until the scan loop calls it. This task also adds the two Tauri commands the overlay will invoke.

**Files:**
- Modify: `src-tauri/src/poller.rs:37-91` (`scan_once`) and `:105-124` (loop body)
- Modify: `src-tauri/src/main.rs:12-15` (commands) and `:41` (handler registration)
- Test: `src-tauri/src/poller.rs` (new inline `mod tests`)

**Interfaces:**
- Consumes: `alias::{load, resolve, set_in, aliases_path, Aliases}`, `reap::{reap, now_ms, ENDED_TTL_MS}`
- Produces:
  - `scan_once(dir: &Path, enrich: bool) -> Vec<AgentState>` now returns alias resolved names, sorted by status priority then resolved name
  - Tauri command `set_alias(cwd: String, name: String) -> Result<(), String>`
  - Tauri command `get_aliases() -> Aliases`

- [ ] **Step 1: Write the failing test**

Append to `src-tauri/src/poller.rs`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run from `src-tauri/`: `cargo test poller`
Expected: FAIL to compile, `cannot find function `scan_once_with` in this scope`.

- [ ] **Step 3: Write minimal implementation**

In `src-tauri/src/poller.rs`, add the import and split `scan_once` so the alias map can be injected for testing:

```rust
use crate::alias::{self, Aliases};
```

Rename the existing `pub fn scan_once(dir: &Path, enrich: bool) -> Vec<AgentState>` to
`pub fn scan_once_with(dir: &Path, enrich: bool, aliases: &Aliases) -> Vec<AgentState>`,
then replace its trailing sort block with:

```rust
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
```

Also add `ended_at: None,` to the `AgentState` literal if Task 3 did not already.

In the same file, change the loop body so reaping runs before comparison. Replace `let next = scan_once(&dir, window_visible);` and the `let prev` line with:

```rust
            let prev = { shared.lock().unwrap().clone() };
            let mut next = scan_once(&dir, window_visible);
            crate::reap::reap(&prev, &mut next, crate::reap::now_ms(), crate::reap::ENDED_TTL_MS);
```

Reaping mutates rows every poll while an ended session is inside its TTL, but `ended_at` is carried rather than restamped, so `next != prev` stays false and the loop does not spin.

The TTL expiry needs its own wakeup, because nothing else changes when the grace period lapses. That is already covered: the loop re-scans every 2000ms regardless, so an expired row disappears within one tick of its deadline.

In `src-tauri/src/main.rs`, add the commands above `fn toggle_main`:

```rust
#[tauri::command]
fn set_alias(cwd: String, name: String) -> Result<(), String> {
    homa_lib::alias::set_in(&homa_lib::alias::aliases_path(), &cwd, &name)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_aliases() -> homa_lib::alias::Aliases {
    homa_lib::alias::load()
}
```

and widen the handler registration:

```rust
        .invoke_handler(tauri::generate_handler![get_agents, set_alias, get_aliases])
```

- [ ] **Step 4: Run tests to verify they pass**

Run from `src-tauri/`: `cargo test`
Expected: PASS, all tests including the two new poller tests. Then `cargo build` to confirm `main.rs` compiles with the new commands.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/poller.rs src-tauri/src/main.rs
git commit -m "feat: apply aliases and reaping in the scan loop, expose alias commands"
```

---

### Task 6: Overlay roster component

Replaces the count pill with the named list. Display only; renaming lands in Task 7.

**Files:**
- Create: `src/components/OverlayRoster.tsx`
- Create: `src/components/OverlayRoster.test.tsx`
- Modify: `src/overlay.tsx`
- Modify: `src/types.ts`
- Delete: `src/components/OverlayPill.tsx`

**Interfaces:**
- Consumes: `AgentState` from `../types`; the `agents-updated` event and `get_agents` command, both already provided by the `useAgents` hook. Tauri v2 `app.emit` broadcasts to every webview, so the overlay window receives `agents-updated` today with no Rust change.
- Produces: `export function OverlayRoster(): JSX.Element`

- [ ] **Step 1: Write the failing tests**

Create `src/components/OverlayRoster.test.tsx`:

```tsx
import { render, screen, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { OverlayRoster } from "./OverlayRoster";
import type { AgentState } from "../types";

// vi.mock factories are hoisted above the file's own declarations, so the
// mutable fixture has to be created inside vi.hoisted or the factory hits
// a temporal dead zone error on `mockAgents`.
const h = vi.hoisted(() => ({ agents: [] as AgentState[] }));

vi.mock("../hooks/useAgents", () => ({ useAgents: () => h.agents }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    setSize: vi.fn().mockResolvedValue(undefined),
    startDragging: vi.fn(),
  }),
}));
vi.mock("@tauri-apps/api/dpi", () => ({
  LogicalSize: class { constructor(public width: number, public height: number) {} },
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const setAgents = (a: AgentState[]) => {
  h.agents = a;
};

const mk = (over: Partial<AgentState>): AgentState => ({
  pid: 1,
  session_id: "s",
  name: "n",
  cwd: "c",
  repo: "r",
  branch: null,
  status: "working",
  raw_status: "busy",
  started_at: 0,
  status_updated_at: 0,
  model: null,
  context_pct: null,
  last_activity: null,
  ended_at: null,
  ...over,
});

test("renders one row per session showing its name", () => {
  setAgents([
    mk({ session_id: "a", name: "migration site", status: "waiting" }),
    mk({ session_id: "b", name: "homa", status: "working" }),
  ]);
  render(<OverlayRoster />);
  expect(screen.getByText("migration site")).toBeInTheDocument();
  expect(screen.getByText("homa")).toBeInTheDocument();
});

test("orders waiting above idle above working", () => {
  setAgents([
    mk({ session_id: "a", name: "third", status: "working" }),
    mk({ session_id: "b", name: "first", status: "waiting" }),
    mk({ session_id: "c", name: "second", status: "idle" }),
  ]);
  render(<OverlayRoster />);
  const names = screen.getAllByTestId("row-name").map((n) => n.textContent);
  expect(names).toEqual(["first", "second", "third"]);
});

test("shows a muted empty state rather than nothing", () => {
  setAgents([]);
  render(<OverlayRoster />);
  expect(screen.getByText(/no sessions/i)).toBeInTheDocument();
});

test("marks each row with its status for colouring", () => {
  setAgents([mk({ session_id: "a", name: "x", status: "waiting" })]);
  render(<OverlayRoster />);
  expect(screen.getByTestId("row-dot")).toHaveAttribute("data-status", "waiting");
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run from the repo root: `npm test`
Expected: FAIL, `Failed to resolve import "./OverlayRoster"`.

- [ ] **Step 3: Write minimal implementation**

Add `ended_at: number | null;` to the `AgentState` interface in `src/types.ts`, after `last_activity`.

Create `src/components/OverlayRoster.tsx`:

```tsx
import { useEffect, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { useAgents } from "../hooks/useAgents";
import type { AgentStatus } from "../types";

const DOT: Record<AgentStatus, string> = {
  waiting: "bg-red-500",
  idle: "bg-amber-400",
  working: "bg-sky-500",
  ended: "bg-neutral-500",
};

const RANK: Record<AgentStatus, number> = { waiting: 3, idle: 2, working: 1, ended: 0 };

const ROW_H = 30;
const CHROME_H = 16;
const MAX_H = 420;
const WIDTH = 240;

export function OverlayRoster() {
  const agents = useAgents();
  const rows = [...agents].sort(
    (a, b) => RANK[b.status] - RANK[a.status] || a.name.localeCompare(b.name)
  );
  const lastH = useRef(0);

  useEffect(() => {
    const h = Math.min(MAX_H, Math.max(1, rows.length) * ROW_H + CHROME_H);
    if (h === lastH.current) return;
    lastH.current = h;
    // The window follows the content: the overlay is always up, so it must
    // never be taller than what it is showing.
    getCurrentWindow().setSize(new LogicalSize(WIDTH, h)).catch(() => {});
  }, [rows.length]);

  return (
    <div
      data-tauri-drag-region
      className="h-screen w-screen overflow-y-auto rounded-xl bg-neutral-950/90 px-2 py-2 text-neutral-100 select-none"
    >
      {rows.length === 0 && (
        <div className="px-2 py-1 text-sm text-neutral-500">no sessions</div>
      )}
      {rows.map((a) => (
        <div key={a.session_id} className="flex items-center gap-2 rounded px-2 py-1 text-sm">
          <span
            data-testid="row-dot"
            data-status={a.status}
            className={`h-2.5 w-2.5 shrink-0 rounded-full ${DOT[a.status]}`}
          />
          <span data-testid="row-name" className="truncate">
            {a.name}
          </span>
        </div>
      ))}
    </div>
  );
}
```

Replace the body of `src/overlay.tsx` so it mounts `OverlayRoster` instead of `OverlayPill`, keeping whatever imports of `index.css` and `createRoot` it already has:

```tsx
import { OverlayRoster } from "./components/OverlayRoster";
```

and render `<OverlayRoster />` in place of `<OverlayPill />`.

Then delete the old component:

```bash
git rm src/components/OverlayPill.tsx
```

- [ ] **Step 4: Run tests to verify they pass**

Run from the repo root: `npm test`
Expected: PASS, 4 new tests plus the 2 existing ones. Then `npm run build` to confirm the overlay entry still compiles and no import of `OverlayPill` remains.

- [ ] **Step 5: Commit**

```bash
git add src/components/OverlayRoster.tsx src/components/OverlayRoster.test.tsx src/overlay.tsx src/types.ts
git commit -m "feat: overlay shows a named session roster instead of a count pill"
```

---

### Task 7: Inline rename

**Files:**
- Modify: `src/components/OverlayRoster.tsx`
- Modify: `src/components/OverlayRoster.test.tsx` (append)

**Interfaces:**
- Consumes: `invoke` from `@tauri-apps/api/core`; the `set_alias` command from Task 5, whose parameters are `{ cwd: string, name: string }`
- Produces: no new exports

Tauri's `invoke` converts camelCase JS keys to snake_case Rust parameters, and `cwd` is already a single lowercase word, so the argument object is `{ cwd, name }` verbatim.

- [ ] **Step 1: Write the failing tests**

Append to `src/components/OverlayRoster.test.tsx`. `fireEvent` and `invoke` are already imported at the top of that file from Task 6, so add no new import lines.

```tsx
const oneRow = () => setAgents([mk({ session_id: "a", name: "homa", cwd: "C:\\Homa" })]);

const edit = (value: string) => {
  render(<OverlayRoster />);
  fireEvent.doubleClick(screen.getByTestId("row-name"));
  const box = screen.getByRole("textbox");
  fireEvent.change(box, { target: { value } });
  return box;
};

test("double click turns the name into an input seeded with the current name", () => {
  oneRow();
  render(<OverlayRoster />);
  fireEvent.doubleClick(screen.getByTestId("row-name"));
  expect(screen.getByRole("textbox")).toHaveValue("homa");
});

test("enter commits the new name against the row's cwd", () => {
  vi.mocked(invoke).mockClear();
  oneRow();
  fireEvent.keyDown(edit("tray app"), { key: "Enter" });
  expect(invoke).toHaveBeenCalledWith("set_alias", { cwd: "C:\\Homa", name: "tray app" });
});

test("escape cancels without saving", () => {
  vi.mocked(invoke).mockClear();
  oneRow();
  fireEvent.keyDown(edit("discarded"), { key: "Escape" });
  expect(invoke).not.toHaveBeenCalled();
  expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
});

test("an empty commit clears the alias rather than storing a blank", () => {
  vi.mocked(invoke).mockClear();
  oneRow();
  fireEvent.keyDown(edit("   "), { key: "Enter" });
  expect(invoke).toHaveBeenCalledWith("set_alias", { cwd: "C:\\Homa", name: "   " });
});

test("blur commits", () => {
  vi.mocked(invoke).mockClear();
  oneRow();
  fireEvent.blur(edit("blurred"));
  expect(invoke).toHaveBeenCalledWith("set_alias", { cwd: "C:\\Homa", name: "blurred" });
});
```

The blank case asserts the raw value reaches Rust: trimming and clearing are the store's job, already covered by `empty_name_removes_entry_rather_than_storing_blank` in Task 2. Keeping one owner for that rule avoids two places disagreeing about what counts as empty.

- [ ] **Step 2: Run tests to verify they fail**

Run from the repo root: `npm test`
Expected: FAIL, `Unable to find an accessible element with the role "textbox"`.

- [ ] **Step 3: Write minimal implementation**

In `src/components/OverlayRoster.tsx`, add the imports:

```tsx
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
```

Add editing state inside the component, above the `return`:

```tsx
  const [editing, setEditing] = useState<string | null>(null);
  const [draft, setDraft] = useState("");

  const beginEdit = (sessionId: string, current: string) => {
    setEditing(sessionId);
    setDraft(current);
  };

  const commit = (cwd: string) => {
    if (editing === null) return;
    setEditing(null);
    invoke("set_alias", { cwd, name: draft }).catch(() => {});
  };

  const cancel = () => setEditing(null);
```

Replace the name span in the row with a conditional:

```tsx
          {editing === a.session_id ? (
            <input
              autoFocus
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") commit(a.cwd);
                if (e.key === "Escape") cancel();
              }}
              onBlur={() => commit(a.cwd)}
              className="min-w-0 flex-1 rounded border border-sky-500 bg-neutral-900 px-1 text-sm outline-none"
            />
          ) : (
            <span
              data-testid="row-name"
              onDoubleClick={() => beginEdit(a.session_id, a.name)}
              className="truncate"
            >
              {a.name}
            </span>
          )}
```

Escape must run before blur clears the draft, which it does: `cancel` sets `editing` to null, and the subsequent blur returns early because `commit` checks `editing === null`.

The row currently sits under `data-tauri-drag-region` on the container. A drag region swallows mouse events on Windows, so move that attribute off the container and onto a dedicated grab strip so the rows stay clickable:

```tsx
      <div data-tauri-drag-region className="h-2 w-full cursor-grab" />
```

placed as the first child inside the container, and remove `data-tauri-drag-region` from the container itself. Update `CHROME_H` to `24` to account for the strip.

- [ ] **Step 4: Run tests to verify they pass**

Run from the repo root: `npm test`
Expected: PASS, all 11 frontend tests.

- [ ] **Step 5: Commit**

```bash
git add src/components/OverlayRoster.tsx src/components/OverlayRoster.test.tsx
git commit -m "feat: rename a session inline by double-clicking its row"
```

---

### Task 8: Always visible overlay with remembered position

The last behavioural change, and the one that cannot be proved by automated tests. Take the manual checklist seriously: v1 shipped without it and that is why we are here.

**Files:**
- Modify: `src-tauri/src/overlay.rs` (rewrite)
- Modify: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/poller.rs` (drop the `overlay::drive` call)
- Modify: `src-tauri/tauri.conf.json`
- Modify: `README.md`
- Test: `src-tauri/src/settings.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `Settings` from Task 2's untouched store
- Produces:
  - `Settings { sound_enabled, sound_on_idle, overlay_x: Option<f64>, overlay_y: Option<f64> }`
  - `pub fn overlay::restore_and_show(app: &AppHandle)`
  - `pub fn overlay::remember_position(x: f64, y: f64)`

- [ ] **Step 1: Write the failing test**

Append a test module to `src-tauri/src/settings.rs`:

```rust
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
}
```

- [ ] **Step 2: Run test to verify it fails**

Run from `src-tauri/`: `cargo test settings`
Expected: FAIL to compile, `no field `overlay_x` on type `Settings``.

- [ ] **Step 3: Write minimal implementation**

In `src-tauri/src/settings.rs`, extend the struct and default:

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub sound_enabled: bool,
    pub sound_on_idle: bool,
    #[serde(default)]
    pub overlay_x: Option<f64>,
    #[serde(default)]
    pub overlay_y: Option<f64>,
}
```

and add `overlay_x: None, overlay_y: None,` to `Default::default`. Remove the `#[allow(dead_code)]` above `save`, which now has a caller.

Rewrite `src-tauri/src/overlay.rs` entirely:

```rust
use crate::settings::Settings;
use tauri::{AppHandle, Manager, PhysicalPosition};

/// The overlay is a permanent dashboard, not an alert: it is shown once at
/// startup and never hidden. Attention is carried by dot colour, the tray
/// icon, toasts, and sound.
pub fn restore_and_show(app: &AppHandle) {
    let Some(w) = app.get_webview_window("overlay") else {
        return;
    };
    let s = Settings::load();
    if let (Some(x), Some(y)) = (s.overlay_x, s.overlay_y) {
        let _ = w.set_position(PhysicalPosition::new(x, y));
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
```

In `src-tauri/src/poller.rs`, delete the line `crate::overlay::drive(&app, &summary);`.

In `src-tauri/src/main.rs`, inside `setup` and before `poller::start_watching`, add:

```rust
            if let Some(w) = app.get_webview_window("overlay") {
                w.on_window_event(move |e| {
                    if let WindowEvent::Moved(pos) = e {
                        homa_lib::overlay::remember_position(pos.x as f64, pos.y as f64);
                    }
                });
            }
            homa_lib::overlay::restore_and_show(&app.handle().clone());
```

and extend the `use homa_lib::...` line to `use homa_lib::{model::AgentState, overlay, poller};` if you prefer the shorter call sites.

In `src-tauri/tauri.conf.json`, add `"focus": false` to the overlay window object so showing it never steals focus from a game or terminal. Leave `visible: false` as it is: `restore_and_show` shows it after the saved position is applied, which avoids a visible jump from the default position to the remembered one.

- [ ] **Step 4: Run tests, then verify manually**

Run from `src-tauri/`: `cargo test`
Expected: PASS, all Rust tests.

Then build and run the real app. Kill the currently running Homa first (it is running as PID from a v1 install, and `tauri_plugin_single_instance` will refuse a second copy):

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
Stop-Process -Name homa -Force -ErrorAction SilentlyContinue
npm run tauri dev
```

Walk this checklist and record the result of each line:

- [ ] The overlay appears at startup without any session needing attention
- [ ] It lists live sessions by name, one row each
- [ ] Starting a new Claude session adds a row within about 2 seconds
- [ ] A session going idle turns its dot amber, and the overlay does not move or hide
- [ ] The toast and the tray icon colour still fire as they did in v1
- [ ] Double clicking a row lets you type, Enter saves, and the name persists after `npm run tauri dev` is restarted
- [ ] Renaming a folder with two live sessions shows `name #1` and `name #2`
- [ ] Dragging the top strip moves the window; after a restart it reappears where you left it
- [ ] The overlay does not appear in alt tab and does not appear in the taskbar
- [ ] Clicking the overlay does not pull focus away from a focused terminal
- [ ] The window height matches the row count from one row up to five, and scrolls rather than growing past roughly 420px
- [ ] With no sessions running it shows "no sessions" and stays visible
- [ ] Ending a session leaves a grey row that disappears about 10 seconds later

If any line fails, fix it before committing rather than filing it as follow up work. This checklist is the acceptance criterion for the whole plan.

- [ ] **Step 5: Update the README and commit**

In `README.md`, update the overlay description to say it is always visible and lists sessions by name, and add a short section documenting that names are per folder, set by double clicking a row, stored in `%APPDATA%\homa\aliases.json`, and cleared by saving an empty name.

```bash
git add src-tauri/src/overlay.rs src-tauri/src/settings.rs src-tauri/src/main.rs src-tauri/src/poller.rs src-tauri/tauri.conf.json README.md
git commit -m "feat: overlay is always visible and remembers its position"
```

---

## Done when

All eight tasks committed, `cargo test` and `npm test` green, and every line of the Task 8 manual checklist verified by eye in a running build.
