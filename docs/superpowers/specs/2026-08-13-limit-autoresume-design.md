# Homa v3: Resizable Overlay, Click to Focus, and Usage Limit Auto Resume

Design approved in chat 2026-08-13. Branch: `feat/limit-autoresume`, based on `feat/named-roster`.

## Goals

1. The overlay window is bigger, resizable, has a real top bar for dragging, and can be minimized (hidden) and restored from the tray.
2. Clicking an agent row brings that agent's terminal window to the foreground.
3. When a Claude Code session is stopped by a usage limit, Homa detects it, shows it as a Limited state with a countdown, and when the reset time passes it automatically types a resume nudge into the terminals of sessions that were working when the limit hit.

## Substrate findings (verified 2026-08-13 on live data)

- A usage limit stop is recorded in the session transcript as a synthetic assistant message: `"model":"<synthetic>"` with text `You've hit your session limit · resets 12:40am (Europe/Oslo)`. Variants observed: `Credit balance is too low`, `Login expired · Please run /login`.
- The reset time appears only as local wall clock text (`12:40am`), so Homa parses hour and minute and schedules the next future occurrence in local time.
- Transcripts also carry `{"type":"ai-title","aiTitle":"..."}` lines. The terminal window title is a status glyph plus that exact text.
- Process ancestry on this machine: `claude.exe <- powershell.exe <- WindowsTerminal.exe`. One WT process owns one top level window per session (verified with three live sessions), so matching a window title suffix against the session's last `aiTitle` resolves the correct HWND even when several sessions share the WT process.

## Part A: Window changes

- `tauri.conf.json`: overlay becomes `resizable: true`, default 320x280.
- `OverlayRoster.tsx`: the thin drag strip becomes a top bar: drag region across the width, a minimize button at the right that calls a new `hide_overlay` command. Rows scroll inside whatever size the user sets; the auto height `setSize` effect is deleted.
- `settings.rs`: add `overlay_w`, `overlay_h` (Option<f64>, serde default). `overlay.rs` restores size with position and remembers both on Moved and Resized window events.
- Tray: new menu item `Show overlay`; left click still toggles the main window.

## Part B: Click to focus

- New `terminal.rs`: `resolve_hwnd(pid, ai_title) -> Option<isize>` implemented as: walk process ancestry via sysinfo to collect ancestor pids, enumerate top level windows (EnumWindows), prefer a visible window owned by an ancestor whose title ends with the session's `aiTitle`, else the innermost ancestor's main window. `focus_hwnd(hwnd)` uses SetForegroundWindow with the AttachThreadInput fallback for the foreground lock.
- `enrich.rs` additionally captures the last `aiTitle` per session; stored on `AgentState.ai_title`.
- New Tauri command `focus_session(pid, ai_title)`; single click on a roster row invokes it. Double click still renames; the click handler ignores the second click via a small delay-free guard (single click fires focus, which does not conflict with rename entering edit mode on double click).
- Win32 calls go through `windows-sys` (already in Tauri's tree; added as a direct dependency with the needed features). This is the one deviation from the v2 no-new-deps rule, approved in chat.

## Part C: Usage limit auto resume

- `limit.rs`: `detect_limit(lines) -> Option<LimitEvent>` scans the transcript tail for the newest synthetic message. `LimitEvent { kind: SessionLimit { resets_at_ms } | CreditBalance | LoginExpired, at_ms }`. Reset parsing: `resets H:MMam/pm` -> next future local occurrence. A limit event is only honored if it is newer than the last user or assistant turn that follows it (a limit the user already resumed past is stale and ignored).
- `model.rs`: `AgentStatus::Limited` variant (priority between Waiting and Idle), plus `limited_until: Option<i64>` and `was_busy_at_limit: bool` on `AgentState`.
- Poller: a session whose raw status is idle or waiting but whose transcript tail shows a fresh limit event is displayed as Limited. `was_busy_at_limit` is true when the previous poll's status was Working (busy or shell) or the transcript shows the limit arrived mid task (tool_use immediately before the synthetic message).
- Scheduler: each poll tick, sessions with `limited_until` in the past and `was_busy_at_limit` fire auto resume once (a per session `resumed` latch prevents repeats), then are treated as normal again.
- `inject.rs`: resolve the HWND as in Part B, focus it, SendInput the text `continue` then Enter. Immediately, always (user's explicit choice). After injecting all due sessions, one toast: `Resumed N agent(s) after limit reset`.
- Kill switch: tray menu item `Auto-resume: on/off`, persisted as `auto_resume_enabled` in settings (default on). When off, Limited display and countdown still work; only injection is suppressed.
- CreditBalance and LoginExpired display as Limited with no countdown and never auto resume.
- Safety rails: injection fires only for sessions Homa itself observed transition into Limited while mid task, only once per limit event, and only when the resolved window title still matches the session's `aiTitle` at injection time (re-resolved immediately before typing). If resolution fails the fallback is a loud toast naming the session.

## Overlay UI for Limited

- Dot color: purple (`bg-purple-500`), `data-status="limited"`.
- The row shows `resets 12:40am` (or `resuming...` once fired) as muted text right of the name.

## Testing

- Rust: unit tests for limit parsing (all three variants, stale event rejection, reset time rollover past midnight), scheduler latch behaviour, settings round trip. `terminal.rs`/`inject.rs` Win32 internals are thin wrappers, verified manually.
- Frontend: vitest for top bar render, minimize button invoke, limited row content, click handler invoking `focus_session`.
- Manual acceptance checklist in the plan (resize persistence, minimize/restore, click to focus each of three live sessions, and a staged limit rehearsal by pointing HOMA_SESSIONS_DIR at a fixture with a fake transcript whose reset time is one minute in the future).

## Known limitations (accepted)

- Two sessions in tabs of one terminal window: title matching picks the window, but WT activates whatever tab is active; the nudge can land in the wrong tab. On this machine each session has its own window; flagged in README.
- VS Code integrated terminals: not an ancestor pattern Homa recognises in v3; those sessions get the toast fallback instead of injection.
- If the terminal was closed during the limit wait, resolution fails and the toast fallback fires.
