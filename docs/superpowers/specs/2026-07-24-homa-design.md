# Homa — Design Spec

> A featherweight Windows tray app that watches your Claude Code agents and tells you, at a glance, which ones need you.

- **Date:** 2026-07-24
- **Status:** Approved design, pending spec review
- **Owner:** Saeid
- **Inspiration:** [gavraz/recon](https://github.com/gavraz/recon) (a tmux-native Rust TUI for the same job), reimagined Windows-native and ambient.

---

## 1. Problem

Saeid runs several Claude Code agents at once, each in its own Windows Terminal tab. There is no single place to see which agents are working, which are blocked on him, and which have gone quiet. Hunting through terminal tabs breaks focus, and when he steps away to game, a finished or blocked agent can sit idle for a long time unnoticed.

`recon` solves this on Unix by sitting on tmux: it enumerates sessions and scrapes each pane's visible status bar. That substrate does not exist on Windows — there is no tmux, and Windows Terminal exposes **no supported API to enumerate tabs or read pane content**. So Homa is not a port; it is a reimagining on a different, cleaner substrate.

## 2. Goal

An always-on, ambient tray monitor that:

- Reads the live roster and status of every Claude Code session **without touching the terminal**.
- Signals attention through a layered model that works even while a game is running.
- Treats **idle** as an attention state (a finished agent waiting for its next task), not a calm one — only *actively working* is calm.
- Stays featherweight enough to run behind a fullscreen game without stealing meaningful RAM/CPU.

## 3. Substrate (the key finding)

Windows Claude Code already writes a per-session status file — no terminal scraping required:

`~/.claude/sessions/<pid>.json`
```json
{
  "pid": 13732,
  "sessionId": "1fe8e5c7-20c9-40cf-80b2-4f1c1968088b",
  "cwd": "C:\\Users\\saeid\\Desktop\\Agent Simorgh",
  "name": "agent-simorgh-61",
  "status": "busy",
  "startedAt": 1784898651809,
  "updatedAt": 1784898664841,
  "statusUpdatedAt": 1784898664841,
  "version": "2.1.218",
  "kind": "interactive",
  "entrypoint": "cli"
}
```

This hands us the entire roster recon rebuilds from tmux — name, `cwd` (→ repo/branch), a live `status` field, and timestamps — terminal-agnostic.

The matching transcript, `~/.claude/projects/<slug>/<sessionId>.jsonl`, provides richer detail (model, token/context usage, last-activity summary) when we want it.

**Open item for implementation:** the full set of values Claude writes to `status` must be enumerated empirically (only `busy` observed so far). The status mapper (§6) must fail safe for any value it does not recognize.

## 4. Form factor & stack

- **Form:** Windows system-tray app with a summonable main window and an optional always-on-top mini-pill overlay. Normally hidden; lives in the tray.
- **Stack:** **Tauri** — Rust core + React/Tailwind UI.
  - *Why Tauri over Electron:* it uses the WebView2 already present on Windows 11, so install and idle footprint are ~10 MB / low RAM. This is the deciding factor — it must not compete with a running game. Electron's bundled Chromium (hundreds of MB idle RAM) fails that test.
  - *Why Tauri over C#/.NET:* the aesthetic layer (animated pixel-art tamagotchi) is far faster to build in React/Tailwind, the stack Saeid is most fluent and opinionated in. Rust handles only the OS glue.

## 5. Architecture

### 5.1 Rust core

| Component | Responsibility |
|---|---|
| **Poller** | `notify` file-watcher on `~/.claude/sessions/` for instant reaction, plus a ~2 s reconcile tick to catch in-place file rewrites and process death. |
| **Liveness checker** | Cross-check each file's `pid` against running processes (`sysinfo`) so a crashed session does not linger as a fake "idle" agent. |
| **Status mapper** | Normalize Claude's raw `status` into Homa's 3-state model (§7). Unknown values fail **toward attention**, never hidden. |
| **Enricher (lazy)** | Only while the main window is open, read the tail of each transcript JSONL for model, context %, and last-activity. Skipped while hidden → near-idle cost during gaming. |
| **Tray manager** | Drive the tray icon variant + badge and the tray menu (show/hide, quit, per-session quick list). |
| **Notifier** | Fire native Windows toasts on state transitions. Windows auto-suppresses these in fullscreen games — which is the intended behavior. |
| **Overlay controller** | Show/hide and position the mini-pill overlay window (§7). |

### 5.2 React / Tailwind UI

- **Main window** (hidden by default, summoned from the tray):
  - **Roster view** — agents grouped by repo (derived from `cwd`): name, repo/branch, state, context %, last activity. (recon's table analog.)
  - **Tamagotchi view** — each agent is an animated pixel creature whose animation reflects its state (working = busy; idle = bored/sleepy; waiting = alert/waving). Toggle between roster and tamagotchi.
- **Mini-pill overlay** — a small frameless always-on-top window (§7).

## 6. Data flow

```
~/.claude/sessions/*.json ──(notify + 2s reconcile)──▶ Poller
        │                                                  │
        │  pid liveness (sysinfo)                          ▼
        │                                          Status mapper ──▶ normalized AgentState[]
        │  transcript tail (lazy, window-open only) ──────────────────────┘
        │
        └──▶ AgentState[] fans out to three sinks:
               (a) Tray icon variant + badge
               (b) Toast on state transition
               (c) Tauri event ──▶ React renders roster / tamagotchi / overlay
```

## 7. State & signaling model

**Only *working* is calm.** Idle and waiting are both attention states, kept visually distinct so a glance tells them apart.

| State | Meaning | Tray icon | Badge | Toast | Overlay |
|---|---|---|---|---|---|
| **Working** | Actively doing work | Calm (blue/green) | — | no | hidden |
| **Idle** | Finished its turn, unused | Amber, pulsing | idle count | "…is idle, feed it" | shown |
| **Waiting on input** | Blocked on your answer/permission | Red | waiting count | "…is waiting on you" | shown |
| **Ended** | pid dead / session gone | Grey, then removed after grace | — | no | hidden |

- **Tray icon** reflects the highest-priority state present: **waiting > idle > working**. Always accurate even in-game (a taskbar glance suffices).
- **Toast** fires on transitions into idle/waiting, debounced so a momentary blip does not fire. Auto-suppressed by Windows in fullscreen games (intended).
- **Sound cue** — optional, off by default; a short configurable chime on transition to waiting (and optionally idle). Pierces games for users who want it.
- **Mini-pill overlay** — small frameless always-on-top pill, draggable and corner-parkable (position persisted). **Auto-shows only when a state needs attention and hides when all agents are calm**, so it never clutters during focused work. Visible over borderless/windowed-fullscreen games. **Honest limit:** *exclusive*-fullscreen games suppress every overlay — nothing on Windows can defeat that; the tray-icon glance and optional sound remain the fallbacks there.

## 8. Error handling

| Condition | Behavior |
|---|---|
| Mid-write / partial JSON | Retry/skip; keep last good parse for that session. |
| Dead pid, file still present | Mark **Ended**, grey out, drop after a grace period. |
| Sessions dir missing / empty | Neutral tray icon + empty state in window. |
| Unknown `status` value | Treat as attention (do not hide). |
| Transcript missing / very large | Cap the tail read (last N KB); degrade to "no context info." |
| Enricher error | Roster still renders from the session file alone. |

## 9. Testing

- **Rust unit tests** — parser, status mapper, and liveness logic against fixtures: well-formed, malformed/partial, dead-pid, unknown-status.
- **Replay / fixture mode** — point the poller at a fake sessions dir with scripted files to simulate `working → waiting → idle → ended` transitions without live agents. Drives tray, toast, and overlay behavior deterministically.
- **Frontend component tests** — roster and tamagotchi render correctly for each state; overlay show/hide logic.
- **Manual smoke** — run several real `claude` sessions; verify states, toasts, tray variants, and overlay end-to-end. (Per Saeid's rule: type-check passing ≠ feature working.)

## 10. Scope

**In (v1):**
- Tray app with summonable main window.
- Substrate poller (watch + reconcile + pid liveness + lazy transcript enrichment).
- 3-state model + Ended, with waiting > idle > working priority.
- Tray icon variants + badge counts.
- Windows toasts on transitions; optional sound toggle.
- Roster view + tamagotchi view.
- **Mini-pill always-on-top overlay** (auto-show on attention, corner-parkable, position persisted).
- Autostart with Windows + start-minimized.

**Out (later):**
- recon's session command suite (launch / resume / park / unpark).
- "Jump to that session" — Windows Terminal cannot focus a specific tab via any supported API; best-effort "bring WT window to foreground" is a possible fast-follow.
- History / usage graphs over time.
- Multi-machine / remote aggregation.

## 11. Naming

**Homa** — the Persian mythical bird said to never land, living its whole life in flight. Fitting for a monitor that watches over a flock of always-running agents. (Ties to the Simorgh assistant's bird-of-many-birds motif without colliding with the `simorgh-*` namespace.)

## 12. Open items carried into planning

1. Enumerate the real value set of the session `status` field (only `busy` observed).
2. Confirm transcript JSONL fields for context % / token usage extraction.
3. Decide the reconcile interval and enrichment throttle empirically against real RAM/CPU while a game runs.
4. Choose the tamagotchi art approach (sprite sheets vs CSS/SVG animation).
