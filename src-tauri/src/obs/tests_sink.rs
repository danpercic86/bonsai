//! P91 §6/§11 sink tests — zero loss on exit, non-blocking backpressure with a
//! `drop` record, and the §6 decision-4 guarantee that stopping Dev mode deletes
//! nothing.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::record::{LogLevel, LogPayload, LogRecord, LogSource, RedactionMode};
use super::redact::Redactor;
use super::sink::Sink;
use super::writer::{list_log_files, part_name, Limits, LogWriter, WriterConfig};
use super::ObsState;

fn cfg(dir: &Path) -> WriterConfig {
    WriterConfig {
        dir: dir.to_path_buf(),
        session_id: "sfeedface".into(),
        started_secs: 1_787_839_391,
        app_version: "1.5.0".into(),
        os: "windows".into(),
        level: LogLevel::Debug,
        redaction: RedactionMode::Strict,
        home_mask: None,
        limits: Limits::default(),
    }
}

fn rec(n: u64) -> LogRecord {
    LogRecord {
        seq: 0,
        ts: 1_787_839_391_000,
        mono: n,
        src: LogSource::Ui,
        lvl: LogLevel::Debug,
        trace: None,
        span: None,
        caused_by: None,
        repo: None,
        payload: LogPayload::Gesture {
            origin: "click".into(),
            gesture: format!("test.{n}"),
        },
    }
}

fn all_lines(dir: &Path) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for (name, _) in list_log_files(dir) {
        let text = std::fs::read_to_string(dir.join(name)).expect("read log part");
        for line in text.lines() {
            out.push(serde_json::from_str(line).expect("valid JSON line"));
        }
    }
    out
}

/// §12 row 1: "exit flush loses 0 records".
#[test]
fn shutdown_flushes_every_accepted_record() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sink = Sink::start(cfg(dir.path())).expect("start");
    for i in 0..500 {
        sink.enqueue(rec(i));
    }
    assert_eq!(sink.dropped(), 0, "500 records fit the 4096-deep queue");
    sink.shutdown();

    let rows = all_lines(dir.path());
    assert_eq!(rows.len(), 501, "one session header + 500 records");
    assert_eq!(rows[0]["kind"], "session");
    // seq is dense and monotonic — the ordering guarantee of §3.
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row["seq"], (i as u64) + 1);
    }
    for i in 0..500u64 {
        assert_eq!(rows[(i as usize) + 1]["gesture"], format!("test.{i}"));
    }
}

/// §11: "never blocks a caller (bounded `try_send`)". Producers must return
/// immediately and every record must be either written or counted — never
/// silently lost and never waited on.
#[test]
fn producers_never_block_and_overflow_is_accounted_for() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sink = Sink::start(cfg(dir.path())).expect("start");
    const N: u64 = 40_000;
    let start = std::time::Instant::now();
    for i in 0..N {
        sink.enqueue(rec(i));
    }
    let elapsed = start.elapsed();
    assert_eq!(
        sink.accepted() + sink.dropped(),
        N,
        "every record is either queued or counted as dropped"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(20),
        "enqueue applied backpressure to the caller: {elapsed:?}"
    );
    sink.shutdown();

    let rows = all_lines(dir.path());
    let dropped_total: u64 = rows
        .iter()
        .filter(|r| r["kind"] == "drop")
        .map(|r| r["dropped"].as_u64().unwrap_or(0))
        .sum();
    assert_eq!(
        dropped_total,
        sink.dropped(),
        "every dropped record is reported by a `drop` record"
    );
}

