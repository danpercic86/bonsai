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

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use notify::Watcher;

use bonsai_core::error::AppError;

mod classify;

pub(crate) use classify::classify;
pub use classify::PathClass;

/// The running reduction of ONE debounce window: the coalesced counts plus the
/// conservative burst class. Split out of the drain loop so the accumulation
/// rule this whole feature rests on is testable without wall-clock timing.
#[derive(Clone, Copy)]
struct BurstAcc {
    paths: u32,
    relevant: u32,
    refs: bool,
}

impl BurstAcc {
    fn new(first: WatchTick) -> Self {
        Self {
            paths: first.paths,
            relevant: first.relevant,
            refs: first.refs,
        }
    }

    /// Folds one more notify batch into the window. CONSERVATIVE: one Refs path
    /// anywhere in the coalesced burst makes the WHOLE burst Refs.
    fn absorb(&mut self, t: WatchTick) {
        self.paths = self.paths.saturating_add(t.paths);
        self.relevant = self.relevant.saturating_add(t.relevant);
        self.refs |= t.refs;
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
        acc.absorb(*t);
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
#[derive(Clone, Copy, Default)]
struct WatchTick {
    paths: u32,
    relevant: u32,
    /// True when at least one relevant path in this batch was `PathClass::Refs`
    /// (or the batch was a notify error, which is treated as ref-affecting).
    refs: bool,
}

/// Logs a `watcher` record via the global obs sink (§2.4, `root("watcher")`).
/// A no-op when Dev mode is off (the sink is `None`). The watcher is
/// Tauri-decoupled and holds no `AppHandle`, so it goes through the same
/// process-wide sink handle the phase recorder uses.
fn log_watcher(paths: u32, relevant: u32, fired: bool, class: Option<BurstClass>) {
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
    let (tx, rx) = mpsc::channel::<WatchTick>();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        // P91 §2.4: log the raw notify batch (counts only), then forward a tick
        // carrying the same counts so the debounce firing can report the total
        // paths/relevant that were coalesced into it.
        let tick = match &res {
            Ok(event) => {
                let paths = event.paths.len() as u32;
                let classes = event.paths.iter().filter_map(|p| classify(p, &git_dir));
                let mut relevant = 0u32;
                let mut refs = false;
                for c in classes {
                    relevant += 1;
                    refs |= c == PathClass::Refs;
                }
                WatchTick { paths, relevant, refs }
            }
            // watcher error → trigger a refresh, cheap and safe; count it as one
            // relevant path so the debounce fires, and as ref-affecting so the
            // frontend runs the WIDE refresh (we don't know what was dropped).
            Err(_) => WatchTick { paths: 1, relevant: 1, refs: true },
        };
        log_watcher(tick.paths, tick.relevant, false, None);
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
                                    log_watcher(acc.paths, acc.relevant, true, Some(class));
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
