//! Unit tests for [`super`] (`git::activity`). Split out of `activity.rs` to
//! keep the logic module under the ~500-line soft limit (included via
//! `#[cfg(test)] #[path = "activity_tests.rs"] mod tests;`).

use super::*;
use std::sync::{Arc, Mutex};

/// Collects every emitted event so an assertion can inspect the full sequence.
/// No run target (the pre-FU-1 shape).
fn recording() -> (Arc<ActivityEmitter>, Arc<Mutex<Vec<GitActivityEvent>>>) {
    recording_with_target(None)
}

/// [`recording`] plus an FU-1 run target set at construction.
fn recording_with_target(
    target: Option<ActivityTarget>,
) -> (Arc<ActivityEmitter>, Arc<Mutex<Vec<GitActivityEvent>>>) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let emitter = Arc::new(ActivityEmitter::new(
        "git-test-0".to_string(),
        target,
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
    assert_eq!(
        events[1].phase.as_ref().map(|p| p.hook.as_deref()),
        Some(Some("pre-commit"))
    );
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
    let markers: Vec<&&str> = lines
        .iter()
        .filter(|l| l.contains("output truncated"))
        .collect();
    assert_eq!(markers.len(), 1, "exactly one truncation marker");
    // Exactly CAP real "flood" lines + the single marker line.
    assert_eq!(
        lines.len(),
        MAX_ACTIVITY_LINE_EVENTS + 1,
        "CAP lines + marker"
    );
    assert!(
        markers[0].contains(&format!("{over} more lines suppressed")),
        "marker names the exact suppressed count: {}",
        markers[0]
    );
    // Finished is still the terminal event, AFTER the marker.
    assert_eq!(
        events.last().map(|e| e.kind),
        Some(GitActivityKind::Finished)
    );
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
        !events.iter().any(|e| e
            .line
            .as_deref()
            .is_some_and(|l| l.contains("output truncated"))),
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

// ------------------------------------ P87b FU-1: the run target (§6)

/// Guarantee 3 — the target goes through the SAME funnel as `activity_line`:
/// bidi overrides/isolates, zero-width chars, and C0/C1 controls are all gone.
#[test]
fn target_strips_bidi_and_zero_width() {
    let t = ActivityTarget::remote_branch("origin", "ma\u{202e}in").expect("target");
    assert_eq!(t.as_str(), "origin/main");

    let zero_width = ActivityTarget::branch("fe\u{200b}at\u{200d}ure\u{feff}").expect("target");
    assert_eq!(zero_width.as_str(), "feature");

    // C0 (`\n`, `\t`) cannot forge a second row; C1 (`\u{0085}` NEL) neither.
    let c0 = ActivityTarget::branch("ma\nin\tx\u{0085}y").expect("target");
    assert_eq!(c0.as_str(), "mainxy");

    // Nothing survives ⇒ no target at all, rather than an empty pill.
    assert_eq!(ActivityTarget::branch("\u{202e}\u{200b}\n"), None);
    assert_eq!(ActivityTarget::branch("   "), None);
    // Each part is validated separately, so a blank part never yields "/main".
    assert_eq!(ActivityTarget::remote_branch("", "main"), None);
    assert_eq!(ActivityTarget::remote_branch("origin", "\u{200b}"), None);
}

/// Guarantee 4 — a hostile ref cannot park a megabyte string in the frontend's
/// run store: exactly [`MAX_ACTIVITY_TARGET_CHARS`] chars, ending in `…`.
#[test]
fn target_capped_at_255_chars() {
    let long = "b".repeat(400);
    let t = ActivityTarget::branch(&long).expect("target");
    assert_eq!(t.as_str().chars().count(), MAX_ACTIVITY_TARGET_CHARS);
    assert!(t.as_str().ends_with('…'), "capped marker missing");

    // A real ref name is far under the cap and is never touched.
    let real = ActivityTarget::remote_branch("origin", "refs/heads/feature/x").expect("target");
    assert_eq!(real.as_str(), "origin/feature/x");
}

/// Guarantee 2 — the target rides on `started` and NOTHING else, even though
/// every other kind flows through the same `base()`.
#[test]
fn target_appears_only_on_started() {
    let (em, log) = recording_with_target(ActivityTarget::remote_branch("origin", "main"));
    em.started(GitActivityCategory::Push, GitPhaseKind::Preparing);
    em.phase(GitPhaseKind::RunningHook, Some("pre-push"));
    em.line(GitStream::Stdout, "hello");
    em.hook_done("pre-push", Some(0), true);
    em.progress(GitTransferProgress {
        received_objects: 1,
        total_objects: 2,
        indexed_objects: 1,
        received_bytes: 3,
        total_deltas: None,
        indexed_deltas: None,
    });
    em.finished(Some(0), true);

    let events = log.lock().expect("lock");
    let carriers: Vec<GitActivityKind> = events
        .iter()
        .filter(|e| e.target.is_some())
        .map(|e| e.kind)
        .collect();
    assert_eq!(
        carriers,
        vec![GitActivityKind::Started],
        "exactly one event may carry a target"
    );
    assert_eq!(events[0].target.as_deref(), Some("origin/main"));
}

/// §9.4 — `None` is ABSENT on the wire (matching the TS `target?: string`), not
/// `"target": null`.
#[test]
fn target_none_omits_the_field() {
    let (em, log) = recording();
    em.started(GitActivityCategory::Commit, GitPhaseKind::Preparing);
    let events = log.lock().expect("lock");
    let json = serde_json::to_value(&events[0]).expect("json");
    assert!(
        json.get("target").is_none(),
        "a None target must be omitted, got {json}"
    );

    // …and present, camelCase, when there IS one.
    let (em, log) = recording_with_target(ActivityTarget::branch("main"));
    em.started(GitActivityCategory::Commit, GitPhaseKind::Preparing);
    let events = log.lock().expect("lock");
    let json = serde_json::to_value(&events[0]).expect("json");
    assert_eq!(json.get("target").and_then(|v| v.as_str()), Some("main"));
}

// ---------------------------------------------------------------- P119 T-R6

fn recording_with_subject(
    subject: RunSubject,
) -> (Arc<ActivityEmitter>, Arc<Mutex<Vec<GitActivityEvent>>>) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let emitter = Arc::new(ActivityEmitter::with_subject(
        "git-test-0".to_string(),
        subject,
        Box::new(move |ev| sink.lock().expect("lock").push(ev)),
    ));
    (emitter, log)
}

fn json_keys(ev: &GitActivityEvent) -> Vec<String> {
    let v = serde_json::to_value(ev).expect("json");
    let mut keys: Vec<String> = v.as_object().expect("object").keys().cloned().collect();
    keys.sort_unstable();
    keys
}

/// `targetCount` rides on `started` ONLY, and a counted run has no `target`.
#[test]
fn target_count_only_on_started() {
    use crate::git::activity_target::TargetArg;
    let subject = RunSubject::many(
        [
            TargetArg::Branch("a"),
            TargetArg::Branch("b"),
            TargetArg::Branch("c"),
        ]
        .into_iter(),
    );
    let (em, log) = recording_with_subject(subject);
    em.started(GitActivityCategory::DeleteBranches, GitPhaseKind::Preparing);
    em.phase(GitPhaseKind::Network, None);
    em.line(GitStream::Stdout, "x");
    em.finish(Some(0), true, None, None);
    let events = log.lock().expect("lock");
    assert_eq!(events[0].target_count, Some(3));
    assert_eq!(events[0].target, None);
    let json = serde_json::to_value(&events[0]).expect("json");
    assert_eq!(json.get("targetCount"), Some(&serde_json::json!(3)));
    assert!(json.get("target").is_none());
    for ev in &events[1..] {
        assert_eq!(ev.target_count, None, "{:?}", ev.kind);
    }
}

/// `outcome` rides on `finished` ONLY, and only for a successful run.
#[test]
fn outcome_only_on_successful_finished() {
    let (em, log) = recording();
    em.started(GitActivityCategory::Merge, GitPhaseKind::Preparing);
    em.finish(Some(0), true, Some(GitRunOutcome::FastForwarded), None);
    let events = log.lock().expect("lock");
    assert_eq!(events[0].outcome, None);
    let last = events.last().expect("finished");
    assert_eq!(last.kind, GitActivityKind::Finished);
    assert_eq!(last.outcome, Some(GitRunOutcome::FastForwarded));
    let json = serde_json::to_value(last).expect("json");
    assert_eq!(
        json.get("outcome"),
        Some(&serde_json::json!("fastForwarded"))
    );

    let (em, log) = recording();
    em.finish(Some(1), false, Some(GitRunOutcome::Conflicts), None);
    let events = log.lock().expect("lock");
    assert_eq!(events[0].outcome, None, "a failed run carries no outcome");
}

/// The reason line survives the line-event cap and lands just before
/// `finished` (after the truncation marker), sanitized like any line.
#[test]
fn reason_line_is_delivered_after_the_cap_just_before_finished() {
    let (em, log) = recording();
    em.started(GitActivityCategory::DeleteBranch, GitPhaseKind::Preparing);
    for i in 0..(MAX_ACTIVITY_LINE_EVENTS + 10) {
        em.line(GitStream::Stdout, &format!("l{i}"));
    }
    em.finish(
        Some(1),
        false,
        None,
        Some("branch 'x' not found\nforged row"),
    );
    let events = log.lock().expect("lock");
    let n = events.len();
    // started + 5000 lines + marker + reason + finished.
    assert_eq!(n, 1 + MAX_ACTIVITY_LINE_EVENTS + 3);
    assert!(events[n - 3]
        .line
        .as_deref()
        .is_some_and(|l| l.contains("10 more lines suppressed")));
    assert_eq!(events[n - 2].kind, GitActivityKind::StderrLine);
    assert_eq!(
        events[n - 2].line.as_deref(),
        Some("branch 'x' not found forged row"),
        "line breaks become spaces, then control-stripped into one line"
    );
    assert_eq!(events[n - 1].kind, GitActivityKind::Finished);
    assert_eq!(events[n - 1].success, Some(false));
}

/// `finished(code, success)` is byte-identical to `finish(.., None, None)` and
/// adds no P119 keys to the wire.
#[test]
fn finish_with_nothing_matches_finished() {
    let (a, log_a) = recording();
    a.finished(Some(0), true);
    let (b, log_b) = recording();
    b.finish(Some(0), true, None, None);
    let ea = log_a.lock().expect("lock");
    let eb = log_b.lock().expect("lock");
    assert_eq!(ea.len(), 1);
    assert_eq!(eb.len(), 1);
    let (mut x, mut y) = (ea[0].clone(), eb[0].clone());
    (x.elapsed_ms, y.elapsed_ms) = (0, 0);
    assert_eq!(x, y);
    assert_eq!(
        json_keys(&ea[0]),
        ["code", "elapsedMs", "id", "kind", "seq", "success"]
    );
}
