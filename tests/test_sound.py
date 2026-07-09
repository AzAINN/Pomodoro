from __future__ import annotations

import time

from pomo.sound import Ringer


def test_ringer_repeats_until_stopped() -> None:
    calls: list[str] = []
    ringer = Ringer(sound="Ping", interval=0.01, play=calls.append)
    ringer.start()
    time.sleep(0.08)
    ringer.stop()
    count_at_stop = len(calls)
    assert count_at_stop >= 3  # played repeatedly
    assert all(name == "Ping" for name in calls)
    time.sleep(0.05)
    assert len(calls) <= count_at_stop + 1  # at most one in-flight play after stop


def test_start_twice_runs_single_loop() -> None:
    calls: list[str] = []
    ringer = Ringer(interval=0.01, play=calls.append)
    ringer.start()
    ringer.start()
    time.sleep(0.05)
    ringer.stop()
    # a doubled loop would play ~2x per interval; allow generous headroom
    assert len(calls) <= 10


def test_stop_without_start_is_safe() -> None:
    Ringer(play=lambda name: None).stop()  # must not raise


def test_ringer_can_restart_after_stop() -> None:
    calls: list[str] = []
    ringer = Ringer(interval=0.01, play=calls.append)
    ringer.start()
    time.sleep(0.03)
    ringer.stop()
    first = len(calls)
    ringer.start()
    time.sleep(0.03)
    ringer.stop()
    assert len(calls) > first
