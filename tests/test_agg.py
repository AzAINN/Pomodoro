from __future__ import annotations

from datetime import date, datetime, timedelta

from pomo.store import Segment, day_total, hour_coverage, hour_range, week_total

MONDAY = date(2026, 7, 6)


def seg(start: datetime, end: datetime, kind: str = "focus") -> Segment:
    return Segment(id=0, kind=kind, started_at=start, ended_at=end)


def test_hour_coverage_full_and_partial() -> None:
    segments = [seg(datetime(2026, 7, 6, 10, 0), datetime(2026, 7, 6, 11, 30))]
    assert hour_coverage(segments, date(2026, 7, 6), 10) == 3600.0
    assert hour_coverage(segments, date(2026, 7, 6), 11) == 1800.0
    assert hour_coverage(segments, date(2026, 7, 6), 12) == 0.0
    assert hour_coverage(segments, date(2026, 7, 7), 10) == 0.0


def test_hour_coverage_sums_multiple_segments() -> None:
    segments = [
        seg(datetime(2026, 7, 6, 10, 0), datetime(2026, 7, 6, 10, 10)),
        seg(datetime(2026, 7, 6, 10, 40), datetime(2026, 7, 6, 10, 50)),
    ]
    assert hour_coverage(segments, date(2026, 7, 6), 10) == 1200.0


def test_hour_coverage_ignores_open_segments() -> None:
    open_seg = Segment(id=1, kind="focus", started_at=datetime(2026, 7, 6, 10, 0), ended_at=None)
    assert hour_coverage([open_seg], date(2026, 7, 6), 10) == 0.0


def test_day_total_clamps_overnight_segments() -> None:
    segments = [seg(datetime(2026, 7, 6, 23, 0), datetime(2026, 7, 7, 1, 0))]
    assert day_total(segments, date(2026, 7, 6)) == timedelta(hours=1)
    assert day_total(segments, date(2026, 7, 7)) == timedelta(hours=1)
    assert day_total(segments, date(2026, 7, 8)) == timedelta()


def test_week_total_sums_days() -> None:
    segments = [
        seg(datetime(2026, 7, 6, 9, 0), datetime(2026, 7, 6, 10, 30)),
        seg(datetime(2026, 7, 9, 14, 0), datetime(2026, 7, 9, 15, 0)),
    ]
    assert week_total(segments, MONDAY) == timedelta(hours=2, minutes=30)


def test_hour_range_defaults_when_empty() -> None:
    assert hour_range([], MONDAY) == (9, 17)


def test_hour_range_pads_and_clamps() -> None:
    segments = [seg(datetime(2026, 7, 6, 9, 15), datetime(2026, 7, 6, 12, 0))]
    # touched hours 9..11 (end 12:00 exactly does not touch hour 12), padded -> 8..12
    assert hour_range(segments, MONDAY) == (8, 12)
    early = [seg(datetime(2026, 7, 6, 0, 10), datetime(2026, 7, 6, 0, 40))]
    assert hour_range(early, MONDAY) == (0, 1)
    late = [seg(datetime(2026, 7, 6, 22, 30), datetime(2026, 7, 7, 0, 0))]
    # touched hours 22..23, padded and clamped -> 21..23
    assert hour_range(late, MONDAY) == (21, 23)
