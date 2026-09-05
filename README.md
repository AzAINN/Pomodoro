# pomo

A quiet Pomodoro timer for your terminal. Built with Rust and Ratatui.

- **Stay in flow.** Focus ends silently and continues into overtime.
- **Keep it minimal.** Thin-line clock, three restrained themes, and a quiet mode.
- **Count real work.** Live totals, a week calendar, and exact session times.
- **Resume safely.** Pauses and detected sleep are excluded; interrupted sessions
  reopen paused. History stays on your machine.

## Install

### From GitHub

With [Rust](https://rustup.rs/) 1.89 or newer and a C compiler installed:

```sh
cargo install --git https://github.com/AzAINN/Pomodoro --locked pomo-tui
pomo
```

The package is named **pomo-tui**; the command is **pomo**. SQLite is bundled,
so there is no separate database server or library to install.

### Download a binary

These installers download and verify the appropriate binary from the latest
GitHub Release. Rust and Python are not required.

**macOS / Linux:**

```sh
curl -fsSL https://raw.githubusercontent.com/AzAINN/Pomodoro/main/install.sh | sh
pomo
```

Installs to `~/.local/bin`. Add it to your PATH if needed:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

**Windows PowerShell:**

```powershell
irm https://raw.githubusercontent.com/AzAINN/Pomodoro/main/install.ps1 | iex
pomo
```

Installs to `%LOCALAPPDATA%\Programs\pomo` and adds it to your user PATH.

You can also download an archive from [Releases](https://github.com/AzAINN/Pomodoro/releases)
and place `pomo` / `pomo.exe` on PATH. macOS and Linux builds cover Intel/AMD
and ARM64; Windows has an x86-64 build. See [installer options](https://github.com/AzAINN/Pomodoro/blob/main/docs/releasing.md#installer-options)
to pin a version or change the installation directory.

### From a checkout

```sh
cargo install --path . --locked
```

## Use

Press **Space** to start. Focus quietly runs into overtime until you press **b**
for a break. Completed breaks wait for you to start the next focus.

| Key | Action |
| --- | --- |
| `space` | Start / pause / resume |
| `b` | Take / skip break |
| `tab` | Timer / Calendar |
| `s` | Settings |
| `z` | Quiet mode |
| `r` | Reset timer, keeping recorded history |
| `?` | All controls |
| `q` | Save, pause, and quit |

Mouse controls are supported. The calendar adapts to smaller terminals.
Break alerts can play once, repeat, or stay silent; the screen never flashes.

```sh
pomo stats
pomo export --week 2026-08-31 --output focus.csv
pomo paths
```

Data is stored locally in SQLite and JSON using platform-specific directories.
Existing history and settings from earlier versions are retained. Try it with
isolated data using `pomo --data-dir /tmp/pomo-trial`.

[Full controls, timing behavior, and data details →](https://github.com/AzAINN/Pomodoro/blob/main/docs/usage.md)

## Development

```sh
cargo run --locked
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
```

See [CONTRIBUTING.md](https://github.com/AzAINN/Pomodoro/blob/main/CONTRIBUTING.md) for the code layout and terminal tests.
[Release instructions](https://github.com/AzAINN/Pomodoro/blob/main/docs/releasing.md) cover GitHub binaries and optional
crates.io publication. There is no Python application or packaging layer;
the small Python scripts are development-only test and packaging tools.

## License

[MIT](LICENSE).
