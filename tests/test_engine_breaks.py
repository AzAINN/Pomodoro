from __future__ import annotations

from pomo.config import Settings
from pomo.engine import LONG_BREAK, SHORT_BREAK, Phase, TimerEngine
from tests.test_engine_focus import FakeClock, SpyEvents


def make_engine(settings: Settings | None = None) -> tuple[TimerEngine, FakeClock, SpyEvents]:
    clock = FakeClock()
    events = SpyEvents()
    engine = TimerEngine(settings or Settings(), now=clock, events=events)
    return engine, clock, events


def start_focus_then_break(engine: TimerEngine, clock: FakeClock) -> None:
    engine.toggle()
    clock.advance(25 * 60)
    engine.start_break()


def test_start_break_closes_focus_and_opens_short_break() -> None:
    engine, clock, events = make_engine()
    start_focus_then_break(engine, clock)
    assert engine.phase is Phase.BREAK
    assert engine.break_kind == SHORT_BREAK
    assert engine.completed_pomodoros == 1
    assert engine.display_time() == "05:00"
    assert events.log == [("open", "focus"), ("close",), ("open", "short_break")]


def test_start_break_ignored_outside_focus() -> None:
    engine, _, events = make_engine()
    engine.start_break()  # IDLE: no-op
    assert engine.phase is Phase.IDLE
    assert events.log == []


def test_start_break_from_paused_focus_does_not_double_close() -> None:
    engine, clock, events = make_engine()
    engine.toggle()
    clock.advance(60)
    engine.toggle()  # pause -> close
    engine.start_break()
    assert engine.phase is Phase.BREAK
    assert events.log == [
        ("open", "focus"),
        ("close",),
        ("open", "short_break"),
    ]


def test_every_fourth_break_is_long() -> None:
    engine, clock, _ = make_engine(Settings(long_break_interval=4))
    for n in range(1, 5):
        start_focus_then_break(engine, clock)
        expected = LONG_BREAK if n == 4 else SHORT_BREAK
        assert engine.break_kind == expected, f"pomodoro {n}"
        engine.reset()
    assert engine.completed_pomodoros == 4


def test_long_break_uses_long_duration() -> None:
    engine, clock, _ = make_engine(Settings(long_break_interval=1, long_break_minutes=15))
    start_focus_then_break(engine, clock)
    assert engine.break_kind == LONG_BREAK
    assert engine.display_time() == "15:00"


def test_break_end_starts_ringing_and_closes_segment() -> None:
    engine, clock, events = make_engine()
    start_focus_then_break(engine, clock)
    clock.advance(5 * 60 - 1)
    engine.tick()
    assert engine.phase is Phase.BREAK  # not yet
    clock.advance(2)
    engine.tick()
    assert engine.phase is Phase.RINGING
    assert engine.display_time() == "00:00"
    assert events.log[-2:] == [("close",), ("ring_on",)]


def test_tick_is_idempotent_while_ringing() -> None:
    engine, clock, events = make_engine()
    start_focus_then_break(engine, clock)
    clock.advance(5 * 60 + 1)
    engine.tick()
    engine.tick()
    engine.tick()
    assert events.log.count(("ring_on",)) == 1


def test_dismiss_ring_stops_ring_and_starts_focus() -> None:
    engine, clock, events = make_engine()
    start_focus_then_break(engine, clock)
    clock.advance(5 * 60 + 1)
    engine.tick()
    engine.dismiss_ring()
    assert engine.phase is Phase.FOCUS
    assert engine.running
    assert engine.display_time() == "25:00"
    assert events.log[-2:] == [("ring_off",), ("open", "focus")]


def test_toggle_during_ringing_dismisses() -> None:
    engine, clock, _ = make_engine()
    start_focus_then_break(engine, clock)
    clock.advance(5 * 60 + 1)
    engine.tick()
    engine.toggle()
    assert engine.phase is Phase.FOCUS


def test_reset_during_ringing_stops_ring_without_extra_close() -> None:
    engine, clock, events = make_engine()
    start_focus_then_break(engine, clock)
    clock.advance(5 * 60 + 1)
    engine.tick()
    engine.reset()
    assert engine.phase is Phase.IDLE
    assert events.log[-2:] == [("ring_on",), ("ring_off",)]


def test_break_pause_and_resume_reopens_break_segment() -> None:
    engine, clock, events = make_engine()
    start_focus_then_break(engine, clock)
    clock.advance(60)
    engine.toggle()  # pause break
    assert not engine.running
    clock.advance(300)
    engine.toggle()  # resume break
    assert engine.display_time() == "04:00"
    assert events.log[-2:] == [("close",), ("open", "short_break")]


def test_dots_progress_and_long_break_full() -> None:
    engine, clock, _ = make_engine(Settings(long_break_interval=4))
    assert engine.dots() == (0, 4)
    start_focus_then_break(engine, clock)  # 1 done
    assert engine.dots() == (1, 4)
    engine.reset()
    start_focus_then_break(engine, clock)
    engine.reset()
    start_focus_then_break(engine, clock)
    engine.reset()
    start_focus_then_break(engine, clock)  # 4th -> long break running
    assert engine.dots() == (4, 4)
    engine.reset()
    engine.toggle()  # next focus: cycle restarts
    assert engine.dots() == (0, 4)
