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
//! - P113d: nothing is logged per raw notify batch. A `relevant == 0` batch
//!   (pure `.git` noise) only bumps the shared `noise::NoiseCounters`; every
//!   `watcher` record comes from the debounce thread — one per firing (with
//!   `batches`/`burstMs`/`errors` + any drained noise), or a noise summary when
//!   noise sat undrained for `NOISE_FLUSH` or the watcher is torn down;
//! - clean shutdown: dropping `WatcherHandle` drops the watcher first (the
//!   channel sender disconnects), then joins the debounce thread.
//!
//! Limitation (v1, by contract): linked git worktrees (git_dir outside the
//! workdir) are out of scope — bare repos are rejected at open, so
//! `git_dir = workdir/.git` is always valid here. Also remember that
//! ReadDirectoryChangesW can miss events on Windows; the manual refresh
//! button and window-focus rescan are mandatory companions, not extras.

use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use notify::Watcher;

use bonsai_core::error::AppError;

mod burst;
mod classify;
mod emit;
mod noise;

#[cfg(test)]
use burst::accumulate;
use burst::{burst_verdict, drop_eligible, ignore_stats, BurstAcc, BurstVerdict, WatchTick};
#[cfg(test)]
use burst::{IgnoreStats, DROP_MAX_PATHS, IGNORED_HEARTBEAT, IGNORE_SAMPLE_CAP};
use emit::{log_watch_fired, log_watch_noise};
use noise::{NoiseCounters, NOISE_FLUSH};

pub(crate) use classify::classify;
pub use classify::PathClass;

/// The class of a whole debounced burst — the same two-valued lattice as
/// [`PathClass`], accumulated CONSERVATIVELY: if any path in the coalesced
/// burst is `Refs`, the burst is `Refs`.
pub type BurstClass = PathClass;

const DEBOUNCE: Duration = Duration::from_millis(300);

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
        // returns promptly on `Err(Disconnected)`, after flushing its noise
        // tail (P113d §6), so `drop` returning ⇒ the tail record is enqueued.
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
    // P113d: `.git`-internals noise counts, shared with the debounce thread.
    let noise = Arc::new(NoiseCounters::default());
    let handler_noise = Arc::clone(&noise);

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        // P113d: no obs call here. A relevant batch is forwarded as a tick
        // carrying its counts (the debounce firing reports the burst sums);
        // an irrelevant one only bumps two relaxed counters.
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
                    error: false,
                }
            }
            // watcher error → trigger a refresh, cheap and safe; count it as one
            // relevant path so the debounce fires, and as ref-affecting so the
            // frontend runs the WIDE refresh (we don't know what was dropped).
            // P113d: `error` makes the fired record closing the burst count it.
            Err(_) => WatchTick {
                paths: 1,
                relevant: 1,
                refs: true,
                error: true,
                ..WatchTick::default()
            },
        };
        if tick.relevant == 0 {
            handler_noise.absorb(tick.paths);
        } else {
            let _ = tx.send(tick); // receiver gone: watcher is being torn down
        }
    })
    .map_err(|e| AppError::Other(format!("failed to create file watcher: {e}")))?;

    watcher
        .watch(&workdir, notify::RecursiveMode::Recursive)
        .map_err(|e| AppError::Other(format!("failed to watch {}: {e}", workdir.display())))?;

    let debounce_thread = std::thread::Builder::new()
        .name("bonsai-watch-debounce".into())
        .spawn(move || debounce_loop(rx, noise, fire_workdir, on_change, NOISE_FLUSH))
        .map_err(|e| AppError::Other(format!("failed to spawn debounce thread: {e}")))?;

    Ok(WatcherHandle {
        watcher: Some(watcher),
        debounce_thread: Some(debounce_thread),
    })
}

/// The debounce thread body (P113d §2.4; extracted so tests can drive it over a
/// hand-fed channel) — the ONLY writer of Rust `watcher` records. `noise_flush`
/// bounds the OUTER idle wait only (production: `NOISE_FLUSH`), which runs only
/// with no burst in flight, so it can never move fire timing; the inner drain
/// loop, `DEBOUNCE` and the P124b heartbeat are unchanged.
fn debounce_loop(
    rx: mpsc::Receiver<WatchTick>,
    noise: Arc<NoiseCounters>,
    workdir: PathBuf,
    on_change: Box<dyn Fn(BurstClass) + Send>,
    noise_flush: Duration,
) {
    // P124b: the heartbeat clock — the last `on_change` (spawn counts).
    let mut last_fire = Instant::now();
    loop {
        let first = match rx.recv_timeout(noise_flush) {
            Ok(first) => first,
            // Idle: report noise that found no firing to ride on.
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(s) = noise.take() {
                    log_watch_noise(s);
                }
                continue;
            }
            // Watcher dropped → flush the noise tail, clean shutdown.
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Some(s) = noise.take() {
                    log_watch_noise(s);
                }
                return;
            }
        };
        // Accumulate the coalesced counts + burst class across the debounce
        // window so the `fired` record and the callback both describe the
        // whole burst.
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
                    // P124b: the git check runs only for a drop-eligible burst,
                    // or in Dev mode (the P121d measurement). Logged BEFORE
                    // `on_change` so the record still precedes the refresh it
                    // causes.
                    let dev = crate::obs::trace::active_sink().is_some();
                    let stats = if drop_eligible(&acc) || dev {
                        ignore_stats(&workdir, &acc)
                    } else {
                        None
                    };
                    let verdict = burst_verdict(&acc, stats.as_ref(), last_fire.elapsed());
                    let dropped = verdict == BurstVerdict::DropIgnored;
                    // Noise is drained unconditionally (even with no sink), so
                    // a Dev-mode-off period never leaks into a later record.
                    log_watch_fired(&acc, class, stats.as_ref(), dropped, noise.take());
                    if !dropped {
                        on_change(class);
                        last_fire = Instant::now();
                    }
                    break;
                }
                // Teardown mid-burst: flush noise but write NO fired record —
                // no `on_change` ran, so `fired: true` would lie (and feed a
                // spurious `watcher-storm` on app exit).
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    if let Some(s) = noise.take() {
                        log_watch_noise(s);
                    }
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_ignore;
#[cfg(test)]
mod tests_noise;
