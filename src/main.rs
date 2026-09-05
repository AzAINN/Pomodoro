use std::{
    env,
    fs::OpenOptions,
    io::{self, IsTerminal, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use chrono::{Days, Local, NaiveDate};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, MouseButton,
        MouseEventKind,
    },
    execute,
};
use pomo::{
    app::{App, View},
    config::Paths,
    store::{self, Store, coverage, day_bounds, duration_label},
    ui,
};

const HELP: &str = "pomo — quiet focus, honest time

Usage: pomo [--data-dir DIR] [COMMAND]

Commands:
  (none)             Open the timer
  stats              Show daily focus totals for this week
  export             Write this week's focus segments as CSV to stdout
  paths              Show data and settings paths

Options:
  --week YYYY-MM-DD  Week containing this date (stats / export)
  --output FILE     Save CSV to a new file (export only; never overwrites)
  --data-dir DIR    Keep all data in DIR (or set POMO_HOME)
  -h, --help        Show help
  -V, --version     Show version

In the timer: Space start/pause · b break · Tab calendar · s settings
             z quiet mode · ? all keys · q save, pause & quit
";

struct Args {
    data: Option<PathBuf>,
    command: String,
    week: Option<NaiveDate>,
    output: Option<PathBuf>,
}

fn main() -> Result<()> {
    let mut arguments = env::args_os().skip(1).peekable();
    let mut args = Args {
        data: None,
        command: String::new(),
        week: None,
        output: None,
    };
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("-h" | "--help") => {
                print!("{HELP}");
                return Ok(());
            }
            Some("-V" | "--version") => {
                println!("pomo {} (Ratatui)", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            Some("--data-dir") => {
                args.data = Some(
                    arguments
                        .next()
                        .context("--data-dir needs a directory")?
                        .into(),
                )
            }
            Some("--week") => {
                let value = arguments.next().context("--week needs YYYY-MM-DD")?;
                let date = NaiveDate::parse_from_str(&value.to_string_lossy(), "%Y-%m-%d")
                    .context("Invalid --week date; use YYYY-MM-DD")?;
                use chrono::Datelike;
                if !(1900..=9998).contains(&date.year()) {
                    bail!("Choose a date from 1900 through 9998");
                }
                args.week = Some(store::monday(date));
            }
            Some("--output") => {
                args.output = Some(
                    arguments
                        .next()
                        .context("--output needs a file path")?
                        .into(),
                )
            }
            Some(command @ ("stats" | "export" | "paths")) if args.command.is_empty() => {
                args.command = command.into()
            }
            _ => bail!(
                "Unknown argument: {}. Run pomo --help.",
                argument.to_string_lossy()
            ),
        }
    }
    if args.output.is_some() && args.command != "export" {
        bail!("--output is only available with export");
    }
    if args.week.is_some() && !matches!(args.command.as_str(), "stats" | "export") {
        bail!("--week is only available with stats or export");
    }
    let paths = Paths::discover(args.data)?;
    if args.command == "paths" {
        println!(
            "Sessions: {}\nSettings: {}",
            paths.database().display(),
            paths.config.display()
        );
        return Ok(());
    }
    if !args.command.is_empty() {
        return report(&paths, &args.command, args.week, args.output);
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!("The timer needs an interactive terminal. Use pomo stats or pomo export in scripts.");
    }
    let mut app = App::open(paths, Local::now())?;
    let stop = Arc::new(AtomicBool::new(false));
    #[cfg(unix)]
    for signal in [
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
        signal_hook::consts::SIGINT,
    ] {
        signal_hook::flag::register(signal, Arc::clone(&stop))?;
    }
    let mut terminal = ratatui::try_init()?;
    let guard = TerminalGuard;
    let result = (|| {
        execute!(io::stdout(), EnableMouseCapture)?;
        run(&mut terminal, &mut app, &stop)
    })();
    let saved = app.shutdown();
    drop(guard);
    result.and(saved)
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App, stop: &AtomicBool) -> Result<()> {
    let start = Instant::now();
    let mut last_ms = 0;
    let mut last_wall = app.now.timestamp_millis();
    let mut checkpoint_ms = 0;
    loop {
        let elapsed = start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        let now = Local::now();
        let wall = now.timestamp_millis();
        app.advance(
            elapsed.saturating_sub(last_ms),
            wall.saturating_sub(last_wall),
            now,
        );
        last_ms = elapsed;
        last_wall = wall;
        if elapsed.saturating_sub(checkpoint_ms) >= 1_000 {
            if app.engine.running || app.active.is_some() {
                app.persist();
            }
            checkpoint_ms = elapsed;
        }
        if app.quit || stop.load(Ordering::Relaxed) {
            break;
        }
        app.alerts.update(
            app.engine.phase == pomo::engine::Phase::Ready,
            elapsed,
            &app.settings,
        );
        terminal.draw(|frame| ui::draw(frame, app))?;
        if event::poll(Duration::from_millis(250))? {
            let event = event::read()?;
            // Account for time spent waiting before an action closes a segment.
            let elapsed = start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
            let now = Local::now();
            let wall = now.timestamp_millis();
            app.advance(
                elapsed.saturating_sub(last_ms),
                wall.saturating_sub(last_wall),
                now,
            );
            last_ms = elapsed;
            last_wall = wall;
            match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => app.key(key),
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        let point = ratatui::layout::Position::new(mouse.column, mouse.row);
                        if let Some((_, action)) =
                            app.hits.iter().find(|(rect, _)| rect.contains(point))
                        {
                            app.action(*action);
                        }
                    }
                    MouseEventKind::ScrollUp
                        if app.view == View::Calendar && app.modal.is_none() =>
                    {
                        app.scroll = app.scroll.saturating_sub(2)
                    }
                    MouseEventKind::ScrollDown
                        if app.view == View::Calendar && app.modal.is_none() =>
                    {
                        app.scroll = app.scroll.saturating_add(2).min(app.max_scroll())
                    }
                    _ => {}
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }
    Ok(())
}

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableMouseCapture);
        ratatui::restore();
    }
}

