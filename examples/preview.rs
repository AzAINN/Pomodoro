//! Render deterministic fixtures without opening a terminal or touching real data.
//! cargo run --example preview -- [focus|quiet|calendar|day|settings|ready] [width] [height] [--svg]
use chrono::{Days, Local, TimeZone};
use pomo::{
    app::{App, Modal, SettingsEditor, View},
    config::Paths,
    engine::Phase,
    store::{Segment, day_bounds},
    ui,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    style::{Color, Modifier},
};
use std::fmt::Write;

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
    app.settings.theme = "Sage".into();
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
    if args.get(3).is_some_and(|arg| arg == "--svg") {
        println!("{}", svg(terminal.backend().buffer()));
        return Ok(());
    }
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

/// Export the actual Ratatui cells, using a quiet dark terminal palette.
fn svg(buffer: &Buffer) -> String {
    let cell_width = 10;
    let cell_height = 20;
    let padding = 24;
    let width = u32::from(buffer.area.width) * cell_width + padding * 2;
    let height = u32::from(buffer.area.height) * cell_height + padding * 2;
    let mut result = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-labelledby="title desc">
<title id="title">pomo — a quiet terminal Pomodoro timer</title>
<desc id="desc">Rendered directly from the Ratatui interface with sample data and the Sage theme.</desc>
<rect width="{width}" height="{height}" rx="12" fill="#151816"/>
<g font-family="Menlo, Consolas, 'Liberation Mono', monospace" font-size="16" xml:space="preserve">
"##
    );
    for (index, cell) in buffer.content().iter().enumerate() {
        let x = padding + (index as u32 % u32::from(buffer.area.width)) * cell_width;
        let y = padding + (index as u32 / u32::from(buffer.area.width)) * cell_height;
        let mut fg = color(cell.fg, "#d6ddd7");
        let mut bg = color(cell.bg, "#151816");
        if cell.modifier.contains(Modifier::REVERSED) {
            std::mem::swap(&mut fg, &mut bg);
        }
        if bg != "#151816" {
            writeln!(
                result,
                r#"<rect x="{x}" y="{y}" width="{cell_width}" height="{cell_height}" fill="{bg}"/>"#
            )
            .unwrap();
        }
        if !cell.symbol().trim().is_empty() {
            let symbol = cell
                .symbol()
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            let weight = if cell.modifier.contains(Modifier::BOLD) {
                "bold"
            } else {
                "normal"
            };
            writeln!(
                result,
                r#"<text x="{x}" y="{}" fill="{fg}" font-weight="{weight}">{symbol}</text>"#,
                y + 16
            )
            .unwrap();
        }
    }
    result.push_str("</g>\n</svg>");
    result
}

fn color(color: Color, reset: &str) -> String {
    let index = match color {
        Color::Reset => return reset.into(),
        Color::Rgb(r, g, b) => return format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Indexed(index) => index,
        Color::Black => 0,
        Color::Red => 1,
        Color::Green => 2,
        Color::Yellow => 3,
        Color::Blue => 4,
        Color::Magenta => 5,
        Color::Cyan => 6,
        Color::Gray => 7,
        Color::DarkGray => 8,
        Color::LightRed => 9,
        Color::LightGreen => 10,
        Color::LightYellow => 11,
        Color::LightBlue => 12,
        Color::LightMagenta => 13,
        Color::LightCyan => 14,
        Color::White => 15,
    };
    const PALETTE: [&str; 16] = [
        "#151816", "#d08770", "#a1b89d", "#d8b785", "#81a1c1", "#b48ead", "#88c0d0", "#d6ddd7",
        "#7c857e", "#e49b85", "#b4cbb0", "#ebca98", "#94b4d4", "#c7a1c0", "#9bd3e3", "#edf2ee",
    ];
    match index {
        0..=15 => PALETTE[usize::from(index)].into(),
        16..=231 => {
            let index = index - 16;
            let levels = [0, 95, 135, 175, 215, 255];
            format!(
                "#{:02x}{:02x}{:02x}",
                levels[usize::from(index / 36)],
                levels[usize::from(index / 6 % 6)],
                levels[usize::from(index % 6)]
            )
        }
        _ => {
            let level = 8 + (index - 232) * 10;
            format!("#{level:02x}{level:02x}{level:02x}")
        }
    }
}
