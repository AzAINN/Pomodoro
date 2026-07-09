"""The main timer view: mode pills, big clock, dots, one button."""
from __future__ import annotations

from textual.app import ComposeResult
from textual.containers import Center, Horizontal, Vertical
from textual.widgets import Button, Digits, Label, Static

from pomo.engine import LONG_BREAK, Phase, TimerEngine


class ModePills(Horizontal):
    def compose(self) -> ComposeResult:
        yield Label("FOCUS", id="pill-focus", classes="pill")
        yield Label("SHORT BREAK", id="pill-short", classes="pill")
        yield Label("LONG BREAK", id="pill-long", classes="pill")

    def set_mode(self, mode: str) -> None:
        for name in ("focus", "short", "long"):
            self.query_one(f"#pill-{name}", Label).set_class(name == mode, "active")


class TimerTab(Vertical):
    def __init__(self, engine: TimerEngine, **kwargs) -> None:
        super().__init__(**kwargs)
        self.engine = engine

    def compose(self) -> ComposeResult:
        with Center():
            yield ModePills()
        with Center():
            yield Digits("25:00", id="clock")
        with Center():
            yield Static("", id="dots")
        with Center():
            yield Button("START", id="main-button", variant="primary")
        yield Static("", id="ring-prompt")

    def on_mount(self) -> None:
        self.refresh_state()

    def refresh_state(self) -> None:
        engine = self.engine
        self.query_one("#clock", Digits).update(engine.display_time())

        filled, total = engine.dots()
        self.query_one("#dots", Static).update(
            " ".join(["●"] * filled + ["○"] * (total - filled))
        )
        self.query_one("#main-button", Button).label = self._button_label()

        mode = "focus"
        if engine.phase in (Phase.BREAK, Phase.RINGING):
            mode = "long" if engine.break_kind == LONG_BREAK else "short"
        self.query_one(ModePills).set_mode(mode)

        self.set_class(engine.phase is Phase.FOCUS and not engine.in_overtime, "-focus")
        self.set_class(engine.in_overtime, "-overtime")
        self.set_class(engine.phase is Phase.BREAK and mode == "short", "-short")
        self.set_class(engine.phase is Phase.BREAK and mode == "long", "-long")
        self.set_class(engine.phase is Phase.RINGING, "-ringing")

        prompt = "Break over — press any key to focus" if engine.phase is Phase.RINGING else ""
        self.query_one("#ring-prompt", Static).update(prompt)

    def _button_label(self) -> str:
        if self.engine.phase is Phase.IDLE:
            return "START"
        if self.engine.phase is Phase.RINGING:
            return "STOP RING"
        return "PAUSE" if self.engine.running else "RESUME"
