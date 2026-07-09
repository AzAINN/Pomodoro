"""Week-grid calendar of logged focus time, Google-Calendar style."""
from __future__ import annotations

from datetime import date, datetime, timedelta

from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Vertical, VerticalScroll
from textual.widgets import Static

from pomo.store import Segment, Store, day_total, hour_coverage, hour_range, week_total

CELL_WIDTH = 7
MIN_WIDTH = 6 + 7 * (CELL_WIDTH + 1) + 1  # 63 columns


def monday_of(day: date) -> date:
    return day - timedelta(days=day.weekday())


def format_td(td: timedelta) -> str:
    total_minutes = int(td.total_seconds() // 60)
    hours, minutes = divmod(total_minutes, 60)
    return f"{hours}h {minutes:02d}m"


def _hour_label(hour: int) -> str:
    if hour == 0:
        return "12am"
    if hour < 12:
        return f"{hour}am"
    if hour == 12:
        return "12pm"
    return f"{hour - 12}pm"


def render_week(
    segments: list[Segment], monday: date, today: date, cell_width: int = CELL_WIDTH
) -> str:
    """Pure text renderer: header + one row per hour. Rich markup is used only
    to highlight today's column header."""
    lo, hi = hour_range(segments, monday)
    days = [monday + timedelta(days=offset) for offset in range(7)]

    header = " " * 6
    for day in days:
        label = f"{day:%a} {day.day}"
        cell = f"{label:^{cell_width}}"
        if day == today:
            cell = f"[reverse]{cell}[/reverse]"
        header += " " + cell
    lines = [header]

    for hour in range(lo, hi + 1):
        row = f"{_hour_label(hour):>5} "
        for day in days:
            coverage = hour_coverage(segments, day, hour)
            filled = 0
            if coverage > 0:
                filled = min(cell_width, max(1, round(coverage / 3600 * cell_width)))
            row += "│" + "█" * filled + " " * (cell_width - filled)
        row += "│"
        lines.append(row)
    return "\n".join(lines)


class CalendarTab(Vertical):
    can_focus = True

    BINDINGS = [
        Binding("left", "prev_week", "Prev week"),
        Binding("right", "next_week", "Next week"),
        Binding("t", "this_week", "This week"),
    ]

    def __init__(self, store: Store, **kwargs) -> None:
        super().__init__(**kwargs)
        self.store = store
        self.monday = monday_of(date.today())

    def compose(self) -> ComposeResult:
        yield Static("", id="week-title")
        with VerticalScroll(id="grid-scroll"):
            yield Static("", id="week-grid")
        yield Static("", id="week-totals")

    def on_mount(self) -> None:
        self.refresh_week()

    def on_show(self) -> None:
        self.focus()
        self.refresh_week()

    def on_resize(self) -> None:
        self.refresh_week()

    def refresh_week(self) -> None:
        sunday = self.monday + timedelta(days=6)
        self.query_one("#week-title", Static).update(
            f"Week of {self.monday:%b} {self.monday.day} – {sunday:%b} {sunday.day}, {sunday.year}"
        )
        grid = self.query_one("#week-grid", Static)
        if self.size.width and self.size.width < MIN_WIDTH:
            grid.update(f"Terminal too narrow — widen to at least {MIN_WIDTH} columns")
            self.query_one("#week-totals", Static).update("")
            return
        segments = self.store.week_segments(self.monday)
        grid.update(render_week(segments, self.monday, date.today()))
        today = date.today()
        today_segments = self.store.week_segments(monday_of(today))
        self.query_one("#week-totals", Static).update(
            f"Week total: {format_td(week_total(segments, self.monday))}"
            f"        Today: {format_td(day_total(today_segments, today))}"
        )

    def action_prev_week(self) -> None:
        self.monday -= timedelta(days=7)
        self.refresh_week()

    def action_next_week(self) -> None:
        self.monday += timedelta(days=7)
        self.refresh_week()

    def action_this_week(self) -> None:
        self.monday = monday_of(date.today())
        self.refresh_week()
