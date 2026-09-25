//! P121d — tests for the dev-mode ignore measurement (path sampling, the
//! fire-time ignore count, the additive record fields). Split from `tests.rs`
//! to keep both files under the ~500-line limit.

use super::tests::{fixture_repo, serialize_watcher_test, watch_into_channel};
use super::*;

/// P121d: the fired-burst worktree path SAMPLE — counts every worktree path
/// but retains at most `IGNORE_SAMPLE_CAP`; refs-only ticks contribute none.
#[test]
fn burst_samples_worktree_paths_with_a_cap() {
    let tick = |n: usize| WatchTick {
        paths: n as u32,
        relevant: n as u32,
        refs: false,
        worktree_paths: (0..n)
            .map(|i| std::path::PathBuf::from(format!("f{i}")))
            .collect(),
    };
    let mut acc = BurstAcc::new(tick(100));
    for _ in 0..5 {
        acc.absorb(tick(100));
    }
    assert_eq!(acc.worktree_total, 600);
    assert_eq!(acc.worktree_paths.len(), IGNORE_SAMPLE_CAP);
    assert_eq!(IGNORE_SAMPLE_CAP, 512);

    // A refs-only tick adds no paths (and no count).
    let mut refs_only = BurstAcc::new(WatchTick {
        paths: 1,
        relevant: 1,
        refs: true,
        ..WatchTick::default()
    });
    refs_only.absorb(WatchTick {
        paths: 2,
        relevant: 2,
        refs: true,
        ..WatchTick::default()
    });
    assert_eq!(refs_only.worktree_total, 0);
    assert!(refs_only.worktree_paths.is_empty());
}

/// P121d: the ignore-measurement fields serialize camelCase on a `fired`
/// record and are omitted entirely when absent (non-fired / dev-mode-off
/// shape unchanged).
#[test]
fn watcher_ignore_fields_serde() {
    use crate::obs::record::LogPayload;
    let fired = LogPayload::Watcher {
        paths: 9,
        relevant: 9,
        debounce_ms: 300,
        fired: true,
        suppressed: false,
        suppress_reason: None,
        burst_class: Some("worktree".into()),
        worktree_paths: Some(9),
        ignore_checked: Some(8),
        ignored: Some(7),
    };
    let v = serde_json::to_value(&fired).unwrap();
    assert_eq!(v["worktreePaths"], 9);
    assert_eq!(v["ignoreChecked"], 8);
    assert_eq!(v["ignored"], 7);

    let raw = LogPayload::Watcher {
        paths: 2,
        relevant: 1,
        debounce_ms: 300,
        fired: false,
        suppressed: false,
        suppress_reason: None,
        burst_class: None,
        worktree_paths: None,
        ignore_checked: None,
        ignored: None,
    };
    let v = serde_json::to_value(&raw).unwrap();
    let obj = v.as_object().unwrap();
    for k in ["worktreePaths", "ignoreChecked", "ignored"] {
        assert!(!obj.contains_key(k), "{k} must be omitted, got {v}");
    }
}

/// P121d: at fire time the sampled absolute paths are made workdir-relative
/// and counted against the repo's ignore rules; a path outside the workdir is
/// not checked. No sink involved — `ignore_stats` is the pure helper.
#[test]
fn ignore_stats_counts_relative_paths() {
    let (_dir, workdir) = fixture_repo();
    let workdir = std::fs::canonicalize(&workdir).unwrap();
    std::fs::write(workdir.join(".gitignore"), "dist/\n").unwrap();
    let mut acc = BurstAcc::new(WatchTick {
        paths: 3,
        relevant: 3,
        refs: false,
        worktree_paths: vec![
            workdir.join("dist").join("a.js"),
            workdir.join("src").join("a.ts"),
            std::path::PathBuf::from("elsewhere").join("b.js"),
        ],
    });
    acc.absorb(WatchTick::default());
    let s = ignore_stats(&workdir, &acc).expect("stats for a worktree burst");
    assert_eq!(s.worktree_paths, 3);
    assert_eq!(s.ignore_checked, Some(2));
    assert_eq!(s.ignored, Some(1));

    // No worktree paths → nothing to measure.
    let refs = BurstAcc::new(WatchTick {
        paths: 1,
        relevant: 1,
        refs: true,
        ..WatchTick::default()
    });
    assert!(ignore_stats(&workdir, &refs).is_none());
}

/// P121d end-to-end (Dev mode on): a burst of writes under an ignored dir
/// produces a `fired` record whose sampled paths were all made relative (the
/// canonical-prefix strip works on the real notify paths) and all counted as
/// ignored. Counts only — the record carries no path.
#[test]
fn fired_record_counts_ignored_worktree_paths() {
    let _serial = serialize_watcher_test();
    let _sink_serial = crate::obs::trace::test_sink_lock();
    let (_dir, workdir) = fixture_repo();
    std::fs::write(workdir.join(".gitignore"), "dist/\n").unwrap();
    std::fs::create_dir_all(workdir.join("dist")).unwrap();
    let log_dir = tempfile::TempDir::new().unwrap();
    let sink = std::sync::Arc::new(
        crate::obs::sink::Sink::start(crate::obs::writer::WriterConfig {
            dir: log_dir.path().to_path_buf(),
            session_id: "sign0001".into(),
            started_secs: 1_787_839_391,
            app_version: "1.5.0".into(),
            os: "windows".into(),
            level: crate::obs::record::LogLevel::Debug,
            redaction: crate::obs::record::RedactionMode::Strict,
            home_mask: None,
            limits: crate::obs::writer::Limits::default(),
        })
        .unwrap(),
    );
    let (handle, rx) = watch_into_channel(&workdir);
    crate::obs::trace::set_active_sink(Some(std::sync::Arc::clone(&sink)));
    for i in 0..5 {
        std::fs::write(workdir.join("dist").join(format!("o{i}.js")), "x").unwrap();
    }
    rx.recv_timeout(Duration::from_secs(10))
        .expect("ignored writes should still fire (behaviour unchanged)");
    while rx.recv_timeout(Duration::from_millis(800)).is_ok() {}
    crate::obs::trace::set_active_sink(None);
    drop(handle);
    sink.shutdown();

    let mut measured = Vec::new();
    for (name, _) in crate::obs::writer::list_log_files(log_dir.path()) {
        let text = std::fs::read_to_string(log_dir.path().join(name)).unwrap();
        for line in text.lines() {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            if v["kind"] == "watcher" && v["fired"] == true && !v["worktreePaths"].is_null() {
                assert!(
                    !line.contains("o0.js"),
                    "a path leaked into the log: {line}"
                );
                measured.push(v);
            }
        }
    }
    assert!(
        !measured.is_empty(),
        "no fired record carried the ignore measurement"
    );
    for v in &measured {
        assert_eq!(v["ignoreChecked"], v["worktreePaths"], "strip failed: {v}");
        assert_eq!(v["ignored"], v["ignoreChecked"], "not all ignored: {v}");
    }
}
