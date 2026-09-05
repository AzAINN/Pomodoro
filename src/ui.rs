use chrono::{DateTime, Days, Local, Utc};
use ratatui::{
    Frame,
    layout::{Alignment, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, LineGauge, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    app::{Action, App, Modal, View},
    engine::Phase,
    store::{self, Segment, coverage, day_bounds, duration_label},
};

#[derive(Clone, Copy)]
struct Palette {
    accent: Color,
    muted: Color,
    faint: Color,
}

impl Palette {
    fn for_app(app: &App) -> Self {
        Self {
            accent: match app.settings.theme.as_str() {
                "Sage" => Color::Rgb(161, 184, 157),
                "Amber" => Color::Rgb(216, 183, 133),
                _ => Color::Reset,
            },
            muted: Color::DarkGray,
            faint: Color::DarkGray,
        }
    }
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    app.hits.clear();
    let area = frame.area();
    let palette = Palette::for_app(app);
    if area.width < 24 || area.height < 8 {
        frame.render_widget(
            Paragraph::new(format!(
                "pomo  {}\nSpace start/pause\nq quit · resize to 24×8",
                app.engine.display(&app.settings)
            )),
            area,
        );
        return;
    }
    let margin = if area.width >= 70 { 3 } else { 1 };
    let inner = area.inner(Margin::new(margin, 0));
    if app.quiet && app.view == View::Timer {
        timer(frame, app, inner, palette);
        if app.engine.phase != Phase::Ready && app.notice.is_none() {
            line(
                frame,
                row(inner, inner.height - 1),
                "z show controls",
                palette.muted,
                Alignment::Center,
            );
        }
    } else {
        header(frame, app, row(inner, 0), palette);
        let footer_y = inner.height.saturating_sub(1);
        let content = Rect::new(
            inner.x,
            inner.y + 2,
            inner.width,
            inner.height.saturating_sub(5),
        );
        match app.view {
            View::Timer => timer(frame, app, content, palette),
            View::Calendar => calendar(frame, app, content, palette),
        }
        let footer = match app.view {
            _ if inner.width < 48 => "space toggle  ?  q quit",
            View::Timer if inner.width >= 76 => {
                "space start/pause   b break   tab calendar   s settings   z quiet   ? help   q quit"
            }
            View::Timer => "space toggle · b break · tab view · ? help · q quit",
            View::Calendar if inner.width >= 76 => {
                "← → week   ↑ ↓ scroll   [ ] day   enter details   t today   tab timer   ? help   q quit"
            }
            View::Calendar => "← → week · ↑ ↓ scroll · enter details · ? help · q quit",
        };
        line(
            frame,
            row(inner, footer_y),
            footer,
            palette.muted,
            Alignment::Center,
        );
        if app.view == View::Timer && inner.height >= 20 {
            let text = if app.history_error.is_some() {
                "Focus totals unavailable".into()
            } else {
                let (start, end) = day_bounds(app.now.date_naive());
                let today = coverage(&app.live_history(true), start, end);
                // Timer totals always use the current week, independent of calendar navigation.
                let week = store::monday(app.now.date_naive());
                let week_total = coverage(
                    &app.live_current_week(),
                    day_bounds(week).0,
                    day_bounds(week + Days::new(7)).0,
                );
                format!(
                    "today  {}     ·     this week  {}",
                    duration_label(today),
                    duration_label(week_total)
                )
            };
            line(
                frame,
                row(inner, inner.height - 3),
                text,
                palette.muted,
                Alignment::Center,
            );
        }
    }
    if let Some(notice) = app.notice.as_ref().or(app.history_error.as_ref()) {
        let rect = row(inner, inner.height.saturating_sub(2));
        frame.render_widget(Clear, rect);
        line(
            frame,
            rect,
            notice.as_str(),
            palette.accent,
            Alignment::Center,
        );
    }
    if app.modal.is_some() {
        app.hits.clear();
        modal(frame, app, area, palette);
    }
}

fn header(frame: &mut Frame, app: &mut App, area: Rect, p: Palette) {
    line(frame, area, "pomo", p.accent, Alignment::Left);
    let width = 23.min(area.width);
    let tabs = centered_width(area, width);
    let timer = Rect::new(tabs.x, tabs.y, 9, 1);
    let calendar = Rect::new(tabs.x + 11, tabs.y, 12, 1);
    line(
        frame,
        timer,
        if app.view == View::Timer {
            "[ focus ]"
        } else {
            "  focus  "
        },
        if app.view == View::Timer {
            p.accent
        } else {
            p.muted
        },
        Alignment::Center,
    );
    line(
        frame,
        calendar,
        if app.view == View::Calendar {
            "[ calendar ]"
        } else {
            "  calendar  "
        },
        if app.view == View::Calendar {
            p.accent
        } else {
            p.muted
        },
        Alignment::Center,
    );
    app.hits
        .extend([(timer, Action::Timer), (calendar, Action::Calendar)]);
    if area.width >= 64 {
        let text = match app.engine.phase {
            _ if app.view == View::Timer => app.now.format("%a, %d %b").to_string(),
            Phase::Idle => app.now.format("%a, %d %b").to_string(),
            phase => format!(
                "{} {}",
                if phase == Phase::Ready {
                    "ready"
                } else if !app.engine.running {
                    "paused"
                } else if phase == Phase::Focus {
                    "focus"
                } else {
                    "break"
                },
                app.engine.display(&app.settings)
            ),
        };
        line(
            frame,
            Rect::new(area.right() - 19, area.y, 19, 1),
            text,
            p.muted,
            Alignment::Right,
        );
    }
}

fn timer(frame: &mut Frame, app: &mut App, area: Rect, p: Palette) {
    let large = area.width >= 42 && area.height >= 14;
    let height = if large { 14 } else { 7 };
    let body = Rect::new(
        area.x,
        area.y + area.height.saturating_sub(height) / 2,
        area.width,
        height.min(area.height),
    );
    let phase = match app.engine.phase {
        Phase::Idle => "ready to focus",
        Phase::Ready => "break complete",
        _ if !app.engine.running => "paused",
        Phase::Focus if app.engine.overtime() => "focus · overtime",
        Phase::Focus => "focus",
        Phase::ShortBreak => "short break",
        Phase::LongBreak => "long break",
    };
    line(frame, row(body, 0), phase, p.muted, Alignment::Center);
    let display = app.engine.display(&app.settings);
    let clock = large_clock(&display);
    let wide_enough = clock
        .first()
        .is_some_and(|text| text.width() <= usize::from(area.width));
    if large && wide_enough {
        for (i, text) in clock.into_iter().enumerate() {
            line(
                frame,
                row(body, i as u16 + 2),
                text,
                p.accent,
                Alignment::Center,
            );
        }
    } else {
        frame.render_widget(
            Paragraph::new(display)
                .style(Style::default().fg(p.accent).add_modifier(Modifier::BOLD))
                .alignment(Alignment::Center),
            row(body, if large { 4 } else { 2 }),
        );
    }
    if large {
        let gauge_area = centered_width(row(body, 8), 30);
        frame.render_widget(
            LineGauge::default()
                .ratio(app.engine.progress())
                .filled_style(Style::default().fg(p.accent))
                .unfilled_style(Style::default().fg(p.muted))
                .label("")
                .filled_symbol("━")
                .unfilled_symbol("─"),
            Rect::new(gauge_area.x, gauge_area.y, gauge_area.width, 1),
        );
    }
    let status = match app.engine.phase {
        Phase::Idle => "One thing at a time.".into(),
        Phase::Ready => "Space to begin your next focus.".into(),
        _ if !app.engine.running => "Time paused. Space to resume.".into(),
        Phase::Focus if app.engine.overtime() => "Keep going. Break when you're ready.".into(),
        _ => {
            let remaining = app.engine.target_ms.saturating_sub(app.engine.elapsed_ms);
            let finish =
                app.now + chrono::Duration::milliseconds(remaining.min(i64::MAX as u64) as i64);
            format!(
                "{} at {}",
                if app.engine.phase == Phase::Focus {
                    "Target"
                } else {
                    "Back"
                },
                finish.format("%H:%M")
            )
        }
    };
    line(
        frame,
        row(body, if large { 10 } else { 4 }),
        status,
        p.muted,
        Alignment::Center,
    );
    if !app.quiet {
        let action_y = if large { 12 } else { 6 };
        if action_y >= body.height {
            return;
        }
        let label = match app.engine.phase {
            Phase::Idle | Phase::Ready => "space  start",
            _ if app.engine.running => "space  pause",
            _ => "space  resume",
        };
        let secondary = if app.engine.phase == Phase::Focus {
            "b  take break"
        } else if app.engine.phase.is_break() {
            "b  skip break"
        } else {
            ""
        };
        let total = label.width()
            + if secondary.is_empty() {
                4
            } else {
                secondary.width() + 8
            };
        let actions = centered_width(row(body, action_y), total as u16);
        let primary = Rect::new(actions.x, actions.y, (label.width() + 4) as u16, 1);
        line(
            frame,
            primary,
            format!("[ {label} ]"),
            p.accent,
            Alignment::Left,
        );
        app.hits.push((primary, Action::Toggle));
        if !secondary.is_empty() && total <= usize::from(area.width) {
            let secondary_area =
                Rect::new(primary.right() + 4, actions.y, secondary.width() as u16, 1);
            line(frame, secondary_area, secondary, p.muted, Alignment::Left);
            app.hits.push((secondary_area, Action::Break));
        }
        if area.height >= 18 {
            let filled = app.engine.cycle(&app.settings);
            let interval = app.settings.long_break_interval;
            let cycle = if interval <= 8 {
                let dots = (0..interval)
                    .map(|i| if i < filled { "●" } else { "○" })
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("{dots}   {filled}/{interval} toward long break")
            } else {
                format!("{filled}/{interval} toward long break")
            };
            line(
                frame,
                row(area, area.height - 1),
                cycle,
                p.muted,
                Alignment::Center,
            );
        }
    }
}

fn calendar(frame: &mut Frame, app: &mut App, area: Rect, p: Palette) {
    if area.height < 3 {
        return;
    }
    let data = app.live_history(false);
    let week_end = app.week + Days::new(7);
    let total = coverage(&data, day_bounds(app.week).0, day_bounds(week_end).0);
    let title = format!(
        "{} – {}",
        app.week.format("%d %b"),
        (week_end - Days::new(1)).format("%d %b %Y")
    );
    line(frame, row(area, 0), &title, p.accent, Alignment::Left);
    if area.width >= 58 {
        let nav = Rect::new(area.right() - 22, area.y, 22, 1);
        line(
            frame,
            nav,
            "‹ prev   today   next ›",
            p.muted,
            Alignment::Right,
        );
        app.hits.extend([
            (Rect::new(nav.x, nav.y, 6, 1), Action::PrevWeek),
            (Rect::new(nav.x + 8, nav.y, 5, 1), Action::Today),
            (Rect::new(nav.x + 16, nav.y, 6, 1), Action::NextWeek),
        ]);
    }
    if let Some(error) = &app.history_error {
        frame.render_widget(
            Paragraph::new(error.as_str()).wrap(Wrap { trim: true }),
            Rect::new(area.x, area.y + 2, area.width, area.height - 2),
        );
        return;
    }
    let body = Rect::new(
        area.x,
        area.y + 2,
        area.width,
        area.height.saturating_sub(3),
    );
    if app.day_view {
        day_detail(frame, app, &data, body, p);
    } else if area.width < 68 || body.height < 6 {
        compact_week(frame, app, &data, body, p);
    } else {
        week_grid(frame, app, &data, body, p);
    }
    let summary = format!("week  {}   ·   focus time only", duration_label(total));
    line(
        frame,
        row(area, area.height - 1),
        summary,
        p.muted,
        Alignment::Left,
    );
}

fn compact_week(frame: &mut Frame, app: &mut App, data: &[Segment], area: Rect, p: Palette) {
    let visible = usize::from(area.height).min(7);
    let first = app.selected_day.saturating_sub(visible.saturating_sub(1));
    for i in first..(first + visible).min(7) {
        let date = app.week + Days::new(i as u64);
        let (start, end) = day_bounds(date);
        let selected = i == app.selected_day;
        let text = format!(
            "{} {}   {:>8}{}",
            if selected { "›" } else { " " },
            date.format("%a %d"),
            duration_label(coverage(data, start, end)),
            if date == app.now.date_naive() {
                "  today"
            } else {
                ""
            }
        );
        let target = row(area, (i - first) as u16);
        line(
            frame,
            target,
            text,
            if selected { p.accent } else { p.muted },
            Alignment::Left,
        );
        app.hits.push((target, Action::Day(i)));
    }
    if area.height > 8 {
        line(
            frame,
            row(area, 8),
            "[ ] select day · enter for exact times",
            p.muted,
            Alignment::Left,
        );
    }
}

fn week_grid(frame: &mut Frame, app: &mut App, data: &[Segment], area: Rect, p: Palette) {
    let label_width = 6;
    let cell_width = (area.width - label_width) / 7;
    let visible = area.height.saturating_sub(3).min(48) as usize;
    let offset = app.scroll.min(48_usize.saturating_sub(visible));
    app.scroll = offset;
    for day in 0..7 {
        let date = app.week + Days::new(day as u64);
        let cell = Rect::new(
            area.x + label_width + day as u16 * cell_width,
            area.y,
            cell_width,
            1,
        );
        let is_today = date == app.now.date_naive();
        line(
            frame,
            cell,
            format!(
                "{}{}",
                if day == app.selected_day { "›" } else { " " },
                date.format("%a %d")
            ),
            if is_today || day == app.selected_day {
                p.accent
            } else {
                p.muted
            },
            Alignment::Center,
        );
        app.hits.push((cell, Action::Day(day)));
        let (start, end) = day_bounds(date);
        line(
            frame,
            Rect::new(cell.x, cell.y + 1, cell.width, 1),
            duration_label(coverage(data, start, end)),
            p.muted,
            Alignment::Center,
        );
        for i in 0..visible {
            let slot = offset + i;
            let hour = slot as u32 / 2;
            let minute = (slot as u32 % 2) * 30;
            // Resolve half-hours independently, including repeated/skipped DST hours.
            let intervals = store::half_hour_intervals(&Local, date, hour, minute);
            let ms: u64 = intervals
                .iter()
                .map(|(start, end)| coverage(data, *start, *end))
                .sum();
            let available: i64 = intervals.iter().map(|(start, end)| end - start).sum();
            let width = usize::from(cell_width.saturating_sub(2));
            let bar = if ms == 0 {
                "·".into()
            } else {
                let fraction = ms as f64 / available.max(1) as f64;
                let blocks = ((fraction * width as f64).ceil() as usize).clamp(1, width.max(1));
                "━".repeat(blocks)
            };
            let target = Rect::new(cell.x, area.y + 3 + i as u16, cell.width, 1);
            line(
                frame,
                target,
                bar,
                if ms > 0 { p.accent } else { p.faint },
                Alignment::Center,
            );
        }
    }
    for i in 0..visible {
        let slot = offset + i;
        let label = format!("{:02}:{:02}", slot / 2, (slot % 2) * 30);
        line(
            frame,
            Rect::new(area.x, area.y + 3 + i as u16, 5, 1),
            label,
            p.muted,
            Alignment::Right,
        );
    }
    // State the visible range so early/late work is discoverable.
    if area.height > 3 {
        line(
            frame,
            Rect::new(area.x, area.y + 1, 5, 1),
            "↑ ↓",
            p.muted,
            Alignment::Center,
        );
    }
}

fn day_detail(frame: &mut Frame, app: &mut App, data: &[Segment], area: Rect, p: Palette) {
    if area.height == 0 {
        return;
    }
    let day = app.week + Days::new(app.selected_day as u64);
    let (start, end) = day_bounds(day);
    line(
        frame,
        row(area, 0),
        format!(
            "{}   ·   {}",
            day.format("%A, %d %b"),
            duration_label(coverage(data, start, end))
        ),
        p.accent,
        Alignment::Left,
    );
    let mut segments: Vec<_> = data
        .iter()
        .filter(|segment| {
            segment.end_ms > start && segment.start_ms < end && segment.end_ms > segment.start_ms
        })
        .collect();
    segments.sort_by_key(|segment| segment.start_ms);
    if segments.is_empty() {
        line(
            frame,
            row(area, 2),
            "No focus time recorded for this day.",
            p.muted,
            Alignment::Left,
        );
    } else {
        let visible = usize::from(area.height.saturating_sub(3));
        let offset = app
            .scroll
            .min(segments.len().saturating_sub(visible.max(1)));
        app.scroll = offset;
        for (i, segment) in segments.iter().skip(offset).take(visible).enumerate() {
            let start_ms = segment.start_ms.max(start);
            let end_ms = segment.end_ms.min(end);
            let start_text = local_time(start_ms, "%H:%M:%S");
            let end_text = local_time(end_ms, "%H:%M:%S");
            let live = app
                .active
                .as_ref()
                .is_some_and(|active| active.id == segment.id);
            let text = format!(
                "{start_text} – {end_text}   {:>8}{}",
                duration_label((end_ms - start_ms) as u64),
                if live { "  active" } else { "" }
            );
            line(
                frame,
                row(area, i as u16 + 2),
                text,
                if live { p.accent } else { Color::Reset },
                Alignment::Left,
            );
        }
    }
    if area.height > 3 {
        line(
            frame,
            row(area, area.height - 1),
            "[ ] day   ↑ ↓ scroll   enter / esc week",
            p.muted,
            Alignment::Left,
        );
    }
}

fn local_time(ms: i64, format: &str) -> String {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .map(|date| date.with_timezone(&Local).format(format).to_string())
        .unwrap_or_else(|| "?".into())
}

fn modal(frame: &mut Frame, app: &mut App, area: Rect, p: Palette) {
    if area.width < 32 || area.height < 12 {
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new("Resize to at least 32×12\nto use this dialog.\n\nEsc to return.")
                .wrap(Wrap { trim: true }),
            area.inner(Margin::new(1, 1)),
        );
        return;
    }
    let (title, desired_height) = match &app.modal {
        Some(Modal::Settings(_)) => (" settings ", 21),
        Some(Modal::Help) => (" keys & behavior ", 23),
        _ => (" reset timer? ", 9),
    };
    let width = area.width.saturating_sub(2).min(66);
    let height = area.height.saturating_sub(2).min(desired_height);
    let box_area = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, box_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(p.muted))
        .title(title);
    let body = block.inner(box_area).inner(Margin::new(1, 0));
    frame.render_widget(block, box_area);
    match &app.modal {
        Some(Modal::Help) => {
            let lines = [
                "Space / Enter    start, pause, resume",
                "b                take / skip break",
                "r                reset (keeps history)",
                "s                settings",
                "Tab / 1 / 2      focus / calendar",
                "z                quiet timer",
                "q / Ctrl+C       save, pause & quit",
                "",
                "Calendar",
                "← →              previous / next week",
                "↑ ↓ / j k        scroll · Home/End jump",
                "[ ]              select day",
                "Enter / Esc      day details / week",
                "t                this week",
                "",
                "Focus ends silently and continues in overtime.",
                "Break complete waits for Space; any key silences.",
                "Only full focus blocks advance the break cycle.",
                "Sleep and pauses are excluded from focus time.",
                "",
                "Esc / any key to close",
            ];
            frame.render_widget(
                Paragraph::new(lines.join("\n")).wrap(Wrap { trim: false }),
                body,
            );
        }
        Some(Modal::Reset) => {
            frame.render_widget(Paragraph::new("End this timer and return to idle?\n\nRecorded focus time and cycle progress are kept.\n\nEnter reset   ·   Esc keep going").wrap(Wrap { trim: true }), body);
        }
        Some(Modal::Settings(editor)) => {
            let labels = [
                "Focus minutes",
                "Short break minutes",
                "Long break minutes",
                "Long break every",
                "Theme",
                "Break alert",
                "Sound",
            ];
            let compact = body.height < 16;
            let footer_space = 4;
            let visible = if compact {
                usize::from(body.height.saturating_sub(footer_space)).min(7)
            } else {
                7
            };
            let first = editor.selected.saturating_sub(visible.saturating_sub(1));
            for (line_index, index) in (first..(first + visible).min(7)).enumerate() {
                let y = if compact {
                    line_index as u16
                } else {
                    1 + line_index as u16 * 2
                };
                let target = row(body, y);
                let label_width = if body.width < 42 { 20 } else { 24 };
                let text = format!(
                    "{} {:<label_width$} {}",
                    if index == editor.selected { "›" } else { " " },
                    labels[index],
                    editor.values[index]
                );
                line(
                    frame,
                    target,
                    text,
                    if index == editor.selected {
                        p.accent
                    } else {
                        p.muted
                    },
                    Alignment::Left,
                );
                app.hits.push((target, Action::Field(index)));
            }
            let bottom = body.height.saturating_sub(3);
            if let Some(error) = &editor.error {
                line(
                    frame,
                    row(body, bottom),
                    error.as_str(),
                    Color::Yellow,
                    Alignment::Left,
                );
            } else if body.width >= 48 {
                line(
                    frame,
                    row(body, bottom),
                    "↑ ↓ select   ← → change   type a number",
                    p.muted,
                    Alignment::Left,
                );
            }
            let controls = row(body, body.height.saturating_sub(1));
            let text = if body.width >= 43 {
                "enter save   esc cancel   p preview sound"
            } else {
                "enter save · esc cancel · p preview"
            };
            line(frame, controls, text, p.accent, Alignment::Left);
            app.hits.extend([
                (
                    Rect::new(controls.x, controls.y, 10, 1),
                    Action::SaveSettings,
                ),
                (
                    Rect::new(controls.x + 13, controls.y, 10, 1),
                    Action::Cancel,
                ),
                (
                    Rect::new(
                        controls.x + 26,
                        controls.y,
                        controls.width.saturating_sub(26),
                        1,
                    ),
                    Action::Preview,
                ),
            ]);
        }
        None => {}
    }
}

