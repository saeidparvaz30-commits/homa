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
- **State model:** `busy` -> Working, `idle` -> Idle, dead process -> Ended, and any
  waiting/blocked status -> Waiting. Unknown values fail toward attention.
- **Signals:** tray icon color (blue working, amber idle, red waiting), tray tooltip
  counts, Windows toasts on transitions, an optional sound, and an always-on-top
  overlay that stays visible as a permanent dashboard, listing every live session
  by name with a colored dot for its state.

Tray priority is Waiting > Idle > Working. The tray icon always reflects the most
urgent agent.

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
{ "sound_enabled": true, "sound_on_idle": false }
```

`sound_enabled` plays a chime on the waiting transition; set `sound_on_idle` to also
chime when an agent goes idle.

## Naming sessions

Each row in the overlay shows a name derived from the session's working folder.
Double click a row to rename it. Names are per folder, so every session started
from that folder picks up the same name until it is changed again. Press Enter
to save; the name is written to `%APPDATA%\homa\aliases.json`. Saving an empty
name clears the alias and the row falls back to the folder-derived name.

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
