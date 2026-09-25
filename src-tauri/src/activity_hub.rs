//! P87/P123: the git-activity fan-out hub. Every `git_activity_subscribe`
//! channel is registered here with the ONE repo it listens to; a run emits only
//! onto the channels of its own repo (a hidden tab of another repo receives
//! zero messages), and an unscoped run (clone/init, P123 D3) broadcasts.
//!
//! A subscription lives until `git_activity_unsubscribe` drops it (the
//! frontend's unmount); a channel whose `send` fails (reload/window close) is
//! also pruned lazily on the next emit — the safety net, not the mechanism.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use bonsai_core::git::activity::GitActivityEvent;
use tauri::ipc::Channel;

/// One registered channel. `repo_id: None` = wildcard (receives every run);
/// reachable only from Rust test helpers — the command always passes `Some`.
struct Sub {
    id: String,
    repo_id: Option<String>,
    ch: Channel<GitActivityEvent>,
}

/// route `None`    → every sub (an unscoped run broadcasts, D3).
/// route `Some(r)` → the subs of repo `r`, plus any wildcard sub.
fn matches(sub: &Sub, route: Option<&str>) -> bool {
    match route {
        None => true,
        Some(r) => sub.repo_id.as_deref().is_none_or(|own| own == r),
    }
}

/// CLONE-able over a shared `Arc` so a bracket can move a handle into its
/// emitter closure while `git_activity_subscribe` mutates the same list.
#[derive(Clone, Default)]
pub struct GitActivityHub {
    subs: Arc<Mutex<Vec<Sub>>>,
    next_id: Arc<AtomicU64>,
}

impl GitActivityHub {
    /// Registers a channel and returns its subscription id (`gas-<n>`, unique
    /// per hub). A string, not a number: the obs proxy scopes supersede by the
    /// first string arg (P123 D5).
    pub fn subscribe(&self, repo_id: Option<String>, ch: Channel<GitActivityEvent>) -> String {
        let id = format!("gas-{}", self.next_id.fetch_add(1, Ordering::Relaxed));
        self.lock().push(Sub {
            id: id.clone(),
            repo_id,
            ch,
        });
        id
    }

    /// Removes exactly the matching sub. IDEMPOTENT: an unknown or
    /// already-removed id returns `false` and does nothing else.
    pub fn unsubscribe(&self, sub_id: &str) -> bool {
        let mut subs = self.lock();
        match subs.iter().position(|s| s.id == sub_id) {
            Some(i) => {
                subs.remove(i);
                true
            }
            None => false,
        }
    }

    /// Fans `ev` out to the subs matching `route`, pruning any whose `send`
    /// errors (the frontend dropped it). A cheap no-op with no subscribers.
    pub fn emit_to(&self, route: Option<&str>, ev: GitActivityEvent) {
        let mut subs = self.lock();
        if subs.is_empty() {
            return;
        }
        subs.retain(|s| !matches(s, route) || s.ch.send(ev.clone()).is_ok());
    }

    /// True if any sub matches `route`.
    pub fn is_active_for(&self, route: Option<&str>) -> bool {
        self.lock().iter().any(|s| matches(s, route))
    }

    /// True if any sub exists. Kept for the pure, repo-less gates
    /// (`arg_target`, `arg_subject_many`), which only save work; the scoped
    /// bracket makes the real decision.
    pub fn is_active(&self) -> bool {
        !self.lock().is_empty()
    }

    /// Test/diagnostic count of live subs.
    pub fn subscriber_count(&self) -> usize {
        self.lock().len()
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Sub>> {
        // Plain Vec push/retain — a poisoned lock is recoverable (no invariant
        // spans a panic), so recover rather than fail every later emit.
        self.subs.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A hub handle bound to one route. The brackets take this instead of the bare
/// hub, so a bracket cannot emit unscoped by accident.
#[derive(Clone)]
pub struct ScopedActivityHub {
    hub: GitActivityHub,
    repo_id: Option<String>,
}

impl ScopedActivityHub {
    pub fn new(hub: GitActivityHub, repo_id: Option<String>) -> Self {
        ScopedActivityHub { hub, repo_id }
    }

    pub fn repo_id(&self) -> Option<&str> {
        self.repo_id.as_deref()
    }

    /// Anyone listening to THIS route? `false` ⇒ the bracket takes the
    /// buffered passthrough.
    pub fn is_active(&self) -> bool {
        self.hub.is_active_for(self.repo_id())
    }

    pub fn emit(&self, ev: GitActivityEvent) {
        self.hub.emit_to(self.repo_id(), ev);
    }
}

#[cfg(test)]
#[path = "activity_hub_tests.rs"]
mod tests;
