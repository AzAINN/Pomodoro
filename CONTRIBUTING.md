# Contributing

pomo is a Rust application with a Crossterm/Ratatui interface and a local SQLite
database. Keep the default screen quiet and new features easy to ignore.

## Development

Install Rust 1.89 or newer, clone the repository, and run:

```sh
cargo run --locked
```

Use a disposable data directory while developing:

```sh
cargo run --locked -- --data-dir /tmp/pomo-dev
```

Before opening a pull request:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --locked
python3 scripts/test_distribution.py
python3 scripts/smoke_tui.py
```

The Python scripts are development-only tools using the standard library, with
no virtual environment or packages to install. The PTY test runs on macOS/Linux;
Windows uses `pwsh -File scripts/test_installer.ps1` for its installer. CI runs
Rust tests on all three operating systems and verifies the minimum Rust version.

## Code layout

| File | Responsibility |
| --- | --- |
| `src/main.rs` | CLI arguments, reports, and terminal event loop |
| `src/engine.rs` | Deterministic timer state machine |
| `src/store.rs` | SQLite checkpoints and time aggregation |
| `src/app.rs` | Input handling and application state |
| `src/ui.rs` | Responsive screens |
| `src/config.rs` | Settings and platform paths |
| `src/sound.rs` | Nonblocking break alerts |

`src/lib.rs` shares these modules with the binary, tests, and rendering example;
it is an internal implementation, not a stable library API.

Use the existing deterministic tests for timer changes. Keep timestamps and
elapsed time separate, exclude paused time, and preserve existing databases.

Render sample screens without opening a terminal or writing real focus data:

```sh
cargo run --example preview -- focus
cargo run --example preview -- calendar 100 32
cargo run --example preview -- settings 40 16
```

Regenerate the README visual directly from the Ratatui screen buffer:

```sh
cargo run --quiet --locked --example preview -- focus 100 30 --svg > docs/assets/pomo.svg
```

The fixture uses sample history and the Sage theme. SVG export needs no extra tools.

See [releasing](docs/releasing.md) for package and GitHub Release maintenance.
