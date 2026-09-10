//! P87 git-activity observability command surface.
//!
//! `git_activity_subscribe` registers ONE long-lived channel (Option B — a
//! session log spans many ops, and future ops appear automatically) with the
//! AppState [`GitActivityHub`]. Every op inner wraps its core call in
//! [`with_activity`], which emits `started`/`finished` and threads an
//! [`ActivityEmitter`] (as `&dyn GitActivityRecorder`) into `spawn_blocking`.
//!
//! Fire-and-forget: nothing here gates or changes an op's success/error. With no
//! subscriber the bracket is a straight passthrough (buffered path, no events).

use std::future::Future;
use std::sync::Arc;

use bonsai_core::error::AppError;
use bonsai_core::git::activity::{
    new_activity_id, ActivityEmitter, ActivityTarget, GitActivityCategory, GitActivityEvent,
    GitPhaseKind,
};
use bonsai_core::git::activity_target::resolve_activity_target;

use crate::commands::shared::repo_path;
use crate::state::{AppState, GitActivityHub};

/// Registers a long-lived channel that receives `GitActivityEvent`s for EVERY
/// git op this session. Called once by the frontend on app/repo mount; re-invoked
/// after an HMR/reload (stale channels are pruned on send failure). Returns
/// immediately.
#[tauri::command]
pub fn git_activity_subscribe(
    state: tauri::State<'_, AppState>,
    on_event: tauri::ipc::Channel<GitActivityEvent>,
) {
    state.git_activity.subscribe(on_event);
}

/// The command bracket every activity-emitting op inner runs its core call
/// inside. When someone is listening it mints an [`ActivityEmitter`], emits
/// `started` (seq 0) then `finished` around `run`, and hands the emitter to
/// `run` as `Some(..)`. When NObody is listening it is a straight passthrough
/// (`run(None)`) — the buffered path, no emitter, no events (contract §10).
///
/// `target` (FU-1 §5.2) is the run's ref, resolved by [`activity_target`] BEFORE
/// the bracket; it rides on the `started` event only. A target computed under a
/// race with the last unsubscribe is simply dropped by the passthrough below.
///
/// `run` threads the emitter into the core call inside its own `spawn_blocking`
/// (deriving a `&dyn GitActivityRecorder` from the `Arc`).
pub(crate) async fn with_activity<T, F, Fut>(
    hub: GitActivityHub,
    category: GitActivityCategory,
    target: Option<ActivityTarget>,
    run: F,
) -> Result<T, AppError>
where
    F: FnOnce(Option<Arc<ActivityEmitter>>) -> Fut,
    Fut: Future<Output = Result<T, AppError>>,
{
    if !hub.is_active() {
        return run(None).await;
    }
    let hub2 = hub.clone();
    let emitter = Arc::new(ActivityEmitter::new(
        new_activity_id(),
        target,
        Box::new(move |ev| hub2.emit(ev)),
    ));
    emitter.started(category, GitPhaseKind::Preparing);
    let res = run(Some(Arc::clone(&emitter))).await;
    match &res {
        Ok(_) => emitter.finished(Some(0), true),
        Err(e) => emitter.finished(activity_exit_code(e), false),
    }
    res
}

/// Resolve a run's target ref before the run starts (FU-1 §5.1).
///
/// `None` when nobody is subscribed (so **no repo open is paid** on the hot
/// path — the `is_active` gate short-circuits before any path work), when the
/// repo id is unknown, or when the resolver declined. NEVER returns an error: a
/// failure here must not fail the op, and swallowing the `repo_path` error is
/// deliberate — the op's own `repo_path?` inside `run` still produces the real
/// error and the failed run row, exactly as before.
pub(crate) async fn activity_target(
    state: &AppState,
    repo_id: &str,
    category: GitActivityCategory,
) -> Option<ActivityTarget> {
    if !state.git_activity.is_active() {
        return None;
    }
    let path = repo_path(state, repo_id).ok()?;
    // git2 is blocking, so the read goes to the blocking pool like every other
    // git call; a join error is just another `None`.
    tauri::async_runtime::spawn_blocking(move || resolve_activity_target(&path, category))
        .await
        .ok()
        .flatten()
}

