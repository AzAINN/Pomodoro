"""Pure pomodoro timer state machine. No Textual, no I/O.

Driven by commands (toggle, start_break, dismiss_ring, reset) plus a periodic
tick(). Reports lifecycle moments through an EngineEvents object so the caller
can persist segments and ring sounds. Time comes from an injected `now()`
callable returning seconds (monotonic), which makes the engine fully testable.
"""
from __future__ import annotations

import time
from enum import Enum
from typing import Callable, Protocol

from pomo.config import Settings


class Phase(Enum):
    IDLE = "idle"
    FOCUS = "focus"
    BREAK = "break"
    RINGING = "ringing"


FOCUS_KIND = "focus"
SHORT_BREAK = "short_break"
LONG_BREAK = "long_break"


class EngineEvents(Protocol):
    def segment_opened(self, kind: str) -> None: ...

    def segment_closed(self) -> None: ...

    def ringing_started(self) -> None: ...

    def ringing_stopped(self) -> None: ...


class NullEvents:
    def segment_opened(self, kind: str) -> None:
        pass

    def segment_closed(self) -> None:
        pass

    def ringing_started(self) -> None:
        pass

    def ringing_stopped(self) -> None:
        pass


def _mmss(seconds: float) -> str:
    minutes, secs = divmod(int(seconds), 60)
    return f"{minutes:02d}:{secs:02d}"


class TimerEngine:
    def __init__(
        self,
        settings: Settings,
        now: Callable[[], float] = time.monotonic,
        events: EngineEvents | None = None,
    ) -> None:
        self.settings = settings
        self._now = now
        self.events: EngineEvents = events or NullEvents()
        self.phase = Phase.IDLE
        self.break_kind = SHORT_BREAK  # kind of the current/most recent break
        self.completed_pomodoros = 0
        self._target = 0.0  # seconds for the current phase, captured at entry
        self._accum = 0.0  # seconds accumulated across previous run stretches
        self._run_started: float | None = None  # now() when running, else None

    # -- queries ----------------------------------------------------------

    @property
    def running(self) -> bool:
        return self._run_started is not None

    def elapsed(self) -> float:
        extra = (self._now() - self._run_started) if self._run_started is not None else 0.0
        return self._accum + extra

    def remaining(self) -> float:
        """Seconds left in the current phase; negative means focus overtime."""
        return self._target - self.elapsed()

    @property
    def in_overtime(self) -> bool:
        return self.phase is Phase.FOCUS and self.remaining() < 0

    def display_time(self) -> str:
        if self.phase is Phase.IDLE:
            return _mmss(self.settings.focus_minutes * 60)
        if self.phase is Phase.RINGING:
            return "00:00"
        rem = self.remaining()
        if self.phase is Phase.FOCUS and rem < 0:
            return "+" + _mmss(-rem)
        return _mmss(max(rem, 0))

    def dots(self) -> tuple[int, int]:
        """(filled, total) pomodoro progress toward the long break."""
        interval = self.settings.long_break_interval
        filled = self.completed_pomodoros % interval
        if (
            self.completed_pomodoros > 0
            and filled == 0
            and self.phase in (Phase.BREAK, Phase.RINGING)
        ):
            filled = interval
        return filled, interval

    # -- commands ----------------------------------------------------------

    def toggle(self) -> None:
        """Space / main button: start, pause, resume, or dismiss ringing."""
        if self.phase is Phase.IDLE:
            self._enter_focus()
        elif self.phase is Phase.RINGING:
            self.dismiss_ring()
        elif self.running:
            self._pause()
        else:
            self._resume()

    def start_break(self) -> None:
        """End the focus session (b key / button) and start the due break."""
        if self.phase is not Phase.FOCUS:
            return
        if self.running:
            self._run_started = None
            self.events.segment_closed()
        self.completed_pomodoros += 1
        long_due = self.completed_pomodoros % self.settings.long_break_interval == 0
        self.break_kind = LONG_BREAK if long_due else SHORT_BREAK
        minutes = (
            self.settings.long_break_minutes if long_due else self.settings.short_break_minutes
        )
        self._enter_phase(Phase.BREAK, minutes * 60, self.break_kind)

    def dismiss_ring(self) -> None:
        """Any key while ringing: silence and immediately start the next focus."""
        if self.phase is not Phase.RINGING:
            return
        self.events.ringing_stopped()
        self._enter_focus()

    def reset(self) -> None:
        if self.phase is Phase.RINGING:
            self.events.ringing_stopped()
        elif self.running:
            self._run_started = None
            self.events.segment_closed()
        self.phase = Phase.IDLE
        self._accum = 0.0
        self._target = 0.0

    def tick(self) -> None:
        """Advance time-driven transitions. Call at least once per second."""
        if self.phase is Phase.BREAK and self.running and self.remaining() <= 0:
            self._accum = self._target
            self._run_started = None
            self.phase = Phase.RINGING
            self.events.segment_closed()
            self.events.ringing_started()

    # -- internals ----------------------------------------------------------

    def _enter_focus(self) -> None:
        self._enter_phase(Phase.FOCUS, self.settings.focus_minutes * 60, FOCUS_KIND)

    def _enter_phase(self, phase: Phase, target: float, kind: str) -> None:
        self.phase = phase
        self._target = target
        self._accum = 0.0
        self._run_started = self._now()
        self.events.segment_opened(kind)

    def _pause(self) -> None:
        self._accum = self.elapsed()
        self._run_started = None
        self.events.segment_closed()

    def _resume(self) -> None:
        self._run_started = self._now()
        kind = self.break_kind if self.phase is Phase.BREAK else FOCUS_KIND
        self.events.segment_opened(kind)
