//! File watcher for the open repository's working directory.
//!
//! Decoupled from Tauri on purpose: `commands.rs` wires `on_change` to an
//! `app.emit("repo-changed", …)`, tests wire it to a channel.
//!
//! Design (M1 contract §4):
//! - one recursive `notify` watch on the workdir (`.git` lives inside it);
//! - `.git`-internals filter: only `HEAD`, `index`, `refs/**`, `packed-refs`
//!   are relevant; `*.lock` files and everything else under `.git/` are noise;
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
        // returns promptly on `Err(Disconnected)`.
        drop(self.watcher.take());
        if let Some(t) = self.debounce_thread.take() {
            let _ = t.join();
        }
    }
}

/// Returns `true` if a filesystem event at `path` should trigger a refresh.
///
/// Anything outside `.git/` is workdir content — relevant. Inside `.git/`,
/// only ref/HEAD/index state matters; lock files and object churn are noise.
fn is_relevant(path: &Path, git_dir: &Path) -> bool {
    let rel = match path.strip_prefix(git_dir) {
        Err(_) => return true, // not under .git → workdir content change
        Ok(rel) => rel,
    };
    if rel
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".lock"))
    {
        return false; // index.lock etc. — churn
    }
    rel == Path::new("HEAD")
        || rel == Path::new("index")
        || rel.starts_with("refs")
        || rel.starts_with("packed-refs")
}