/// Best-effort `AppError` → terminal exit code. A `HookRejected` has no single
/// exit code (the hook's own code is on its `hookDone` event) ⇒ `None` ("killed /
/// no exit code"); every other failure is a generic non-zero.
fn activity_exit_code(e: &AppError) -> Option<i32> {
    match e {
        AppError::HookRejected(_) => None,
        _ => Some(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Drains events straight into a Vec via a Channel-shaped closure is awkward
    /// (Channel needs an ipc runtime), so exercise the emitter directly through a
    /// recording closure — the bracket's own logic (is_active gate, started/
    /// finished, exit-code map) is what we assert here.
    fn hub_with_recorder() -> (GitActivityHub, Arc<Mutex<Vec<GitActivityEvent>>>) {
        // A real hub with no subscriber is inactive; to test the active path we
        // reach through `emit` indirectly is not possible without a Channel, so
        // these tests focus on `activity_exit_code` + the inactive passthrough.
        (GitActivityHub::default(), Arc::new(Mutex::new(Vec::new())))
    }

    #[test]
    fn inactive_hub_is_passthrough_with_none_recorder() {
        let (hub, _log) = hub_with_recorder();
        let saw: Arc<Mutex<Option<bool>>> = Arc::new(Mutex::new(None));
        let saw2 = Arc::clone(&saw);
        let out: Result<u32, AppError> =
            tauri::async_runtime::block_on(with_activity(hub, GitActivityCategory::Push, None, move |em| {
                let saw2 = saw2.clone();
                async move {
                    *saw2.lock().expect("lock") = Some(em.is_none());
                    Ok(7)
                }
            }));
        assert_eq!(out.ok(), Some(7));
        assert_eq!(*saw.lock().expect("lock"), Some(true), "no subscriber ⇒ None recorder");
    }

    /// FU-1 §9.5 — with nobody subscribed, `activity_target` must short-circuit
    /// on `is_active()` BEFORE any path work, so a repo whose workdir does not
    /// exist still yields a plain `None` (no error, no panic, no repo open). The
    /// missing path is the observable proxy: had the resolver run, git2 would
    /// have been asked to open it.
    #[test]
    fn inactive_hub_target_is_none_without_touching_the_repo() {
        let state = AppState::default();
        {
            let mut repos = state
                .repos
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            repos.insert(
                "bogus-repo".to_string(),
                crate::state::RepoEntry {
                    path: std::path::PathBuf::from("bonsai-p87b-fu1-nonexistent-workdir"),
                    watcher: None,
                    graph_cache: Arc::new(Mutex::new(None)),
                },
            );
        }
        assert!(!state.git_activity.is_active(), "no subscriber ⇒ inactive");
        for category in [
            GitActivityCategory::Push,
            GitActivityCategory::Commit,
            GitActivityCategory::Fetch,
        ] {
            let out = tauri::async_runtime::block_on(activity_target(
                &state,
                "bogus-repo",
                category,
            ));
            assert_eq!(out, None, "inactive hub ⇒ no target ({category:?})");
        }
        // An unknown repo id is equally silent (the op's own `repo_path?` is what
        // surfaces the real error).
        let out = tauri::async_runtime::block_on(activity_target(
            &state,
            "not-open",
            GitActivityCategory::Push,
        ));
        assert_eq!(out, None, "unknown repo id ⇒ no target");
    }

    #[test]
    fn exit_code_map() {
        assert_eq!(activity_exit_code(&AppError::HookRejected("x".into())), None);
        assert_eq!(activity_exit_code(&AppError::Git("x".into())), Some(1));
        assert_eq!(activity_exit_code(&AppError::NoRepo), Some(1));
    }
}
