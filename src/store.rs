//! Compatible with the original `sessions` table. UTC columns remove offset ambiguity
//! for new records; old local timestamps are interpreted in the current local zone.
use std::{fs, path::Path, time::Duration};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Days, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use rusqlite::{Connection, OptionalExtension, params};

use crate::engine::Engine;

#[derive(Clone, Debug)]
pub struct Segment {
    pub id: i64,
    pub kind: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

impl Segment {
    pub fn add_elapsed(&mut self, ms: u64) {
        self.end_ms = self.end_ms.saturating_add(ms.min(i64::MAX as u64) as i64);
    }
}

pub struct Store {
    conn: Connection,
    utc_columns: bool,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut conn = Connection::open(path).context("Could not open focus history")?;
        conn.busy_timeout(Duration::from_millis(500))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS sessions (
               id INTEGER PRIMARY KEY,
               kind TEXT NOT NULL CHECK (kind IN ('focus','short_break','long_break')),
               started_at TEXT NOT NULL,
               ended_at TEXT
             );
             CREATE TABLE IF NOT EXISTS pomo_state (
               id INTEGER PRIMARY KEY CHECK (id = 1), snapshot TEXT NOT NULL
             );",
        )?;
        let columns: Vec<String> = conn
            .prepare("PRAGMA table_info(sessions)")?
            .query_map([], |row| row.get(1))?
            .collect::<rusqlite::Result<_>>()?;
        let tx = conn.transaction()?;
        for name in ["started_unix_ms", "ended_unix_ms"] {
            if !columns.iter().any(|column| column == name) {
                tx.execute_batch(&format!("ALTER TABLE sessions ADD COLUMN {name} INTEGER;"))?;
            }
        }
        tx.execute_batch("CREATE INDEX IF NOT EXISTS sessions_focus_time ON sessions(kind, started_unix_ms, ended_unix_ms);")?;
        tx.commit()?;
        Ok(Self {
            conn,
            utc_columns: true,
        })
    }

    /// Reporting must work without acquiring the timer lock or migrating a database.
    pub fn read_only(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        conn.busy_timeout(Duration::from_millis(500))?;
        let columns: Vec<String> = conn
            .prepare("PRAGMA table_info(sessions)")?
            .query_map([], |row| row.get(1))?
            .collect::<rusqlite::Result<_>>()?;
        let utc_columns = ["started_unix_ms", "ended_unix_ms"]
            .iter()
            .all(|name| columns.iter().any(|column| column == name));
        Ok(Self { conn, utc_columns })
    }

    pub fn restore(&self) -> Result<Engine> {
        let snapshot: Option<String> = self
            .conn
            .query_row("SELECT snapshot FROM pomo_state WHERE id=1", [], |row| {
                row.get(0)
            })
            .optional()?;
        let mut engine: Engine = match snapshot {
            Some(snapshot) => serde_json::from_str(&snapshot)
                .context("Saved timer is unreadable; history is still intact")?,
            None => Engine::default(),
        };
        if engine.phase.kind().is_some() && (engine.target_ms == 0 || engine.target_ms > 86_400_000)
        {
            bail!("Saved timer has an invalid duration; history is still intact");
        }
        // Never charge the time between the last checkpoint and this launch.
        engine.running = false;
        Ok(engine)
    }

    /// Segment heartbeat and timer checkpoint commit together. The caller retains
    /// the previous active row if any statement fails, allowing a safe retry.
    pub fn checkpoint(
        &mut self,
        engine: &Engine,
        active: &mut Option<Segment>,
        now_ms: i64,
    ) -> Result<()> {
        let mut next_active = active.clone();
        let tx = self.conn.transaction()?;
        if let Some(segment) = &next_active {
            tx.execute(
                "UPDATE sessions SET ended_at=?1, ended_unix_ms=?2 WHERE id=?3",
                params![legacy_time(segment.end_ms)?, segment.end_ms, segment.id],
            )?;
        }
        let desired_kind = engine.phase.kind().filter(|_| engine.running);
        if next_active.as_ref().map(|segment| segment.kind.as_str()) != desired_kind {
            next_active = None;
            if let Some(kind) = desired_kind {
                let local = legacy_time(now_ms)?;
                tx.execute(
                    "INSERT INTO sessions(kind,started_at,ended_at,started_unix_ms,ended_unix_ms)
                     VALUES (?1,?2,?2,?3,?3)",
                    params![kind, local, now_ms],
                )?;
                next_active = Some(Segment {
                    id: tx.last_insert_rowid(),
                    kind: kind.into(),
                    start_ms: now_ms,
                    end_ms: now_ms,
                });
            }
        }
        tx.execute(
            "INSERT INTO pomo_state(id,snapshot) VALUES (1,?1)
             ON CONFLICT(id) DO UPDATE SET snapshot=excluded.snapshot",
            [serde_json::to_string(engine)?],
        )?;
        tx.commit()?;
        *active = next_active;
        Ok(())
    }

    pub fn range(&self, start_ms: i64, end_ms: i64) -> Result<Vec<Segment>> {
        let sql = if self.utc_columns {
            "SELECT id,kind,started_at,ended_at,started_unix_ms,ended_unix_ms FROM sessions
             WHERE kind='focus' AND ended_at IS NOT NULL
             AND (started_unix_ms IS NULL OR ended_unix_ms IS NULL
                  OR (started_unix_ms < ?2 AND ended_unix_ms > ?1))"
        } else {
            "SELECT id,kind,started_at,ended_at,NULL,NULL FROM sessions
             WHERE kind='focus' AND ended_at IS NOT NULL AND ?1 < ?2"
        };
        let mut statement = self.conn.prepare(sql)?;
        let mut rows = statement.query(params![start_ms, end_ms])?;
        let mut result = Vec::new();
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let start = row.get::<_, Option<i64>>(4)?.or_else(|| {
                row.get::<_, String>(2)
                    .ok()
                    .and_then(|s| parse_timestamp(&s))
            });
            let end = row.get::<_, Option<i64>>(5)?.or_else(|| {
                row.get::<_, String>(3)
                    .ok()
                    .and_then(|s| parse_timestamp(&s))
            });
            let (Some(start), Some(end)) = (start, end) else {
                bail!("History row {id} has an unreadable timestamp; no data was changed");
            };
            if start < end_ms && end > start_ms && end > start {
                result.push(Segment {
                    id,
                    kind: row.get(1)?,
                    start_ms: start,
                    end_ms: end,
                });
            }
        }
        result.sort_by_key(|segment| segment.start_ms);
        Ok(result)
    }
}

