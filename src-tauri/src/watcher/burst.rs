//! The per-debounce-window reduction (`BurstAcc`), the fire-time ignore
//! measurement (P121d) and the pure drop rule for all-ignored bursts (P124b).
//!
//! Split out of `mod.rs` so the rules the watcher's firing decision rests on
//! are testable without wall-clock timing or a real `notify` watch.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::BurstClass;

/// P121d: how many worktree paths a burst retains for the ignore check. The
/// total is still counted past the cap (and a capped burst is never dropped).
pub(super) const IGNORE_SAMPLE_CAP: usize = 512;

/// P124b: a burst with more raw notify paths than this (incl. `.git` noise) is
/// storm-shaped and always fires — a Windows buffer overflow hides inside
/// storms, and a lost tracked event must not be dropped with it.
pub(super) const DROP_MAX_PATHS: u32 = 128;

/// P124b: an all-ignored burst is dropped only while the last `on_change` is
/// younger than this, so a tracked event lost to an overflow inside
/// ignored-only churn surfaces at most this late (or at focus / manual).
pub(super) const IGNORED_HEARTBEAT: Duration = Duration::from_secs(300);

/// One notify batch, reduced to the counts P91 §2.4 records. The paths
/// themselves never cross into the log — only how many, and how many passed the
/// `.git`-internals filter.
#[derive(Clone, Default)]
pub(super) struct WatchTick {
    pub(super) paths: u32,
    pub(super) relevant: u32,
    /// True when at least one relevant path in this batch was `PathClass::Refs`
    /// (or the batch was a notify error, which is treated as ref-affecting).
    pub(super) refs: bool,
    /// The batch's `Worktree`-class absolute paths, cloned only up to
    /// `IGNORE_SAMPLE_CAP` per tick. Never logged — only counted.
    pub(super) worktree_paths: Vec<PathBuf>,
    /// P124b: EVERY `Worktree`-class path in the batch, past the cap too.
    pub(super) worktree_count: u32,
    /// P113d: this tick is a notify `Err(_)` (true only on that arm).
    pub(super) error: bool,
}

/// The running reduction of ONE debounce window: the coalesced counts plus the
/// conservative burst class. Split out of the drain loop so the accumulation
/// rule this whole feature rests on is testable without wall-clock timing.
pub(super) struct BurstAcc {
    pub(super) paths: u32,
    pub(super) relevant: u32,
    pub(super) refs: bool,
    /// Absolute worktree paths (≤ `IGNORE_SAMPLE_CAP`, not deduped). Never
    /// leaves the debounce thread and is never logged — only counted.
    pub(super) worktree_paths: Vec<PathBuf>,
    /// Every worktree path the window saw, past the cap too.
    pub(super) worktree_total: u32,
    /// P113d: notify batches coalesced (saturating, +1 per absorb incl. `new`).
    pub(super) batches: u32,
    /// P113d: `Err(_)` ticks coalesced (saturating, +1 per `t.error`).
    pub(super) errors: u32,
    /// P113d: `Instant::now()` in `new`; `burst_ms = started.elapsed()` at fire.
    pub(super) started: Instant,
}

impl BurstAcc {
    pub(super) fn new(first: WatchTick) -> Self {
        let mut acc = Self {
            paths: 0,
            relevant: 0,
            refs: false,
            worktree_paths: Vec::new(),
            worktree_total: 0,
            batches: 0,
            errors: 0,
            started: Instant::now(),
        };
        acc.absorb(first);
        acc
    }

    /// Folds one more notify batch into the window. CONSERVATIVE: one Refs path
    /// anywhere in the coalesced burst makes the WHOLE burst Refs.
    pub(super) fn absorb(&mut self, t: WatchTick) {
        self.paths = self.paths.saturating_add(t.paths);
        self.relevant = self.relevant.saturating_add(t.relevant);
        self.refs |= t.refs;
        self.batches = self.batches.saturating_add(1);
        self.errors = self.errors.saturating_add(u32::from(t.error));
        self.worktree_total = self.worktree_total.saturating_add(t.worktree_count);
        let room = IGNORE_SAMPLE_CAP.saturating_sub(self.worktree_paths.len());
        self.worktree_paths
            .extend(t.worktree_paths.into_iter().take(room));
    }

