//! Deterministic timer. Elapsed time is supplied by the monotonic event loop.
use serde::{Deserialize, Serialize};

use crate::config::Settings;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    #[default]
    Idle,
    Focus,
    ShortBreak,
    LongBreak,
    Ready,
}

impl Phase {
    pub fn is_break(self) -> bool {
        matches!(self, Self::ShortBreak | Self::LongBreak)
    }

    pub fn kind(self) -> Option<&'static str> {
        match self {
            Self::Focus => Some("focus"),
            Self::ShortBreak => Some("short_break"),
            Self::LongBreak => Some("long_break"),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Engine {
    pub phase: Phase,
    pub running: bool,
    pub elapsed_ms: u64,
    pub target_ms: u64,
    pub completed: u64,
    pub last_break_long: bool,
}

impl Engine {
    /// Return only the time actually consumed. Breaks stop at their deadline.
    pub fn advance(&mut self, delta_ms: u64) -> u64 {
        if !self.running {
            return 0;
        }
        let used = if self.phase.is_break() {
            delta_ms.min(self.target_ms.saturating_sub(self.elapsed_ms))
        } else {
            delta_ms
        };
        self.elapsed_ms = self.elapsed_ms.saturating_add(used);
        if self.phase.is_break() && self.elapsed_ms >= self.target_ms {
            self.phase = Phase::Ready;
            self.running = false;
        }
        used
    }

    pub fn toggle(&mut self, settings: &Settings) {
        if matches!(self.phase, Phase::Idle | Phase::Ready) {
            self.enter(Phase::Focus, settings.focus_minutes);
        } else {
            self.running = !self.running;
        }
    }

    pub fn take_break(&mut self, settings: &Settings) -> bool {
        if self.phase != Phase::Focus {
            return false;
        }
        // An early break is useful, but is not a completed Pomodoro.
        let completed = self.elapsed_ms >= self.target_ms;
        if completed {
            self.completed = self.completed.saturating_add(1);
        }
        self.last_break_long = completed
            && self
                .completed
                .is_multiple_of(u64::from(settings.long_break_interval));
        if self.last_break_long {
            self.enter(Phase::LongBreak, settings.long_break_minutes);
        } else {
            self.enter(Phase::ShortBreak, settings.short_break_minutes);
        }
        completed
    }

    pub fn skip_break(&mut self, settings: &Settings) {
        if self.phase.is_break() || self.phase == Phase::Ready {
            self.enter(Phase::Focus, settings.focus_minutes);
        }
    }

    pub fn reset(&mut self) {
        *self = Self {
            completed: self.completed,
            ..Self::default()
        };
    }

    pub fn overtime(&self) -> bool {
        self.phase == Phase::Focus && self.elapsed_ms > self.target_ms
    }

    pub fn display(&self, settings: &Settings) -> String {
        let target = if self.phase == Phase::Idle {
            u64::from(settings.focus_minutes) * 60_000
        } else {
            self.target_ms
        };
        if self.overtime() {
            format!("+{}", mmss((self.elapsed_ms - target) / 1_000))
        } else {
            mmss(target.saturating_sub(self.elapsed_ms).div_ceil(1_000))
        }
    }

    pub fn progress(&self) -> f64 {
        if self.target_ms == 0 {
            0.0
        } else {
            (self.elapsed_ms as f64 / self.target_ms as f64).clamp(0.0, 1.0)
        }
    }

    pub fn cycle(&self, settings: &Settings) -> u32 {
        let interval = u64::from(settings.long_break_interval);
        let remainder = (self.completed % interval) as u32;
        if remainder == 0
            && self.last_break_long
            && (self.phase.is_break() || self.phase == Phase::Ready)
        {
            settings.long_break_interval
        } else {
            remainder
        }
    }

    fn enter(&mut self, phase: Phase, minutes: u32) {
        self.phase = phase;
        self.running = true;
        self.elapsed_ms = 0;
        self.target_ms = u64::from(minutes) * 60_000;
    }
}

pub fn mmss(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// A suspended process, sleeping computer, or clock adjustment must not invent work.
pub fn interrupted(monotonic_ms: u64, wall_ms: i64) -> bool {
    monotonic_ms > 5_000 || wall_ms.abs_diff(monotonic_ms.min(i64::MAX as u64) as i64) > 2_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdown_rounds_up_and_overtime_is_silent() {
        let settings = Settings::default();
        let mut engine = Engine::default();
        engine.toggle(&settings);
        engine.advance(100);
        assert_eq!(engine.display(&settings), "25:00");
        engine.advance(1_499_900);
        assert_eq!(engine.display(&settings), "00:00");
        assert!(!engine.overtime());
        engine.advance(151_000);
        assert_eq!(engine.display(&settings), "+02:31");
        assert_eq!(engine.phase, Phase::Focus);
        assert!(engine.running);
    }

    #[test]
    fn pause_excludes_time_and_settings_do_not_change_active_target() {
        let mut settings = Settings::default();
        let mut engine = Engine::default();
        engine.toggle(&settings);
        engine.advance(60_000);
        engine.toggle(&settings);
        assert_eq!(engine.advance(500_000), 0);
        settings.focus_minutes = 50;
        engine.toggle(&settings);
        engine.advance(60_000);
        assert_eq!(engine.display(&settings), "23:00");
        engine.reset();
        assert_eq!(engine.display(&settings), "50:00");
    }

    #[test]
    fn only_full_focus_blocks_advance_long_break_cycle() {
        let settings = Settings::default();
        let mut engine = Engine::default();
        engine.toggle(&settings);
        engine.advance(1_000);
        assert!(!engine.take_break(&settings));
        assert_eq!(engine.completed, 0);
        for n in 1..=4 {
            engine.skip_break(&settings);
            engine.advance(1_500_000);
            assert!(engine.take_break(&settings));
            assert_eq!(engine.completed, n);
            assert_eq!(
                engine.phase,
                if n == 4 {
                    Phase::LongBreak
                } else {
                    Phase::ShortBreak
                }
            );
        }
        assert_eq!(engine.cycle(&settings), 4);
        engine.skip_break(&settings);
        assert_eq!(engine.cycle(&settings), 0);
    }

    #[test]
    fn break_caps_at_deadline_and_waits_for_explicit_start() {
        let settings = Settings::default();
        let mut engine = Engine::default();
        engine.toggle(&settings);
        engine.take_break(&settings);
        assert_eq!(engine.advance(301_250), 300_000);
        assert_eq!(engine.phase, Phase::Ready);
        assert_eq!(engine.advance(10_000), 0);
        assert_eq!(engine.display(&settings), "00:00");
        engine.toggle(&settings);
        assert_eq!(engine.phase, Phase::Focus);
    }

    #[test]
    fn detect_sleep_stalls_and_clock_changes() {
        assert!(!interrupted(250, 250));
        assert!(!interrupted(1_500, 1_510));
        assert!(interrupted(250, 60_250)); // macOS sleep: Instant may exclude sleep
        assert!(interrupted(60_250, 60_250)); // Linux sleep or stopped process
        assert!(interrupted(250, -60_000));
    }
}