/// The `drop` record itself, driven deterministically (a live sink may or may
/// not overflow depending on the machine, so the mechanism is tested directly).
#[test]
fn drop_records_report_the_backpressure_gap() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut w = LogWriter::open(
        cfg(dir.path()),
        std::sync::Arc::new(super::redact::Redactor::with_salt([3; 16])),
    )
    .expect("open");
    let name = w.active_file().to_string();
    w.write_record(rec(1)).expect("write");
    let dropped = AtomicU64::new(7);
    let mut reported = 0;
    super::sink::emit_pending_drops(&mut w, &dropped, &mut reported);
    // Nothing new to report the second time — no duplicate record.
    super::sink::emit_pending_drops(&mut w, &dropped, &mut reported);
    dropped.store(9, Ordering::Relaxed);
    super::sink::emit_pending_drops(&mut w, &dropped, &mut reported);
    w.flush().expect("flush");
    drop(w);

    let text = std::fs::read_to_string(dir.path().join(name)).expect("read");
    let drops: Vec<serde_json::Value> = text
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).expect("json"))
        .filter(|r| r["kind"] == "drop")
        .collect();
    assert_eq!(drops.len(), 2, "one record per reporting gap: {drops:?}");
    assert_eq!(drops[0]["dropped"], 7);
    assert_eq!(
        drops[0]["sinceSeq"], 2,
        "the last seq written before the gap"
    );
    assert_eq!(drops[1]["dropped"], 2);
}

/// §6 decision 4 + §12 row 1: "turning Dev mode off deletes no file".
#[test]
fn stopping_the_sink_deletes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    // A file from an EARLIER session, plus a non-log sibling.
    std::fs::write(
        dir.path()
            .join("bonsai-2026-08-01T10-00-00-s00000001.jsonl"),
        "{}\n",
    )
    .expect("seed");
    std::fs::write(dir.path().join("settings-like.json"), "{}").expect("seed");

    let state = ObsState::from_sink(Sink::start(cfg(dir.path())).expect("start"));
    for i in 0..10 {
        state.sink().expect("sink").enqueue(rec(i));
    }
    super::stop(&state);

    assert!(!state.is_enabled(), "the session is closed");
    let left = list_log_files(dir.path());
    assert_eq!(left.len(), 2, "both log files survive: {left:?}");
    assert!(dir.path().join("settings-like.json").exists());
    // And the just-closed session kept its records (flushed, not truncated).
    let rows = all_lines(dir.path());
    assert_eq!(rows.iter().filter(|r| r["kind"] == "gesture").count(), 10);
}

/// §7.2 / §12 row 1 — **the salt is never written to any file or export.**
///
/// It is the one secret in the system: it seeds the ordinal counters and the
/// `argsHash`, so a salt on disk next to the file it protects would make both
/// mechanisms decorative. The test writes records that deliberately CONTAIN the
/// salt (a hostile/buggy producer echoing it back) and then greps every byte the
/// session produced, log parts and export zip alike.
#[test]
fn the_session_salt_never_reaches_disk() {
    let dir = tempfile::tempdir().expect("tempdir");
    let logs = dir.path().join("logs");
    let exports = dir.path().join("exports");
    std::fs::create_dir_all(&logs).expect("logs dir");

    let mut c = cfg(&logs);
    c.dir.clone_from(&logs);
    let sink = Sink::start(c).expect("start");
    let salt = sink.redactor().salt_hex();
    assert_eq!(salt.len(), 32);

    for i in 0..5 {
        sink.enqueue(rec(i));
    }
    // A record whose payload echoes the salt back at us.
    sink.enqueue(LogRecord {
        payload: LogPayload::Gesture {
            origin: "click".into(),
            gesture: format!("echo.{salt}"),
        },
        ..rec(99)
    });
    sink.shutdown();

    crate::commands::export_session(&logs, &exports, None).expect("export");

    let mut scanned = 0usize;
    for dir in [&logs, &exports] {
        for entry in std::fs::read_dir(dir).expect("read dir").flatten() {
            let bytes = std::fs::read(entry.path()).expect("read file");
            let text = String::from_utf8_lossy(&bytes);
            assert!(
                !text.contains(&salt),
                "the session salt reached {:?}",
                entry.path()
            );
            scanned += 1;
        }
    }
    assert!(scanned >= 2, "the scan must cover a log part and the zip");
}

