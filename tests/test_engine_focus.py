from __future__ import annotations

from pomo.config import Settings
from pomo.engine import Phase, TimerEngine


class FakeClock:
    def __init__(self) -> None:
        self.t = 0.0

    def __call__(self) -> float:
        return self.t

    def advance(self, seconds: float) -> None:
        self.t += seconds


class SpyEvents:
    def __init__(self) -> None:
        self.log: list[tuple[str, ...]] = []

    def segment_opened(self, kind: str) -> None:
        self.log.append(("open", kind))

    def segment_closed(self) -> None:
        self.log.append(("close",))

    def ringing_started(self) -> None:
        self.log.append(("ring_on",))

    def ringing_stopped(self) -> None:
        self.log.append(("ring_off",))


def make_engine() -> tuple[TimerEngine, FakeClock, SpyEvents]:
    clock = FakeClock()
    events = SpyEvents()
    engine = TimerEngine(Settings(), now=clock, events=events)
    return engine, clock, events


def test_idle_shows_focus_duration() -> None:
    engine, _, _ = make_engine()
    assert engine.phase is Phase.IDLE
    assert not engine.running
    assert engine.display_time() == "25:00"


def test_toggle_from_idle_starts_focus_and_opens_segment() -> None:
    engine, clock, events = make_engine()
    engine.toggle()
    assert engine.phase is Phase.FOCUS
    assert engine.running
    assert events.log == [("open", "focus")]
    clock.advance(61)
    assert engine.display_time() == "23:59"


def test_pause_freezes_clock_and_closes_segment() -> None:
    engine, clock, events = make_engine()
    engine.toggle()
    clock.advance(60)
    engine.toggle()  # pause
    assert engine.phase is Phase.FOCUS
    assert not engine.running
    assert events.log == [("open", "focus"), ("close",)]
    clock.advance(999)  # paused time must not count
    assert engine.display_time() == "24:00"


def test_resume_opens_new_segment_and_continues() -> None:
    engine, clock, events = make_engine()
    engine.toggle()
    clock.advance(60)
    engine.toggle()  # pause
    clock.advance(500)
    engine.toggle()  # resume
    assert engine.running
    assert events.log == [("open", "focus"), ("close",), ("open", "focus")]
    clock.advance(60)
    assert engine.display_time() == "23:00"


def test_focus_rolls_into_overtime_silently() -> None:
    engine, clock, events = make_engine()
    engine.toggle()
    clock.advance(25 * 60)
    assert engine.display_time() == "00:00"  # exactly at target: not overtime yet
    assert not engine.in_overtime
    clock.advance(151)
    assert engine.phase is Phase.FOCUS
    assert engine.in_overtime
    assert engine.display_time() == "+02:31"
    # no ringing, no segment close — still recording
    assert events.log == [("open", "focus")]


def test_reset_returns_to_idle_and_closes_running_segment() -> None:
    engine, clock, events = make_engine()
    engine.toggle()
    clock.advance(120)
    engine.reset()
    assert engine.phase is Phase.IDLE
    assert not engine.running
    assert engine.display_time() == "25:00"
    assert events.log == [("open", "focus"), ("close",)]


def test_reset_while_paused_does_not_double_close() -> None:
    engine, clock, events = make_engine()
    engine.toggle()
    clock.advance(60)
    engine.toggle()  # pause -> close
    engine.reset()
    assert events.log == [("open", "focus"), ("close",)]


def test_settings_change_applies_to_next_session_only() -> None:
    engine, clock, _ = make_engine()
    engine.toggle()
    engine.settings = Settings(focus_minutes=50)
    clock.advance(60)
    assert engine.display_time() == "24:00"  # running session keeps 25m target
    engine.reset()
    assert engine.display_time() == "50:00"  # idle display uses new settings
    engine.toggle()
    clock.advance(60)
    assert engine.display_time() == "49:00"  # next session uses new target