    pub(super) fn class(&self) -> BurstClass {
        if self.refs {
            BurstClass::Refs
        } else {
            BurstClass::Worktree
        }
    }
}

/// Pure form of the window reduction (tests drive this directly; the debounce
/// loop drives the same `BurstAcc` incrementally, so there is ONE fold).
#[cfg(test)]
pub(super) fn accumulate(first: WatchTick, rest: &[WatchTick]) -> (u32, u32, BurstClass) {
    let mut acc = BurstAcc::new(first);
    for t in rest {
        acc.absorb(t.clone());
    }
    (acc.paths, acc.relevant, acc.class())
}

/// The ignore measurement of one fired burst (counts only).
pub(super) struct IgnoreStats {
    /// `BurstAcc::worktree_total`.
    pub(super) worktree_paths: u32,
    /// Sampled paths that were workdir-relative and checked; None if counting
    /// failed (repo / index unreadable — then both counts are None too).
    pub(super) ignore_checked: Option<u32>,
    pub(super) ignored: Option<u32>,
    /// P124b: ignored AND untracked AND not inside a submodule.
    pub(super) ignored_untracked: Option<u32>,
    /// P124b: every sampled path had a NON-EMPTY workdir-relative form (a path
    /// outside the workdir, or the workdir root itself, makes this false).
    pub(super) all_relative: bool,
}

/// Measures how many of the burst's sampled worktree paths are gitignored (and
/// untracked). None when the burst had no worktree paths. Blocking (opens the
/// repo + reads the index); runs on the debounce thread, never a tokio worker.
pub(super) fn ignore_stats(workdir: &Path, acc: &BurstAcc) -> Option<IgnoreStats> {
    if acc.worktree_total == 0 {
        return None;
    }
    let rel: Vec<PathBuf> = acc
        .worktree_paths
        .iter()
        .filter_map(|p| p.strip_prefix(workdir).ok().map(Path::to_path_buf))
        .collect();
    let all_relative =
        rel.len() == acc.worktree_paths.len() && rel.iter().all(|r| !r.as_os_str().is_empty());
    let count = bonsai_core::git::ignore::count_ignored_paths(workdir, &rel);
    let checked = u32::try_from(rel.len()).unwrap_or(u32::MAX);
    Some(IgnoreStats {
        worktree_paths: acc.worktree_total,
        ignore_checked: count.map(|_| checked),
        ignored: count.map(|c| c.ignored),
        ignored_untracked: count.map(|c| c.ignored_untracked),
        all_relative,
    })
}

/// The four git-free drop conditions — decides whether the ignore check is
/// worth running at all (it is skipped for every other burst unless Dev mode
/// wants the measurement).
pub(super) fn drop_eligible(acc: &BurstAcc) -> bool {
    !acc.refs
        && acc.worktree_total > 0
        && acc.paths <= DROP_MAX_PATHS
        && acc.worktree_total as usize == acc.worktree_paths.len()
}

/// What the debounce loop does with a quiet window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BurstVerdict {
    Fire,
    DropIgnored,
}

/// PURE — P124b's drop rule. Drops ONLY a small, uncapped, refs-free burst
/// whose every path is relative, ignored and untracked, while the last firing
/// is younger than `IGNORED_HEARTBEAT`. Anything unknown fires.
pub(super) fn burst_verdict(
    acc: &BurstAcc,
    stats: Option<&IgnoreStats>,
    since_last_fire: Duration,
) -> BurstVerdict {
    let Some(s) = stats else {
        return BurstVerdict::Fire;
    };
    if drop_eligible(acc)
        && s.all_relative
        && s.ignored_untracked == Some(acc.worktree_total)
        && since_last_fire < IGNORED_HEARTBEAT
    {
        BurstVerdict::DropIgnored
    } else {
        BurstVerdict::Fire
    }
}
