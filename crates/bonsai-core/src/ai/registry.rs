//! Run-id -> cancel/reply plumbing for streaming AI runs (P68 §B/D7).
//!
//! A Tauri command cannot be aborted from JS, so cancellation is a SECOND
//! command that flips an `AtomicBool` this registry owns; the session notices on
//! its next `RECV_TICK`. Mint here, hand the [`RunControl`] to the session, and
//! `finish` on every exit path so nothing leaks.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use super::kill_pid_tree;
use super::session::RunControl;
use crate::error::AppError;

/// Per-run handles kept by the registry. The session owns the matching
/// [`RunControl`] (same `Arc`s, the receiving end of `reply_tx`).
pub struct AiRunHandle {
    pub cancel: Arc<AtomicBool>,
    pub awaiting: Arc<AtomicBool>,
    pub pid: Arc<AtomicU32>,
    reply_tx: Sender<String>,
}

/// CLONE-able handle over a shared map so it can be `.manage()`d on the Tauri app
/// AND moved into `spawn_blocking` (`tauri::State` only yields a borrow).
/// Mirrors `McpServerState` as managed state (`src-tauri/src/mcp.rs:104`).
#[derive(Clone, Default)]
pub struct AiRunRegistry {
    inner: Arc<Mutex<HashMap<String, AiRunHandle>>>,
}

impl AiRunRegistry {
    /// Poisoning is never fatal here: the map is plain data, so recover the guard
    /// rather than panicking in a command (no `unwrap` on shared state).
    fn map(&self) -> MutexGuard<'_, HashMap<String, AiRunHandle>> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Mint a run id and register it. NO new dependency (A7): the id is
    /// `ai-<nanos since UNIX_EPOCH, hex>-<process-global counter>` — unique per
    /// process and unguessable enough for a local channel key.
    pub fn register(&self) -> (String, RunControl) {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let run_id = format!("ai-{nanos:x}-{n}");

        let cancel = Arc::new(AtomicBool::new(false));
        let awaiting = Arc::new(AtomicBool::new(false));
        let pid = Arc::new(AtomicU32::new(0));
        let (reply_tx, replies) = channel::<String>();

        self.map().insert(
            run_id.clone(),
            AiRunHandle {
                cancel: Arc::clone(&cancel),
                awaiting: Arc::clone(&awaiting),
                pid: Arc::clone(&pid),
                reply_tx,
            },
        );
        let ctl = RunControl { run_id: run_id.clone(), cancel, awaiting, pid, replies };
        (run_id, ctl)
    }

    /// Flip the cancel flag. IDEMPOTENT: an unknown id is `false` (the command
    /// still returns Ok — a cancel racing a completion is normal, D8).
    pub fn cancel(&self, run_id: &str) -> bool {
        match self.map().get(run_id) {
            Some(h) => {
                h.cancel.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    /// Queue a user reply for a run that is awaiting input. `AiFailed` when the id
    /// is unknown OR the run is not awaiting input, so a stray reply can never be
    /// silently swallowed.
    pub fn reply(&self, run_id: &str, text: String) -> Result<(), AppError> {
        let map = self.map();
        let Some(h) = map.get(run_id) else {
            return Err(AppError::AiFailed(format!("AI run {run_id} is no longer active")));
        };
        if !h.awaiting.load(Ordering::Relaxed) {
            return Err(AppError::AiFailed(format!("AI run {run_id} is not waiting for input")));
        }
        h.reply_tx
            .send(text)
            .map_err(|_| AppError::AiFailed(format!("AI run {run_id} stopped listening for input")))
    }

    /// True while the run is blocked on a question (drives the reply affordance).
    pub fn is_awaiting(&self, run_id: &str) -> bool {
        self.map().get(run_id).is_some_and(|h| h.awaiting.load(Ordering::Relaxed))
    }

    /// Drop the entry. MUST be called on EVERY exit path of the command (success,
    /// failure, cancel) — use a drop guard so a panic cannot leak an entry.
    pub fn finish(&self, run_id: &str) {
        self.map().remove(run_id);
    }

    /// App-exit hook (D7): flip every cancel flag AND kill every recorded pid tree,
    /// then clear. With no hard timeout a leaked child could otherwise run forever.
    /// Best-effort; never blocks meaningfully.
    pub fn cancel_all(&self) {
        let mut map = self.map();
        for h in map.values() {
            h.cancel.store(true, Ordering::Relaxed);
            kill_pid_tree(h.pid.load(Ordering::Relaxed));
        }
        map.clear();
    }

    pub fn active(&self) -> usize {
        self.map().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_mints_unique_ids_and_tracks_active() {
        let reg = AiRunRegistry::default();
        let (a, ctl_a) = reg.register();
        let (b, _ctl_b) = reg.register();
        assert_ne!(a, b, "ids must be unique");
        assert!(a.starts_with("ai-"), "unexpected id shape: {a}");
        assert_eq!(reg.active(), 2);
        assert_eq!(ctl_a.run_id, a);
        reg.finish(&a);
        assert_eq!(reg.active(), 1);
    }

    #[test]
    fn cancel_sets_the_flag_and_is_idempotent_for_unknown_ids() {
        let reg = AiRunRegistry::default();
        let (id, ctl) = reg.register();
        assert!(!ctl.cancel.load(Ordering::Relaxed));
        assert!(reg.cancel(&id));
        assert!(ctl.cancel.load(Ordering::Relaxed));
        assert!(reg.cancel(&id), "repeat cancel stays true");
        assert!(!reg.cancel("ai-nope"), "unknown id -> false, not an error");
    }

    #[test]
    fn reply_requires_an_awaiting_run() {
        let reg = AiRunRegistry::default();
        let (id, ctl) = reg.register();
        let err = reg.reply(&id, "x".into()).expect_err("not awaiting -> Err");
        assert!(
            matches!(&err, AppError::AiFailed(m) if m.contains("not waiting")),
            "got {err:?}"
        );
        assert!(reg.reply("ai-nope", "x".into()).is_err(), "unknown id -> Err");

        ctl.awaiting.store(true, Ordering::Relaxed);
        assert!(!reg.is_awaiting("ai-nope"));
        assert!(reg.is_awaiting(&id));
        reg.reply(&id, "answer".into()).expect("awaiting -> Ok");
        assert_eq!(ctl.replies.try_recv().ok().as_deref(), Some("answer"));
    }

    #[test]
    fn cancel_all_flips_every_flag_and_clears() {
        let reg = AiRunRegistry::default();
        let (_a, ctl_a) = reg.register();
        let (_b, ctl_b) = reg.register();
        // pid 0 = "not spawned": cancel_all must skip it, not kill pid 0.
        reg.cancel_all();
        assert!(ctl_a.cancel.load(Ordering::Relaxed));
        assert!(ctl_b.cancel.load(Ordering::Relaxed));
        assert_eq!(reg.active(), 0);
    }
}