fn legacy_time(ms: i64) -> Result<String> {
    Ok(DateTime::<Utc>::from_timestamp_millis(ms)
        .context("Timestamp out of range")?
        .with_timezone(&Local)
        .format("%Y-%m-%dT%H:%M:%S%.6f")
        .to_string())
}

pub fn parse_timestamp(value: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(value)
        .map(|date| date.timestamp_millis())
        .ok()
        .or_else(|| {
            let naive = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f").ok()?;
            Local
                .from_local_datetime(&naive)
                .earliest()
                .map(|date| date.timestamp_millis())
        })
}

pub fn monday(date: NaiveDate) -> NaiveDate {
    use chrono::Datelike;
    date - Days::new(u64::from(date.weekday().num_days_from_monday()))
}

/// Resolve local boundaries separately; DST days are not necessarily 24 hours.
/// A skipped midnight resolves to the first valid minute in that local day.
pub fn local_boundary<T: TimeZone>(zone: &T, date: NaiveDate, hour: u32) -> Option<i64> {
    let naive = date.and_hms_opt(hour, 0, 0)?;
    (0..=180).find_map(|minute| {
        let candidate = naive + chrono::Duration::minutes(minute);
        zone.from_local_datetime(&candidate)
            .earliest()
            .map(|date| date.timestamp_millis())
    })
}

pub fn day_bounds(date: NaiveDate) -> (i64, i64) {
    (
        local_boundary(&Local, date, 0).expect("valid local day"),
        local_boundary(&Local, date + Days::new(1), 0).expect("valid next local day"),
    )
}

