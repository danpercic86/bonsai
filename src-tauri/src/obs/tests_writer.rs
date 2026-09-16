//! P91 §6 writer tests — rotation at the part cap, start-of-session pruning,
//! the `session` header that every file must open with, and (2026-09-11) the
//! §7.2 home-masking WIRING: that the writer actually passes its configured home
//! to the scrubber and stamps `homeMasking` truthfully. The masking rules
//! themselves are `tests_scrub_home`'s job.

use std::path::Path;

use super::record::{LogLevel, LogPayload, LogRecord, LogSource, RedactionMode};
use super::writer::{
    list_log_files, part_name, prune, session_group, utc_stamp, LogWriter, Limits, WriterConfig,
};

/// A fixed-salt redactor so ordinal assertions are reproducible.
fn test_redactor() -> std::sync::Arc<super::redact::Redactor> {
    std::sync::Arc::new(super::redact::Redactor::with_salt([5; 16]))
}

fn cfg(dir: &Path, limits: Limits) -> WriterConfig {
    WriterConfig {
        dir: dir.to_path_buf(),
        session_id: "sdeadbeef".into(),
        started_secs: 1_772_200_991,
        app_version: "1.5.0".into(),
        os: "windows".into(),
        level: LogLevel::Debug,
        redaction: RedactionMode::Strict,
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
        .map(|l| serde_json::from_str(l).expect("each line is valid JSON"))
        .collect()
}

/// §12 row 1: "Enabling Dev mode creates a `logs/*.jsonl` whose first line is a
/// valid `session` record naming the redaction mode."
#[test]
fn first_line_is_a_valid_session_header() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut w = LogWriter::open(cfg(dir.path(), Limits::default()), test_redactor()).expect("open");
    let name = w.active_file().to_string();
    w.write_record(rec("Sidebar")).expect("write");
    w.flush().expect("flush");
    drop(w);

    assert!(name.starts_with("bonsai-") && name.ends_with(".jsonl"), "{name}");
    let rows = lines(&dir.path().join(&name));
    assert_eq!(rows[0]["kind"], "session");
    assert_eq!(rows[0]["schema"], 2);
    assert_eq!(rows[0]["redaction"], "strict");
    assert_eq!(rows[0]["devMode"], true);
    assert_eq!(rows[0]["sessionId"], "sdeadbeef");
    // VERBATIM, not a substring match: the header passes through strict-mode
    // enforcement like every other record, so a phrase containing `/` would be
    // ordinalised into `path#N` gibberish inside the file's own §7.3 disclosure —
    // the one sentence a third-party reviewer relies on. Pinning the whole string
    // makes that class of defect impossible to introduce silently.
    assert_eq!(rows[0]["redactionNote"], RedactionMode::Strict.note());
    assert!(rows[0].get("afterPurge").is_none(), "not a purge roll");
    // §7.2 — ALWAYS stamped, even as `false`: "masking was off" and "this file
    // predates the stamp" must not look the same to an export reader. `cfg()`
    // configures no home, and strict enforcement must not strip the key.
    assert_eq!(rows[0]["homeMasking"], false);
    // seq is assigned by the sink side, in write order, starting at 1.
    assert_eq!(rows[0]["seq"], 1);
    assert_eq!(rows[1]["seq"], 2);
    assert_eq!(rows[1]["kind"], "render");
}

#[test]
fn raw_mode_is_named_in_the_header() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut c = cfg(dir.path(), Limits::default());
    c.redaction = RedactionMode::Raw;
    let w = LogWriter::open(c, test_redactor()).expect("open");
    let name = w.active_file().to_string();
    drop(w);
    let rows = lines(&dir.path().join(name));
    assert_eq!(rows[0]["redaction"], "raw");
    assert_eq!(rows[0]["redactionNote"], RedactionMode::Raw.note());
}

