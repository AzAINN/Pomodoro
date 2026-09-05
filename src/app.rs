use std::fs::{self, File, OpenOptions};

use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Days, Local, NaiveDate, Timelike};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;

use crate::{
    config::{ALERTS, Paths, SOUNDS, Settings, THEMES},
    engine::{Engine, Phase, interrupted},
    sound::Alerts,
    store::{self, Segment, Store},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Timer,
    Calendar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Toggle,
    Break,
    Timer,
    Calendar,
    Settings,
    PrevWeek,
    NextWeek,
    Today,
    Day(usize),
    Field(usize),
    SaveSettings,
    Cancel,
    Preview,
}

pub enum Modal {
    Help,
    Reset,
    Settings(SettingsEditor),
}

pub struct SettingsEditor {
    pub values: Vec<String>,
    pub selected: usize,
    pub error: Option<String>,
    pub replace: bool,
}

impl SettingsEditor {
    pub fn new(settings: &Settings) -> Self {
        Self {
            values: vec![
                settings.focus_minutes.to_string(),
                settings.short_break_minutes.to_string(),
                settings.long_break_minutes.to_string(),
                settings.long_break_interval.to_string(),
                settings.theme.clone(),
                settings.alert.clone(),
                settings.sound.clone(),
            ],
            selected: 0,
            error: None,
            replace: true,
        }
    }

    pub fn settings(&self) -> Result<Settings> {
        let parse = |i: usize| {
            self.values[i]
                .parse::<u32>()
                .context("Enter whole numbers for durations and interval")
        };
        let settings = Settings {
            focus_minutes: parse(0)?,
            short_break_minutes: parse(1)?,
            long_break_minutes: parse(2)?,
            long_break_interval: parse(3)?,
            theme: self.values[4].clone(),
            alert: self.values[5].clone(),
            sound: self.values[6].clone(),
        };
        settings.validate()?;
        Ok(settings)
    }

    fn adjust(&mut self, direction: i32) {
        let index = self.selected;
        if index < 4 {
            let max = if index == 3 { 99 } else { 1440 };
            let value = self.values[index].parse::<i32>().unwrap_or(1);
            self.values[index] = (value + direction).clamp(1, max).to_string();
        } else {
            let choices = match index {
                4 => THEMES.as_slice(),
                5 => ALERTS.as_slice(),
                _ => SOUNDS.as_slice(),
            };
            let current = choices
                .iter()
                .position(|value| *value == self.values[index])
                .unwrap_or(0);
            let next = (current as i32 + direction).rem_euclid(choices.len() as i32) as usize;
            self.values[index] = choices[next].into();
        }
        self.replace = true;
        self.error = None;
    }

    fn key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Down | KeyCode::Tab => {
                self.selected = (self.selected + 1) % 7;
                self.replace = true;
            }
            KeyCode::Up | KeyCode::BackTab => {
                self.selected = (self.selected + 6) % 7;
                self.replace = true;
            }
            KeyCode::Left | KeyCode::Char('-') => self.adjust(-1),
            KeyCode::Right | KeyCode::Char('+') => self.adjust(1),
            KeyCode::Char(c) if c.is_ascii_digit() && self.selected < 4 => {
                if self.replace {
                    self.values[self.selected].clear();
                    self.replace = false;
                }
                if self.values[self.selected].len() < 4 {
                    self.values[self.selected].push(c);
                }
                self.error = None;
            }
            KeyCode::Backspace if self.selected < 4 => {
                if self.replace {
                    self.values[self.selected].clear();
                } else {
                    self.values[self.selected].pop();
                }
                self.replace = false;
                self.error = None;
            }
            _ => {}
        }
    }
}

