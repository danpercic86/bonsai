//! P91 §3 `mono` — the writer's session-clock stamp (2026-09-16).
//!
//! Split out of `tests_writer.rs` (which is already at the ~500-line limit).
//! ONE concern: that `LogWriter::append_record` fills `mono` for records minted
//! WITHOUT a session clock (`anomaly`, `truncate`, `drop`, part headers) and
//! touches nothing else.
//!
//! Both guards in that stamp are asserted independently, because dropping
//! either is a silent data defect rather than a compile error:
//!   * `mono == 0` — a producer that set `mono` deliberately must win.
//!   * `src == Rust` — the UI side keeps its own `mono` base (`obs/anomaly.rs`
//!     module doc), so restamping a `ui` record would corrupt it with our clock.

use std::path::Path;
use std::time::Duration;

use super::record::{LogLevel, LogPayload, LogRecord, LogSource, RedactionMode};
use super::writer::{LogWriter, Limits, WriterConfig};

/// Ms the test sleeps before writing, so "stamped" is distinguishable from
/// "left 0" — at session start `elapsed()` is legitimately 0 ms. `Instant` is
/// monotone and `sleep` guarantees AT LEAST this much elapsed, so the lower-bound
/// assertion below is a guarantee, not a timing race.
const SETTLE_MS: u64 = 20;

fn test_redactor() -> std::sync::Arc<super::redact::Redactor> {
    std::sync::Arc::new(super::redact::Redactor::with_salt([7; 16]))
}

fn cfg(dir: &Path) -> WriterConfig {
    WriterConfig {
        dir: dir.to_path_buf(),
        session_id: "smonobase".into(),
        started_secs: 1_772_200_991,
        app_version: "1.5.0".into(),
        os: "windows".into(),
        level: LogLevel::Debug,
        redaction: RedactionMode::Raw,
        home_mask: None,
        limits: Limits::default(),
    }
}

/// A `gesture` record — payload-irrelevant here; only `src`/`mono` matter.
fn rec(src: LogSource, mono: u64, gesture: &str) -> LogRecord {
    LogRecord {
        seq: 0,
        ts: 1_772_200_991_000,
        mono,
        src,
        lvl: LogLevel::Debug,
        trace: None,
        span: None,
        caused_by: None,
        payload: LogPayload::Gesture {
            origin: "click".into(),
            gesture: gesture.into(),
        },
    }
}

fn lines(path: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path)
        .expect("read log file")
        .lines()
        .map(|l| serde_json::from_str(l).expect("each line is valid JSON"))
        .collect()
}

fn mono_of(rows: &[serde_json::Value], gesture: &str) -> u64 {
    rows.iter()
        .find(|r| r["gesture"] == gesture)
        .and_then(|r| r["mono"].as_u64())
        .unwrap_or_else(|| panic!("no record for gesture {gesture}: {rows:?}"))
}

/// The whole stamp in one file read: a Rust record at 0 is stamped, a UI record
/// at 0 is left alone, and a producer-set `mono` survives on both sides.
///
/// Fails if EITHER guard is dropped:
///   * without `mono == 0` → `rust-preset` / `ui-preset` stop being 12.
///   * without `src == Rust` → `ui-unset` stops being 0.
#[test]
fn writer_stamps_mono_only_for_unset_rust_records() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut w = LogWriter::open(cfg(dir.path()), test_redactor()).expect("open");
    let name = w.active_file().to_string();
    // Let the session clock advance past 0 so a stamp is observable.
    std::thread::sleep(Duration::from_millis(SETTLE_MS));

    w.write_record(rec(LogSource::Rust, 0, "rust-unset"))
        .expect("write");
    w.write_record(rec(LogSource::Ui, 0, "ui-unset"))
        .expect("write");
    w.write_record(rec(LogSource::Rust, 12, "rust-preset"))
        .expect("write");
    w.write_record(rec(LogSource::Ui, 12, "ui-preset"))
        .expect("write");
    w.flush().expect("flush");
    drop(w);

    let rows = lines(&dir.path().join(&name));
    // GUARD 1 (`src == Rust`): the unset Rust record got the session clock.
    assert!(
        mono_of(&rows, "rust-unset") >= SETTLE_MS,
        "rust record with mono 0 must be stamped (>= {SETTLE_MS}ms elapsed), got {}",
        mono_of(&rows, "rust-unset")
    );
    // GUARD 1, other half: a `ui` record's own base must NOT be overwritten with
    // ours — 0 is a legitimate UI value and the writer cannot improve on it.
    assert_eq!(
        mono_of(&rows, "ui-unset"),
        0,
        "ui record must keep its own mono base"
    );
    // GUARD 2 (`mono == 0`): a deliberately set value wins on either side.
    assert_eq!(mono_of(&rows, "rust-preset"), 12, "producer-set mono wins");
    assert_eq!(mono_of(&rows, "ui-preset"), 12, "producer-set mono wins");
}

