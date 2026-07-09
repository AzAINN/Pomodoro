from __future__ import annotations

import json
from dataclasses import asdict, dataclass, fields
from pathlib import Path

from platformdirs import user_config_dir, user_data_dir

APP_NAME = "pomo"


@dataclass
class Settings:
    focus_minutes: int = 25
    short_break_minutes: int = 5
    long_break_minutes: int = 15
    long_break_interval: int = 4
    sound: str = "Ping"


def config_path() -> Path:
    return Path(user_config_dir(APP_NAME)) / "config.json"


def db_path() -> Path:
    return Path(user_data_dir(APP_NAME)) / "pomo.db"


def load_settings(path: Path | None = None) -> Settings:
    """Load settings; any missing/corrupt/invalid field falls back to its default."""
    path = path or config_path()
    try:
        raw = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return Settings()
    if not isinstance(raw, dict):
        return Settings()

    settings = Settings()
    for field in fields(Settings):
        value = raw.get(field.name)
        if type(value) is not type(field.default):  # exact type: rejects bool for int
            continue
        if isinstance(value, int) and value < 1:
            continue
        setattr(settings, field.name, value)
    return settings


def save_settings(settings: Settings, path: Path | None = None) -> None:
    path = path or config_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(asdict(settings), indent=2) + "\n")
