//! Render deterministic fixtures without opening a terminal or touching real data.
//! cargo run --example preview -- [focus|quiet|calendar|day|settings|ready] [width] [height]
use chrono::{Days, Local, TimeZone};
use pomo::{
    app::{App, Modal, SettingsEditor, View},
    config::Paths,
    engine::Phase,
    store::{Segment, day_bounds},
    ui,
};
use ratatui::{Terminal, backend::TestBackend};

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("focus");
    let width: u16 = args
        .get(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(100);
    let height: u16 = args
        .get(2)
        .and_then(|value| value.parse().ok())
        .unwrap_or(32);
    let dir = tempfile::tempdir()?;
    let now = Local
        .with_ymd_and_hms(2026, 9, 5, 14, 30, 0)
        .single()
        .unwrap();
    let mut app = App::open(Paths::discover(Some(dir.path().into()))?, now)?;
    app.engine.phase = Phase::Focus;
    app.engine.running = true;
    app.engine.target_ms = 1_500_000;
    app.engine.elapsed_ms = 377_000;
    app.engine.completed = 2;
    for day in 0..6 {
        let date = app.week + Days::new(day);
        for (n, hour) in [9, 10, 13, 14].into_iter().enumerate() {
            if (day + n as u64).is_multiple_of(3) {
                continue;
            }
            let start = Local
                .from_local_datetime(&date.and_hms_opt(hour, 0, 0).unwrap())
                .single()
                .unwrap()
                .timestamp_millis();
            app.history.push(Segment {
                id: app.history.len() as i64 + 1,
                kind: "focus".into(),
                start_ms: start,
                end_ms: start + 1_500_000 + n as i64 * 60_000,
            });
        }
    }
    app.current_week_history = app.history.clone();
    let (today_start, today_end) = day_bounds(now.date_naive());
    app.today_history = app
        .history
        .iter()
        .filter(|row| row.start_ms >= today_start && row.end_ms <= today_end)
        .cloned()
        .collect();
    match mode {
        "quiet" => app.quiet = true,
        "calendar" => {
            app.view = View::Calendar;
            app.scroll = 16;
        }
        "day" => {
            app.view = View::Calendar;
            app.day_view = true;
        }
        "settings" => app.modal = Some(Modal::Settings(SettingsEditor::new(&app.settings))),
        "ready" => {
            app.engine.phase = Phase::Ready;
            app.engine.running = false;
            app.engine.elapsed_ms = app.engine.target_ms;
        }
        "focus" => {}
        _ => anyhow::bail!("Use focus, quiet, calendar, day, settings, or ready"),
    }
    let mut terminal = Terminal::new(TestBackend::new(width.max(1), height.max(1)))?;
    terminal.draw(|frame| ui::draw(frame, &mut app))?;
    for row in terminal
        .backend()
        .buffer()
        .content()
        .chunks(usize::from(width.max(1)))
    {
        println!(
            "{}",
            row.iter()
                .map(|cell| cell.symbol())
                .collect::<String>()
                .trim_end()
        );
    }
    Ok(())
}
