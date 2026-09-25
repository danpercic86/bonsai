//! P123 R1-R2: `repoId` rides on `started` ONLY (included from `activity.rs`
//! via `#[cfg(test)] #[path = "activity_repo_tests.rs"] mod repo_tests;`).

use super::*;
use std::sync::{Arc, Mutex};

type Log = Arc<Mutex<Vec<GitActivityEvent>>>;

fn recording_for(repo_id: Option<&str>) -> (Arc<ActivityEmitter>, Log) {
    let log: Log = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let emitter = Arc::new(ActivityEmitter::with_subject(
        "git-test-repo".to_string(),
        repo_id.map(str::to_string),
        RunSubject::from(None),
        Box::new(move |ev| sink.lock().expect("lock").push(ev)),
    ));
    (emitter, log)
}

fn json(ev: &GitActivityEvent) -> serde_json::Value {
    serde_json::to_value(ev).expect("json")
}

/// R1: `started` serialises `"repoId"` when scoped, and omits the key when not.
#[test]
fn started_carries_repo_id_only_when_scoped() {
    let (em, log) = recording_for(Some("repo-a"));
    em.started(GitActivityCategory::Push, GitPhaseKind::Preparing);
    let (plain, plain_log) = recording_for(None);
    plain.started(GitActivityCategory::CloneRepo, GitPhaseKind::Preparing);

    let scoped = log.lock().expect("lock");
    assert_eq!(scoped[0].repo_id.as_deref(), Some("repo-a"));
    assert_eq!(
        json(&scoped[0]).get("repoId"),
        Some(&serde_json::json!("repo-a"))
    );
    let unscoped = plain_log.lock().expect("lock");
    assert!(json(&unscoped[0]).get("repoId").is_none());
}

/// R2: every non-started kind — phase, both line streams, hookDone, progress,
/// the truncation marker, the reason line and finished — has no `repoId` key.
#[test]
fn no_other_kind_carries_repo_id() {
    let (em, log) = recording_for(Some("repo-a"));
    em.started(GitActivityCategory::Push, GitPhaseKind::Preparing);
    em.phase(GitPhaseKind::RunningHook, Some("pre-push"));
    em.hook_done("pre-push", Some(0), true);
    em.phase(GitPhaseKind::Network, None);
    em.progress(GitTransferProgress {
        received_objects: 1,
        total_objects: 2,
        indexed_objects: 1,
        received_bytes: 10,
        total_deltas: None,
        indexed_deltas: None,
    });
    em.line(GitStream::Stderr, "err");
    for _ in 0..=MAX_ACTIVITY_LINE_EVENTS {
        em.line(GitStream::Stdout, "x");
    }
    em.finish(Some(1), false, None, Some("rejected"));

    let events = log.lock().expect("lock");
    let kinds: Vec<GitActivityKind> = events.iter().map(|e| e.kind).collect();
    for want in [
        GitActivityKind::Phase,
        GitActivityKind::HookDone,
        GitActivityKind::Progress,
        GitActivityKind::StdoutLine,
        GitActivityKind::StderrLine,
        GitActivityKind::Finished,
    ] {
        assert!(kinds.contains(&want), "missing {want:?}");
    }
    let tail: Vec<&str> = events
        .iter()
        .rev()
        .skip(1)
        .take(2)
        .filter_map(|e| e.line.as_deref())
        .collect();
    assert!(tail.iter().any(|l| l.contains("output truncated")));
    assert!(tail.iter().any(|l| l.contains("rejected")));
    for ev in events.iter().filter(|e| e.kind != GitActivityKind::Started) {
        assert!(ev.repo_id.is_none());
        assert!(
            json(ev).get("repoId").is_none(),
            "{:?} carried repoId",
            ev.kind
        );
    }
}