fn row(area: Rect, offset: u16) -> Rect {
    if offset >= area.height {
        return Rect::new(area.x, area.bottom(), area.width, 0);
    }
    Rect::new(area.x, area.y + offset, area.width, 1)
}

fn centered_width(area: Rect, width: u16) -> Rect {
    let width = width.min(area.width);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y,
        width,
        area.height,
    )
}

fn line(
    frame: &mut Frame,
    area: Rect,
    text: impl Into<String>,
    color: Color,
    alignment: Alignment,
) {
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            text.into(),
            Style::default().fg(color),
        )))
        .alignment(alignment),
        area,
    );
}

fn large_clock(value: &str) -> Vec<String> {
    let glyphs: Vec<_> = value
        .chars()
        .map(|c| match c {
            '0' => ["╭───╮", "│   │", "│   │", "│   │", "╰───╯"],
            '1' => ["  ╷  ", "  │  ", "  │  ", "  │  ", "  ╵  "],
            '2' => ["╶───╮", "    │", "╭───╯", "│    ", "╰───╴"],
            '3' => ["╶───╮", "    │", " ╶──┤", "    │", "╶───╯"],
            '4' => ["╷   ╷", "│   │", "╰───┤", "    │", "    ╵"],
            '5' => ["╭───╴", "│    ", "╰───╮", "    │", "╶───╯"],
            '6' => ["╭───╴", "│    ", "├───╮", "│   │", "╰───╯"],
            '7' => ["╶───╮", "    │", "   ╱ ", "  ╱  ", " ╱   "],
            '8' => ["╭───╮", "│   │", "├───┤", "│   │", "╰───╯"],
            '9' => ["╭───╮", "│   │", "╰───┤", "    │", "╶───╯"],
            ':' => [" ", "·", " ", "·", " "],
            '+' => ["     ", "  ╷  ", "╶─┼─╴", "  ╵  ", "     "],
            _ => [" "; 5],
        })
        .collect();
    (0..5)
        .map(|row| {
            glyphs
                .iter()
                .map(|glyph| glyph[row])
                .collect::<Vec<_>>()
                .join("  ")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app::SettingsEditor, config::Paths};
    use ratatui::{Terminal, backend::TestBackend};

    fn render(app: &mut App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .chunks(usize::from(width).max(1))
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn every_screen_handles_small_and_large_terminals() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::open(
            Paths::discover(Some(dir.path().into())).unwrap(),
            Local::now(),
        )
        .unwrap();
        for (width, height) in [
            (1, 1),
            (23, 7),
            (24, 8),
            (32, 12),
            (60, 18),
            (80, 24),
            (120, 40),
            (180, 60),
        ] {
            for view in [View::Timer, View::Calendar] {
                app.view = view;
                for day_view in [false, true] {
                    app.day_view = day_view;
                    app.modal = None;
                    render(&mut app, width, height);
                    app.modal = Some(Modal::Settings(SettingsEditor::new(&app.settings)));
                    render(&mut app, width, height);
                    app.modal = Some(Modal::Help);
                    render(&mut app, width, height);
                    app.modal = Some(Modal::Reset);
                    render(&mut app, width, height);
                }
            }
        }
    }

    #[test]
    fn main_view_is_actionable_and_quiet_mode_hides_chrome() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::open(
            Paths::discover(Some(dir.path().into())).unwrap(),
            Local::now(),
        )
        .unwrap();
        let normal = render(&mut app, 100, 32);
        assert!(normal.contains("space  start"));
        assert!(normal.contains("today  0s"));
        assert!(normal.contains("0/4 toward long break"));
        app.quiet = true;
        let quiet = render(&mut app, 100, 32);
        assert!(!quiet.contains("calendar"));
        assert!(!quiet.contains("today"));
        assert!(quiet.contains("z show controls"));
    }

    #[test]
    fn narrow_calendar_keeps_day_totals_and_details_available() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::open(
            Paths::discover(Some(dir.path().into())).unwrap(),
            Local::now(),
        )
        .unwrap();
        app.view = View::Calendar;
        let compact = render(&mut app, 48, 24);
        assert!(compact.contains("Mon"));
        assert!(compact.contains("Sun"));
        assert!(compact.contains("week  0s"));
        app.day_view = true;
        assert!(render(&mut app, 48, 24).contains("No focus time recorded"));
    }

    #[test]
    fn day_details_include_live_time_without_double_counting() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::open(
            Paths::discover(Some(dir.path().into())).unwrap(),
            Local::now(),
        )
        .unwrap();
        app.action(Action::Toggle);
        let now = app.now + chrono::Duration::seconds(2);
        app.advance(2_000, 2_000, now);
        app.view = View::Calendar;
        app.day_view = true;
        let rendered = render(&mut app, 100, 32);
        assert!(rendered.contains("active"));
        assert!(rendered.contains("week  2s"));
    }

    #[test]
    fn timer_week_total_does_not_follow_calendar_navigation() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::open(
            Paths::discover(Some(dir.path().into())).unwrap(),
            Local::now(),
        )
        .unwrap();
        app.action(Action::Toggle);
        let now = app.now + chrono::Duration::seconds(2);
        app.advance(2_000, 2_000, now);
        app.action(Action::PrevWeek);
        let rendered = render(&mut app, 100, 32);
        assert!(rendered.contains("today  2s"));
        assert!(rendered.contains("this week  2s"));
    }

    #[test]
    fn scrolling_up_from_end_moves_immediately() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::open(
            Paths::discover(Some(dir.path().into())).unwrap(),
            Local::now(),
        )
        .unwrap();
        app.view = View::Calendar;
        app.key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        let bottom = render(&mut app, 100, 24);
        assert!(bottom.contains("23:30"));
        app.key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        let previous = render(&mut app, 100, 24);
        assert!(!previous.contains("23:30"));
        assert!(previous.contains("23:00"));
    }
}
