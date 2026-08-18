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

    /// Mint a run id and register it, unconditionally. Prefer
    /// [`Self::register_within`] anywhere a concurrency cap applies.
    pub fn register(&self) -> (String, RunControl) {
        Self::insert(&mut self.map())
    }

    /// Register ONLY while fewer than `cap` runs are live. `Err(live)` = at
    /// capacity, carrying the count observed under the SAME lock as the check.
    ///
    /// This exists because a check-then-act pair is a real race here, not a
    /// theoretical one: Tauri polls every command as its own task, so two invokes
    /// arriving in the same JS tick (a double-clicked "Resolve all with AI", a
    /// retried IPC call, a second window) can both read `active() == cap - 1`,
    /// both fall through, and both `register()` — giving `cap + 1` live `claude`
    /// process trees against a metered subscription, with no wall-clock deadline
    /// and no default spend cap to bound them (D3/D7). Doing the check and the
    /// insert under one `MutexGuard` makes that impossible by construction.
    pub fn register_within(&self, cap: usize) -> Result<(String, RunControl), usize> {
        let mut map = self.map();
        let live = map.len();
        if live >= cap {
            return Err(live);
        }
        Ok(Self::insert(&mut map))
    }

    /// Mint + insert with the map lock ALREADY held (the caller owns the guard, so
    /// a capped registration cannot be split into two critical sections).
    ///
    /// NO new dependency (A7): the id is
    /// `ai-<nanos since UNIX_EPOCH, hex>-<process-global counter>` — unique per
    /// process and unguessable enough for a local channel key.
    fn insert(map: &mut HashMap<String, AiRunHandle>) -> (String, RunControl) {
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

        map.insert(
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
    fn register_within_refuses_at_the_cap_and_reports_the_live_count() {
        let reg = AiRunRegistry::default();
        let (a, _ctl_a) = reg.register_within(2).expect("first fits");
        let (_b, _ctl_b) = reg.register_within(2).expect("second fits");
        assert_eq!(reg.register_within(2).err(), Some(2), "at cap -> Err(live)");
        reg.finish(&a);
        assert!(reg.register_within(2).is_ok(), "a freed slot is reusable");
    }

    /// P68d FIX 1 (the P68c review's must-fix): the OLD shape was
    /// `active()` → drop the lock → `register()`, which two tasks racing inside one
    /// tick could both pass.
    ///
    /// WHAT ACTUALLY GUARANTEES THE CAP is structural, not this test: `register_within`
    /// does the count-and-insert under a SINGLE `MutexGuard` and returns `Err(live)`
    /// when it would overflow, so no interleaving exists in which two callers both
    /// observe a free slot. This test is a PROBABILISTIC smoke check over that
    /// property: `std::sync::Barrier` (a blocking rendezvous, not a spin barrier)
    /// releases 8 threads as close to simultaneously as the OS allows, 50 times, and
    /// the cap must hold every time. It reproduced the old bug reliably; it does not,
    /// and cannot, prove the new code correct.
    #[test]
    fn racing_registrations_can_never_both_pass_the_cap() {
        use std::sync::atomic::AtomicUsize;
        use std::sync::Barrier;

        const THREADS: usize = 8;
        const CAP: usize = 1;

        for _ in 0..50 {
            let reg = AiRunRegistry::default();
            let barrier = Arc::new(Barrier::new(THREADS));
            let wins = Arc::new(AtomicUsize::new(0));
            let mut handles = Vec::with_capacity(THREADS);
            for _ in 0..THREADS {
                let reg = reg.clone();
                let barrier = Arc::clone(&barrier);
                let wins = Arc::clone(&wins);
                handles.push(std::thread::spawn(move || {
                    barrier.wait();
                    // Keep the RunControl alive for the whole attempt window so a
                    // winner really does occupy its slot.
                    let got = reg.register_within(CAP);
                    if got.is_ok() {
                        wins.fetch_add(1, Ordering::SeqCst);
                    }
                    got.ok()
                }));
            }
            let kept: Vec<_> = handles.into_iter().filter_map(|h| h.join().ok()).collect();
            assert_eq!(wins.load(Ordering::SeqCst), CAP, "exactly `cap` registrations may win");
            assert_eq!(reg.active(), CAP, "the map itself must never exceed the cap");
            drop(kept);
        }
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