/// §6.1 row-7 (b), end-to-end through the sink: `roll_and_purge` rolls to a fresh
/// `afterPurge` file, purges every prior file (log parts AND exports), and
/// logging continues into the new file with no record lost across the roll.
#[test]
fn roll_and_purge_rolls_forward_and_erases_the_past() {
    let root = tempfile::tempdir().expect("tempdir");
    let logs = root.path().join("logs");
    let exports = root.path().join("exports");
    std::fs::create_dir_all(&exports).expect("exports dir");

    let sink = Sink::start(cfg(&logs)).expect("start");
    for i in 0..50 {
        sink.enqueue(rec(i));
    }
    std::fs::write(exports.join("prior.zip"), vec![b'x'; 321]).expect("seed export");

    let reply = sink.roll_and_purge(exports.clone()).expect("purge");
    assert_eq!(reply.deleted_exports, 1, "the prior export was purged");
    assert!(
        reply.deleted_files >= 2,
        "prior log part(s) + the export removed"
    );
    assert!(reply.active_file.ends_with(".jsonl"));

    for i in 50..60 {
        sink.enqueue(rec(i));
    }
    sink.shutdown();

    let files = list_log_files(&logs);
    assert_eq!(files.len(), 1, "only the fresh file remains: {files:?}");
    assert_eq!(files[0].0, reply.active_file);
    assert!(!exports.join("prior.zip").exists(), "the export is gone");

    let rows = all_lines(&logs);
    assert_eq!(rows[0]["afterPurge"], true);
    // Post-roll records survive; pre-roll records were erased with the old file.
    assert!(rows.iter().any(|r| r["gesture"] == "test.59"));
    assert!(!rows.iter().any(|r| r["gesture"] == "test.0"));
}

/// §6.3 row-7 (l): the sink surfaces the writer's cap-eviction count so
/// `log_session_info` can render the truncation warning.
#[test]
fn dropped_parts_is_visible_through_the_sink() {
    let root = tempfile::tempdir().expect("tempdir");
    let logs = root.path().join("logs");
    let mut c = cfg(&logs);
    c.limits = Limits {
        part_bytes: 400,
        max_parts: 3,
        flush_bytes: 1,
        ..Limits::default()
    };
    let sink = Sink::start(c).expect("start");
    assert_eq!(sink.dropped_parts(), 0, "nothing evicted yet");
    for i in 0..300 {
        sink.enqueue(rec(i));
    }
    sink.shutdown();
    assert!(
        sink.dropped_parts() > 0,
        "cap evictions are visible to the UI"
    );
}

/// §8.4 — a PERSISTENT write failure surfaces as `write_failed`, and a recovered
/// disk clears it (sticky-until-a-flush-reaches-disk). The failure is forced by
/// pre-creating a DIRECTORY at the exact path the next rotation part must open —
/// `OpenOptions::open` on a directory fails on every OS, every retry, so the
/// failure is genuinely persistent rather than a one-shot blip.
#[test]
fn write_failed_flag_reflects_a_persistent_rotation_failure_and_clears_on_recovery() {
    let root = tempfile::tempdir().expect("tempdir");
    let logs = root.path().join("logs");
    let mut c = cfg(&logs);
    c.limits = Limits {
        part_bytes: 200,
        max_parts: 8,
        flush_bytes: 1,
        ..Limits::default()
    };
    let redactor = Arc::new(Redactor::new());
    let mut writer = LogWriter::open(c.clone(), redactor).expect("open");
    let flag = writer.write_failed_flag();
    assert!(!flag.load(Ordering::Relaxed), "healthy at session start");

    // Block the next part (part 1) with a directory at its exact name.
    let blocked = c.dir.join(part_name(&c.session_id, c.started_secs, 1));
    std::fs::create_dir_all(&blocked).expect("block next part path");

    // Write past the tiny part cap so a rotation into the blocked part is forced.
    for i in 0..50 {
        let _ = writer.write_record(rec(i));
    }
    assert!(
        flag.load(Ordering::Relaxed),
        "a persistent rotation-open failure sets write_failed",
    );

    // Recover: remove the blocker, then a successful write + flush clears it.
    std::fs::remove_dir(&blocked).expect("unblock next part path");
    writer.write_record(rec(999)).expect("write after recovery");
    writer.flush().expect("flush after recovery");
    assert!(
        !flag.load(Ordering::Relaxed),
        "a flush that reaches disk clears write_failed",
    );
}