/// Repeated local half-hours have two intervals; skipped local times have none.
pub fn half_hour_intervals<T: TimeZone>(
    zone: &T,
    date: NaiveDate,
    hour: u32,
    minute: u32,
) -> Vec<(i64, i64)> {
    use chrono::LocalResult;
    let Some(naive) = date.and_hms_opt(hour, minute, 0) else {
        return Vec::new();
    };
    let starts = match zone.from_local_datetime(&naive) {
        LocalResult::Single(start) => vec![start.timestamp_millis()],
        LocalResult::Ambiguous(first, second) => {
            vec![first.timestamp_millis(), second.timestamp_millis()]
        }
        LocalResult::None => Vec::new(),
    };
    starts
        .into_iter()
        // Validate by converting back: some OS transition tables include a
        // candidate exactly at a DST boundary that belongs to the other hour.
        .filter(|start| {
            zone.timestamp_millis_opt(*start)
                .single()
                .is_some_and(|date| date.naive_local() == naive)
        })
        .map(|start| (start, start + 1_800_000))
        .collect()
}

/// Union intervals before summing: legacy overlapping timers cannot inflate totals.
pub fn coverage(segments: &[Segment], start_ms: i64, end_ms: i64) -> u64 {
    let mut ranges: Vec<_> = segments
        .iter()
        .filter(|segment| segment.kind == "focus")
        .map(|segment| (segment.start_ms.max(start_ms), segment.end_ms.min(end_ms)))
        .filter(|(start, end)| end > start)
        .collect();
    ranges.sort_unstable();
    let mut total = 0_u64;
    let mut merged: Option<(i64, i64)> = None;
    for (start, end) in ranges {
        match merged {
            Some((left, right)) if start <= right => merged = Some((left, right.max(end))),
            Some((left, right)) => {
                total += (right - left) as u64;
                merged = Some((start, end));
            }
            None => merged = Some((start, end)),
        }
    }
    if let Some((left, right)) = merged {
        total += (right - left) as u64;
    }
    total
}

