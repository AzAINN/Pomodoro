"""Validated timer and sound settings modal."""
from __future__ import annotations

from textual.app import ComposeResult
from textual.containers import Grid, Horizontal
from textual.screen import ModalScreen
from textual.widgets import Button, Input, Label

from pomo.config import Settings


class SettingsModal(ModalScreen[Settings | None]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    def __init__(self, settings: Settings) -> None:
        super().__init__()
        self._settings = settings

    def compose(self) -> ComposeResult:
        with Grid(id="settings-grid"):
            yield Label("Settings", id="settings-title")
            yield Label("Focus (min)")
            yield Input(str(self._settings.focus_minutes), id="focus", type="integer")
            yield Label("Short break (min)")
            yield Input(str(self._settings.short_break_minutes), id="short", type="integer")
            yield Label("Long break (min)")
            yield Input(str(self._settings.long_break_minutes), id="long", type="integer")
            yield Label("Long break every")
            yield Input(str(self._settings.long_break_interval), id="interval", type="integer")
            yield Label("Sound")
            yield Input(self._settings.sound, id="sound")
            yield Label("", id="settings-error")
            with Horizontal(id="settings-buttons"):
                yield Button("Save", id="save", variant="primary")
                yield Button("Cancel", id="cancel")

    def on_button_pressed(self, event: Button.Pressed) -> None:
        event.stop()
        if event.button.id == "cancel":
            self.dismiss(None)
            return
        if event.button.id != "save":
            return

        try:
            new_settings = Settings(
                focus_minutes=self._read_int("focus"),
                short_break_minutes=self._read_int("short"),
                long_break_minutes=self._read_int("long"),
                long_break_interval=self._read_int("interval"),
                sound=self.query_one("#sound", Input).value.strip() or "Ping",
            )
        except ValueError:
            self.query_one("#settings-error", Label).update("All numbers must be ≥ 1")
            return
        self.dismiss(new_settings)

    def _read_int(self, input_id: str) -> int:
        value = int(self.query_one(f"#{input_id}", Input).value)
        if value < 1:
            raise ValueError(f"{input_id} must be >= 1")
        return value

    def action_cancel(self) -> None:
        self.dismiss(None)
