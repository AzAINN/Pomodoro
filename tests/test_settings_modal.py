from __future__ import annotations

from pathlib import Path

from textual.widgets import Input

from pomo.app import PomoApp
from pomo.config import Settings
from pomo.screens.settings import SettingsModal
from pomo.store import Store
from tests.test_engine_focus import FakeClock


def make_app(tmp_path: Path, config_path: Path) -> PomoApp:
    app = PomoApp(
        settings=Settings(),
        store=Store(tmp_path / "pomo.db"),
        now=FakeClock(),
        notifier=lambda title, message: None,
    )
    app.config_file = config_path
    return app


async def test_s_opens_settings_modal(tmp_path: Path) -> None:
    app = make_app(tmp_path, tmp_path / "config.json")
    async with app.run_test() as pilot:
        await pilot.press("s")
        assert isinstance(app.screen, SettingsModal)
        await pilot.press("escape")
        assert not isinstance(app.screen, SettingsModal)


async def test_save_applies_and_persists_settings(tmp_path: Path) -> None:
    config_file = tmp_path / "config.json"
    app = make_app(tmp_path, config_file)
    async with app.run_test() as pilot:
        await pilot.press("s")
        app.screen.query_one("#focus", Input).value = "50"
        app.screen.query_one("#sound", Input).value = "Glass"
        await pilot.click("#save")
        assert app.settings.focus_minutes == 50
        assert app.engine.settings.focus_minutes == 50
        assert app.ringer.sound == "Glass"
        assert app.engine.display_time() == "50:00"
        assert config_file.exists()

        from pomo.config import load_settings

        assert load_settings(config_file).focus_minutes == 50


async def test_invalid_value_shows_error_and_keeps_modal_open(tmp_path: Path) -> None:
    app = make_app(tmp_path, tmp_path / "config.json")
    async with app.run_test() as pilot:
        await pilot.press("s")
        app.screen.query_one("#focus", Input).value = "0"
        await pilot.click("#save")
        assert isinstance(app.screen, SettingsModal)
        assert app.settings.focus_minutes == 25


async def test_cancel_changes_nothing(tmp_path: Path) -> None:
    config_file = tmp_path / "config.json"
    app = make_app(tmp_path, config_file)
    async with app.run_test() as pilot:
        await pilot.press("s")
        app.screen.query_one("#focus", Input).value = "50"
        await pilot.click("#cancel")
        assert app.settings.focus_minutes == 25
        assert not config_file.exists()


async def test_global_keys_ignored_while_modal_open(tmp_path: Path) -> None:
    from pomo.engine import Phase

    app = make_app(tmp_path, tmp_path / "config.json")
    async with app.run_test() as pilot:
        await pilot.press("s")
        await pilot.press("space")
        assert app.engine.phase is Phase.IDLE
