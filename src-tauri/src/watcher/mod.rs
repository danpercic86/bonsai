//! File watcher for the open repository's working directory.
//!
//! Decoupled from Tauri on purpose: `commands.rs` wires `on_change` to an
//! `app.emit("repo-changed", …)`, tests wire it to a channel.
//!
//! Design (M1 contract §4):
//! - one recursive `notify` watch on the workdir (`.git` lives inside it);
//! - `.git`-internals filter: only `HEAD`, `index`, `refs/**`, `packed-refs`
//!   are relevant; `*.lock` files and everything else under `.git/` are noise.
//!   The same filter (`classify`) also CLASSIFIES each relevant path as
//!   `Worktree` or `Refs` so the debounced firing can tell the frontend whether
//!   the commit graph can possibly have changed (P110);
//! - 300 ms trailing-edge debounce on a dedicated thread (std mpsc,
//!   `recv_timeout`) — at most one callback per quiet period, no timers idle;
//! - clean shutdown: dropping `WatcherHandle` drops the watcher first (the
//!   channel sender disconnects), then joins the debounce thread.
//!
//! Limitation (v1, by contract): linked git worktrees (git_dir outside the
//! workdir) are out of scope — bare repos are rejected at open, so
//! `git_dir = workdir/.git` is always valid here. Also remember that
//! ReadDirectoryChangesW can miss events on Windows; the manual refresh
//! button and window-focus rescan are mandatory companions, not extras.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use notify::Watcher;

use bonsai_core::error::AppError;

mod classify;

pub(crate) use classify::classify;
pub use classify::PathClass;

/// P121d: how many worktree paths a fired burst retains for the dev-mode
/// ignore measurement. The total is still counted past the cap.
const IGNORE_SAMPLE_CAP: usize = 512;

/// The running reduction of ONE debounce window: the coalesced counts plus the
/// conservative burst class. Split out of the drain loop so the accumulation
/// rule this whole feature rests on is testable without wall-clock timing.
struct BurstAcc {
    paths: u32,
    relevant: u32,
    refs: bool,
    /// P121d: absolute worktree paths (≤ `IGNORE_SAMPLE_CAP`, not deduped).
    /// Empty unless Dev mode was on when the ticks were built. Never leaves the
    /// debounce thread and is never logged — only counted.
    worktree_paths: Vec<PathBuf>,
    /// P121d: every worktree path the window saw, past the cap too.
    worktree_total: u32,
}

impl BurstAcc {
    fn new(first: WatchTick) -> Self {
        let mut acc = Self {
            paths: 0,
            relevant: 0,
            refs: false,
            worktree_paths: Vec::new(),
            worktree_total: 0,
        };
        acc.absorb(first);
        acc
    }

    /// Folds one more notify batch into the window. CONSERVATIVE: one Refs path
    /// anywhere in the coalesced burst makes the WHOLE burst Refs.
    fn absorb(&mut self, t: WatchTick) {
        self.paths = self.paths.saturating_add(t.paths);
        self.relevant = self.relevant.saturating_add(t.relevant);
        self.refs |= t.refs;
        let n = u32::try_from(t.worktree_paths.len()).unwrap_or(u32::MAX);
        self.worktree_total = self.worktree_total.saturating_add(n);
        let room = IGNORE_SAMPLE_CAP.saturating_sub(self.worktree_paths.len());
        self.worktree_paths
            .extend(t.worktree_paths.into_iter().take(room));
    }

