"""pomo — application shell wiring engine, store, sound, and screens."""
from __future__ import annotations

import time
from datetime import datetime
from typing import Callable

from textual import events
from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.widgets import Button, Footer, TabbedContent, TabPane

from pomo import sound
from pomo.config import Settings, db_path, load_settings
from pomo.engine import Phase, TimerEngine
from pomo.screens.calendar_tab import CalendarTab
from pomo.screens.timer_tab import TimerTab
from pomo.store import Store

HEARTBEAT_SECONDS = 30.0
BOUND_KEYS = ("space", "b", "r", "s", "tab", "q")


class PomoApp(App):
    CSS_PATH = "styles/app.tcss"
    TITLE = "pomo"

    BINDINGS = [
        Binding("space", "toggle", "Start/Pause", priority=True),
        Binding("b", "start_break", "Break"),
        Binding("r", "reset", "Reset"),
        Binding("tab", "switch_tab", "Timer/Calendar", priority=True),
        Binding("q", "quit", "Quit", priority=True),
    ]

    def __init__(
        self,
        settings: Settings | None = None,
        store: Store | None = None,
        ringer: sound.Ringer | None = None,
        now: Callable[[], float] | None = None,
        notifier: Callable[[str, str], None] = sound.notify,
    ) -> None:
        super().__init__()
        self.settings = settings or load_settings()
        self.store = store or Store(db_path())
        self.ringer = ringer or sound.Ringer(self.settings.sound)
        self._notifier = notifier
        self.engine = TimerEngine(self.settings, now=now or time.monotonic, events=self)
        self._segment_id: int | None = None

    def segment_opened(self, kind: str) -> None:
        try:
            self._segment_id = self.store.open_segment(kind, datetime.now())
        except Exception:
            self._segment_id = None
            self.notify("Could not write to the database", severity="error")

    def segment_closed(self) -> None:
        if self._segment_id is None:
            return
        try:
            self.store.close_segment(self._segment_id, datetime.now())
        except Exception:
            self.notify("Could not write to the database", severity="error")
        self._segment_id = None

    def ringing_started(self) -> None:
        self.ringer.start()
        self._notifier("pomo", "Break over — press any key to focus")

    def ringing_stopped(self) -> None:
        self.ringer.stop()

    def compose(self) -> ComposeResult:
        with TabbedContent(initial="timer-pane"):
            with TabPane("Timer", id="timer-pane"):
                yield TimerTab(self.engine, id="timer-tab")
            with TabPane("Calendar", id="calendar-pane"):
                yield CalendarTab(self.store, id="calendar-tab")
        yield Footer()

    def on_mount(self) -> None:
        self.set_interval(0.5, self._tick)
        self.set_interval(HEARTBEAT_SECONDS, self._heartbeat)
        self.set_interval(0.5, self._flash)

    def _tick(self) -> None:
        self.engine.tick()
        self._refresh_timer()

    def _refresh_timer(self) -> None:
        self.query_one(TimerTab).refresh_state()

    def _heartbeat(self) -> None:
        if self._segment_id is None:
            return
        try:
            self.store.heartbeat(self._segment_id, datetime.now())
        except Exception:
            pass

    def _flash(self) -> None:
        timer_tab = self.query_one(TimerTab)
        if self.engine.phase is Phase.RINGING:
            timer_tab.toggle_class("flash")
        else:
            timer_tab.remove_class("flash")

    def _modal_open(self) -> bool:
        return len(self.screen_stack) > 1

    def _dismiss_if_ringing(self) -> bool:
        if self.engine.phase is Phase.RINGING:
            self.engine.dismiss_ring()
            self._refresh_timer()
            return True
        return False

    def action_toggle(self) -> None:
        if self._modal_open():
            return
        self.engine.toggle()
        self._refresh_timer()

    def action_start_break(self) -> None:
        if self._modal_open() or self._dismiss_if_ringing():
            return
        self.engine.start_break()
        self._refresh_timer()

    def action_reset(self) -> None:
        if self._modal_open():
            return
        self.engine.reset()
        self._refresh_timer()

    def action_switch_tab(self) -> None:
        if self._modal_open() or self._dismiss_if_ringing():
            return
        tabs = self.query_one(TabbedContent)
        if tabs.active == "timer-pane":
            tabs.active = "calendar-pane"
        else:
            tabs.active = "timer-pane"
            self.query_one("#main-button", Button).focus()

    def action_quit(self) -> None:
        self.engine.reset()
        self.exit()

    def on_key(self, event: events.Key) -> None:
        """Any key without an app binding dismisses ringing into focus."""
        if self._modal_open() or event.key in BOUND_KEYS:
            return
        if self.engine.phase is Phase.RINGING:
            event.stop()
            event.prevent_default()
            self.engine.dismiss_ring()
            self._refresh_timer()

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "main-button":
            self.action_toggle()


def main() -> None:
    PomoApp().run()


if __name__ == "__main__":
    main()
