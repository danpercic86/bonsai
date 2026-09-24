//! P119 §6 — shared capture helpers for the `tests_activity_coverage*`
//! suites: a real `tauri::ipc::Channel::new` sink subscribed to the hub, so
//! the assertions read the exact JSON the frontend would see.

use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::tests_support::*;
use super::*;

pub(super) fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}

pub(super) type Log = Arc<Mutex<Vec<Value>>>;

/// Subscribes a capturing channel to `state`'s hub; every event arrives as the
/// exact JSON the frontend receives.
pub(super) fn subscribe(state: &AppState) -> Log {
    let log: Log = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    state
        .git_activity
        .subscribe(tauri::ipc::Channel::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(s) = body {
                let v: Value = serde_json::from_str(&s).expect("event is JSON");
                sink.lock().expect("log lock").push(v);
            }
            Ok(())
        }));
    log
}

/// Drains everything captured so far.
pub(super) fn take(log: &Log) -> Vec<Value> {
    std::mem::take(&mut *log.lock().expect("log lock"))
}

pub(super) fn kind(ev: &Value) -> &str {
    ev["kind"].as_str().unwrap_or("")
}

/// Asserts the drained events are exactly ONE run (one `started`, one
/// `finished`, same id) and returns `(started, finished)`.
pub(super) fn one_run(events: &[Value]) -> (Value, Value) {
    let started: Vec<&Value> = events.iter().filter(|e| kind(e) == "started").collect();
    let finished: Vec<&Value> = events.iter().filter(|e| kind(e) == "finished").collect();
    assert_eq!(started.len(), 1, "exactly one started: {events:#?}");
    assert_eq!(finished.len(), 1, "exactly one finished: {events:#?}");
    assert_eq!(started[0]["id"], finished[0]["id"], "one run id");
    assert_eq!(kind(&events[0]), "started", "started comes first");
    assert_eq!(kind(&events[events.len() - 1]), "finished", "finished last");
    (started[0].clone(), finished[0].clone())
}

/// Runs `op`, asserts it logged one run of `category` targeting `target`
/// (`None` = no `target` key), and returns `(op result, finished event)`.
pub(super) fn logged<T>(
    log: &Log,
    category: &str,
    target: Option<&str>,
    op: impl std::future::Future<Output = Result<T, AppError>>,
) -> (Result<T, AppError>, Value) {
    take(log);
    let out = block_on(op);
    let events = take(log);
    let (started, finished) = one_run(&events);
    assert_eq!(started["category"], category, "{events:#?}");
    match target {
        Some(t) => assert_eq!(started["target"], t, "{category}: {events:#?}"),
        None => assert!(started.get("target").is_none(), "{category}: {events:#?}"),
    }
    assert!(
        started.get("targetCount").is_none(),
        "{category}: single-target run"
    );
    (out, finished)
}

pub(super) fn short(oid: &str) -> &str {
    &oid[..7]
}

/// C0 ← main; "feature" from C0. Both sides edit a.txt (so a merge / rebase /
/// cherry-pick of feature conflicts). Ends on main. Returns (main, feature tip).
pub(super) fn conflicting_sides(
    state: &AppState,
    id: &str,
    dir: &std::path::Path,
    c0: &str,
) -> (String, String) {
    let main = head_branch(dir).expect("attached head");
    block_on(create_branch_here_inner(
        state,
        id,
        "feature".into(),
        c0.into(),
    ))
    .expect("feature");
    let f_tip = write_stage_commit(state, id, dir, "a.txt", "feature\n", "feature edit").oid;
    block_on(checkout_branch_inner(state, id, main.clone())).expect("back to main");
    write_stage_commit(state, id, dir, "a.txt", "main\n", "main edit");
    (main, f_tip)
}