/// The defect this fix closes: an `anomaly` record (built by `obs/anomaly.rs`
/// with `mono: 0` because it has no session clock) must reach disk with a real
/// session-relative position — all 431 `anomaly` lines of the 2026-09-15 Dev log
/// carried `mono: 0`, i.e. 431 warn-level records with no ordering aid.
#[test]
fn writer_minted_anomaly_record_reaches_disk_with_mono() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut w = LogWriter::open(cfg(dir.path()), test_redactor()).expect("open");
    let name = w.active_file().to_string();
    std::thread::sleep(Duration::from_millis(SETTLE_MS));

    let anomaly = super::anomaly::build_anomaly(
        "dup-ipc",
        super::record::AnomalySeverity::Warn,
        "2 identical calls within 300ms".into(),
        vec![1, 2],
        Vec::new(),
        1_772_200_991_000,
    );
    assert_eq!(anomaly.mono, 0, "the builder itself has no session clock");
    w.write_record(anomaly).expect("write");
    w.flush().expect("flush");
    drop(w);

    let rows = lines(&dir.path().join(&name));
    let row = rows
        .iter()
        .find(|r| r["kind"] == "anomaly")
        .expect("anomaly line");
    assert_eq!(row["rule"], "dup-ipc");
    assert!(
        row["mono"].as_u64().unwrap_or(0) >= SETTLE_MS,
        "anomaly line must carry a stamped mono, got {}",
        row["mono"]
    );
}

/// The recorded decision (not an accident): part 0's header is written AT session
/// start, so the stamp leaves it at ~0, while a LATER part's header (rotation, or
/// a purge roll) carries the session-relative ms at which that part began. The
/// writer's base is NOT reset by a roll — `mono` stays session-relative.
#[test]
fn a_rotation_part_header_carries_a_later_mono_than_part_zero() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut c = cfg(dir.path());
    // Tiny part cap so one record forces a rotation.
    c.limits.part_bytes = 1;
    let mut w = LogWriter::open(c, test_redactor()).expect("open");
    let first = w.active_file().to_string();
    std::thread::sleep(Duration::from_millis(SETTLE_MS));
    w.write_record(rec(LogSource::Ui, 5, "after-rotation"))
        .expect("write");
    w.flush().expect("flush");
    let second = w.active_file().to_string();
    drop(w);

    assert_ne!(first, second, "the record must have forced a rotation");
    let head0 = lines(&dir.path().join(&first));
    assert_eq!(head0[0]["kind"], "session");
    let head1 = lines(&dir.path().join(&second));
    assert_eq!(head1[0]["kind"], "session");
    // RELATIVE, not absolute: part 0's header is written inside `open`, so its
    // value is "however long the first header write took" (sub-ms in practice) —
    // asserting an absolute ceiling on it would be a timing bet on the host's
    // file system. The claim being pinned is the ORDER: the rotation part's
    // header is at least the elapsed sleep later than part 0's.
    // Both `mono`s stay OPTIONAL through the comparison: a missing or
    // non-integer value must fail THIS assertion with both values printed, not
    // panic inside the arithmetic on a sentinel and hide what the headers said.
    let (m0, m1) = (head0[0]["mono"].as_u64(), head1[0]["mono"].as_u64());
    assert!(
        m0.zip(m1)
            .is_some_and(|(a, b)| b >= a.saturating_add(SETTLE_MS)),
        "a rotation part's header carries the ms at which THAT part began:          part0={m0:?} part1={m1:?}"
    );
}