/// Rotation is never REFUSED — the newest records are the evidence — so the
/// in-session bound is enforced by dropping the OLDEST part instead
/// (orchestrator-directed, P91 review round 1; to be ratified into §6).
#[test]
fn rotation_drops_the_oldest_part_and_keeps_the_newest_records() {
    let dir = tempfile::tempdir().expect("tempdir");
    let limits = Limits {
        part_bytes: 400,
        max_parts: 3,
        flush_bytes: 1,
        ..Limits::default()
    };
    let mut w = LogWriter::open(cfg(dir.path(), limits), test_redactor()).expect("open");
    for i in 0..200 {
        w.write_record(rec(&format!("Component{i}"))).expect("write");
    }
    w.flush().expect("flush");
    let last_part = w.active_file().to_string();
    drop(w);

    let files = list_log_files(dir.path());
    assert_eq!(files.len(), 3, "the session is bounded to max_parts: {files:?}");
    assert_eq!(files[2].0, last_part, "the newest part is the live one");
    // The oldest part is gone — a storm session cannot grow without bound.
    assert!(
        !files.iter().any(|(n, _)| n.ends_with("-sdeadbeef.jsonl")),
        "part 0 should have been dropped: {files:?}"
    );
    // Every surviving part opens with its own session header (self-describing).
    for (name, _) in &files {
        assert_eq!(lines(&dir.path().join(name))[0]["kind"], "session");
    }
    // The NEWEST evidence survived: the last record written is on disk.
    let tail = lines(&dir.path().join(&last_part));
    assert!(
        tail.iter().any(|r| r["component"] == "Component199"),
        "the newest record must never be the one discarded"
    );
}

/// `seq` is per-SESSION, not per-file: it must keep counting across a rotation
/// boundary, or the ordering guarantee of §3 breaks exactly where a storm makes
/// it matter most.
#[test]
fn seq_stays_monotonic_across_a_rotation_boundary() {
    let dir = tempfile::tempdir().expect("tempdir");
    let limits = Limits {
        part_bytes: 400,
        // Generous: this test is about `seq`, so nothing may be trimmed away.
        max_parts: 50,
        flush_bytes: 1,
        ..Limits::default()
    };
    let mut w = LogWriter::open(cfg(dir.path(), limits), test_redactor()).expect("open");
    for i in 0..20 {
        w.write_record(rec(&format!("Component{i}"))).expect("write");
    }
    w.flush().expect("flush");
    drop(w);

    let files = list_log_files(dir.path());
    assert!(files.len() > 1, "the test must actually rotate: {files:?}");
    let mut seqs: Vec<u64> = Vec::new();
    for (name, _) in &files {
        for row in lines(&dir.path().join(name)) {
            seqs.push(row["seq"].as_u64().expect("seq is a number"));
        }
    }
    // Dense and strictly increasing in file order, headers included.
    assert_eq!(seqs, (1..=seqs.len() as u64).collect::<Vec<_>>());
}

#[test]
fn pruning_keeps_the_n_most_recent_sessions() {
    let dir = tempfile::tempdir().expect("tempdir");
    for i in 0..14 {
        let name = format!("bonsai-2026-08-{:02}T10-00-00-s{i:08x}.jsonl", i + 1);
        std::fs::write(dir.path().join(name), "{}\n").expect("seed");
    }
    // A rotation part belongs to its session and must prune WITH it.
    std::fs::write(
        dir.path().join("bonsai-2026-08-01T10-00-00-s00000000-1.jsonl"),
        "{}\n",
    )
    .expect("seed part");

    prune(
        dir.path(),
        Limits {
            keep_sessions: 10,
            ..Limits::default()
        },
    );

    let left = list_log_files(dir.path());
    assert_eq!(left.len(), 10, "{left:?}");
    assert!(left.iter().all(|(n, _)| !n.contains("2026-08-01")));
    assert!(left.iter().any(|(n, _)| n.contains("2026-08-14")));
}

#[test]
fn pruning_enforces_the_total_size_cap_oldest_first() {
    let dir = tempfile::tempdir().expect("tempdir");
    for i in 0..4 {
        let name = format!("bonsai-2026-08-{:02}T10-00-00-s{i:08x}.jsonl", i + 1);
        std::fs::write(dir.path().join(name), vec![b'x'; 1000]).expect("seed");
    }
    prune(
        dir.path(),
        Limits {
            keep_sessions: 10,
            total_bytes: 2500,
            ..Limits::default()
        },
    );
    let left = list_log_files(dir.path());
    assert_eq!(left.len(), 2, "{left:?}");
    assert!(left[0].0.contains("2026-08-03"));
    assert!(left[1].0.contains("2026-08-04"));
}

#[test]
fn pruning_never_removes_the_only_session() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("bonsai-2026-08-01T10-00-00-s00000000.jsonl"),
        vec![b'x'; 10_000],
    )
    .expect("seed");
    prune(
        dir.path(),
        Limits {
            total_bytes: 10,
            ..Limits::default()
        },
    );
    assert_eq!(list_log_files(dir.path()).len(), 1);
}

#[test]
fn pruning_ignores_files_it_does_not_own() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("notes.txt"), "keep me").expect("seed");
    std::fs::write(dir.path().join("usage.json"), "{}").expect("seed");
    for i in 0..12 {
        let name = format!("bonsai-2026-08-{:02}T10-00-00-s{i:08x}.jsonl", i + 1);
        std::fs::write(dir.path().join(name), "{}\n").expect("seed");
    }
    prune(dir.path(), Limits::default());
    assert!(dir.path().join("notes.txt").exists());
    assert!(dir.path().join("usage.json").exists());
    assert_eq!(list_log_files(dir.path()).len(), 10);
}

