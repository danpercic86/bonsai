//! P91 §6.1/§6.2/§6.3 tests — the delete-all purge scope, the purge roll, and the
//! on-disk `truncate` record emitted when a part is evicted at the cap.
//!
//! Backend half of §12 row 7, items (a)-(k). (l) — `log_session_info` reporting
//! `dropped_parts` — is exercised through the sink in `tests_sink.rs`.

use std::path::Path;

use super::record::{LogLevel, LogPayload, LogRecord, LogSource, RedactionMode};
use super::writer::{list_log_files, purge_scope, Limits, LogWriter, WriterConfig};

fn test_redactor() -> std::sync::Arc<super::redact::Redactor> {
    std::sync::Arc::new(super::redact::Redactor::with_salt([7; 16]))
}

fn cfg(dir: &Path, started_secs: i64, limits: Limits) -> WriterConfig {
    WriterConfig {
        dir: dir.to_path_buf(),
        session_id: "sdeadbeef".into(),
        started_secs,
        app_version: "1.5.0".into(),
        os: "windows".into(),
        level: LogLevel::Debug,
        redaction: RedactionMode::Raw,
        home_mask: None,
        limits,
    }
}

fn rec(component: &str) -> LogRecord {
    LogRecord {
        seq: 0,
        ts: 1_772_200_991_000,
        mono: 12,
        src: LogSource::Ui,
        lvl: LogLevel::Debug,
        trace: Some("t1".into()),
        span: None,
        caused_by: None,
        payload: LogPayload::Render {
            component: component.into(),
            count: 1,
            since_ms: 4.0,
            changed_props: None,
        },
    }
}

fn lines(path: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path)
        .expect("read log file")
        .lines()
        .map(|l| serde_json::from_str(l).expect("valid JSON line"))
        .collect()
}

/// (a) Dev mode OFF ⇒ every in-scope file removed; the direct purge path
/// (`keep: None`) is what `logs_delete_all` runs when no writer exists.
/// (c) `deleted_files`/`deleted_bytes` EXACTLY match a pre-seeded fixture dir.
/// (e) nothing outside `logs/`+`exports/` is touched.
/// (h) a `.zip` in `logs/` is removed. (g) an export zip is removed + counted.
#[test]
fn purge_off_removes_every_in_scope_file_and_nothing_else() {
    let root = tempfile::tempdir().expect("tempdir");
    let logs = root.path().join("logs");
    let exports = root.path().join("exports");
    let metrics = root.path().join("metrics");
    std::fs::create_dir_all(&logs).unwrap();
    std::fs::create_dir_all(&exports).unwrap();
    std::fs::create_dir_all(&metrics).unwrap();

    // In scope: two log parts, a stray tmp, a stray zip in logs, an export zip.
    std::fs::write(logs.join("bonsai-a.jsonl"), vec![b'x'; 100]).unwrap();
    std::fs::write(logs.join("bonsai-a-1.jsonl"), vec![b'x'; 200]).unwrap();
    std::fs::write(logs.join("bonsai-a.jsonl.tmp"), vec![b'x'; 50]).unwrap();
    std::fs::write(logs.join("legacy.zip"), vec![b'x'; 300]).unwrap(); // (h)
    std::fs::write(exports.join("session.zip"), vec![b'x'; 400]).unwrap(); // (g)
                                                                           // Out of scope: must survive (e).
    std::fs::write(metrics.join("usage.json"), b"{}").unwrap();
    std::fs::write(root.path().join("settings.json"), b"{}").unwrap();
    std::fs::write(logs.join("notes.txt"), b"keep me").unwrap();

    let c = purge_scope(&logs, &exports, None);

    // (c) exact counts: 5 files, 100+200+50+300+400 = 1050 bytes.
    assert_eq!(c.deleted_files, 5, "5 in-scope files removed");
    assert_eq!(c.deleted_bytes, 1050);
    assert_eq!(c.failed_files, 0);
    // (g)+(h): both zips counted as exports.
    assert_eq!(c.deleted_exports, 2);

    // (e) survivors. NOTE (§F6): `logs_delete_all` DOES clear usage counts now —
    // but through `MetricsState::clear`, on the command's own blocking task.
    // `purge_scope` itself is still logs+exports only, and that separation is what
    // keeps the metrics delete out of the sink writer thread. The command-level
    // scope is covered by `tests_metrics_clear.rs`; inverting the assertion HERE
    // would instead require `purge_scope` to reach into `metrics/`, which §4.3
    // explicitly rejects.
    assert!(
        metrics.join("usage.json").exists(),
        "purge_scope alone leaves metrics/ — the metrics half is MetricsState::clear"
    );
    assert!(
        root.path().join("settings.json").exists(),
        "settings.json untouched"
    );
    assert!(
        logs.join("notes.txt").exists(),
        "a non-scope file in logs/ survives"
    );
    // Every in-scope file is gone.
    assert!(!logs.join("bonsai-a.jsonl").exists());
    assert!(!logs.join("bonsai-a-1.jsonl").exists());
    assert!(!logs.join("bonsai-a.jsonl.tmp").exists());
    assert!(!logs.join("legacy.zip").exists());
    assert!(!exports.join("session.zip").exists());
}

