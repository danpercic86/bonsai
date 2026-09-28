//! A small scoped worker pool: at most `workers` items in flight, the first
//! error stops every worker, the cancel token is polled before each item.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

use super::{CancelToken, CheckError, InternalFault};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Maps `items` with `f` on up to `workers` threads, preserving order.
/// Returns the FIRST error any worker hit; a cancellation mid-way is
/// `Cancelled`.
pub(crate) fn run<T, R, F>(
    items: &[T],
    workers: usize,
    cancel: &CancelToken,
    f: F,
) -> Result<Vec<R>, CheckError>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> Result<R, CheckError> + Sync,
{
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let first_err: Mutex<Option<CheckError>> = Mutex::new(None);
    let results: Mutex<Vec<Option<R>>> = Mutex::new((0..items.len()).map(|_| None).collect());
    let fail = |e: CheckError| {
        let mut slot = lock(&first_err);
        if slot.is_none() {
            *slot = Some(e);
        }
        stop.store(true, Ordering::SeqCst);
    };
    std::thread::scope(|s| {
        for _ in 0..workers.clamp(1, items.len()) {
            s.spawn(|| loop {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                if cancel.is_cancelled() {
                    fail(CheckError::Cancelled);
                    break;
                }
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(item) = items.get(i) else { break };
                match catch_unwind(AssertUnwindSafe(|| f(item))) {
                    Ok(Ok(r)) => lock(&results)[i] = Some(r),
                    Ok(Err(e)) => fail(e),
                    Err(_) => fail(CheckError::Internal(InternalFault::Io)),
                }
            });
        }
    });
    if let Some(e) = *lock(&first_err) {
        return Err(e);
    }
    let results = results.into_inner().unwrap_or_else(|e| e.into_inner());
    results
        .into_iter()
        .map(|r| r.ok_or(CheckError::Cancelled))
        .collect()
}