#[test]
fn session_group_splits_the_part_suffix_only() {
    assert_eq!(
        session_group("bonsai-2026-08-27T14-03-11-sdeadbeef.jsonl"),
        "bonsai-2026-08-27T14-03-11-sdeadbeef"
    );
    assert_eq!(
        session_group("bonsai-2026-08-27T14-03-11-sdeadbeef-7.jsonl"),
        "bonsai-2026-08-27T14-03-11-sdeadbeef"
    );
}

#[test]
fn utc_stamp_is_filename_safe_and_correct() {
    // 2026-08-27T14:03:11Z
    assert_eq!(utc_stamp(1_787_839_391), "2026-08-27T14-03-11");
    assert_eq!(utc_stamp(0), "1970-01-01T00-00-00");
}

/// A record whose payload carries a credential must reach disk scrubbed: §7.2
/// says the scrubber runs LAST, and the writer is that last step.
#[test]
fn records_are_scrubbed_on_the_way_to_disk() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut w = LogWriter::open(cfg(dir.path(), Limits::default()), test_redactor()).expect("open");
    let name = w.active_file().to_string();
    w.write_record(LogRecord {
        payload: LogPayload::Error {
            location: "forge.push".into(),
            code: None,
            message: "auth failed for https://u:hunter2@github.com/o/r.git".into(),
            stack_hash: None,
        },
        ..rec("x")
    })
    .expect("write");
    w.flush().expect("flush");
    drop(w);
    let text = std::fs::read_to_string(dir.path().join(name)).expect("read");
    assert!(!text.contains("hunter2"), "credential reached disk: {text}");
    // In STRICT mode the whole URL — userinfo included — is ordinalised before
    // the credential scrubber even sees it, which is the stronger outcome.
    assert!(!text.contains("github.com"), "host reached disk: {text}");
    assert!(text.contains("remote#"), "{text}");
}

/// The same record in RAW mode keeps the host (the user opted in) but the
/// credential is still scrubbed — §7.1's "tokens: NEVER, under any setting".
#[test]
fn raw_mode_keeps_names_but_never_credentials() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut c = cfg(dir.path(), Limits::default());
    c.redaction = RedactionMode::Raw;
    let mut w = LogWriter::open(c, test_redactor()).expect("open");
    let name = w.active_file().to_string();
    w.write_record(LogRecord {
        payload: LogPayload::Error {
            location: "forge.push".into(),
            code: None,
            message: "auth failed for https://u:hunter2@github.com/o/r.git".into(),
            stack_hash: None,
        },
        ..rec("x")
    })
    .expect("write");
    w.flush().expect("flush");
    drop(w);
    let text = std::fs::read_to_string(dir.path().join(name)).expect("read");
    assert!(!text.contains("hunter2"), "credential reached disk: {text}");
    assert!(text.contains("<redacted:token>"), "{text}");
    assert!(text.contains("github.com"), "raw mode keeps the host: {text}");
}