/// (i) a zip OUTSIDE both `logs/` and `exports/` is unreachable and survives.
#[test]
fn purge_never_touches_a_zip_outside_the_two_dirs() {
    let root = tempfile::tempdir().expect("tempdir");
    let logs = root.path().join("logs");
    let exports = root.path().join("exports");
    std::fs::create_dir_all(&logs).unwrap();
    std::fs::create_dir_all(&exports).unwrap();
    let outside = root.path().join("elsewhere.zip");
    std::fs::write(&outside, vec![b'x'; 10]).unwrap();

    let c = purge_scope(&logs, &exports, None);
    assert_eq!(c.deleted_files, 0);
    assert!(
        outside.exists(),
        "a zip the user saved elsewhere is not deleted"
    );
}

/// (d) a file that cannot be removed increments `failed_files`; the purge still
/// completes with partial counts (never fails). A DIRECTORY named `*.jsonl`
/// makes `remove_file` fail cross-platform without any share-mode gymnastics.
#[test]
fn purge_reports_an_undeletable_entry_as_failed_not_fatal() {
    let root = tempfile::tempdir().expect("tempdir");
    let logs = root.path().join("logs");
    let exports = root.path().join("exports");
    std::fs::create_dir_all(&logs).unwrap();
    std::fs::create_dir_all(&exports).unwrap();
    std::fs::create_dir_all(logs.join("stubborn.jsonl")).unwrap(); // a dir, not a file
    std::fs::write(logs.join("bonsai-a.jsonl"), vec![b'x'; 42]).unwrap();

    let c = purge_scope(&logs, &exports, None);
    assert_eq!(
        c.failed_files, 1,
        "the directory-as-jsonl could not be removed"
    );
    assert_eq!(c.deleted_files, 1, "the real part was still removed");
    assert_eq!(c.deleted_bytes, 42);
    assert!(
        logs.join("stubborn.jsonl").exists(),
        "the un-removable entry remains"
    );
}

/// (b) A purge roll opens a NEW file whose header carries `afterPurge: true`,
/// logging continues into it, and `keep`-scoped purge then deletes every OTHER
/// in-scope file — including the just-closed one — while sparing the new file.
/// (f) the delete writes NO record into the surviving file.
#[test]
fn roll_then_purge_keeps_only_the_fresh_file_with_an_after_purge_header() {
    let root = tempfile::tempdir().expect("tempdir");
    let logs = root.path().join("logs");
    let exports = root.path().join("exports");
    std::fs::create_dir_all(&exports).unwrap();
    let mut w = LogWriter::open(
        cfg(&logs, 1_772_200_991, Limits::default()),
        test_redactor(),
    )
    .expect("open");
    let old = w.active_file().to_string();
    w.write_record(rec("BeforeClick")).expect("write");
    w.flush().expect("flush");
    // A stray export the purge must also remove.
    std::fs::write(exports.join("old.zip"), vec![b'x'; 500]).unwrap();

    w.roll(true).expect("roll");
    let new = w.active_file().to_string();
    assert_ne!(old, new, "the roll opens a distinctly-named file");
    w.write_record(rec("AfterClick")).expect("write continues");
    w.flush().expect("flush");

    let c = purge_scope(&logs, &exports, Some(&new));
    // old part + old.zip removed; the new file spared.
    assert_eq!(c.deleted_files, 2);
    assert_eq!(c.deleted_exports, 1);
    assert!(!logs.join(&old).exists(), "the pre-click file is gone");
    assert!(logs.join(&new).exists(), "the fresh file survives");
    drop(w);

    let rows = lines(&logs.join(&new));
    assert_eq!(rows[0]["kind"], "session");
    assert_eq!(rows[0]["afterPurge"], true);
    // (f) nothing about the delete is recorded into the survivor; only the header
    // and the post-roll record are present.
    assert!(rows.iter().all(|r| r["kind"] != "truncate"));
    assert!(rows.iter().any(|r| r["component"] == "AfterClick"));
}

