"""Break-end ringing. afplay system sounds on macOS, terminal bell elsewhere."""
from __future__ import annotations

import shutil
import subprocess
import sys
import threading
from pathlib import Path
from typing import Callable

SOUND_DIR = Path("/System/Library/Sounds")


def _play_once(sound: str) -> None:
    afplay = shutil.which("afplay")
    path = SOUND_DIR / f"{sound}.aiff"
    if afplay and path.exists():
        try:
            subprocess.run(
                [afplay, str(path)],
                check=False,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            return
        except OSError:
            pass
    try:
        sys.__stdout__.write("\a")
        sys.__stdout__.flush()
    except Exception:
        pass


def notify(title: str, message: str) -> None:
    """Post one macOS notification banner. No-op if osascript is unavailable."""
    osascript = shutil.which("osascript")
    if not osascript:
        return
    try:
        subprocess.run(
            [osascript, "-e", f'display notification "{message}" with title "{title}"'],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    except OSError:
        pass


class Ringer:
    """Repeats a sound until stopped. Daemon-thread based so it works from
    any context and never blocks app shutdown."""

    def __init__(
        self,
        sound: str = "Ping",
        interval: float = 2.0,
        play: Callable[[str], None] = _play_once,
    ) -> None:
        self.sound = sound
        self._interval = interval
        self._play = play
        self._stop_event = threading.Event()
        self._thread: threading.Thread | None = None

    def start(self) -> None:
        if self._thread is not None and self._thread.is_alive() and not self._stop_event.is_set():
            return
        self._stop_event.clear()
        self._thread = threading.Thread(target=self._loop, daemon=True)
        self._thread.start()

    def _loop(self) -> None:
        while not self._stop_event.is_set():
            self._play(self.sound)
            self._stop_event.wait(self._interval)

    def stop(self) -> None:
        self._stop_event.set()
