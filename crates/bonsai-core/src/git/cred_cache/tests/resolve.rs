//! Cache lifecycle: classify, miss/hit, TTL, stale-while-revalidate,
//! single-flight, bypass, fill failure, warm.

use super::*;

// 1. classify boundaries.
#[test]
fn classify_boundaries() {
    let ttl = Duration::from_millis(200);
    let refresh = Duration::from_millis(100);
    assert_eq!(
        classify(Duration::from_millis(0), ttl, refresh),
        Freshness::Fresh
    );
    assert_eq!(
        classify(Duration::from_millis(99), ttl, refresh),
        Freshness::Fresh
    );
    assert_eq!(
        classify(Duration::from_millis(100), ttl, refresh),
        Freshness::StaleButValid
    );
    assert_eq!(
        classify(Duration::from_millis(199), ttl, refresh),
        Freshness::StaleButValid
    );
    assert_eq!(
        classify(Duration::from_millis(200), ttl, refresh),
        Freshness::Expired
    );
    assert_eq!(
        classify(Duration::from_millis(500), ttl, refresh),
        Freshness::Expired
    );
}

// 2. miss then hit.
#[test]
fn miss_then_hit() {
    let counter = Arc::new(AtomicUsize::new(0));
    let cache = CredCache::new(
        counting_fill(counter.clone()),
        Duration::from_secs(60),
        Duration::from_secs(48),
    );
    let r1 = cache
        .resolve(None, "https://host.com/a", false)
        .into_option()
        .expect("some");
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    assert!(!r1.from_cache);
    assert_eq!(r1.creds, ("u".to_string(), "p".to_string()));

    let r2 = cache
        .resolve(None, "https://host.com/a", false)
        .into_option()
        .expect("some");
    assert_eq!(counter.load(Ordering::SeqCst), 1, "hit must not re-fill");
    assert!(r2.from_cache);
}

// 3. TTL expiry.
#[test]
fn ttl_expiry_refills() {
    let counter = Arc::new(AtomicUsize::new(0));
    let cache = CredCache::new(
        counting_fill(counter.clone()),
        Duration::from_millis(80),
        Duration::from_millis(60),
    );
    cache
        .resolve(None, "https://host.com/a", false)
        .into_option()
        .expect("some");
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    std::thread::sleep(Duration::from_millis(140)); // past ttl
    let r = cache
        .resolve(None, "https://host.com/a", false)
        .into_option()
        .expect("some");
    assert_eq!(counter.load(Ordering::SeqCst), 2, "expired entry re-fills");
    assert!(!r.from_cache);
}

// 4. stale-while-revalidate: return stale immediately + background refill.
#[test]
fn stale_while_revalidate_swaps_value() {
    let counter = Arc::new(AtomicUsize::new(0));
    let cache = CredCache::new(
        versioned_fill(counter.clone()),
        Duration::from_millis(600), // ttl
        Duration::from_millis(80),  // refresh_age
    );
    let r1 = cache
        .resolve(None, "https://host.com/a", false)
        .into_option()
        .expect("some");
    assert_eq!(r1.creds.1, "p1");
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    std::thread::sleep(Duration::from_millis(120)); // into stale-but-valid window
    let r2 = cache
        .resolve(None, "https://host.com/a", false)
        .into_option()
        .expect("some");
    assert!(r2.from_cache);
    assert_eq!(
        r2.creds.1, "p1",
        "stale read returns the OLD value immediately"
    );

    // Wait on store COMPLETION (the refreshed value becoming observable),
    // not the call-counter — the counter bumps at fill START, before the
    // background thread re-locks to store, so keying off it would race.
    assert!(
        poll_until_value(&cache, "https://host.com/a", "p2"),
        "background refresh did not store the new value in time"
    );

    let r3 = cache
        .resolve(None, "https://host.com/a", false)
        .into_option()
        .expect("some");
    assert_eq!(r3.creds.1, "p2", "next read sees the refreshed value");
    assert!(r3.from_cache);
}

// 5. single-flight: N concurrent resolves -> exactly one fill.
#[test]
fn single_flight_one_fill_for_concurrent_resolves() {
    let counter = Arc::new(AtomicUsize::new(0));
    let fill: FillFn = {
        let counter = counter.clone();
        Box::new(move |_repo, _url| {
            counter.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(150)); // widen the race window
            FillOutcome::Filled {
                username: "u".to_string(),
                password: "p".to_string(),
            }
        })
    };
    let cache = CredCache::new(fill, Duration::from_secs(60), Duration::from_secs(48));

    let mut handles = Vec::new();
    for _ in 0..8 {
        let c = Arc::clone(&cache);
        handles.push(std::thread::spawn(move || {
            c.resolve(None, "https://host.com/a", false)
                .into_option()
                .map(|r| r.creds)
        }));
    }
    let results: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("join"))
        .collect();

    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "single-flight: exactly one fill"
    );
    for r in results {
        assert_eq!(r, Some(("u".to_string(), "p".to_string())));
    }
}