pub fn duration_label(ms: u64) -> String {
    let seconds = ms / 1_000;
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        format!("{minutes}m")
    } else {
        format!("{}h {:02}m", minutes / 60, minutes % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Settings, engine::Phase};

    fn segment(start_ms: i64, end_ms: i64) -> Segment {
        Segment {
            id: 0,
            kind: "focus".into(),
            start_ms,
            end_ms,
        }
    }

    #[test]
    fn overlapping_paused_and_cross_midnight_time_is_counted_once() {
        let data = vec![
            segment(0, 100),
            segment(50, 150),
            segment(200, 250),
            Segment {
                kind: "short_break".into(),
                ..segment(150, 200)
            },
        ];
        assert_eq!(coverage(&data, 0, 250), 200);
        assert_eq!(coverage(&data, 75, 225), 100);
        assert_eq!(coverage(&data, 150, 200), 0);
    }

    #[test]
    fn migrates_legacy_database_without_changing_original_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pomo.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE sessions(id INTEGER PRIMARY KEY,kind TEXT NOT NULL,started_at TEXT NOT NULL,ended_at TEXT);
            INSERT INTO sessions VALUES(1,'focus','2026-07-06T23:30:00','2026-07-07T00:30:00');
            INSERT INTO sessions VALUES(2,'focus','2026-07-08T09:00:00',NULL);").unwrap();
        drop(conn);
        let store = Store::open(&path).unwrap();
        let day = NaiveDate::from_ymd_opt(2026, 7, 7).unwrap();
        let (start, end) = day_bounds(day);
        let records = store.range(start, end).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(coverage(&records, start, end), 1_800_000);
        assert_eq!(
            store
                .conn
                .query_row("SELECT started_at FROM sessions WHERE id=1", [], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap(),
            "2026-07-06T23:30:00"
        );
        drop(store);
        assert!(Store::open(&path).is_ok()); // migration is idempotent
    }

    #[test]
    fn checkpoint_restores_paused_with_exact_elapsed_and_cycle() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pomo.db");
        let mut store = Store::open(&path).unwrap();
        let mut engine = Engine {
            completed: 3,
            ..Engine::default()
        };
        let mut active = None;
        engine.toggle(&Settings::default());
        store.checkpoint(&engine, &mut active, 1_000_000).unwrap();
        active.as_mut().unwrap().add_elapsed(engine.advance(61_250));
        store.checkpoint(&engine, &mut active, 1_061_250).unwrap();
        drop(store); // simulate a crash, without closing the running timer
        let store = Store::open(&path).unwrap();
        let restored = store.restore().unwrap();
        assert_eq!(restored.phase, Phase::Focus);
        assert!(!restored.running);
        assert_eq!(restored.elapsed_ms, 61_250);
        assert_eq!(restored.completed, 3);
        assert_eq!(
            coverage(&store.range(0, 2_000_000).unwrap(), 0, 2_000_000),
            61_250
        );
    }

    #[test]
    fn resume_creates_a_gap_and_break_overrun_is_not_recorded() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("pomo.db")).unwrap();
        let settings = Settings::default();
        let mut engine = Engine::default();
        let mut active = None;
        engine.toggle(&settings);
        store.checkpoint(&engine, &mut active, 0).unwrap();
        active.as_mut().unwrap().add_elapsed(engine.advance(10_000));
        engine.toggle(&settings);
        store.checkpoint(&engine, &mut active, 10_000).unwrap();
        assert!(active.is_none());
        engine.toggle(&settings);
        store.checkpoint(&engine, &mut active, 60_000).unwrap();
        active.as_mut().unwrap().add_elapsed(engine.advance(10_000));
        engine.take_break(&settings);
        store.checkpoint(&engine, &mut active, 70_000).unwrap();
        active
            .as_mut()
            .unwrap()
            .add_elapsed(engine.advance(301_000));
        store.checkpoint(&engine, &mut active, 371_000).unwrap();
        assert!(active.is_none());
        assert_eq!(
            coverage(&store.range(-1, 500_000).unwrap(), -1, 500_000),
            20_000
        );
        let end: i64 = store
            .conn
            .query_row(
                "SELECT ended_unix_ms FROM sessions WHERE kind='short_break'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(end, 370_000);
    }

    #[test]
    fn rfc3339_offsets_preserve_real_duration_at_dst_fallback() {
        let start = parse_timestamp("2026-11-01T01:30:00-04:00").unwrap();
        let end = parse_timestamp("2026-11-01T01:30:00-05:00").unwrap();
        assert_eq!(end - start, 3_600_000);
    }

    #[test]
    fn half_hour_rows_cover_dst_days_exactly() {
        // Also run under TZ=America/New_York to exercise both 23- and 25-hour days.
        for (month, day) in [(3, 8), (11, 1)] {
            let date = NaiveDate::from_ymd_opt(2026, month, day).unwrap();
            let (start, end) = day_bounds(date);
            let total: i64 = (0..48)
                .flat_map(|slot| half_hour_intervals(&Local, date, slot / 2, slot % 2 * 30))
                .map(|(start, end)| end - start)
                .sum();
            assert_eq!(total, end - start);
            if let chrono::LocalResult::Ambiguous(_, second) =
                Local.from_local_datetime(&date.and_hms_opt(1, 0, 0).unwrap())
            {
                let row = segment(
                    second.timestamp_millis(),
                    second.timestamp_millis() + 600_000,
                );
                let covered = |minute| {
                    half_hour_intervals(&Local, date, 1, minute)
                        .iter()
                        .map(|(start, end)| coverage(std::slice::from_ref(&row), *start, *end))
                        .sum::<u64>()
                };
                assert_eq!(covered(0), 600_000);
                assert_eq!(covered(30), 0);
            }
        }
    }

    #[test]
    fn reports_read_legacy_data_without_migration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pomo.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions(id INTEGER PRIMARY KEY,kind TEXT,started_at TEXT,ended_at TEXT);
            INSERT INTO sessions VALUES(1,'focus','2026-07-06T09:00:00','2026-07-06T09:25:00');",
        )
        .unwrap();
        let store = Store::read_only(&path).unwrap();
        assert_eq!(store.range(0, i64::MAX).unwrap().len(), 1);
        let columns: i64 = conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('sessions')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(columns, 4);
    }

    #[test]
    fn failed_transaction_keeps_snapshot_and_segment_at_same_checkpoint() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("pomo.db")).unwrap();
        let mut engine = Engine::default();
        let mut active = None;
        engine.toggle(&Settings::default());
        store.checkpoint(&engine, &mut active, 0).unwrap();
        store.conn.execute_batch("CREATE TRIGGER fail_snapshot BEFORE UPDATE ON pomo_state BEGIN SELECT RAISE(FAIL,'test failure'); END;").unwrap();
        active.as_mut().unwrap().add_elapsed(engine.advance(1_000));
        assert!(store.checkpoint(&engine, &mut active, 1_000).is_err());
        assert_eq!(store.restore().unwrap().elapsed_ms, 0);
        let end: i64 = store
            .conn
            .query_row("SELECT ended_unix_ms FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(end, 0); // heartbeat update rolled back with the failed snapshot
    }
}
