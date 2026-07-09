"""SQLite segment log. One row per continuous run of focus/break time."""
from __future__ import annotations

import sqlite3
from dataclasses import dataclass
from datetime import date, datetime, time as dtime, timedelta
from pathlib import Path

_SCHEMA = """
CREATE TABLE IF NOT EXISTS sessions (
  id INTEGER PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('focus','short_break','long_break')),
  started_at TEXT NOT NULL,
  ended_at TEXT
);
"""


@dataclass
class Segment:
    id: int
    kind: str
    started_at: datetime
    ended_at: datetime | None


class Store:
    def __init__(self, db_path: Path | str) -> None:
        path = Path(db_path)
        path.parent.mkdir(parents=True, exist_ok=True)
        self._conn = sqlite3.connect(path)
        self._conn.execute(_SCHEMA)
        self._conn.commit()

    def open_segment(self, kind: str, at: datetime) -> int:
        """Insert a new segment. ended_at starts equal to started_at and is
        pushed forward by heartbeats, so a crash never leaves a NULL end."""
        cur = self._conn.execute(
            "INSERT INTO sessions (kind, started_at, ended_at) VALUES (?, ?, ?)",
            (kind, at.isoformat(), at.isoformat()),
        )
        self._conn.commit()
        return int(cur.lastrowid)

    def heartbeat(self, segment_id: int, at: datetime) -> None:
        self._conn.execute(
            "UPDATE sessions SET ended_at = ? WHERE id = ?",
            (at.isoformat(), segment_id),
        )
        self._conn.commit()

    def close_segment(self, segment_id: int, at: datetime) -> None:
        self.heartbeat(segment_id, at)

    def week_segments(self, monday: date, kind: str = "focus") -> list[Segment]:
        """Segments of `kind` overlapping [monday 00:00, monday+7d)."""
        start = datetime.combine(monday, dtime.min)
        end = start + timedelta(days=7)
        rows = self._conn.execute(
            "SELECT id, kind, started_at, ended_at FROM sessions"
            " WHERE kind = ? AND started_at < ? AND ended_at IS NOT NULL"
            " AND ended_at > ? ORDER BY started_at",
            (kind, end.isoformat(), start.isoformat()),
        ).fetchall()
        return [
            Segment(
                id=row[0],
                kind=row[1],
                started_at=datetime.fromisoformat(row[2]),
                ended_at=datetime.fromisoformat(row[3]) if row[3] else None,
            )
            for row in rows
        ]


def _clamp_to_day(seg: Segment, day: date) -> tuple[datetime, datetime] | None:
    """Portion of a closed segment that falls within `day`, or None."""
    if seg.ended_at is None:
        return None
    day_start = datetime.combine(day, dtime.min)
    day_end = day_start + timedelta(days=1)
    start = max(seg.started_at, day_start)
    end = min(seg.ended_at, day_end)
    if end <= start:
        return None
    return start, end


def hour_coverage(segments: list[Segment], day: date, hour: int) -> float:
    """Seconds of overlap between segments and [day hour:00, hour+1:00)."""
    cell_start = datetime.combine(day, dtime(hour=hour))
    cell_end = cell_start + timedelta(hours=1)
    total = 0.0
    for seg in segments:
        if seg.ended_at is None:
            continue
        start = max(seg.started_at, cell_start)
        end = min(seg.ended_at, cell_end)
        if end > start:
            total += (end - start).total_seconds()
    return total


def day_total(segments: list[Segment], day: date) -> timedelta:
    total = timedelta()
    for seg in segments:
        clamped = _clamp_to_day(seg, day)
        if clamped:
            total += clamped[1] - clamped[0]
    return total


def week_total(segments: list[Segment], monday: date) -> timedelta:
    return sum(
        (day_total(segments, monday + timedelta(days=d)) for d in range(7)),
        timedelta(),
    )


def hour_range(segments: list[Segment], monday: date) -> tuple[int, int]:
    """Inclusive (first, last) hour rows to draw, padded by one, default 9-17."""
    touched: list[int] = []
    for d in range(7):
        day = monday + timedelta(days=d)
        for seg in segments:
            clamped = _clamp_to_day(seg, day)
            if clamped is None:
                continue
            start, end = clamped
            # last touched hour: back off a microsecond so an end exactly on
            # the hour (or at midnight) does not count the next hour
            last = (end - timedelta(microseconds=1)).hour
            touched.append(start.hour)
            touched.append(max(last, start.hour))
    if not touched:
        return 9, 17
    return max(min(touched) - 1, 0), min(max(touched) + 1, 23)