    fn class(&self) -> BurstClass {
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
fn accumulate(first: WatchTick, rest: &[WatchTick]) -> (u32, u32, BurstClass) {
    let mut acc = BurstAcc::new(first);
    for t in rest {
        acc.absorb(t.clone());
    }
    (acc.paths, acc.relevant, acc.class())
}

/// The class of a whole debounced burst — the same two-valued lattice as
/// [`PathClass`], accumulated CONSERVATIVELY: if any path in the coalesced
/// burst is `Refs`, the burst is `Refs`.
pub type BurstClass = PathClass;

const DEBOUNCE: Duration = Duration::from_millis(300);

/// One notify batch, reduced to the counts P91 §2.4 records. The paths
/// themselves never cross into the log — only how many, and how many passed the
/// `.git`-internals filter.
#[derive(Clone, Default)]
struct WatchTick {
    paths: u32,
    relevant: u32,
    /// True when at least one relevant path in this batch was `PathClass::Refs`
    /// (or the batch was a notify error, which is treated as ref-affecting).
    refs: bool,
    /// P121d: the batch's `Worktree`-class absolute paths, collected ONLY while
    /// Dev mode is on (`active_sink().is_some()`) — zero work otherwise.
    worktree_paths: Vec<PathBuf>,
}

/// P121d: the dev-mode ignore measurement of one fired burst (counts only).
struct IgnoreStats {
    /// `BurstAcc::worktree_total`.
    worktree_paths: u32,
    /// Sampled paths that were workdir-relative and checked; None if the repo
    /// could not be opened (then `ignored` is None too).
    ignore_checked: Option<u32>,
    ignored: Option<u32>,
}

/// P121d: measures how many of the burst's sampled worktree paths are
/// gitignored. None when the burst had no worktree paths (refs-only, or Dev
/// mode off so nothing was sampled). Blocking (opens the repo); runs on the
/// debounce thread, never a tokio worker. Paths outside `workdir` are skipped.
fn ignore_stats(workdir: &Path, acc: &BurstAcc) -> Option<IgnoreStats> {
    if acc.worktree_total == 0 {
        return None;
    }
    let rel: Vec<PathBuf> = acc
        .worktree_paths
        .iter()
        .filter_map(|p| p.strip_prefix(workdir).ok().map(Path::to_path_buf))
        .collect();
    let ignored = bonsai_core::git::ignore::count_ignored(workdir, &rel);
    let checked = u32::try_from(rel.len()).unwrap_or(u32::MAX);
    Some(IgnoreStats {
        worktree_paths: acc.worktree_total,
        ignore_checked: ignored.map(|_| checked),
        ignored,
    })
}

/// Logs a `watcher` record via the global obs sink (§2.4, `root("watcher")`).
/// A no-op when Dev mode is off (the sink is `None`). The watcher is
/// Tauri-decoupled and holds no `AppHandle`, so it goes through the same
/// process-wide sink handle the phase recorder uses.
fn log_watcher(
    paths: u32,
    relevant: u32,
    fired: bool,
    class: Option<BurstClass>,
    ignore: Option<IgnoreStats>,
) {
    use crate::obs::record::{LogLevel, LogPayload};
    let Some(sink) = crate::obs::trace::active_sink() else {
        return;
    };
    let meta = crate::obs::TraceMeta::root("watcher");
    let rec = crate::obs::trace::make_record(
        &sink,
        LogLevel::Debug,
        &meta,
        LogPayload::Watcher {
            paths,
            relevant,
            debounce_ms: DEBOUNCE.as_millis() as u32,
            fired,
            suppressed: false,
            suppress_reason: None,
            burst_class: class.map(|c| {
                match c {
                    PathClass::Worktree => "worktree",
                    PathClass::Refs => "refs",
                }
                .to_string()
            }),
            worktree_paths: ignore.as_ref().map(|s| s.worktree_paths),
            ignore_checked: ignore.as_ref().and_then(|s| s.ignore_checked),
            ignored: ignore.as_ref().and_then(|s| s.ignored),
        },
    );
    sink.enqueue(rec);
}

/// Keeps the watcher and its debounce thread alive; dropping it stops both.
pub struct WatcherHandle {
    // Field order matters: `watcher` is declared (and thus dropped) first,
    // which disconnects the channel sender and lets the debounce thread exit.
    watcher: Option<notify::RecommendedWatcher>,
    debounce_thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        // Drop the watcher first so its event handler (holding the tx) stops
        // and the channel disconnects, then join the debounce thread — it
        // returns promptly on `Err(Disconnected)`.
        drop(self.watcher.take());
        if let Some(t) = self.debounce_thread.take() {
            let _ = t.join();
        }
    }
}

/// Starts watching `workdir` recursively and invokes `on_change` (on the
/// debounce thread) at most once per 300 ms quiet period after relevant
/// activity, passing the burst's [`BurstClass`]. Notify error events count as
/// relevant AND as `Refs` (a full refresh is cheap and safe when we don't know
/// what we missed). Fast to call; only the initial watch registration is
/// synchronous.
pub fn spawn_watcher(
    workdir: &Path,
    on_change: Box<dyn Fn(BurstClass) + Send + 'static>,
) -> Result<WatcherHandle, AppError> {
    // Canonicalize: macOS FSEvents resolves symlinks in the paths it reports
    // (e.g. `/var/...` -> `/private/var/...`, true of every macOS temp dir),
    // so comparing against an unresolved `workdir` silently breaks the
    // `.git`-internals filter below — every event looks "outside .git".
    let workdir = std::fs::canonicalize(workdir)
        .map_err(|e| AppError::Other(format!("failed to resolve {}: {e}", workdir.display())))?;
    let git_dir = workdir.join(".git");
    // P121d: the debounce thread needs the same canonical root to make the
    // sampled paths relative for the ignore check.
    let fire_workdir = workdir.clone();
    let (tx, rx) = mpsc::channel::<WatchTick>();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        // P91 §2.4: log the raw notify batch (counts only), then forward a tick
        // carrying the same counts so the debounce firing can report the total
        // paths/relevant that were coalesced into it.
        let tick = match &res {
            Ok(event) => {
                let paths = event.paths.len() as u32;
                // P121d: sample worktree paths only in Dev mode (no allocation
                // when off — an empty Vec does not allocate).
                let sample = crate::obs::trace::active_sink().is_some();
                let mut relevant = 0u32;
                let mut refs = false;
                let mut worktree_paths = Vec::new();
                for p in &event.paths {
                    let Some(c) = classify(p, &git_dir) else {
                        continue;
                    };
                    relevant += 1;
                    refs |= c == PathClass::Refs;
                    if sample && c == PathClass::Worktree {
                        worktree_paths.push(p.clone());
                    }
                }
                WatchTick {
                    paths,
                    relevant,
                    refs,
                    worktree_paths,
                }
            }
            // watcher error → trigger a refresh, cheap and safe; count it as one
            // relevant path so the debounce fires, and as ref-affecting so the
            // frontend runs the WIDE refresh (we don't know what was dropped).
            Err(_) => WatchTick {
                paths: 1,
                relevant: 1,
                refs: true,
                worktree_paths: Vec::new(),
            },
        };
        log_watcher(tick.paths, tick.relevant, false, None, None);
        if tick.relevant > 0 {
            let _ = tx.send(tick); // receiver gone: watcher is being torn down
        }
    })
    .map_err(|e| AppError::Other(format!("failed to create file watcher: {e}")))?;

    watcher
        .watch(&workdir, notify::RecursiveMode::Recursive)
        .map_err(|e| AppError::Other(format!("failed to watch {}: {e}", workdir.display())))?;

    let debounce_thread = std::thread::Builder::new()
        .name("bonsai-watch-debounce".into())
        .spawn(move || {
            loop {
                match rx.recv() {
                    Err(mpsc::RecvError) => return, // watcher dropped → clean shutdown
                    Ok(first) => {
                        // Accumulate the coalesced counts + burst class across
                        // the debounce window so the `fired` record and the
                        // callback both describe the whole burst.
                        let mut acc = BurstAcc::new(first);
                        // Drain until 300 ms of quiet, then fire once.
                        loop {
                            match rx.recv_timeout(DEBOUNCE) {
                                Ok(t) => {
                                    acc.absorb(t);
                                    continue; // storm ongoing, keep absorbing
                                }
                                Err(mpsc::RecvTimeoutError::Timeout) => {
                                    let class = acc.class();
                                    // P121d: Dev mode only (sampling is gated
                                    // on the sink too). Logged BEFORE
                                    // `on_change` so the record still precedes
                                    // the refresh it causes.
                                    let ignore = match crate::obs::trace::active_sink() {
                                        Some(_) => ignore_stats(&fire_workdir, &acc),
                                        None => None,
                                    };
                                    log_watcher(acc.paths, acc.relevant, true, Some(class), ignore);
                                    on_change(class);
                                    break;
                                }
                                Err(mpsc::RecvTimeoutError::Disconnected) => return,
                            }
                        }
                    }
                }
            }
        })
        .map_err(|e| AppError::Other(format!("failed to spawn debounce thread: {e}")))?;

    Ok(WatcherHandle {
        watcher: Some(watcher),
        debounce_thread: Some(debounce_thread),
    })
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_ignore;
