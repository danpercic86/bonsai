//! Panic / poisoned-lock recovery.

use super::*;

// 10. panic recovery: a filler that PANICS on its first call must not leave
// the key wedged. The RAII drop-guard clears `in_flight` + notifies on
// unwind, so a later `resolve` for the same key does NOT block forever on
// the Condvar — it proceeds to a fresh fill.
#[test]
fn panicking_fill_does_not_wedge_key() {
    let counter = Arc::new(AtomicUsize::new(0));
    let fill: FillFn = {
        let counter = counter.clone();
        Box::new(move |_repo, _url| {
            let n = counter.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                panic!("boom: simulated filler panic on first call");
            }
            FillOutcome::Filled {
                username: "u".to_string(),
                password: "p".to_string(),
            }
        })
    };
    let cache = CredCache::new(fill, Duration::from_secs(60), Duration::from_secs(48));

    // First resolve unwinds inside the filler; catch it so the test can go
    // on to prove the key was un-wedged.
    let c = Arc::clone(&cache);
    let first = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        c.resolve(None, "https://host.com/a", false).into_option()
    }));
    assert!(
        first.is_err(),
        "the filler panic must propagate out of resolve"
    );
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    // Must NOT hang: `in_flight` was cleared by the drop-guard, so this does
    // a fresh fill and returns creds.
    let second = cache
        .resolve(None, "https://host.com/a", false)
        .into_option();
    assert_eq!(
        second.map(|r| r.creds),
        Some(("u".to_string(), "p".to_string())),
        "key un-wedged: a subsequent fresh fill succeeds"
    );
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

/// Audit 2026-08-07 §3.4: a poisoned mutex must be RECOVERED, not
/// propagated — a panic here runs inside libgit2's credentials C-callback
/// trampoline and would kill every remote op until restart.
#[test]
fn poisoned_lock_recovers_instead_of_panicking() {
    let counter = Arc::new(AtomicUsize::new(0));
    let cache = CredCache::new(
        counting_fill(Arc::clone(&counter)),
        Duration::from_secs(60),
        Duration::from_secs(30),
    );

    // Poison the mutex: panic on a helper thread while holding the guard.
    let poisoner = Arc::clone(&cache);
    let _ = std::thread::spawn(move || {
        let _guard = poisoner.state.lock().expect("first lock");
        panic!("deliberate poison");
    })
    .join();
    assert!(
        cache.state.lock().is_err(),
        "mutex must actually be poisoned"
    );

    // The cache keeps working through the recovering lock helpers.
    let r = cache
        .resolve(None, "https://example.com/repo.git", false)
        .into_option()
        .expect("resolve must survive a poisoned mutex");
    assert_eq!(r.creds, ("u".to_string(), "p".to_string()));
}