fn report(
    paths: &Paths,
    command: &str,
    week: Option<NaiveDate>,
    output: Option<PathBuf>,
) -> Result<()> {
    let week = week.unwrap_or_else(|| store::monday(Local::now().date_naive()));
    let start = day_bounds(week).0;
    let end = day_bounds(week + Days::new(7)).0;
    let segments = if paths.database().exists() {
        Store::read_only(&paths.database())?.range(start, end)?
    } else {
        Vec::new()
    };
    if command == "stats" {
        println!("Focus · week of {week}");
        for i in 0..7 {
            let date = week + Days::new(i);
            let (start, end) = day_bounds(date);
            println!(
                "{}  {:>9}",
                date.format("%a %d %b"),
                duration_label(coverage(&segments, start, end))
            );
        }
        println!(
            "Week total  {}",
            duration_label(coverage(&segments, start, end))
        );
    } else {
        let mut writer: Box<dyn Write> = match &output {
            Some(path) => Box::new(
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .with_context(|| {
                        format!(
                            "Could not create {}; export never overwrites files",
                            path.display()
                        )
                    })?,
            ),
            None => Box::new(io::stdout().lock()),
        };
        writeln!(writer, "id,kind,started_at,ended_at,duration_seconds")?;
        for segment in segments {
            let start_ms = segment.start_ms.max(start);
            let end_ms = segment.end_ms.min(end);
            let start = chrono::DateTime::from_timestamp_millis(start_ms)
                .context("Invalid start timestamp")?;
            let end =
                chrono::DateTime::from_timestamp_millis(end_ms).context("Invalid end timestamp")?;
            writeln!(
                writer,
                "{},focus,{},{},{:.3}",
                segment.id,
                start.to_rfc3339(),
                end.to_rfc3339(),
                (end_ms - start_ms) as f64 / 1000.0
            )?;
        }
        writer.flush()?;
        if let Some(path) = output {
            eprintln!("Exported focus segments to {}", path.display());
        }
    }
    Ok(())
}
