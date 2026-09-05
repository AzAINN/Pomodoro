"""Unix PTY integration check. Uses temporary data and never plays sound.

Run after cargo build: python3 scripts/smoke_tui.py [path/to/pomo]
"""
from __future__ import annotations

import fcntl
import errno
import json
import os
from pathlib import Path
import pty
import select
import signal
import sqlite3
import struct
import subprocess
import sys
import tempfile
import termios
import time


class Terminal:
    def __init__(self, binary: Path, data: Path):
        self.master, self.slave = pty.openpty()
        self.before = termios.tcgetattr(self.slave)
        self.resize(100, 32)

        def setup():
            os.setsid()
            fcntl.ioctl(self.slave, termios.TIOCSCTTY, 0)

        self.process = subprocess.Popen(
            [str(binary), "--data-dir", str(data)], stdin=self.slave,
            stdout=self.slave, stderr=self.slave,
            env={**os.environ, "TERM": "xterm-256color"}, preexec_fn=setup,
        )
        self.output = bytearray()

    def resize(self, width: int, height: int):
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))

    def pump(self, duration: float = 0.1):
        deadline = time.monotonic() + duration
        while time.monotonic() < deadline:
            if select.select([self.master], [], [], min(0.05, max(0, deadline - time.monotonic())))[0]:
                try:
                    chunk = os.read(self.master, 65536)
                except OSError as error:
                    if error.errno == errno.EIO:
                        return
                    raise
                if not chunk:
                    return
                self.output.extend(chunk)

    def send(self, keys: bytes):
        os.write(self.master, keys)
        self.pump()

    def wait(self, predicate, description: str):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            self.pump()
            try:
                if predicate():
                    return
            except (sqlite3.OperationalError, TypeError, KeyError):
                pass
            if self.process.poll() is not None:
                break
        raise AssertionError(f"Timed out: {description}\n{self.output[-1500:]!r}")

    def close(self):
        if self.process.poll() is None:
            self.process.send_signal(signal.SIGCONT)
            self.process.send_signal(signal.SIGTERM)
            deadline = time.monotonic() + 5
            while self.process.poll() is None and time.monotonic() < deadline:
                self.pump()
            if self.process.poll() is None:
                self.process.kill()
                self.process.wait()
                raise AssertionError(f"Process did not exit on SIGTERM: {self.output[-2000:]!r}")
        assert self.process.returncode == 0, self.output[-1500:]
        self.pump()
        assert b"\x1b[?1049l" in self.output, "Alternate screen was not restored"
        try:
            after = termios.tcgetattr(self.slave)
            assert after == self.before, "Terminal modes were not restored"
        except termios.error as error:
            # macOS revokes a controlling PTY when its session leader exits.
            if error.args[0] != errno.ENOTTY:
                raise
        os.close(self.master)
        os.close(self.slave)


def snapshot(data: Path):
    with sqlite3.connect(data / "pomo.db") as conn:
        row = conn.execute("SELECT snapshot FROM pomo_state WHERE id=1").fetchone()
    return json.loads(row[0])


def main():
    binary = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/pomo").resolve()
    with tempfile.TemporaryDirectory(prefix="pomo-smoke-") as temp:
        data = Path(temp)
        (data / "config.json").write_text(json.dumps({"alert": "Off"}))
        terminal = Terminal(binary, data)
        try:
            terminal.wait(lambda: b"ready to focus" in terminal.output, "initial timer")
            terminal.send(b"\x1b[<0;50;21M")  # click the centered Start action
            terminal.wait(lambda: snapshot(data)["elapsed_ms"] >= 1000, "running heartbeat")
            terminal.send(b" ")
            assert not snapshot(data)["running"]
            paused = snapshot(data)["elapsed_ms"]
            terminal.send(b"s50\r")
            assert json.loads((data / "config.json").read_text())["focus_minutes"] == 50
            assert snapshot(data)["target_ms"] == 1_500_000
            terminal.send(b"\x1b[<0;55;1M")  # click Calendar in the header
            terminal.wait(lambda: b"focus time only" in terminal.output, "calendar")
            terminal.resize(40, 16)
            terminal.process.send_signal(signal.SIGWINCH)
            terminal.pump()
            terminal.send(b"\r\x1b")  # day details then week
            terminal.send(b"\t ")  # timer, resume
            terminal.wait(lambda: snapshot(data)["running"], "resumed timer")
            terminal.process.send_signal(signal.SIGSTOP)
            terminal.pump(5.3)
            terminal.process.send_signal(signal.SIGCONT)
            terminal.wait(lambda: not snapshot(data)["running"], "pause after stopped process")
            assert snapshot(data)["elapsed_ms"] < paused + 1500, "Suspended time counted as focus"
            saved = snapshot(data)["elapsed_ms"]
        finally:
            terminal.close()
        restored = Terminal(binary, data)
        try:
            restored.wait(lambda: b"paused" in restored.output, "paused recovery")
            assert snapshot(data)["elapsed_ms"] == saved
            restored.send(b"q")
            restored.process.wait(timeout=5)
        finally:
            restored.close()

        # A restored break stops exactly at its deadline and does not auto-start focus.
        with sqlite3.connect(data / "pomo.db") as conn:
            state = {"phase": "ShortBreak", "running": False, "elapsed_ms": 0,
                     "target_ms": 1000, "completed": 2, "last_break_long": False}
            conn.execute("UPDATE pomo_state SET snapshot=? WHERE id=1", (json.dumps(state),))
        ready = Terminal(binary, data)
        try:
            ready.wait(lambda: b"paused" in ready.output, "restored break")
            ready.send(b" ")
            ready.wait(lambda: snapshot(data)["phase"] == "Ready", "break complete")
            ready.send(b"\tq")
            ready.process.wait(timeout=5)
            assert snapshot(data)["phase"] == "Ready"
            assert not snapshot(data)["running"]
            assert b"\x07" not in ready.output, "Off alert made a sound"
        finally:
            ready.close()
    print("PTY smoke passed: keys, mouse, resize, settings, live history, sleep, restart, break end, SIGTERM, terminal restoration.")


if __name__ == "__main__":
    main()
