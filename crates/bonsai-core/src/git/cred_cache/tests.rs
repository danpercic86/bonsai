//! cred_cache unit tests. Extracted verbatim from the former inline
//! `mod tests` (file-size discipline).

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

// ---- injectable fake fillers (NO git spawn, cross-platform) ----

/// Returns fixed creds and bumps `counter` on every call.
fn counting_fill(counter: Arc<AtomicUsize>) -> FillFn {
    Box::new(move |_repo, _url| {
        counter.fetch_add(1, Ordering::SeqCst);
        FillOutcome::Filled {
            username: "u".to_string(),
            password: "p".to_string(),
        }
    })
}

/// Returns a distinguishable password `p{n}` per call, so tests can prove a
/// value swap.
fn versioned_fill(counter: Arc<AtomicUsize>) -> FillFn {
    Box::new(move |_repo, _url| {
        let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
        FillOutcome::Filled {
            username: "u".to_string(),
            password: format!("p{n}"),
        }
    })
}

fn wait_until(counter: &AtomicUsize, target: usize) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while counter.load(Ordering::SeqCst) < target && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Polls `resolve` until the OBSERVABLE password equals `want` (or a
/// deadline elapses). Keys off store COMPLETION — the refreshed value being
/// visible — rather than the filler's call-start counter (which bumps
/// before the background thread re-locks to store).
fn poll_until_value(cache: &Arc<CredCache>, url: &str, want: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Some(r) = cache.resolve(None, url, false).into_option() {
            if r.creds.1 == want {
                return true;
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

mod key;
mod recovery;
mod resolve;
