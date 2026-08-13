# Homa

A featherweight Windows system-tray monitor for your Claude Code agents. It watches
`~/.claude/sessions/*.json` and tells you, at a glance, which agents are working,
which have gone idle, and which are waiting on you. Inspired by
[gavraz/recon](https://github.com/gavraz/recon), reimagined Windows-native and
ambient (no tmux, no terminal scraping).

## Why

When you run several Claude Code agents at once, each in its own Windows Terminal
tab, there is no single place to see their state. Homa lives in the tray so you can
step away (even into a fullscreen game) and still know the moment an agent needs you.

**Only working is calm.** Idle and waiting are both attention states, shown
distinctly, because an idle agent is one that finished and is waiting for its next task.

## How it works

- **Substrate:** reads the per-session status files Windows Claude Code already
  writes to `~/.claude/sessions/<pid>.json` (name, cwd, live `status`, timestamps),
  and enriches from the matching transcript in `~/.claude/projects/` (model,
  context %, git branch) only while the main window is open.
- **State model:** `busy`/`shell` -> Working, `idle` -> Idle, dead process -> Ended,
  any waiting/blocked status -> Waiting, and a fresh usage-limit message in the
  transcript -> Limited. Unknown values fail toward attention.
- **Signals:** tray icon color (blue working, amber idle or limited, red waiting),
  tray tooltip counts, Windows toasts on transitions, an optional sound, and an
  always-on-top overlay that stays visible as a permanent dashboard, listing every
  live session by name with a colored dot for its state (purple = limited).

Tray priority is Waiting > Limited > Idle > Working. The tray icon always reflects
the most urgent agent.

## The overlay

The overlay is a resizable always-on-top panel. Drag the top bar to move it,
drag any edge to resize; position and size persist across restarts. The minus
button in the top bar hides it; bring it back with tray > Show overlay.

Single click a row to bring that agent's terminal window to the foreground.
Double click a row to rename it.

## Usage limit auto resume

When Claude Code stops a session with "You've hit your session limit", Homa
turns the row purple and shows the reset time. If the session was actively
working when the limit hit, then the moment the reset time passes Homa focuses
that session's terminal, types `continue`, and presses Enter, so the agent picks
its task back up unattended. A toast reports how many agents were resumed.

- Only mid-task sessions are auto-resumed; sessions idle at the limit stay put.
- Toggle the whole behaviour with tray > Auto-resume: on/off
  (`auto_resume_enabled` in `%APPDATA%\homa\settings.json`).
- "Credit balance is too low" and "Login expired" also show as Limited but are
  never auto-resumed; those need you.
- Limitations: if two sessions share one terminal window as tabs, the nudge
  lands in the active tab; sessions hosted in VS Code's integrated terminal or
  whose terminal was closed get a toast instead of a nudge.

## Requirements

- Windows 11 with the WebView2 runtime (already present on Win11).
- To build from source: Rust (stable, MSVC toolchain) + MSVC C++ Build Tools, and Node 18+.

## Develop

```bash
npm install
npm run tauri dev      # run the app against your real sessions
npm run test           # frontend component tests (vitest)
cd src-tauri && cargo test   # Rust core tests
```

## Build the installer

```bash
npm run tauri build
```

Produces an NSIS installer under `src-tauri/target/release/bundle/nsis/`.
Installing it registers Homa to start at login and launch minimized to the tray.

## Settings

Optional sound is off by default. To enable it, edit
`%APPDATA%\homa\settings.json`:

```json
{ "sound_enabled": true, "sound_on_idle": false, "auto_resume_enabled": true }
```

`sound_enabled` plays a chime on the waiting transition; set `sound_on_idle` to also
chime when an agent goes idle.

## Naming sessions

Each row in the overlay shows a name derived from the session's working folder.
Double click a row to rename it; the name applies to that session only and is
stored under its session id in `%APPDATA%\homa\aliases.json`. Folder-keyed
aliases in the same file act as a default for sessions never renamed
individually. Saving an empty name clears the alias and the row falls back to
the folder default or the folder-derived name.

## Replay mode (deterministic testing)

To exercise the full signal pipeline without live agents:

```bash
node scripts/replay.mjs           # prints a replay dir, cycles busy/idle/waiting
# then set HOMA_SESSIONS_DIR to that dir and run the app
```

`HOMA_SESSIONS_DIR` overrides the sessions directory Homa watches.

## Notes and limits

- **Waiting detection:** so far Claude Code writes `busy` and `idle`; a distinct
  "waiting on input" value has not yet been observed on this machine. Homa maps a
  configurable set of waiting strings and logs every distinct raw status it sees to
  `%APPDATA%\homa\observed-statuses.log`. If a real waiting value shows up there,
  add it to `WAITING_STATUSES` in `src-tauri/src/mapper.rs`.
- **Exclusive-fullscreen games** suppress all overlays and toasts at the OS level.
  The tray icon color and optional sound remain your fallbacks there.
