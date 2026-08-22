//! Unit tests for [`super`] (`git::activity`). Split out of `activity.rs` to
//! keep the logic module under the ~500-line soft limit (included via
//! `#[cfg(test)] #[path = "activity_tests.rs"] mod tests;`).

use super::*;
use std::sync::{Arc, Mutex};

/// Collects every emitted event so an assertion can inspect the full sequence.
fn recording() -> (Arc<ActivityEmitter>, Arc<Mutex<Vec<GitActivityEvent>>>) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let emitter = Arc::new(ActivityEmitter::new(
        "git-test-0".to_string(),
        Box::new(move |ev| sink.lock().expect("lock").push(ev)),
    ));
    (emitter, log)
}

#[test]
fn started_is_seq_zero_and_carries_category_and_phase() {
    let (em, log) = recording();
    em.started(GitActivityCategory::Push, GitPhaseKind::Preparing);
    let events = log.lock().expect("lock");
    assert_eq!(events.len(), 1);
    let e = &events[0];
    assert_eq!(e.seq, 0);
    assert_eq!(e.kind, GitActivityKind::Started);
    assert_eq!(e.category, Some(GitActivityCategory::Push));
    assert_eq!(
        e.phase,
        Some(GitPhase {
            kind: GitPhaseKind::Preparing,
            hook: None
        })
    );
}

#[test]
fn seq_is_monotonic_across_kinds() {
    let (em, log) = recording();
    em.started(GitActivityCategory::Commit, GitPhaseKind::Preparing);
    em.phase(GitPhaseKind::RunningHook, Some("pre-commit"));
    em.line(GitStream::Stdout, "hello");
    em.hook_done("pre-commit", Some(0), true);
    em.finished(Some(0), true);
    let events = log.lock().expect("lock");
    let seqs: Vec<u64> = events.iter().map(|e| e.seq).collect();
    assert_eq!(seqs, vec![0, 1, 2, 3, 4]);
    assert_eq!(events[1].phase.as_ref().map(|p| p.hook.as_deref()), Some(Some("pre-commit")));
    assert_eq!(events[2].line.as_deref(), Some("hello"));
    assert_eq!(events[3].kind, GitActivityKind::HookDone);
    assert_eq!(events[3].hook.as_deref(), Some("pre-commit"));
}

#[test]
fn line_kind_follows_stream() {
    let (em, log) = recording();
    em.line(GitStream::Stdout, "out");
    em.line(GitStream::Stderr, "err");
    let events = log.lock().expect("lock");
    assert_eq!(events[0].kind, GitActivityKind::StdoutLine);
    assert_eq!(events[1].kind, GitActivityKind::StderrLine);
}

/// `line` is control-stripped so an injected `\n` can never forge extra rows.
#[test]
fn line_strips_controls() {
    let (em, log) = recording();
    em.line(GitStream::Stdout, "safe\nINJECTED\r\tdone");
    let events = log.lock().expect("lock");
    assert_eq!(events[0].line.as_deref(), Some("safeINJECTEDdone"));
}

#[test]
fn activity_line_truncates_to_char_cap_with_ellipsis() {
    let long = "x".repeat(MAX_ACTIVITY_LINE_CHARS + 50);
    let out = activity_line(&long);
    assert_eq!(out.chars().count(), MAX_ACTIVITY_LINE_CHARS);
    assert!(out.ends_with('…'));
    // A short line is unchanged.
    assert_eq!(activity_line("short"), "short");
}

#[test]
fn progress_event_carries_counts_only() {
    let (em, log) = recording();
    em.progress(GitTransferProgress {
        received_objects: 10,
        total_objects: 100,
        indexed_objects: 5,
        received_bytes: 2048,
        total_deltas: None,
        indexed_deltas: None,
    });
    let events = log.lock().expect("lock");
    let e = &events[0];
    assert_eq!(e.kind, GitActivityKind::Progress);
    assert_eq!(e.progress.map(|p| p.total_objects), Some(100));
    assert!(e.line.is_none() && e.phase.is_none() && e.category.is_none());
}

/// Wire shape: camelCase, optionals ABSENT (not null) when unset, so a line
/// event stays tiny. Mirrors the TS `GitActivityEvent`.
#[test]
fn wire_shape_omits_absent_optionals() {
    let (em, log) = recording();
    em.line(GitStream::Stdout, "hi");
    let events = log.lock().expect("lock");
    let v = serde_json::to_value(&events[0]).expect("json");
    let obj = v.as_object().expect("object");
    // Exactly the run-level fields + `line` — no null optionals on the wire.
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["elapsedMs", "id", "kind", "line", "seq"]);
    assert_eq!(obj.get("kind").and_then(|k| k.as_str()), Some("stdoutLine"));
    assert_eq!(obj.get("line").and_then(|k| k.as_str()), Some("hi"));
}

/// The per-activity event cap: after [`MAX_ACTIVITY_LINE_EVENTS`] lines,
/// `line` suppresses further events; `finished` then flushes exactly ONE
/// marker naming the suppressed count. Guards the IPC boundary from a flood.
#[test]
fn line_events_cap_then_finished_emits_one_truncation_marker() {
    let (em, log) = recording();
    let over = 1234;
    for _ in 0..(MAX_ACTIVITY_LINE_EVENTS + over) {
        em.line(GitStream::Stdout, "flood");
    }
    em.finished(Some(0), true);
    let events = log.lock().expect("lock");
    let lines: Vec<&str> = events
        .iter()
        .filter(|e| e.kind == GitActivityKind::StdoutLine)
        .filter_map(|e| e.line.as_deref())
        .collect();
    let markers: Vec<&&str> = lines.iter().filter(|l| l.contains("output truncated")).collect();
    assert_eq!(markers.len(), 1, "exactly one truncation marker");
    // Exactly CAP real "flood" lines + the single marker line.
    assert_eq!(lines.len(), MAX_ACTIVITY_LINE_EVENTS + 1, "CAP lines + marker");
    assert!(
        markers[0].contains(&format!("{over} more lines suppressed")),
        "marker names the exact suppressed count: {}",
        markers[0]
    );
    // Finished is still the terminal event, AFTER the marker.
    assert_eq!(events.last().map(|e| e.kind), Some(GitActivityKind::Finished));
}

/// Under the cap: no marker, every line emitted normally.
#[test]
fn no_truncation_marker_when_under_cap() {
    let (em, log) = recording();
    em.line(GitStream::Stdout, "a");
    em.line(GitStream::Stderr, "b");
    em.finished(Some(0), true);
    let events = log.lock().expect("lock");
    assert!(
        !events
            .iter()
            .any(|e| e.line.as_deref().is_some_and(|l| l.contains("output truncated"))),
        "no marker under the cap"
    );
    // Both lines present, unchanged.
    assert_eq!(events[0].line.as_deref(), Some("a"));
    assert_eq!(events[1].line.as_deref(), Some("b"));
}

/// L1 hardening: zero-width chars (ZWSP/ZWNJ/ZWJ/BOM) are stripped alongside
/// the bidi controls, so they can't splice or obfuscate a log line.
#[test]
fn line_strips_zero_width_chars() {
    let (em, log) = recording();
    em.line(GitStream::Stdout, "a\u{200b}b\u{200c}c\u{200d}d\u{feff}e");
    let events = log.lock().expect("lock");
    assert_eq!(events[0].line.as_deref(), Some("abcde"));
}

#[test]
fn new_activity_id_is_unique_and_prefixed() {
    let a = new_activity_id();
    let b = new_activity_id();
    assert_ne!(a, b);
    assert!(a.starts_with("git-"), "unexpected id: {a}");
}
