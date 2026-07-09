from __future__ import annotations

from datetime import date, datetime
from pathlib import Path

from pomo.store import Store


def make_store(tmp_path: Path) -> Store:
    return Store(tmp_path / "sub" / "pomo.db")


def test_open_segment_is_immediately_closed_at_start(tmp_path: Path) -> None:
    store = make_store(tmp_path)
    start = datetime(2026, 7, 9, 10, 0, 0)
    seg_id = store.open_segment("focus", start)
    [seg] = store.week_segments(date(2026, 7, 6))
    assert seg.id == seg_id
    assert seg.kind == "focus"
    assert seg.started_at == start
    assert seg.ended_at == start  # never NULL: crash loses <= one heartbeat


def test_heartbeat_and_close_update_ended_at(tmp_path: Path) -> None:
    store = make_store(tmp_path)
    start = datetime(2026, 7, 9, 10, 0, 0)
    seg_id = store.open_segment("focus", start)
    store.heartbeat(seg_id, datetime(2026, 7, 9, 10, 0, 30))
    [seg] = store.week_segments(date(2026, 7, 6))
    assert seg.ended_at == datetime(2026, 7, 9, 10, 0, 30)
    store.close_segment(seg_id, datetime(2026, 7, 9, 10, 25, 0))
    [seg] = store.week_segments(date(2026, 7, 6))
    assert seg.ended_at == datetime(2026, 7, 9, 10, 25, 0)


def test_week_segments_filters_kind_and_orders(tmp_path: Path) -> None:
    store = make_store(tmp_path)
    b = store.open_segment("focus", datetime(2026, 7, 8, 14, 0))
    store.close_segment(b, datetime(2026, 7, 8, 14, 25))
    a = store.open_segment("focus", datetime(2026, 7, 7, 9, 0))
    store.close_segment(a, datetime(2026, 7, 7, 9, 25))
    br = store.open_segment("short_break", datetime(2026, 7, 8, 14, 25))
    store.close_segment(br, datetime(2026, 7, 8, 14, 30))
    segs = store.week_segments(date(2026, 7, 6))
    assert [s.id for s in segs] == [a, b]
    breaks = store.week_segments(date(2026, 7, 6), kind="short_break")
    assert [s.id for s in breaks] == [br]


def test_week_segments_excludes_other_weeks_but_keeps_overlap(tmp_path: Path) -> None:
    store = make_store(tmp_path)
    prev = store.open_segment("focus", datetime(2026, 6, 29, 9, 0))
    store.close_segment(prev, datetime(2026, 6, 29, 9, 25))
    # crosses Sunday midnight into the queried week
    crossing = store.open_segment("focus", datetime(2026, 7, 5, 23, 30))
    store.close_segment(crossing, datetime(2026, 7, 6, 0, 30))
    inside = store.open_segment("focus", datetime(2026, 7, 9, 10, 0))
    store.close_segment(inside, datetime(2026, 7, 9, 10, 25))
    segs = store.week_segments(date(2026, 7, 6))
    assert [s.id for s in segs] == [crossing, inside]


def test_reopening_store_sees_existing_rows(tmp_path: Path) -> None:
    path = tmp_path / "pomo.db"
    store1 = Store(path)
    seg = store1.open_segment("focus", datetime(2026, 7, 9, 10, 0))
    store1.close_segment(seg, datetime(2026, 7, 9, 10, 25))
    store2 = Store(path)
    assert len(store2.week_segments(date(2026, 7, 6))) == 1
