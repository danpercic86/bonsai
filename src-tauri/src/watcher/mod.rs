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
//! - P124b: a small burst whose every worktree path is gitignored AND
//!   untracked is DROPPED (logged as a suppressed `fired` record, no
//!   `on_change`), bounded by a 5-min heartbeat — see `burst.rs`;
//! - clean shutdown: dropping `WatcherHandle` drops the watcher first (the
//!   channel sender disconnects), then joins the debounce thread.
//!
//! Limitation (v1, by contract): linked git worktrees (git_dir outside the
//! workdir) are out of scope — bare repos are rejected at open, so
//! `git_dir = workdir/.git` is always valid here. Also remember that
//! ReadDirectoryChangesW can miss events on Windows; the manual refresh
//! button and window-focus rescan are mandatory companions, not extras.

use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::Watcher;

use bonsai_core::error::AppError;

mod burst;
mod classify;

#[cfg(test)]
use burst::accumulate;
use burst::{burst_verdict, drop_eligible, ignore_stats, BurstAcc, BurstVerdict, WatchTick};
#[cfg(test)]
use burst::{IgnoreStats, DROP_MAX_PATHS, IGNORED_HEARTBEAT, IGNORE_SAMPLE_CAP};

pub(crate) use classify::classify;
pub use classify::PathClass;

/// The class of a whole debounced burst — the same two-valued lattice as
/// [`PathClass`], accumulated CONSERVATIVELY: if any path in the coalesced
/// burst is `Refs`, the burst is `Refs`.
pub type BurstClass = PathClass;

const DEBOUNCE: Duration = Duration::from_millis(300);

/// Logs a `watcher` record via the global obs sink (§2.4, `root("watcher")`).
/// A no-op when Dev mode is off (the sink is `None`). The watcher is
/// Tauri-decoupled and holds no `AppHandle`, so it goes through the same
/// process-wide sink handle the phase recorder uses.
fn log_watcher(
    paths: u32,
    relevant: u32,
    fired: bool,
    class: Option<BurstClass>,
    ignore: Option<burst::IgnoreStats>,
    suppressed: bool,
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
            // P124b: a dropped all-ignored burst still logs its `fired`
            // record, marked so, and no refresh round follows it.
            suppressed,
            suppress_reason: suppressed.then(|| "ignored".to_string()),
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
                // P124b: worktree paths are sampled ALWAYS (the drop rule needs
                // them), but cloned only up to the cap; the count runs past it.
                let mut relevant = 0u32;
                let mut refs = false;
                let mut worktree_paths = Vec::new();
                let mut worktree_count = 0u32;
                for p in &event.paths {
                    let Some(c) = classify(p, &git_dir) else {
                        continue;
                    };
                    relevant += 1;
                    refs |= c == PathClass::Refs;
                    if c == PathClass::Worktree {
                        worktree_count = worktree_count.saturating_add(1);
                        if worktree_paths.len() < burst::IGNORE_SAMPLE_CAP {
                            worktree_paths.push(p.clone());
                        }
                    }
                }
                WatchTick {
                    paths,
                    relevant,
                    refs,
                    worktree_paths,
                    worktree_count,
                }
            }
            // watcher error → trigger a refresh, cheap and safe; count it as one
            // relevant path so the debounce fires, and as ref-affecting so the
            // frontend runs the WIDE refresh (we don't know what was dropped).
            Err(_) => WatchTick {
                paths: 1,
                relevant: 1,
                refs: true,
                ..WatchTick::default()
            },
        };
        log_watcher(tick.paths, tick.relevant, false, None, None, false);
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
            // P124b: the heartbeat clock — the last `on_change` (spawn counts).
            let mut last_fire = Instant::now();
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
                                    // P124b: the git check runs only for a
                                    // drop-eligible burst, or in Dev mode (the
                                    // P121d measurement). Logged BEFORE
                                    // `on_change` so the record still precedes
                                    // the refresh it causes.
                                    let dev = crate::obs::trace::active_sink().is_some();
                                    let stats = if drop_eligible(&acc) || dev {
                                        ignore_stats(&fire_workdir, &acc)
                                    } else {
                                        None
                                    };
                                    let verdict =
                                        burst_verdict(&acc, stats.as_ref(), last_fire.elapsed());
                                    let dropped = verdict == BurstVerdict::DropIgnored;
                                    log_watcher(
                                        acc.paths,
                                        acc.relevant,
                                        true,
                                        Some(class),
                                        stats,
                                        dropped,
                                    );
                                    if !dropped {
                                        on_change(class);
                                        last_fire = Instant::now();
                                    }
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