/// §8.4 (increment-7c follow-up): a **persistent** rotation-open block must keep
/// `writeFailed` stuck true across repeated idle flushes.
///
/// The disk-full / permission paths (`write_all`, `flush`) were already sticky,
/// but a blocked `open_part` left the writer holding the PREVIOUS part's healthy
/// `BufWriter`: every 1 s idle flush succeeded and cleared the flag, so the
/// §16.9 status row and the header pill flapped between "not writing" and healthy
/// while nothing could be persisted at all.
#[test]
fn a_persistent_rotation_block_keeps_write_failed_sticky() {
    use std::sync::atomic::Ordering;

    let dir = tempfile::tempdir().expect("tempdir");
    let limits = Limits {
        part_bytes: 400,
        max_parts: 3,
        flush_bytes: 1,
        ..Limits::default()
    };
    let c = cfg(dir.path(), limits);
    let started = c.started_secs;
    let session = c.session_id.clone();
    let mut w = LogWriter::open(c, test_redactor()).expect("open");
    let failed = w.write_failed_flag();
    assert!(!failed.load(Ordering::Relaxed), "a fresh writer is healthy");

    // Park a DIRECTORY on part 1's exact filename: opening it for append fails on
    // Windows and Unix alike, and keeps failing — a persistent rotation block
    // (permission loss on the log dir, path taken by something else).
    let blocker = dir.path().join(part_name(&session, started, 1));
    std::fs::create_dir(&blocker).expect("park a directory on part 1");

    // Fill part 0 until the rotation is attempted and fails.
    let mut blocked = false;
    for i in 0..200 {
        if w.write_record(rec(&format!("Component{i}"))).is_err() {
            blocked = true;
            break;
        }
    }
    assert!(blocked, "the part cap must have been reached");
    assert!(
        failed.load(Ordering::Relaxed),
        "a blocked rotation is the 'stopped writing' state"
    );

    // The writer loop's ~1 s idle flush: the old part's BufWriter still flushes
    // fine, and that must NOT be read as recovery.
    for round in 0..5 {
        let _ = w.flush();
        assert!(
            failed.load(Ordering::Relaxed),
            "idle flush {round} cleared writeFailed while rotation was still blocked"
        );
        // A further record still cannot rotate; interleaving must not help either.
        let _ = w.write_record(rec("StillBlocked"));
        assert!(
            failed.load(Ordering::Relaxed),
            "writeFailed flapped after idle flush {round}"
        );
    }

    // Sticky, not stuck: once the block is gone the next rotation succeeds and the
    // flush that reaches disk clears the flag again.
    std::fs::remove_dir(&blocker).expect("unblock part 1");
    w.write_record(rec("Recovered")).expect("rotation recovers");
    w.flush().expect("flush after recovery");
    assert!(
        !failed.load(Ordering::Relaxed),
        "a recovered rotation must clear writeFailed within one flush"
    );
}

// ---- §7.2 home masking: the WIRING, not the rules (auditor MUST-FIX) ----

/// The writer must hand `cfg.home_mask` to the scrubber. Deleting that argument
/// fails HERE — which is exactly what the previous process-global lacked: the
/// resolution could silently never happen and no test noticed.
#[test]
fn a_configured_home_is_masked_out_of_every_written_field() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut c = cfg(dir.path(), Limits::default());
    // Raw mode, because strict mode collapses paths to `path#N` on its own and
    // would hide whether the home pass ran at all.
    c.redaction = RedactionMode::Raw;
    c.home_mask = Some("c:/users/jane".to_string());
    let mut w = LogWriter::open(c, test_redactor()).expect("open");
    let name = w.active_file().to_string();
    w.write_record(rec(r"C:\Users\jane\Repos\bonsai\src\App.tsx")).expect("write");
    w.flush().expect("flush");
    drop(w);

    let path = dir.path().join(&name);
    let raw = std::fs::read_to_string(&path).expect("read log file");
    assert!(!raw.contains("jane"), "the account name reached disk: {raw}");
    let rows = lines(&path);
    assert_eq!(rows[0]["homeMasking"], true, "header must state masking is ON");
    assert_eq!(rows[1]["component"], r"<home>\Repos\bonsai\src\App.tsx");
}

/// The `None` arm is the honest-but-unmasked one: nothing is replaced, and the
/// header says so, so a reader of the export zip knows not to trust the paths.
#[test]
fn an_unresolved_home_is_stamped_false_in_the_header() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut c = cfg(dir.path(), Limits::default());
    c.redaction = RedactionMode::Raw;
    c.home_mask = None;
    let mut w = LogWriter::open(c, test_redactor()).expect("open");
    let name = w.active_file().to_string();
    w.write_record(rec(r"C:\Users\jane\Repos\bonsai")).expect("write");
    w.flush().expect("flush");
    drop(w);

    let rows = lines(&dir.path().join(&name));
    assert_eq!(rows[0]["homeMasking"], false, "absent or true would both mislead");
    assert_eq!(rows[1]["component"], r"C:\Users\jane\Repos\bonsai");
}

/// Every rotation part carries the stamp, not just part 0 — a part handed over
/// on its own must still describe its own masking (§6 "self-describing").
#[test]
fn every_rotation_part_header_carries_the_masking_stamp() {
    let dir = tempfile::tempdir().expect("tempdir");
    let limits = Limits { part_bytes: 400, flush_bytes: 1, ..Limits::default() };
    let mut c = cfg(dir.path(), limits);
    c.home_mask = Some("c:/users/jane".to_string());
    let mut w = LogWriter::open(c, test_redactor()).expect("open");
    for i in 0..24 {
        w.write_record(rec(&format!("Row{i}"))).expect("write");
    }
    w.flush().expect("flush");
    drop(w);

    let files = list_log_files(dir.path());
    assert!(files.len() > 1, "the tiny part cap must have rotated: {files:?}");
    for (name, _) in files {
        let rows = lines(&dir.path().join(&name));
        assert_eq!(rows[0]["homeMasking"], true, "part {name} lost the stamp");
    }
}