/// Starts watching `workdir` recursively and invokes `on_change` (on the
/// debounce thread) at most once per 300 ms quiet period after relevant
/// activity. Notify error events count as relevant (a refresh is cheap and
/// safe). Fast to call; only the initial watch registration is synchronous.
pub fn spawn_watcher(
    workdir: &Path,
    on_change: Box<dyn Fn() + Send + 'static>,
) -> Result<WatcherHandle, AppError> {
    // Canonicalize: macOS FSEvents resolves symlinks in the paths it reports
    // (e.g. `/var/...` -> `/private/var/...`, true of every macOS temp dir),
    // so comparing against an unresolved `workdir` silently breaks the
    // `.git`-internals filter below — every event looks "outside .git".
    let workdir = std::fs::canonicalize(workdir)
        .map_err(|e| AppError::Other(format!("failed to resolve {}: {e}", workdir.display())))?;
    let git_dir = workdir.join(".git");
    let (tx, rx) = mpsc::channel::<()>();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let relevant = match &res {
            Ok(event) => event.paths.iter().any(|p| is_relevant(p, &git_dir)),
            Err(_) => true, // watcher error → trigger a refresh, cheap and safe
        };
        if relevant {
            let _ = tx.send(()); // receiver gone: watcher is being torn down
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
                    Ok(()) => {
                        // Drain until 300 ms of quiet, then fire once.
                        loop {
                            match rx.recv_timeout(DEBOUNCE) {
                                Ok(()) => continue, // storm ongoing, keep absorbing
                                Err(mpsc::RecvTimeoutError::Timeout) => {
                                    on_change();
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
mod tests {
    use super::*;
    use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
    use std::time::Instant;

    /// Serializes the fixture-based watcher tests. Each spawns a real
    /// ReadDirectoryChangesW watcher whose stale `git2::init` events are
    /// flushed lazily; under parallel load concurrent watcher fixtures delay
    /// each other's flushes past the pre-test drain window (flake seen at
    /// 1.5 s AND 2.5 s — widening further is not the fix, one-at-a-time is).
    /// Poison-recovered so one failing test can't error out the rest.
    static WATCHER_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn serialize_watcher_test() -> std::sync::MutexGuard<'static, ()> {
        WATCHER_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// git2-init'd repo in a temp dir; returns (tempdir, workdir path).
    fn fixture_repo() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::TempDir::new().unwrap();
        git2::Repository::init(dir.path()).unwrap();
        let workdir = dir.path().to_path_buf();
        (dir, workdir)
    }

    fn watch_into_channel(workdir: &Path) -> (WatcherHandle, mpsc::Receiver<Instant>) {
        let (tx, rx): (Sender<Instant>, _) = channel();
        let handle = spawn_watcher(
            workdir,
            Box::new(move || {
                let _ = tx.send(Instant::now());
            }),
        )
        .unwrap();
        // Give ReadDirectoryChangesW a moment to be fully armed, then drain
        // any STALE callbacks until the channel is quiet: on some volumes
        // (observed on this machine's D: scratch drive) directory events from
        // `git2::Repository::init` are flushed lazily and arrive AFTER the
        // watch is registered — e.g. a late `.git\refs` creation event, which
        // correctly passes the relevance filter and would poison the test.
        // The quiet window must comfortably exceed the 300 ms debounce so a
        // stale burst has fully discharged before the test body runs; 1.5 s
        // (up from 700 ms) because heavy parallel-test load (M4 grew the lib
        // suite) delays the lazy flush enough to slip past a shorter drain;
        // 2.5 s (P30) because the scheduler integration tests added git-CLI
        // heavy parallel load that delayed the flush past 1.5 s again.
        std::thread::sleep(Duration::from_millis(500));
        while rx.recv_timeout(Duration::from_millis(2500)).is_ok() {}
        (handle, rx)
    }

    #[test]
    fn fires_once_after_touch() {
        let _serial = serialize_watcher_test();
        let (_dir, workdir) = fixture_repo();
        let (_handle, rx) = watch_into_channel(&workdir);

        std::fs::write(workdir.join("a.txt"), "hello").unwrap();

        // Exactly one debounced callback within a generous window…
        rx.recv_timeout(Duration::from_secs(5))
            .expect("expected one repo-changed callback");
        // …and no second one for a further second.
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1)).unwrap_err(),
            RecvTimeoutError::Timeout,
            "expected no second callback after a single touch"
        );
    }

    #[test]
    fn storm_coalesces() {
        let _serial = serialize_watcher_test();
        let (_dir, workdir) = fixture_repo();
        let (_handle, rx) = watch_into_channel(&workdir);

        // 50 writes in a tight loop (well under 300 ms).
        for i in 0..50 {
            std::fs::write(workdir.join(format!("f{i}.txt")), "x").unwrap();
        }

        // Count callbacks over a 5 s observation window.
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut count = 0u32;
        loop {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            match rx.recv_timeout(deadline - now) {
                Ok(_) => count += 1,
                Err(_) => break,
            }
        }
        assert!(count >= 1, "storm produced no callback");
        assert!(count <= 2, "storm produced {count} callbacks, expected <= 2");
    }

    #[test]
    fn git_internals_filtered() {
        let _serial = serialize_watcher_test();
        let (_dir, workdir) = fixture_repo();
        let (_handle, rx) = watch_into_channel(&workdir);

        // Writes under .git/objects must be filtered out.
        let obj_dir = workdir.join(".git").join("objects").join("aa");
        std::fs::create_dir_all(&obj_dir).unwrap();
        std::fs::write(obj_dir.join("dummy"), "blob").unwrap();
        assert_eq!(
            rx.recv_timeout(Duration::from_millis(1500)).unwrap_err(),
            RecvTimeoutError::Timeout,
            "writes under .git/objects must not trigger a callback"
        );

        // Touching .git/HEAD IS relevant.
        std::fs::write(workdir.join(".git").join("HEAD"), "ref: refs/heads/main\n").unwrap();
        rx.recv_timeout(Duration::from_secs(5))
            .expect("touching .git/HEAD should trigger a callback");
    }

    #[test]
    fn drop_is_clean() {
        let _serial = serialize_watcher_test();
        let (_dir, workdir) = fixture_repo();
        let (handle, rx) = watch_into_channel(&workdir);

        // Explicit drop must not hang (watcher drops, debounce thread joins).
        drop(handle);

        // A write after the drop must produce no callbacks. The channel may
        // still hold callbacks sent BEFORE the drop finished (a stale fixture
        // event firing the debounce right as we dropped — seen under heavy
        // parallel-test load); drain those, then require Disconnected. A
        // Timeout here would mean the debounce thread is still alive.
        std::fs::write(workdir.join("after-drop.txt"), "x").unwrap();
        loop {
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(_) => continue, // pre-drop stale callback: not a violation
                Err(e) => {
                    assert_eq!(
                        e,
                        RecvTimeoutError::Disconnected,
                        "callback sender should be disconnected after drop"
                    );
                    break;
                }
            }
        }
    }

    #[test]
    fn is_relevant_rules() {
        let git_dir = Path::new(r"C:\repo\.git");
        // Workdir content.
        assert!(is_relevant(Path::new(r"C:\repo\src\main.rs"), git_dir));
        // Relevant .git internals.
        assert!(is_relevant(&git_dir.join("HEAD"), git_dir));
        assert!(is_relevant(&git_dir.join("index"), git_dir));
        assert!(is_relevant(&git_dir.join("packed-refs"), git_dir));
        assert!(is_relevant(&git_dir.join("refs").join("heads").join("main"), git_dir));
        // M6 §6.4: remote-tracking ref updates after a fetch must refresh.
        assert!(is_relevant(
            &git_dir.join("refs").join("remotes").join("origin").join("main"),
            git_dir
        ));
        // Noise.
        assert!(!is_relevant(&git_dir.join("index.lock"), git_dir));
        assert!(!is_relevant(
            &git_dir.join("refs").join("heads").join("main.lock"),
            git_dir
        ));
        assert!(!is_relevant(&git_dir.join("objects").join("aa").join("bb"), git_dir));
        assert!(!is_relevant(&git_dir.join("logs").join("HEAD"), git_dir));
        assert!(!is_relevant(&git_dir.join("FETCH_HEAD"), git_dir));
    }
}
