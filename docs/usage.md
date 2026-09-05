# Using pomo

## Controls

| Key | Action |
| --- | --- |
| `space` | Start, pause, resume, or begin focus after a break |
| `enter` on Timer | Same as Space |
| `b` | Take a break / skip the current break |
| `r` | Reset the timer; Enter confirms, Esc cancels |
| `s` | Settings |
| `tab` / `1` / `2` | Switch Timer / Calendar |
| `z` | Toggle quiet timer mode |
| `?` | Show help |
| `q` / `ctrl+c` | Save, pause, and quit |

Tabs, timer actions, calendar navigation, day headers, and settings fields
also respond to mouse clicks. Scroll the calendar with the mouse wheel.
Dialogs consume their own keys; `Esc` closes them without applying changes.

**Calendar:** `←` / `→` change weeks, `↑` / `↓` (or `j` / `k`) scroll,
`Home` / `End` jump, `[` / `]` select a day, `Enter` switches between the
week and exact session times, and `t` returns to this week. A narrow terminal
shows daily totals instead of squeezing the grid. Each grid row represents
30 minutes; the line length indicates focus coverage within that row.

**Settings:** `↑` / `↓` or Tab selects a field; `←` / `→` adjusts it.
Type a number to replace a duration. Enter saves, Esc cancels, and `p`
previews the selected sound. Durations accept 1–1440 minutes and the long-break
interval accepts 1–99. New durations apply when the next phase starts.

## Timing and breaks

The defaults are 25 minutes of focus, 5-minute short breaks, and a 15-minute
long break after every fourth **completed** focus block. An early break saves
your actual focus time but does not advance that cycle. Reset keeps both your
recorded history and completed-block count.

Focus has no alarm or forced transition. At the target, the clock quietly
continues as `+MM:SS`. A break stops exactly at its deadline and waits for
Space to start the next focus. Any other key silences an alert while retaining
its normal action; opening the calendar or quitting won't start a new timer.

Break alerts default to **Once**. Choose **Repeat** for a sound every 1.5 seconds
until dismissed, or **Off** for silence. macOS uses system sounds through
`afplay` and one notification banner. Other platforms, or failed sound playback,
use the terminal bell. There is no screen flashing in any alert mode.

Elapsed time uses a monotonic clock, with millisecond precision and a countdown
that rounds up. Pauses aren't logged. A process stall over 5 seconds, detected
computer sleep, or a clock discontinuity pauses the timer at its last observed
position. Press Space to resume. This conservative rule also excludes time when
the app was suspended or could not run; pomo does not infer activity from typing.

## Your data

History and settings from earlier versions are reused automatically. On macOS:

- Sessions: `~/Library/Application Support/pomo/pomo.db`
- Settings: `~/Library/Application Support/pomo/config.json`

If you previously installed the Python version with uv, remove that launcher
with `uv tool uninstall pomo` before installing the Rust package. This prevents
the old command from taking precedence on PATH and does not delete your history.

Linux retains the XDG data/config paths. Run `pomo paths` to see the exact
locations. Use `POMO_HOME` or `--data-dir` to keep everything in one directory:

```bash
pomo --data-dir /tmp/pomo-trial
```

The SQLite migration adds UTC timestamp columns and a timer-state table without
changing original session timestamps. New records are offset-independent;
calendar totals split at local midnight and handle daylight-saving changes.
Overlapping records count once in totals. Legacy timestamps have no timezone,
so their interpretation uses your current local zone; past timezone changes or
ambiguous DST timestamps cannot be reconstructed from those old records.

Timer state and session time checkpoint together once per second and after each
timer action. A crash can lose the uncheckpointed interval. Recording errors
visibly pause the timer and can be retried with Space. A lock prevents two native
timers from recording into the same directory. Close the old Textual app before
switching; it predates this lock.

Settings are validated field by field and saved atomically. A corrupt config is
left intact until you explicitly save new settings.

```bash
pomo stats
pomo stats --week 2026-08-31
pomo export --week 2026-08-31 --output focus.csv
```

Reports read the database without migrating or modifying it. CSV includes each
focus segment clipped to the selected week, UTC timestamps, and seconds with
millisecond precision. Unlike totals, raw CSV retains overlapping legacy rows.
Export without `--output` writes to stdout; file exports never overwrite an
existing file.
