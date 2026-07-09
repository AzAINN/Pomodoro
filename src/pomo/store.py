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