pub struct App {
    pub settings: Settings,
    pub engine: Engine,
    pub view: View,
    pub quiet: bool,
    pub modal: Option<Modal>,
    pub notice: Option<String>,
    pub history_error: Option<String>,
    pub week: NaiveDate,
    pub selected_day: usize,
    pub day_view: bool,
    pub scroll: usize,
    pub now: DateTime<Local>,
    pub history: Vec<Segment>,
    pub today_history: Vec<Segment>,
    pub current_week_history: Vec<Segment>,
    pub active: Option<Segment>,
    pub hits: Vec<(Rect, Action)>,
    pub quit: bool,
    pub alerts: Alerts,
    pub paths: Paths,
    store: Store,
    // Keep the OS lock alive for this app. It releases even after a crash.
    _lock: File,
}

impl App {
    pub fn open(paths: Paths, now: DateTime<Local>) -> Result<Self> {
        fs::create_dir_all(&paths.data)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(paths.data.join("pomo.lock"))?;
        lock.try_lock()
            .context("pomo is already running with this data directory")?;
        let (settings, warning) = Settings::load(&paths.config)?;
        let store = Store::open(&paths.database())?;
        let engine = store.restore()?;
        let notice = warning.or_else(|| {
            engine.phase.kind().is_some().then(|| {
                "Session restored, paused. Space to resume; time away was not counted.".into()
            })
        });
        let mut app = Self {
            settings,
            engine,
            view: View::Timer,
            quiet: false,
            modal: None,
            notice,
            history_error: None,
            week: store::monday(now.date_naive()),
            selected_day: now.weekday().num_days_from_monday() as usize,
            day_view: false,
            scroll: usize::try_from(now.hour().saturating_sub(2) * 2).unwrap_or(16),
            now,
            history: Vec::new(),
            today_history: Vec::new(),
            current_week_history: Vec::new(),
            active: None,
            hits: Vec::new(),
            quit: false,
            alerts: Alerts::default(),
            paths,
            store,
            _lock: lock,
        };
        app.reload_history();
        Ok(app)
    }

    pub fn advance(&mut self, monotonic_ms: u64, wall_delta_ms: i64, now: DateTime<Local>) {
        let previous_day = self.now.date_naive();
        self.now = now;
        if self.engine.running && interrupted(monotonic_ms, wall_delta_ms) {
            self.engine.running = false;
            self.notice = Some(
                "Paused after sleep or a clock change. Space to resume; time away excluded.".into(),
            );
            self.persist();
        } else {
            let previous = self.engine.phase;
            let used = self.engine.advance(monotonic_ms);
            if let Some(segment) = &mut self.active {
                segment.add_elapsed(used);
            }
            if previous != self.engine.phase {
                self.persist();
            }
        }
        if previous_day != now.date_naive() {
            if self.week == store::monday(previous_day) {
                self.week = store::monday(now.date_naive());
                self.selected_day = now.weekday().num_days_from_monday() as usize;
            }
            self.reload_history();
        }
    }

    pub fn persist(&mut self) {
        if let Err(error) =
            self.store
                .checkpoint(&self.engine, &mut self.active, self.now.timestamp_millis())
        {
            self.engine.running = false;
            self.alerts.silence();
            self.notice = Some(format!(
                "Recording failed; timer paused. Space to retry. {error}"
            ));
        }
        self.reload_history();
    }

    pub fn shutdown(&mut self) -> Result<()> {
        self.engine.running = false;
        self.alerts.silence();
        self.store
            .checkpoint(&self.engine, &mut self.active, self.now.timestamp_millis())
            .context("Could not save the final checkpoint; earlier checkpoints are intact")
    }

    pub fn reload_history(&mut self) {
        let start = store::day_bounds(self.week).0;
        let end = store::day_bounds(self.week + Days::new(7)).0;
        let (today_start, today_end) = store::day_bounds(self.now.date_naive());
        let current_week = store::monday(self.now.date_naive());
        let current_start = store::day_bounds(current_week).0;
        let current_end = store::day_bounds(current_week + Days::new(7)).0;
        match (
            self.store.range(start, end),
            self.store.range(today_start, today_end),
            self.store.range(current_start, current_end),
        ) {
            (Ok(history), Ok(today), Ok(current)) => {
                self.history = history;
                self.today_history = today;
                self.current_week_history = current;
                self.history_error = None;
            }
            (Err(error), _, _) | (_, Err(error), _) | (_, _, Err(error)) => {
                self.history_error = Some(format!("History unavailable: {error}"));
            }
        }
    }