// 5b. different keys fill independently.
#[test]
fn different_keys_independent_fills() {
    let counter = Arc::new(AtomicUsize::new(0));
    let cache = CredCache::new(
        counting_fill(counter.clone()),
        Duration::from_secs(60),
        Duration::from_secs(48),
    );
    cache
        .resolve(None, "https://host-a.com/x", false)
        .into_option()
        .expect("some");
    cache
        .resolve(None, "https://host-b.com/x", false)
        .into_option()
        .expect("some");
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

// 6. bypass evict + refill.
#[test]
fn bypass_evict_and_refill() {
    let counter = Arc::new(AtomicUsize::new(0));
    let cache = CredCache::new(
        versioned_fill(counter.clone()),
        Duration::from_secs(60),
        Duration::from_secs(48),
    );
    let r1 = cache
        .resolve(None, "https://host.com/a", false)
        .into_option()
        .expect("some");
    assert_eq!(r1.creds.1, "p1");
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    cache.evict(None, "https://host.com/a");
    let r2 = cache
        .resolve(None, "https://host.com/a", true)
        .into_option()
        .expect("some");
    assert_eq!(
        counter.load(Ordering::SeqCst),
        2,
        "bypass forces a fresh fill"
    );
    assert!(!r2.from_cache);
    assert_eq!(r2.creds.1, "p2");
}

// 7. fill failure -> None, no entry stored, in_flight cleared.
#[test]
fn fill_failure_returns_none_no_entry() {
    let counter = Arc::new(AtomicUsize::new(0));
    let fill: FillFn = {
        let counter = counter.clone();
        Box::new(move |_repo, _url| {
            let n = counter.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                FillOutcome::NoCredentials // first call: helper had nothing
            } else {
                FillOutcome::Filled {
                    username: "u".to_string(),
                    password: "p".to_string(),
                }
            }
        })
    };
    let cache = CredCache::new(fill, Duration::from_secs(60), Duration::from_secs(48));

    assert!(cache
        .resolve(None, "https://host.com/a", false)
        .into_option()
        .is_none());
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    // No entry stored + in_flight cleared -> a following resolve still works.
    let r = cache
        .resolve(None, "https://host.com/a", false)
        .into_option()
        .expect("some");
    assert_eq!(counter.load(Ordering::SeqCst), 2);
    assert!(!r.from_cache);
}

// 7b. P70: a `GitUnavailable` fill is passed straight through as
// `CredResolve::GitUnavailable` (so `acquire_cred` can record WHY the
// Helper rung failed) and is NEVER cached — a transient launch failure must
// not poison the key for the whole TTL.
#[test]
fn git_unavailable_fill_is_passed_through_and_not_cached() {
    let counter = Arc::new(AtomicUsize::new(0));
    let fill: FillFn = {
        let counter = counter.clone();
        Box::new(move |_repo, _url| {
            let n = counter.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                FillOutcome::GitUnavailable("program not found".to_string())
            } else {
                FillOutcome::Filled {
                    username: "u".to_string(),
                    password: "p".to_string(),
                }
            }
        })
    };
    let cache = CredCache::new(fill, Duration::from_secs(60), Duration::from_secs(48));

    // Matched exhaustively rather than with `{:?}`: `CredResolve` carries a
    // password and deliberately has no `Debug`.
    match cache.resolve(None, "https://host.com/gu", false) {
        CredResolve::GitUnavailable(detail) => assert_eq!(detail, "program not found"),
        CredResolve::NoCredentials => panic!("expected GitUnavailable, got NoCredentials"),
        CredResolve::Resolved(_) => panic!("expected GitUnavailable, got Resolved"),
    }
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    // Nothing was stored and in_flight was cleared: the next resolve re-asks
    // and now succeeds (the "install git, press Re-check" recovery path).
    let r = cache
        .resolve(None, "https://host.com/gu", false)
        .into_option()
        .expect("re-fill after a launch failure");
    assert_eq!(counter.load(Ordering::SeqCst), 2);
    assert!(
        !r.from_cache,
        "a GitUnavailable outcome must not have been cached"
    );
}

// 8. warm: pre-fill an empty key; a Fresh key does not re-fill.
#[test]
fn warm_prefills_then_resolve_is_warm() {
    let counter = Arc::new(AtomicUsize::new(0));
    let cache = CredCache::new(
        counting_fill(counter.clone()),
        Duration::from_secs(60),
        Duration::from_secs(48),
    );
    cache.warm(None, "https://host.com/a");
    wait_until(&counter, 1);
    assert_eq!(counter.load(Ordering::SeqCst), 1, "warm scheduled a fill");

    let r = cache
        .resolve(None, "https://host.com/a", false)
        .into_option()
        .expect("some");
    assert!(r.from_cache);
    assert_eq!(counter.load(Ordering::SeqCst), 1, "resolve finds it warm");

    cache.warm(None, "https://host.com/a"); // already Fresh -> no-op
    std::thread::sleep(Duration::from_millis(40));
    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "warm on Fresh key does not re-fill"
    );
}
