# pomo

Minimal Pomodoro timer for your terminal, with a Google Calendar-style week
view of your logged focus time.

- **Silent focus.** When focus time runs out, the clock rolls into overtime
  (`+02:31`) and keeps recording. Take your break when you are ready.
- **Persistent breaks.** When a break ends, pomo rings every two seconds until
  you press a key, which immediately starts the next focus session.
- **Honest calendar.** The week grid shows real focus time. Paused time and
  breaks are not counted.

## Install and run

pomo requires Python 3.13 or newer and [uv](https://docs.astral.sh/uv/).

```bash
uv sync
uv run pomo
```

To install the `pomo` command globally from this checkout:

```bash
uv tool install .
pomo
```

## Keys

| Key | Action |
| --- | --- |
| `space` / main button | Start, pause, resume, or stop ringing |
| `b` | Finish focus and start the next break |
| `r` | Reset to idle |
| `s` | Edit durations, long-break interval, and sound |
| `tab` | Switch between Timer and Calendar |
| `left` / `right` | Show the previous or next calendar week |
| `t` | Return the calendar to the current week |
| `q` | Quit cleanly |

A long break is selected after every fourth completed focus session by
default. All durations and the interval are configurable from the settings
dialog. Changes affect the next session, not one already in progress.

## Data and sound

Data lives in platform-specific user directories managed by `platformdirs`.
On macOS, the default paths are:

- Sessions: `~/Library/Application Support/pomo/pomo.db`
- Settings: `~/Library/Application Support/pomo/config.json`

Linux uses the corresponding XDG data and config directories. Sessions are
stored in SQLite; settings are stored as JSON.

On macOS, break alerts use system sounds from `/System/Library/Sounds` via
`afplay` and post one notification banner. Other environments fall back to the
terminal bell. Sound names such as `Ping`, `Glass`, and `Submarine` can be set
in the settings dialog.

## Development

Run the test suite with:

```bash
uv run pytest -v
```