    pub fn live_history(&self, today: bool) -> Vec<Segment> {
        self.with_active(if today {
            &self.today_history
        } else {
            &self.history
        })
    }

    pub fn live_current_week(&self) -> Vec<Segment> {
        self.with_active(&self.current_week_history)
    }

    fn with_active(&self, history: &[Segment]) -> Vec<Segment> {
        let mut segments = history.to_vec();
        if let Some(active) = &self.active
            && active.kind == "focus"
        {
            segments.retain(|segment| segment.id != active.id);
            segments.push(active.clone());
        }
        segments
    }

    pub fn action(&mut self, action: Action) {
        if self.engine.phase == Phase::Ready {
            self.alerts.silence();
        }
        match action {
            Action::Cancel => {
                self.modal = None;
            }
            Action::SaveSettings => self.save_settings(),
            Action::Preview => {
                if let Some(Modal::Settings(editor)) = &self.modal {
                    self.alerts.preview(&editor.values[6]);
                }
            }
            Action::Field(index) => {
                if let Some(Modal::Settings(editor)) = &mut self.modal {
                    editor.selected = index;
                    editor.replace = true;
                }
            }
            _ if self.modal.is_some() => {}
            Action::Toggle => {
                self.notice = None;
                // A failed write can leave a row pending closure. Close it before
                // resuming, otherwise the paused gap would be filled by later time.
                if !self.engine.running && self.active.is_some() {
                    self.persist();
                    if self.active.is_some() {
                        return;
                    }
                }
                self.engine.toggle(&self.settings);
                self.persist();
            }
            Action::Break => {
                self.notice = None;
                if self.engine.phase == Phase::Focus {
                    if !self.engine.take_break(&self.settings) {
                        self.notice =
                            Some("Early break. Focus time saved; cycle progress unchanged.".into());
                    }
                } else {
                    self.engine.skip_break(&self.settings);
                }
                self.persist();
            }
            Action::Timer => self.view = View::Timer,
            Action::Calendar => {
                self.view = View::Calendar;
                self.reload_history();
            }
            Action::Settings => {
                self.modal = Some(Modal::Settings(SettingsEditor::new(&self.settings)))
            }
            Action::PrevWeek => self.change_week(-7),
            Action::NextWeek => self.change_week(7),
            Action::Today => {
                self.week = store::monday(self.now.date_naive());
                self.selected_day = self.now.weekday().num_days_from_monday() as usize;
                self.scroll = self.now.hour().saturating_sub(2) as usize * 2;
                self.reload_history();
            }
            Action::Day(index) => {
                self.selected_day = index.min(6);
                self.day_view = true;
                self.scroll = 0;
            }
        }
    }