/// Same-second collision guard: rolling within the wall-clock second the session
/// started must still produce a new file name, or append-mode reopens the old
/// part-0 and pre-purge bytes survive the delete (§6.1 core guarantee).
#[test]
fn roll_within_the_same_second_still_gets_a_fresh_name() {
    let root = tempfile::tempdir().expect("tempdir");
    let logs = root.path().join("logs");
    // A far-future stamp forces `now_secs().max(started+1)` down the `+1` branch,
    // which is the exact case a naive `now_secs()` would collide on.
    let future = super::writer::now_secs() + 10_000;
    let mut w =
        LogWriter::open(cfg(&logs, future, Limits::default()), test_redactor()).expect("open");
    let old = w.active_file().to_string();
    w.write_record(rec("Secret")).expect("write");
    w.flush().expect("flush");
    w.roll(true).expect("roll");
    let new = w.active_file().to_string();
    assert_ne!(old, new, "collision guard must bump the stamp");
    drop(w);
    // The old file (still on disk after a bare roll) never received the new
    // header — its content is untouched and separable from the new file.
    let old_rows = lines(&logs.join(&old));
    assert!(old_rows.iter().any(|r| r["component"] == "Secret"));
    assert!(lines(&logs.join(&new))
        .iter()
        .all(|r| r["component"] != "Secret"));
}

/// (j) driving a writer past `max_parts` emits one `truncate` record per evicted
/// part into the surviving newest part, `dropped_parts` incrementing.
/// (k) every part header opened after an eviction carries `truncated: true` +
/// `dropped_parts`.
#[test]
fn eviction_emits_truncate_records_and_marks_later_headers_truncated() {
    let root = tempfile::tempdir().expect("tempdir");
    let logs = root.path().join("logs");
    let limits = Limits {
        part_bytes: 400,
        max_parts: 3,
        flush_bytes: 1,
        ..Limits::default()
    };
    let mut w = LogWriter::open(cfg(&logs, 1_772_200_991, limits), test_redactor()).expect("open");
    for i in 0..200 {
        w.write_record(rec(&format!("Component{i}")))
            .expect("write");
    }
    w.flush().expect("flush");
    drop(w);

    let files = list_log_files(&logs);
    assert_eq!(files.len(), 3, "bounded to max_parts: {files:?}");

    // Collect every record across surviving files in seq order.
    let mut all: Vec<serde_json::Value> = Vec::new();
    for (name, _) in &files {
        all.extend(lines(&logs.join(name)));
    }
    all.sort_by_key(|r| r["seq"].as_u64().unwrap_or(0));

    // (j) truncate records present, reason max-parts, dropped_parts strictly
    // increasing, dropped_part a redacted `part#N` label (never a path).
    let truncs: Vec<&serde_json::Value> = all.iter().filter(|r| r["kind"] == "truncate").collect();
    assert!(!truncs.is_empty(), "at least one eviction was recorded");
    let mut prev = 0u64;
    for t in &truncs {
        assert_eq!(t["reason"], "max-parts");
        let dp = t["droppedParts"]
            .as_u64()
            .expect("droppedParts is a number");
        assert!(
            dp > prev,
            "dropped_parts strictly increases: {dp} vs {prev}"
        );
        prev = dp;
        let label = t["droppedPart"].as_str().expect("droppedPart string");
        assert!(label.starts_with("part#") && !label.contains('/') && !label.contains('\\'));
        assert!(t["bytes"].as_u64().expect("bytes") > 0);
    }

    // (k) the newest surviving part opened after evictions ⇒ header truncated.
    let newest = lines(&logs.join(&files[2].0));
    assert_eq!(newest[0]["kind"], "session");
    assert_eq!(newest[0]["truncated"], true);
    assert!(newest[0]["droppedParts"].as_u64().unwrap_or(0) > 0);
    // The very first surviving part is one of the newest three, all opened well
    // after the first eviction, so each carries the truncation flag.
    for (name, _) in &files {
        let h = &lines(&logs.join(name))[0];
        assert_eq!(h["truncated"], true, "header of {name} marks truncation");
    }
}
