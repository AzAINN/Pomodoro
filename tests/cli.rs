use std::{
    fs,
    process::{Command, Output},
};

fn run(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pomo"))
        .env("TZ", "UTC")
        .arg("--data-dir")
        .arg(dir)
        .args(args)
        .output()
        .unwrap()
}

fn legacy_fixture(dir: &std::path::Path) {
    let conn = rusqlite::Connection::open(dir.join("pomo.db")).unwrap();
    conn.execute_batch(
        "CREATE TABLE sessions(id INTEGER PRIMARY KEY,kind TEXT,started_at TEXT,ended_at TEXT);
        INSERT INTO sessions VALUES(1,'focus','2026-07-05T23:30:00','2026-07-06T00:30:00');
        INSERT INTO sessions VALUES(2,'focus','2026-07-06T00:00:00','2026-07-06T01:00:00');
        INSERT INTO sessions VALUES(3,'short_break','2026-07-06T01:00:00','2026-07-06T01:05:00');
        INSERT INTO sessions VALUES(4,'focus','2026-07-13T09:00:00','2026-07-13T09:25:00');",
    )
    .unwrap();
}

#[test]
fn help_and_noninteractive_launch_do_not_create_data() {
    let dir = tempfile::tempdir().unwrap();
    let help = run(dir.path(), &["--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("quiet focus"));
    let launch = run(dir.path(), &[]);
    assert!(!launch.status.success());
    assert!(String::from_utf8_lossy(&launch.stderr).contains("interactive terminal"));
    assert!(!dir.path().join("pomo.db").exists());
}

#[test]
fn stats_clamps_week_deduplicates_overlaps_and_excludes_breaks() {
    let dir = tempfile::tempdir().unwrap();
    legacy_fixture(dir.path());
    let result = run(dir.path(), &["stats", "--week", "2026-07-08"]);
    assert!(result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains("Week total  1h 00m"));
    let conn = rusqlite::Connection::open(dir.path().join("pomo.db")).unwrap();
    let tables: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(tables, 1); // report never creates runtime tables or migrations
}

#[test]
fn csv_is_clipped_to_week_and_never_overwrites_a_file() {
    let dir = tempfile::tempdir().unwrap();
    legacy_fixture(dir.path());
    let path = dir.path().join("focus.csv");
    let result = run(
        dir.path(),
        &[
            "export",
            "--week",
            "2026-07-06",
            "--output",
            path.to_str().unwrap(),
        ],
    );
    assert!(result.status.success());
    let csv = fs::read_to_string(&path).unwrap();
    assert_eq!(csv.lines().count(), 3);
    assert!(csv.contains("1,focus,2026-07-06T00:00:00+00:00,2026-07-06T00:30:00+00:00,1800.000"));
    assert!(
        !run(dir.path(), &["export", "--output", path.to_str().unwrap()])
            .status
            .success()
    );
    assert_eq!(fs::read_to_string(path).unwrap(), csv);
}

#[test]
fn invalid_options_fail_before_creating_files() {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        vec!["--week", "nonsense"],
        vec!["--week", "2026-09-05"],
        vec!["--output", "unexpected.csv"],
        vec!["surprise"],
    ] {
        assert!(!run(dir.path(), &args).status.success());
    }
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}