    pub fn key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if self.engine.phase == Phase::Ready {
            self.alerts.silence();
        }
        if self.modal.is_some() {
            match key.code {
                KeyCode::Esc => self.modal = None,
                KeyCode::Enter => match &self.modal {
                    Some(Modal::Settings(_)) => self.save_settings(),
                    Some(Modal::Reset) => {
                        self.engine.reset();
                        self.modal = None;
                        self.notice = Some("Timer reset. Recorded focus time kept.".into());
                        self.persist();
                    }
                    _ => self.modal = None,
                },
                KeyCode::Char('p') if matches!(self.modal, Some(Modal::Settings(_))) => {
                    self.action(Action::Preview)
                }
                _ => match &mut self.modal {
                    Some(Modal::Settings(editor)) => editor.key(key.code),
                    Some(Modal::Help) => self.modal = None,
                    _ => {}
                },
            }
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char(' ') => self.action(Action::Toggle),
            KeyCode::Char('b') => self.action(Action::Break),
            KeyCode::Char('s') => self.action(Action::Settings),
            KeyCode::Char('r') if self.engine.phase != Phase::Idle => {
                self.modal = Some(Modal::Reset)
            }
            KeyCode::Char('?') => self.modal = Some(Modal::Help),
            KeyCode::Char('z') => self.quiet = !self.quiet,
            KeyCode::Esc => {
                if self.day_view {
                    self.day_view = false;
                    self.scroll = 16;
                } else {
                    self.notice = None;
                }
            }
            KeyCode::Tab | KeyCode::BackTab => self.action(if self.view == View::Timer {
                Action::Calendar
            } else {
                Action::Timer
            }),
            KeyCode::Char('1') => self.action(Action::Timer),
            KeyCode::Char('2') => self.action(Action::Calendar),
            KeyCode::Enter if self.view == View::Timer => self.action(Action::Toggle),
            KeyCode::Enter if self.view == View::Calendar => {
                self.day_view = !self.day_view;
                self.scroll = if self.day_view { 0 } else { 16 };
            }
            KeyCode::Left if self.view == View::Calendar => self.action(Action::PrevWeek),
            KeyCode::Right if self.view == View::Calendar => self.action(Action::NextWeek),
            KeyCode::Char('t') if self.view == View::Calendar => self.action(Action::Today),
            KeyCode::Char('[') if self.view == View::Calendar => {
                self.selected_day = (self.selected_day + 6) % 7;
                if self.day_view {
                    self.scroll = 0;
                }
            }
            KeyCode::Char(']') if self.view == View::Calendar => {
                self.selected_day = (self.selected_day + 1) % 7;
                if self.day_view {
                    self.scroll = 0;
                }
            }
            KeyCode::Up | KeyCode::Char('k') if self.view == View::Calendar => {
                self.scroll = self.scroll.saturating_sub(1)
            }
            KeyCode::Down | KeyCode::Char('j') if self.view == View::Calendar => {
                self.scroll = self.scroll.saturating_add(1).min(self.max_scroll())
            }
            KeyCode::PageUp if self.view == View::Calendar => {
                self.scroll = self.scroll.saturating_sub(8)
            }
            KeyCode::PageDown if self.view == View::Calendar => {
                self.scroll = self.scroll.saturating_add(8).min(self.max_scroll())
            }
            KeyCode::Home if self.view == View::Calendar => self.scroll = 0,
            KeyCode::End if self.view == View::Calendar => self.scroll = self.max_scroll(),
            _ => {}
        }
    }

    pub fn max_scroll(&self) -> usize {
        if self.day_view {
            let (start, end) = store::day_bounds(self.week + Days::new(self.selected_day as u64));
            self.live_history(false)
                .iter()
                .filter(|segment| segment.start_ms < end && segment.end_ms > start)
                .count()
                .saturating_sub(1)
        } else {
            47
        }
    }

    fn change_week(&mut self, days: i64) {
        if let Some(date) = self.week.checked_add_signed(chrono::Duration::days(days))
            && (1900..=9998).contains(&date.year())
        {
            self.week = date;
            self.reload_history();
            if self.day_view {
                self.scroll = 0;
            }
        }
    }

    fn save_settings(&mut self) {
        let Some(Modal::Settings(editor)) = &mut self.modal else {
            return;
        };
        match editor.settings().and_then(|settings| {
            settings.save(&self.paths.config)?;
            Ok(settings)
        }) {
            Ok(settings) => {
                self.settings = settings;
                self.modal = None;
                self.notice =
                    Some("Settings saved. New durations apply to the next session.".into());
            }
            Err(error) => editor.error = Some(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    fn press(app: &mut App, code: KeyCode) {
        app.key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    #[test]
    fn session_survives_restart_and_sleep_does_not_inflate_totals() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::discover(Some(dir.path().into())).unwrap();
        let start = Local::now();
        let mut app = App::open(paths.clone(), start).unwrap();
        press(&mut app, KeyCode::Char(' '));
        app.advance(1_250, 1_250, start + chrono::Duration::milliseconds(1_250));
        assert_eq!(
            store::coverage(&app.live_history(true), i64::MIN, i64::MAX),
            1_250
        );
        app.advance(250, 60_250, start + chrono::Duration::milliseconds(61_500));
        assert!(!app.engine.running);
        assert_eq!(app.engine.elapsed_ms, 1_250);
        assert_eq!(
            store::coverage(&app.live_history(true), i64::MIN, i64::MAX),
            1_250
        );
        app.shutdown().unwrap();
        drop(app);
        let restored = App::open(paths, start + chrono::Duration::hours(1)).unwrap();
        assert_eq!(restored.engine.elapsed_ms, 1_250);
        assert!(!restored.engine.running);
    }

    #[test]
    fn modal_blocks_timer_shortcuts_and_escape_does_not_save() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::discover(Some(dir.path().into())).unwrap();
        let mut app = App::open(paths, Local::now()).unwrap();
        press(&mut app, KeyCode::Char('s'));
        press(&mut app, KeyCode::Char('5'));
        press(&mut app, KeyCode::Char('0'));
        press(&mut app, KeyCode::Char(' '));
        press(&mut app, KeyCode::Char('q'));
        assert_eq!(app.engine.phase, Phase::Idle);
        assert!(!app.quit);
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.settings.focus_minutes, 25);
        press(&mut app, KeyCode::Char('s'));
        press(&mut app, KeyCode::Char('5'));
        press(&mut app, KeyCode::Char('0'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.settings.focus_minutes, 50);
        assert_eq!(
            Settings::load(&app.paths.config).unwrap().0.focus_minutes,
            50
        );
    }

    #[test]
    fn reset_is_confirmed_and_quit_never_starts_focus_at_break_end() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::open(
            Paths::discover(Some(dir.path().into())).unwrap(),
            Local::now(),
        )
        .unwrap();
        press(&mut app, KeyCode::Char(' '));
        press(&mut app, KeyCode::Char('r'));
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.engine.phase, Phase::Focus);
        press(&mut app, KeyCode::Char('r'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.engine.phase, Phase::Idle);
        app.engine.phase = Phase::Ready;
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.engine.phase, Phase::Ready);
        press(&mut app, KeyCode::Char('q'));
        assert!(app.quit);
        assert_eq!(app.engine.phase, Phase::Ready);
    }

    #[test]
    fn only_one_app_can_record_in_a_data_directory() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::discover(Some(dir.path().into())).unwrap();
        let app = App::open(paths.clone(), Local::now()).unwrap();
        assert!(App::open(paths.clone(), Local::now()).is_err());
        drop(app);
        assert!(App::open(paths, Local::now()).is_ok());
    }

    #[test]
    fn failed_heartbeat_pauses_and_retry_preserves_the_gap() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::discover(Some(dir.path().into())).unwrap();
        let now = Local::now();
        let mut app = App::open(paths.clone(), now).unwrap();
        app.action(Action::Toggle);
        app.advance(1_000, 1_000, now + chrono::Duration::seconds(1));
        let conn = rusqlite::Connection::open(paths.database()).unwrap();
        conn.execute_batch("CREATE TRIGGER fail_update BEFORE UPDATE ON sessions BEGIN SELECT RAISE(FAIL,'test disk failure'); END;").unwrap();
        app.persist();
        assert!(!app.engine.running);
        assert!(app.notice.as_ref().unwrap().contains("Recording failed"));
        app.advance(1_000, 1_000, now + chrono::Duration::seconds(2));
        app.action(Action::Toggle);
        assert!(!app.engine.running); // retry still fails; no unrecorded timer runs
        conn.execute_batch("DROP TRIGGER fail_update;").unwrap();
        app.advance(1_000, 1_000, now + chrono::Duration::seconds(3));
        app.action(Action::Toggle);
        app.advance(1_000, 1_000, now + chrono::Duration::seconds(4));
        app.action(Action::Toggle);
        let records = app.live_history(true);
        assert_eq!(records.len(), 2);
        assert_eq!(
            records[0].end_ms,
            (now + chrono::Duration::seconds(1)).timestamp_millis()
        );
        assert_eq!(
            records[1].start_ms,
            (now + chrono::Duration::seconds(3)).timestamp_millis()
        );
        assert_eq!(store::coverage(&records, i64::MIN, i64::MAX), 2_000);
    }
}
