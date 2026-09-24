//! P119 §2.5: bridge clone transfer progress onto the git-activity stream.
//!
//! `clone::clone_repo` already reports [`CloneProgress`] through a plain sink
//! (for its own Tauri channel). The activity row wants the same counts as a
//! [`GitTransferProgress`], with the same §14.3 coalescing fetch/pull use, so
//! the dock bar is determinate during a clone too.

use std::time::Instant;

use crate::git::activity::{GitActivityRecorder, GitTransferProgress};
use crate::git::clone::CloneProgress;
use crate::git::remote_activity::progress_should_fire;

/// Throttled `CloneProgress` → recorder bridge. A no-op when `rec` is `None`.
pub struct CloneActivityForwarder<'a> {
    rec: Option<&'a dyn GitActivityRecorder>,
    last: Option<Instant>,
    done_emitted: bool,
}

impl<'a> CloneActivityForwarder<'a> {
    pub fn new(rec: Option<&'a dyn GitActivityRecorder>) -> Self {
        CloneActivityForwarder {
            rec,
            last: None,
            done_emitted: false,
        }
    }

    /// Forward one tick, coalesced to ≤1 per `PROGRESS_MIN_INTERVAL` except the
    /// first terminal (`received == total`) tick, which always fires.
    pub fn tick(&mut self, p: &CloneProgress) {
        self.tick_at(p, Instant::now());
    }

    /// [`Self::tick`] with an explicit clock (the coalescing is testable
    /// without sleeping).
    fn tick_at(&mut self, p: &CloneProgress, now: Instant) {
        let Some(rec) = self.rec else {
            return;
        };
        let done = p.total_objects > 0 && p.received_objects == p.total_objects;
        if progress_should_fire(self.last, now, done, self.done_emitted) {
            rec.progress(to_transfer_progress(p));
            self.last = Some(now);
            self.done_emitted |= done;
        }
    }
}

/// `CloneProgress` has no `indexed_objects`; received stands in for it. Deltas
/// are `Some` only during delta resolution, as for fetch/pull.
fn to_transfer_progress(p: &CloneProgress) -> GitTransferProgress {
    let resolving = p.total_deltas > 0;
    GitTransferProgress {
        received_objects: p.received_objects,
        total_objects: p.total_objects,
        indexed_objects: p.received_objects,
        received_bytes: p.received_bytes,
        total_deltas: resolving.then_some(p.total_deltas),
        indexed_deltas: resolving.then_some(p.indexed_deltas),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::activity::{GitPhaseKind, GitStream};
    use std::sync::Mutex;
    use std::time::Duration;

    #[derive(Default)]
    struct Rec(Mutex<Vec<GitTransferProgress>>);
    impl GitActivityRecorder for Rec {
        fn phase(&self, _k: GitPhaseKind, _h: Option<&str>) {}
        fn line(&self, _s: GitStream, _l: &str) {}
        fn hook_done(&self, _h: &str, _c: Option<i32>, _s: bool) {}
        fn progress(&self, p: GitTransferProgress) {
            self.0.lock().expect("lock").push(p);
        }
    }

    fn tick(received: u32, total: u32) -> CloneProgress {
        CloneProgress {
            received_objects: received,
            total_objects: total,
            indexed_deltas: 0,
            total_deltas: 0,
            received_bytes: u64::from(received) * 10,
        }
    }

    #[test]
    fn coalesces_within_the_window_and_forces_the_terminal_tick() {
        let rec = Rec::default();
        let mut f = CloneActivityForwarder::new(Some(&rec));
        let t0 = Instant::now();
        f.tick_at(&tick(1, 10), t0); // first: fires
        f.tick_at(&tick(2, 10), t0 + Duration::from_millis(10)); // coalesced
        f.tick_at(&tick(10, 10), t0 + Duration::from_millis(20)); // terminal: forced
        f.tick_at(&tick(10, 10), t0 + Duration::from_millis(30)); // already done
        f.tick_at(&tick(10, 10), t0 + Duration::from_millis(100)); // past window
        let got = rec.0.lock().expect("lock");
        let received: Vec<u32> = got.iter().map(|p| p.received_objects).collect();
        assert_eq!(received, vec![1, 10, 10]);
        assert_eq!(got[0].indexed_objects, 1, "received stands in for indexed");
        assert_eq!(got[0].total_deltas, None);
    }

    #[test]
    fn deltas_present_only_while_resolving() {
        let rec = Rec::default();
        let mut f = CloneActivityForwarder::new(Some(&rec));
        let mut p = tick(10, 10);
        p.total_deltas = 4;
        p.indexed_deltas = 3;
        f.tick(&p);
        let got = rec.0.lock().expect("lock");
        assert_eq!(got[0].total_deltas, Some(4));
        assert_eq!(got[0].indexed_deltas, Some(3));
    }

    #[test]
    fn no_recorder_is_a_no_op() {
        let mut f = CloneActivityForwarder::new(None);
        f.tick(&tick(1, 10));
        f.tick(&tick(10, 10));
        assert!(f.last.is_none(), "nothing fired, so no clock was recorded");
    }
}
