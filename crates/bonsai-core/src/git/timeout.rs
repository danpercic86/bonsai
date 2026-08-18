//! Command-layer timeout wrapper for blocking git2 reads (audit #2 §3.2,
//! F-T5-4).
//!
//! libgit2 spins FOREVER inflating a truncated loose object (the zlib stream
//! never terminates and libgit2 has no cooperative cancellation), so any
//! surface that fully inflates a corrupt commit — the graph walk, status's
//! HEAD peel, the history-index extraction — wedges its thread permanently.
//! No bounded libgit2 probe detects the truncation without also hanging
//! (`odb.read_header` passes: the header sits at the intact START of the
//! stream — see docs/testing-campaign-2026-08/FINDINGS.md, F-T5-4).
//!
//! The recorded fix is this wrapper: run the blocking closure on a DEDICATED
//! worker thread and wait with an INACTIVITY deadline. The closure reports
//! liveness through a [`GitProgress`] token (tick per emitted batch /
//! progress event), so a long-but-progressing walk never times out while a
//! silent zlib spin does. On timeout the caller gets a clean
//! [`AppError::Git`] and the wedged worker is **detached, not killed**
//! (threads cannot be cancelled): the process leaks one CPU-spinning OS
//! thread per corrupt-object hit, bounded by user retries — vastly better
//! than a permanently frozen UI or indexer.
//!
//! Deadline: [`DEFAULT_INACTIVITY_MS`] (30 s), overridable via the
//! [`GIT_TIMEOUT_ENV`] env var (milliseconds) — tests shrink it to keep the
//! corrupt-repo suite fast.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::error::AppError;

/// Default inactivity deadline: generous enough that no healthy repo — even a
/// cold-cache monster — goes 30 s without emitting a batch or progress tick.
pub const DEFAULT_INACTIVITY_MS: u64 = 30_000;

/// Env override (milliseconds) for the inactivity deadline. Read per call (not
/// cached) so the corrupt-repo suite can shrink it without process restarts.
pub const GIT_TIMEOUT_ENV: &str = "BONSAI_GIT_TIMEOUT_MS";

/// How often the waiter wakes to check for inactivity. Purely internal; the
/// observable deadline granularity is `deadline + POLL` in the worst case.
const POLL: Duration = Duration::from_millis(200);

/// Liveness token handed to the wrapped closure: `tick()` on every unit of
/// forward progress (a streamed batch, a progress event, a documented commit).
/// Cheap (one relaxed atomic increment) — safe to call per row.
#[derive(Debug, Clone)]
pub struct GitProgress(Arc<AtomicU64>);

impl GitProgress {
    fn new() -> Self {
        GitProgress(Arc::new(AtomicU64::new(0)))
    }

    /// Record forward progress (resets the inactivity deadline).
    pub fn tick(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }

    fn count(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// The effective inactivity deadline: [`GIT_TIMEOUT_ENV`] (ms) when set and
/// parseable to a positive integer, else [`DEFAULT_INACTIVITY_MS`].
pub fn effective_deadline() -> Duration {
    deadline_from(std::env::var(GIT_TIMEOUT_ENV).ok().as_deref())
}

/// Pure parse of the env override (unit-tested without env mutation).
fn deadline_from(raw: Option<&str>) -> Duration {
    let ms = raw
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|&ms| ms > 0)
        .unwrap_or(DEFAULT_INACTIVITY_MS);
    Duration::from_millis(ms)
}

/// Run `f` on a dedicated worker thread, failing with a clean
/// [`AppError::Git`] once it makes no progress for [`effective_deadline`].
/// See [`run_with_git_timeout_with`] for semantics.
pub fn run_with_git_timeout<T, F>(op: &str, f: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce(&GitProgress) -> Result<T, AppError> + Send + 'static,
{
    run_with_git_timeout_with(op, effective_deadline(), f)
}

/// [`run_with_git_timeout`] with an explicit deadline (tests; callers with
/// special needs). Semantics:
///
/// * `f` runs on a fresh named OS thread; its `Result` is returned verbatim
///   when it finishes before the INACTIVITY deadline (time since the last
///   [`GitProgress::tick`], or since start when it never ticks).
/// * On timeout, returns `AppError::Git("repository object database appears
///   corrupt (operation timed out): …")` and **detaches** the worker — the
///   thread cannot be cancelled mid-libgit2-call, so it is deliberately
///   leaked (one spinning thread per corrupt-object hit, bounded by user
///   retries; documented trade-off, audit #2 §3.2). A detached worker may
///   still complete later; its result is discarded.
/// * A worker PANIC surfaces as `AppError::Other` (the channel disconnects
///   without a value), never a hang or a propagated panic.
pub fn run_with_git_timeout_with<T, F>(op: &str, deadline: Duration, f: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce(&GitProgress) -> Result<T, AppError> + Send + 'static,
{
    let progress = GitProgress::new();
    let worker_progress = progress.clone();
    let (tx, rx) = std::sync::mpsc::channel::<Result<T, AppError>>();
    let spawned = std::thread::Builder::new()
        .name(format!("bonsai-git-{op}"))
        .spawn(move || {
            let _ = tx.send(f(&worker_progress));
        });
    if let Err(e) = spawned {
        return Err(AppError::Other(format!(
            "failed to spawn worker thread for {op}: {e}"
        )));
    }

    let mut last_count = progress.count();
    let mut last_activity = Instant::now();
    loop {
        match rx.recv_timeout(POLL.min(deadline)) {
            Ok(result) => return result, // worker finished (join not needed: detached by design)
            Err(RecvTimeoutError::Disconnected) => {
                // Sender dropped without a value ⇒ the worker panicked.
                return Err(AppError::Other(format!(
                    "internal error: the {op} worker thread panicked"
                )));
            }
            Err(RecvTimeoutError::Timeout) => {
                let count = progress.count();
                if count != last_count {
                    last_count = count;
                    last_activity = Instant::now();
                } else if last_activity.elapsed() >= deadline {
                    // Detach the wedged worker (see the doc comment).
                    return Err(AppError::Git(format!(
                        "repository object database appears corrupt (operation timed out): \
                         {op} made no progress for {}s. Run `git fsck` to locate the damaged \
                         object; the stalled worker was abandoned.",
                        deadline.as_secs().max(1)
                    )));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fast closure returns its value verbatim.
    #[test]
    fn returns_result_before_deadline() {
        let out = run_with_git_timeout_with("fast", Duration::from_secs(5), |_p| Ok(42u32))
            .expect("fast op succeeds");
        assert_eq!(out, 42);
    }

    /// The closure's own error passes through untouched.
    #[test]
    fn propagates_inner_error() {
        let err = run_with_git_timeout_with("failing", Duration::from_secs(5), |_p| {
            Err::<(), _>(AppError::Git("boom".to_string()))
        })
        .expect_err("inner error surfaces");
        assert!(matches!(err, AppError::Git(m) if m == "boom"));
    }

    /// A deliberately-hanging closure (never ticks) times out with the corrupt-
    /// odb `AppError::Git` and the worker is detached (the test returns long
    /// before the sleep ends — the leaked thread dies with the process).
    #[test]
    fn hanging_closure_times_out() {
        let err = run_with_git_timeout_with("hang", Duration::from_millis(300), |_p| {
            std::thread::sleep(Duration::from_secs(600));
            Ok(())
        })
        .expect_err("must time out");
        match err {
            AppError::Git(m) => {
                assert!(m.contains("operation timed out"), "message: {m}");
                assert!(m.contains("hang"), "names the op: {m}");
            }
            other => panic!("expected Git timeout error, got {other:?}"),
        }
    }

    /// An op that RUNS longer than the deadline but keeps ticking never times
    /// out — the deadline is inactivity, not total runtime.
    #[test]
    fn ticking_op_outlives_the_deadline() {
        let out = run_with_git_timeout_with("slow-but-alive", Duration::from_millis(300), |p| {
            for _ in 0..8 {
                std::thread::sleep(Duration::from_millis(100));
                p.tick();
            }
            Ok(7u8)
        })
        .expect("progressing op completes");
        assert_eq!(out, 7);
    }

    /// A panicking worker surfaces as `AppError::Other`, never a hang.
    #[test]
    fn worker_panic_is_an_error() {
        let err = run_with_git_timeout_with("panicky", Duration::from_secs(5), |_p| {
            panic!("worker exploded");
            #[allow(unreachable_code)]
            Ok::<(), AppError>(())
        })
        .expect_err("panic surfaces as an error");
        assert!(matches!(err, AppError::Other(m) if m.contains("panicked")));
    }

    /// Env-override parse: unset/garbage/zero ⇒ default; a positive integer ⇒ ms.
    #[test]
    fn deadline_parse_rules() {
        assert_eq!(deadline_from(None), Duration::from_millis(DEFAULT_INACTIVITY_MS));
        assert_eq!(deadline_from(Some("nope")), Duration::from_millis(DEFAULT_INACTIVITY_MS));
        assert_eq!(deadline_from(Some("0")), Duration::from_millis(DEFAULT_INACTIVITY_MS));
        assert_eq!(deadline_from(Some(" 800 ")), Duration::from_millis(800));
    }
}
