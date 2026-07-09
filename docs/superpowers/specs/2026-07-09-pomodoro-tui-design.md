# Pomo — Pomodoro TUI Design

**Date:** 2026-07-09
**Status:** Approved for planning

## Summary

`pomo` is a minimalistic Pomodoro timer that runs in the terminal, inspired by
pomofocus.io for behavior and by the bagels repo for TUI look and feel. It has
two tabs: a **Timer** tab with one big button, and a **Calendar** tab that shows
logged working hours as a Google-Calendar-style week grid. Breaks ring loudly
until dismissed; focus sessions never make noise and silently roll into
overtime.

## Goals

- Launch with `pomo`; timer is immediately usable (Space or click the button).
- Classic pomodoro cycle: focus / short break / long break (every Nth), all
  four parameters user-configurable in-app.
- Focus end is **silent**: the timer rolls into overtime and keeps recording
  until the user starts the break.
- Break end is **loud**: repeating sound until any keypress, which immediately
  starts the next focus session.
- Calendar week view of focus time, formatted like Google/Notion calendar.
- Minimal: no task list, no accounts, no sync.

## Non-Goals

- Task management (pomofocus's task list).
- Cross-device sync or export.
- Windows/Linux sound polish (terminal bell fallback only).

## Tech Stack

- **Python 3.13 + Textual** (same as bagels): TCSS stylesheets, theme
  variables, `Digits` widget for the large timer, `TabbedContent` for tabs,
  `set_interval` for ticking, mouse-clickable `Button`.
- **uv** project layout (like bagels), console script `pomo`.
- Dependencies: `textual`, `platformdirs`. Storage via stdlib `sqlite3`,
  config via stdlib `json`. Tests: `pytest`, `pytest-asyncio` (dev only).

## Architecture

```
src/pomo/
  app.py          # Textual App: tabs, global keybindings, theme switching
  engine.py       # Pure timer state machine (no Textual, injected clock)
  store.py        # SQLite session log + calendar aggregation queries
  config.py       # Load/save settings JSON (platformdirs paths)
  sound.py        # Ring loop: afplay on macOS, terminal bell fallback
  screens/
    timer_tab.py    # Timer UI (Digits, mode pills, dots, button)
    calendar_tab.py # Week-grid renderer
    settings.py     # Settings modal
  styles/*.tcss   # Stylesheets, bagels-style
tests/
```

### Component boundaries

- **engine.py** — the state machine. Pure Python, takes a `now()` callable;
  emits events (`focus_started`, `segment_closed`, `break_finished_ringing`,
  …) via callbacks. Knows nothing about Textual, sound, or SQLite. Fully
  unit-testable.
- **store.py** — owns the DB. API: `open_segment(kind) -> id`,
  `heartbeat(id)`, `close_segment(id)`, `week_segments(monday) -> [Segment]`,
  `totals(...)`. Calendar aggregation (bucketing segments into hour rows) is a
  pure function here so it's testable without a terminal.
- **sound.py** — `start_ringing()` / `stop_ringing()`. On macOS loops
  `afplay /System/Library/Sounds/<sound>.aiff` in a background task every ~2s
  and posts one banner notification via `osascript`; elsewhere prints the
  terminal bell on the same loop.
- **app.py / screens/** — thin UI layer: renders engine state, forwards key
  presses to the engine, calls sound on engine events.

## Timer State Machine

States: `IDLE → FOCUS → (OVERTIME) → BREAK → RINGING → FOCUS → …`

| Event | Transition | Side effects |
|---|---|---|
| Start (Space/button) from IDLE | → FOCUS | open focus segment |
| Focus countdown hits 0:00 | → OVERTIME | **nothing audible**; display flips to `+MM:SS` counting up, subtle color change; segment stays open |
| `b` / button during FOCUS/OVERTIME | → BREAK | close focus segment; increment pomodoro count; pick short vs long break (every Nth); open break segment |
| Break countdown hits 0:00 | → RINGING | close break segment; start ring loop + one banner notification; screen prompts "press any key" |
| Any key/click during RINGING | → FOCUS | stop ringing; open new focus segment |
| Pause (Space/button) in FOCUS/OVERTIME/BREAK | freeze clock | close current segment (paused time is never logged) |
| Resume | unfreeze | open a new segment of the same kind |
| `r` reset from any state | → IDLE | close any open segment; stop ringing; pomodoro dot progress unchanged |
| Quit | exit | close any open segment; stop ringing |

Overtime is **not** a separate state kind — it's FOCUS past its target; the
whole span logs as focus.

## Timer Tab UI

```
        ┌  FOCUS  ·  short break  ·  long break ┐   ← mode pills (active highlighted)

                    2 4 : 5 9                        ← textual Digits, huge, centered
                     ● ● ○ ○                         ← dots: pomodoros until long break

                   ┌───────────┐
                   │   PAUSE   │                     ← one button; Space is equivalent
                   └───────────┘
  space start/pause  b break  r reset  s settings  tab calendar  q quit
```

- Accent/background tint shifts by mode, pomofocus-style: warm red for focus,
  teal for short break, blue for long break. Overtime dims/shifts the focus
  color slightly.
- During RINGING the screen flashes (alternating style every tick) with the
  prompt "Break over — press any key to focus".
- Button label follows state: START / PAUSE / RESUME / STOP RING.

## Calendar Tab UI

Google-Calendar-style week grid:

```
        Mon 7   Tue 8   Wed 9   Thu 10  Fri 11  Sat 12  Sun 13
  9am  │       │██████│       │██████│       │       │       │
 10am  │██████│██████│       │██████│██████│       │       │
 11am  │██████│       │██████│       │██████│       │       │
 12pm  │       │       │██████│       │       │       │       │
  1pm  │██████│██████│       │██████│       │       │       │

  Week total: 14h 30m        Today: 3h 15m
```

- 7 day columns × hour rows; **focus** segments drawn as filled blocks
  proportional to coverage of each hour cell (partial hours render partial
  fill using block characters). Breaks are not drawn.
- Hour range auto-fits to the week's data (min start → max end, padded one
  hour, default 9am–6pm when the week is empty) so it fills the terminal.
- Keys: `←/→` previous/next week, `t` jump to current week.
- Footer: week total + today total of focus time.
- Today's column header is highlighted.

## Data & Settings

**DB** — `~/.local/share/pomo/pomo.db` (via platformdirs), one table:

```sql
CREATE TABLE sessions (
  id INTEGER PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('focus','short_break','long_break')),
  started_at TEXT NOT NULL,   -- ISO 8601, local time
  ended_at TEXT               -- NULL while segment is open
);
```

- One row per **continuous** segment. Opened on start (row inserted
  immediately), `ended_at` heartbeat-updated every 30s while running, finalized
  on close. On launch, any row left with a stale `ended_at` (crash) is kept
  as-is — at most ~30s of time is lost.
- Pause closes a segment; resume opens a new one → calendar shows only real
  work time.

**Config** — `~/.config/pomo/config.json`:

```json
{
  "focus_minutes": 25,
  "short_break_minutes": 5,
  "long_break_minutes": 15,
  "long_break_interval": 4,
  "sound": "Ping"
}
```

Edited via the in-app settings modal (`s`): four duration/interval fields plus
sound name; validated (positive integers, interval ≥ 1); saved on confirm.
Changes apply to the *next* session, never the running one.

## Sound / Notification

- **macOS** (primary target): background loop runs
  `afplay /System/Library/Sounds/{sound}.aiff` every ~2s until dismissed;
  one `osascript -e 'display notification …'` banner fires when ringing
  starts.
- **Fallback** (no afplay): terminal bell `\a` on the same loop.
- Ringing only ever happens at **break** end. Focus end is always silent.

## Error Handling

- Missing/corrupt config → recreate with defaults, keep going.
- DB errors on write → surface a notification toast in-app, timer keeps
  running (recording degrades, timing doesn't).
- `afplay`/`osascript` failures → silently fall back to terminal bell.
- Terminal too narrow for the week grid → calendar shows a "widen terminal"
  message instead of a broken layout (min width ~60 cols).

## Testing

- **engine.py**: pytest with a fake clock — full transition table above,
  overtime rollover, pause/resume segment semantics, long-break-every-Nth.
- **store.py**: tmp SQLite — segment lifecycle, heartbeat, week queries,
  hour-bucket aggregation (pure function).
- **calendar rendering**: pure segment→grid function tested on fixed fixtures
  (partial hours, overnight segments clamped to day, empty week).
- **TUI**: Textual Pilot smoke test — app boots, Space starts timer, tab
  switches, `q` quits cleanly.
