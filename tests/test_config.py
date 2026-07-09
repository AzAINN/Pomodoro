from __future__ import annotations

import json
from pathlib import Path

from pomo.config import Settings, load_settings, save_settings


def test_missing_file_returns_defaults(tmp_path: Path) -> None:
    settings = load_settings(tmp_path / "nope" / "config.json")
    assert settings == Settings()
    assert settings.focus_minutes == 25
    assert settings.short_break_minutes == 5
    assert settings.long_break_minutes == 15
    assert settings.long_break_interval == 4
    assert settings.sound == "Ping"


def test_save_then_load_roundtrip(tmp_path: Path) -> None:
    path = tmp_path / "cfg" / "config.json"
    custom = Settings(
        focus_minutes=50,
        short_break_minutes=10,
        long_break_minutes=30,
        long_break_interval=3,
        sound="Glass",
    )
    save_settings(custom, path)
    assert load_settings(path) == custom


def test_corrupt_json_returns_defaults(tmp_path: Path) -> None:
    path = tmp_path / "config.json"
    path.write_text("{not valid json")
    assert load_settings(path) == Settings()


def test_non_dict_json_returns_defaults(tmp_path: Path) -> None:
    path = tmp_path / "config.json"
    path.write_text(json.dumps([1, 2, 3]))
    assert load_settings(path) == Settings()


def test_invalid_values_fall_back_per_field(tmp_path: Path) -> None:
    path = tmp_path / "config.json"
    path.write_text(
        json.dumps(
            {
                "focus_minutes": 0,          # invalid: < 1 -> default 25
                "short_break_minutes": "x",  # wrong type -> default 5
                "long_break_minutes": 20,    # valid -> kept
                "long_break_interval": -2,   # invalid -> default 4
            }
        )
    )
    settings = load_settings(path)
    assert settings.focus_minutes == 25
    assert settings.short_break_minutes == 5
    assert settings.long_break_minutes == 20
    assert settings.long_break_interval == 4
    assert settings.sound == "Ping"
