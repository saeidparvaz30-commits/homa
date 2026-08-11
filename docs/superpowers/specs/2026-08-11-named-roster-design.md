# Homa v2: Named Session Roster

**Date:** 2026-08-11
**Status:** Approved design, awaiting implementation plan
**Supersedes parts of:** `2026-07-24-homa-design.md` (overlay behaviour, status mapping)

## Problem

Homa v1 shipped and has been running for roughly two weeks. Two things make it
unhelpful in daily use.

**Names are meaningless.** Claude writes its own `name` into each session file,
auto generated from the working directory plus a counter (`agent-simorgh-61`).
Homa displays that name verbatim in the roster (`RosterView.tsx:23`). Looking at
it does not tell Saeid which session is which piece of work.

**The surface he glances at carries no names at all.** The always on top overlay
is a count pill: "2 waiting" (`OverlayPill.tsx:31`). It says how many things want
attention but not which ones, so every alert costs a context switch into the main
window to find out what fired.

There is also a latent irritant. `mapper.rs:21` routes any unrecognised status to
`Idle` on the principle of failing toward attention. The observed status log now
contains a `shell` value that v1 never saw. Every time a session shells out it is
reported as idle, which in Homa's vocabulary means "finished, feed it a task".
That is a false alarm on a recurring event.

## Goals

1. Every session shows a name Saeid chose.
2. A name is typed once and then applies forever, without retyping each session.
3. The named list is visible at a glance without clicking anything.
4. Fewer false attention signals.

## Non goals

- Writing anything into `~/.claude`. Homa stays strictly read only against
  Claude's own files. Names live in Homa's own store.
- Reworking the main window or the Tamagotchi view. They inherit names for free
  and are otherwise untouched.
- Changing the toast, sound, or tray colour logic.

## Design

### 1. Alias store

New module `src-tauri/src/alias.rs`, backed by `%APPDATA%\homa\aliases.json`.

```json
{
  "c:\\users\\saeid\\desktop\\migration site": "migration site",
  "c:\\users\\saeid\\desktop\\claude projects\\homa": "homa"
}
```

**Scope: the working directory, not the session.** A session ID is created fresh
on every run, so a session scoped name would have to be retyped daily. A cwd
scoped name is typed once and every future session started in that folder
inherits it. This is the decision that makes renaming worth building at all.

**Key normalisation.** Lowercase, backslashes normalised, trailing separator
stripped. Path comparison on Windows must be case insensitive, and the same
folder can arrive spelled differently depending on how the session was started.

**Collisions.** Two live sessions in one folder both resolve to the same alias.
They are disambiguated for display by appending ` #1`, ` #2`, ordered by
`started_at` ascending, so the numbering is stable while both live rather than
flipping between polls.

**Resolution order**, applied in the enrichment step so every consumer (overlay,
main window, toast text) sees the same name:

1. alias for the session's cwd, plus collision suffix if needed
2. otherwise Claude's `session.name`
3. otherwise the last path segment of cwd

**Writes.** Written atomically: serialise to a temporary file in the same
directory, then rename over the target. The file is small and rewritten whole on
each change. A malformed or missing file loads as an empty map rather than
failing, matching how `settings.rs` already degrades.

**Commands** exposed to the frontend:

- `set_alias(cwd: String, name: String)` — trims input; an empty or whitespace
  only name removes the entry rather than storing a blank
- `get_aliases() -> Map<String, String>`

### 2. Overlay becomes the roster

`OverlayPill.tsx` is replaced by `OverlayRoster.tsx`.

**Always visible.** `overlay::drive` no longer shows and hides based on attention
state. The window is shown once at startup and stays up. Attention is signalled
by dot colour, plus the existing toast, sound, and tray colour, all unchanged.
The list is a dashboard; the alerts remain alerts.

**Layout.** One row per live session: a status dot then the name. Rows sorted by
status priority descending (waiting, idle, working, ended), then by name for a
stable order inside a group. Colours match the existing vocabulary: waiting red,
idle amber, working blue, ended grey.

**Data.** The overlay window currently receives only the `tray-summary` count
event. The full `AgentState` list must be emitted to it as well. The main window
already consumes this via `useAgents`, so the poller emits to both windows.

**Sizing.** Window height is set from the row count on each update, clamped to a
maximum after which the list scrolls, so a burst of sessions cannot grow the
overlay to fill the screen. Width is fixed.

**Position.** The dragged position is persisted to settings and restored at
startup. An always visible window that resets its position on every launch would
be worse than the auto hiding pill it replaces.

**Focus.** Shown without stealing focus, and it must not appear in the taskbar or
in alt tab. It has to be clickable for renaming while never interrupting a game
or a terminal.

**Ended sessions** remain listed as a grey row for roughly 10 seconds after the
process dies, then disappear. Long enough to notice that something exited,
short enough that the list does not accumulate corpses.

**Empty state.** With no live sessions the overlay collapses to a single muted
row reading "no sessions". It does not hide itself. Hiding on empty would mean
the window vanishes and reappears throughout the day, which is the behaviour this
design is removing, and it would leave nothing to drag when repositioning.

### 3. Rename in place

Double click a row. The name becomes a text input seeded with the current value
and selected. Enter commits, Escape cancels, blur commits. An empty value clears
the alias and the row falls back to Claude's name on the next poll.

Committing calls `set_alias` with the row's cwd. Because the store is cwd keyed,
renaming one row immediately renames every other live session in the same folder.
This is intended, and the collision suffixes make the result readable.

### 4. Status mapping fix

`shell` maps to `Working`.

The reasoning: `shell` appears in the observed status log interleaved with `busy`
and `idle`, and it corresponds to the session running a shell command, which is
work in progress rather than a finished turn. Mapping it to `Working` removes a
recurring false "needs attention" signal.

The unknown status fallback stays as it is. Failing toward attention is still the
right default for a value nobody has seen yet; the fix here is that `shell` is no
longer unknown.

`probe.rs` keeps logging distinct raw statuses so the next unseen value is caught
the same way this one was.

## Testing

**Rust.** Alias key normalisation across case and separator variants. Name
resolution precedence including the fallback chain. Collision suffixing and its
stability across repeated resolution. Atomic write followed by read back.
Corrupt and absent alias files loading as empty. `map_status("shell", true)`
returning `Working`, alongside the existing mapper tests.

**Frontend.** Roster rows render in priority order. Double click enters edit
mode. Enter commits and invokes the command with the row's cwd. Escape cancels
without invoking. An empty commit clears rather than storing blank.

**Manual.** The part that cannot be asserted: overlay stays up across status
changes, does not steal focus, does not appear in alt tab, sizes correctly from
one row to many, and restores its position after a restart. This is what v1
never got, and it is where the real verdict on v2 lives.

## Risks

**Always visible costs screen space permanently.** Mitigated by fixed narrow
width, auto height, and a remembered position. If it turns out to be intrusive
while gaming the fallback is the collapse to a thin bar variant, which was
considered and set aside; nothing in this design blocks adding it later.

**The `shell` reading could be wrong.** If `shell` turns out to mean the session
is blocked on a shell command needing input, `Working` would hide a real
attention state. The observed status log will show it if the mapping feels wrong
in use, and the change is one line in `WAITING_STATUSES`.

**Renaming affects sibling sessions.** By design, but it will feel surprising the
first time two sessions in one repo rename together. The `#1` and `#2` suffixes
are what make it legible.
