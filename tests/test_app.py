from __future__ import annotations

from pathlib import Path

from textual.widgets import Button, TabbedContent

from pomo.app import PomoApp
from pomo.config import Settings
from pomo.engine import Phase
from pomo.screens.calendar_tab import CalendarTab
from pomo.screens.timer_tab import TimerTab
from pomo.store import Store
from tests.test_engine_focus import FakeClock


class FakeRinger:
    def __init__(self) -> None:
        self.sound = "Ping"
        self.started = 0
        self.stopped = 0

    def start(self) -> None:
        self.started += 1

    def stop(self) -> None:
        self.stopped += 1


def make_app(tmp_path: Path) -> tuple[PomoApp, FakeClock, FakeRinger, Store]:
    clock = FakeClock()
    store = Store(tmp_path / "pomo.db")
    ringer = FakeRinger()
    app = PomoApp(
        settings=Settings(),
        store=store,
        ringer=ringer,  # type: ignore[arg-type]
        now=clock,
        notifier=lambda title, message: None,
    )
    return app, clock, ringer, store


def ring(app: PomoApp, clock: FakeClock) -> None:
    """Drive the engine to the RINGING state."""
    app.engine.toggle()
    app.engine.start_break()
    clock.advance(5 * 60 + 1)
    app.engine.tick()
    assert app.engine.phase is Phase.RINGING


async def test_boot_is_idle_with_start_button(tmp_path: Path) -> None:
    app, _, _, _ = make_app(tmp_path)
    async with app.run_test():
        assert app.engine.phase is Phase.IDLE
        assert str(app.query_one("#main-button", Button).label) == "START"
        timer = app.query_one(TimerTab)
        assert timer.has_class("-focus")
        assert timer.size.height > 0


async def test_space_starts_focus_and_logs_segment(tmp_path: Path) -> None:
    from datetime import date

    from pomo.screens.calendar_tab import monday_of

    app, _, _, store = make_app(tmp_path)
    async with app.run_test() as pilot:
        await pilot.press("space")
        assert app.engine.phase is Phase.FOCUS
        assert app.engine.running
        segments = store.week_segments(monday_of(date.today()))
        assert len(segments) == 1
        assert segments[0].kind == "focus"
        assert str(app.query_one("#main-button", Button).label) == "PAUSE"


async def test_space_again_pauses(tmp_path: Path) -> None:
    app, _, _, _ = make_app(tmp_path)
    async with app.run_test() as pilot:
        await pilot.press("space")
        await pilot.press("space")
        assert app.engine.phase is Phase.FOCUS
        assert not app.engine.running
        assert str(app.query_one("#main-button", Button).label) == "RESUME"


async def test_b_starts_break_and_logs_break_segment(tmp_path: Path) -> None:
    from datetime import date

    from pomo.screens.calendar_tab import monday_of

    app, _, _, store = make_app(tmp_path)
    async with app.run_test() as pilot:
        await pilot.press("space")
        await pilot.press("b")
        assert app.engine.phase is Phase.BREAK
        breaks = store.week_segments(monday_of(date.today()), kind="short_break")
        assert len(breaks) == 1


async def test_break_end_rings_and_any_key_starts_focus(tmp_path: Path) -> None:
    app, clock, ringer, _ = make_app(tmp_path)
    async with app.run_test() as pilot:
        ring(app, clock)
        assert ringer.started == 1
        await pilot.press("x")
        assert app.engine.phase is Phase.FOCUS
        assert app.engine.running
        assert ringer.stopped >= 1


async def test_b_during_ringing_dismisses_instead_of_breaking(tmp_path: Path) -> None:
    app, clock, ringer, _ = make_app(tmp_path)
    async with app.run_test() as pilot:
        ring(app, clock)
        pomodoros = app.engine.completed_pomodoros
        await pilot.press("b")
        assert app.engine.phase is Phase.FOCUS
        assert app.engine.completed_pomodoros == pomodoros
        assert ringer.stopped >= 1


async def test_r_resets_to_idle(tmp_path: Path) -> None:
    app, _, _, _ = make_app(tmp_path)
    async with app.run_test() as pilot:
        await pilot.press("space")
        await pilot.press("r")
        assert app.engine.phase is Phase.IDLE
        assert str(app.query_one("#main-button", Button).label) == "START"


async def test_tab_switches_between_timer_and_calendar(tmp_path: Path) -> None:
    app, _, _, _ = make_app(tmp_path)
    async with app.run_test() as pilot:
        tabs = app.query_one(TabbedContent)
        assert tabs.active == "timer-pane"
        await pilot.press("tab")
        assert tabs.active == "calendar-pane"
        assert app.query_one(CalendarTab).size.height > 0
        await pilot.press("tab")
        assert tabs.active == "timer-pane"


async def test_calendar_week_navigation(tmp_path: Path) -> None:
    from datetime import date, timedelta

    from pomo.screens.calendar_tab import CalendarTab, monday_of

    app, _, _, _ = make_app(tmp_path)
    async with app.run_test() as pilot:
        await pilot.press("tab")
        calendar = app.query_one(CalendarTab)
        this_monday = monday_of(date.today())
        await pilot.press("left")
        assert calendar.monday == this_monday - timedelta(days=7)
        await pilot.press("t")
        assert calendar.monday == this_monday


async def test_quit_closes_open_segment(tmp_path: Path) -> None:
    app, _, _, _ = make_app(tmp_path)
    async with app.run_test() as pilot:
        await pilot.press("space")
        await pilot.press("q")
    assert app.engine.phase is Phase.IDLE


async def test_external_shutdown_closes_open_segment(tmp_path: Path) -> None:
    app, _, _, _ = make_app(tmp_path)
    async with app.run_test() as pilot:
        await pilot.press("space")
        assert app.engine.running
    assert app.engine.phase is Phase.IDLE
    assert not app.engine.running


async def test_heartbeat_updates_open_segment(tmp_path: Path) -> None:
    from datetime import date

    from pomo.screens.calendar_tab import monday_of

    app, _, _, store = make_app(tmp_path)
    async with app.run_test() as pilot:
        await pilot.press("space")
        [before] = store.week_segments(monday_of(date.today()))
        app._heartbeat()
        [after] = store.week_segments(monday_of(date.today()))
        assert after.ended_at >= before.ended_at
