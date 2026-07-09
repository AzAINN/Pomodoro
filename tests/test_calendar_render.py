from __future__ import annotations

from datetime import date, datetime, timedelta

from pomo.screens.calendar_tab import format_td, monday_of, render_week
from pomo.store import Segment

MONDAY = date(2026, 7, 6)


def seg(start: datetime, end: datetime) -> Segment:
    return Segment(id=0, kind="focus", started_at=start, ended_at=end)


def test_monday_of() -> None:
    assert monday_of(date(2026, 7, 9)) == MONDAY  # Thursday -> Monday
    assert monday_of(MONDAY) == MONDAY
    assert monday_of(date(2026, 7, 12)) == MONDAY  # Sunday -> same week's Monday


def test_format_td() -> None:
    assert format_td(timedelta(hours=14, minutes=30)) == "14h 30m"
    assert format_td(timedelta(minutes=5)) == "0h 05m"
    assert format_td(timedelta()) == "0h 00m"


def test_render_week_empty_uses_default_hours() -> None:
    out = render_week([], MONDAY, today=date(2026, 7, 9))
    lines = out.splitlines()
    assert "Mon 6" in lines[0]
    assert "Sun 12" in lines[0]
    assert "[reverse]" in lines[0]  # today highlighted
    assert lines[1].lstrip().startswith("9am")
    assert lines[-1].lstrip().startswith("5pm")
    assert "█" not in out


def test_render_week_draws_full_hour_block() -> None:
    segments = [seg(datetime(2026, 7, 6, 10, 0), datetime(2026, 7, 6, 11, 0))]
    out = render_week(segments, MONDAY, today=date(2026, 7, 9), cell_width=6)
    row_10am = next(line for line in out.splitlines() if line.lstrip().startswith("10am"))
    assert "│██████│" in row_10am  # Monday's cell completely filled


def test_render_week_partial_hour_partial_fill() -> None:
    segments = [seg(datetime(2026, 7, 6, 10, 0), datetime(2026, 7, 6, 10, 30))]
    out = render_week(segments, MONDAY, today=date(2026, 7, 9), cell_width=6)
    row_10am = next(line for line in out.splitlines() if line.lstrip().startswith("10am"))
    assert "│███   │" in row_10am  # half the hour -> half the cell


def test_render_week_tiny_coverage_shows_sliver() -> None:
    segments = [seg(datetime(2026, 7, 6, 10, 0), datetime(2026, 7, 6, 10, 1))]
    out = render_week(segments, MONDAY, today=date(2026, 7, 9), cell_width=6)
    row_10am = next(line for line in out.splitlines() if line.lstrip().startswith("10am"))
    assert "█" in row_10am  # never invisible


def test_render_week_hour_labels() -> None:
    segments = [seg(datetime(2026, 7, 6, 11, 0), datetime(2026, 7, 6, 13, 0))]
    out = render_week(segments, MONDAY, today=date(2026, 7, 9))
    labels = [line.split("│")[0].strip() for line in out.splitlines()[1:]]
    assert labels == ["10am", "11am", "12pm", "1pm"]


def test_render_week_columns_align() -> None:
    out = render_week([], MONDAY, today=date(2026, 7, 1))  # today outside week: no markup
    lines = out.splitlines()
    assert "[reverse]" not in lines[0]
    # every grid row has 8 pipes (7 cells + trailing)
    for row in lines[1:]:
        assert row.count("│") == 8
        assert len(row) == 6 + 7 * 8 + 1
