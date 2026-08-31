//! cred_cache unit tests. Extracted verbatim from the former inline
//! `mod tests` (file-size discipline).

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

// ---- injectable fake fillers (NO git spawn, cross-platform) ----

/// Returns fixed creds and bumps `counter` on every call.
fn counting_fill(counter: Arc<AtomicUsize>) -> FillFn {
    Box::new(move |_repo, _url| {
        counter.fetch_add(1, Ordering::SeqCst);
        FillOutcome::Filled { username: "u".to_string(), password: "p".to_string() }
    })
}

/// Returns a distinguishable password `p{n}` per call, so tests can prove a
/// value swap.
fn versioned_fill(counter: Arc<AtomicUsize>) -> FillFn {
    Box::new(move |_repo, _url| {
        let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
        FillOutcome::Filled { username: "u".to_string(), password: format!("p{n}") }
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

// 1. classify boundaries.
#[test]
fn classify_boundaries() {
    let ttl = Duration::from_millis(200);
    let refresh = Duration::from_millis(100);
    assert_eq!(classify(Duration::from_millis(0), ttl, refresh), Freshness::Fresh);
    assert_eq!(classify(Duration::from_millis(99), ttl, refresh), Freshness::Fresh);
    assert_eq!(
        classify(Duration::from_millis(100), ttl, refresh),
        Freshness::StaleButValid
    );
    assert_eq!(
        classify(Duration::from_millis(199), ttl, refresh),
        Freshness::StaleButValid
    );
    assert_eq!(classify(Duration::from_millis(200), ttl, refresh), Freshness::Expired);
    assert_eq!(classify(Duration::from_millis(500), ttl, refresh), Freshness::Expired);
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
    let r1 = cache.resolve(None, "https://host.com/a", false).into_option().expect("some");
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    assert!(!r1.from_cache);
    assert_eq!(r1.creds, ("u".to_string(), "p".to_string()));

    let r2 = cache.resolve(None, "https://host.com/a", false).into_option().expect("some");
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
    cache.resolve(None, "https://host.com/a", false).into_option().expect("some");
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    std::thread::sleep(Duration::from_millis(140)); // past ttl
    let r = cache.resolve(None, "https://host.com/a", false).into_option().expect("some");
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
    let r1 = cache.resolve(None, "https://host.com/a", false).into_option().expect("some");
    assert_eq!(r1.creds.1, "p1");
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    std::thread::sleep(Duration::from_millis(120)); // into stale-but-valid window
    let r2 = cache.resolve(None, "https://host.com/a", false).into_option().expect("some");
    assert!(r2.from_cache);
    assert_eq!(r2.creds.1, "p1", "stale read returns the OLD value immediately");

    // Wait on store COMPLETION (the refreshed value becoming observable),
    // not the call-counter — the counter bumps at fill START, before the
    // background thread re-locks to store, so keying off it would race.
    assert!(
        poll_until_value(&cache, "https://host.com/a", "p2"),
        "background refresh did not store the new value in time"
    );

    let r3 = cache.resolve(None, "https://host.com/a", false).into_option().expect("some");
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
            FillOutcome::Filled { username: "u".to_string(), password: "p".to_string() }
        })
    };
    let cache = CredCache::new(fill, Duration::from_secs(60), Duration::from_secs(48));

    let mut handles = Vec::new();
    for _ in 0..8 {
        let c = Arc::clone(&cache);
        handles.push(std::thread::spawn(move || {
            c.resolve(None, "https://host.com/a", false).into_option().map(|r| r.creds)
        }));
    }
    let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("join")).collect();

    assert_eq!(counter.load(Ordering::SeqCst), 1, "single-flight: exactly one fill");
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
    cache.resolve(None, "https://host-a.com/x", false).into_option().expect("some");
    cache.resolve(None, "https://host-b.com/x", false).into_option().expect("some");
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
    let r1 = cache.resolve(None, "https://host.com/a", false).into_option().expect("some");
    assert_eq!(r1.creds.1, "p1");
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    cache.evict(None, "https://host.com/a");
    let r2 = cache.resolve(None, "https://host.com/a", true).into_option().expect("some");
    assert_eq!(counter.load(Ordering::SeqCst), 2, "bypass forces a fresh fill");
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
                FillOutcome::Filled { username: "u".to_string(), password: "p".to_string() }
            }
        })
    };
    let cache = CredCache::new(fill, Duration::from_secs(60), Duration::from_secs(48));

    assert!(cache.resolve(None, "https://host.com/a", false).into_option().is_none());
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    // No entry stored + in_flight cleared -> a following resolve still works.
    let r = cache.resolve(None, "https://host.com/a", false).into_option().expect("some");
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
                FillOutcome::Filled { username: "u".to_string(), password: "p".to_string() }
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
    assert!(!r.from_cache, "a GitUnavailable outcome must not have been cached");
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

    let r = cache.resolve(None, "https://host.com/a", false).into_option().expect("some");
    assert!(r.from_cache);
    assert_eq!(counter.load(Ordering::SeqCst), 1, "resolve finds it warm");

    cache.warm(None, "https://host.com/a"); // already Fresh -> no-op
    std::thread::sleep(Duration::from_millis(40));
    assert_eq!(counter.load(Ordering::SeqCst), 1, "warm on Fresh key does not re-fill");
}

// 9. key normalization table (host-only, useHttpPath OFF).
#[test]
fn normalize_key_table() {
    assert_eq!(normalize_key("https://Host.COM/a/b.git?x=1#f", false), "https://host.com");
    assert_eq!(normalize_key("https://user:pw@host.com/a", false), "https://host.com");
    assert_eq!(normalize_key("https://host.com/other", false), "https://host.com");
    assert_eq!(normalize_key("https://host.com:8443/a", false), "https://host.com:8443");
    assert_eq!(normalize_key("HTTPS://HOST.com", false), "https://host.com");
    // non-`://` fallback -> lowercased input.
    assert_eq!(
        normalize_key("Git@GitHub.com:owner/repo.git", false),
        "git@github.com:owner/repo.git"
    );

    // The first three collapse to one shared key (host+scheme only).
    let a = normalize_key("https://Host.COM/a/b.git?x=1#f", false);
    let b = normalize_key("https://user:pw@host.com/a", false);
    let c = normalize_key("https://host.com/other", false);
    assert_eq!(a, b);
    assert_eq!(b, c);
}

// 9b. key normalization with useHttpPath ON: the PATH disambiguates
// per-org tokens on a shared host (F-A5-a — the dev.azure.com case). Path
// is case-preserved; scheme+host still lowercased; userinfo/query/fragment
// still dropped.
#[test]
fn normalize_key_with_http_path_includes_path() {
    // Two orgs on the SAME host now yield DISTINCT keys.
    let org_a = normalize_key("https://dev.azure.com/OrgA/_git/repo", true);
    let org_b = normalize_key("https://dev.azure.com/OrgB/_git/repo", true);
    assert_ne!(org_a, org_b, "different paths => different keys");
    assert_eq!(org_a, "https://dev.azure.com/OrgA/_git/repo");

    // Without useHttpPath they collapse to the shared host key (the old,
    // cross-contaminating behavior we are guarding against).
    assert_eq!(
        normalize_key("https://dev.azure.com/OrgA/_git/repo", false),
        normalize_key("https://dev.azure.com/OrgB/_git/repo", false)
    );

    // Path case is preserved; scheme+host lowercased; query/fragment dropped.
    assert_eq!(
        normalize_key("HTTPS://Dev.Azure.COM/OrgA/repo.git?x=1#f", true),
        "https://dev.azure.com/OrgA/repo.git"
    );
    // userinfo still dropped, path kept.
    assert_eq!(
        normalize_key("https://user:pw@host.com/a/b", true),
        "https://host.com/a/b"
    );
    // No path -> just the base (trailing slash-less).
    assert_eq!(normalize_key("https://host.com", true), "https://host.com");
}

// 9c. key_for reads credential.useHttpPath from the repo config: unset =>
// host-only key; set true => path-scoped key (F-A5-a). Also exercises the
// per-URL-scoped `credential.<host>.useHttpPath` form.
#[test]
fn key_for_honors_use_http_path_config() {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init");
    // Use a host that cannot appear in the machine's GLOBAL git config, and
    // set the LOCAL value explicitly so the test is immune to a global
    // `credential.useHttpPath` default (Azure DevOps devs often set one).
    let url = "https://git.example.test/OrgA/repo";

    {
        let mut cfg = repo.config().expect("config");
        cfg.set_bool("credential.useHttpPath", false).expect("off");
    }
    assert_eq!(
        key_for(Some(dir.path()), url),
        "https://git.example.test",
        "useHttpPath off => host-only key"
    );

    {
        let mut cfg = repo.config().expect("config");
        cfg.set_bool("credential.useHttpPath", true).expect("on");
    }
    assert_eq!(
        key_for(Some(dir.path()), url),
        "https://git.example.test/OrgA/repo",
        "useHttpPath on => path-scoped key"
    );

    // Two orgs on the same host now key distinctly.
    assert_ne!(
        key_for(Some(dir.path()), "https://git.example.test/OrgA/repo"),
        key_for(Some(dir.path()), "https://git.example.test/OrgB/repo"),
    );
}

// 9d. the URL-host-scoped config key wins independently of the unscoped one
// (how Azure DevOps users typically enable it).
#[test]
fn key_for_honors_host_scoped_use_http_path() {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init");
    {
        let mut cfg = repo.config().expect("config");
        // Unscoped OFF locally (override any global default), host-scoped ON.
        cfg.set_bool("credential.useHttpPath", false).expect("unscoped off");
        cfg.set_bool("credential.https://azdo.example.test.useHttpPath", true)
            .expect("set scoped");
    }
    // Host-scoped true => path included for that host.
    assert_eq!(
        key_for(Some(dir.path()), "https://azdo.example.test/OrgA/_git/repo"),
        "https://azdo.example.test/OrgA/_git/repo"
    );
    // A different host without the scoped key falls to the unscoped OFF.
    assert_eq!(
        key_for(Some(dir.path()), "https://other.example.test/OrgA/repo.git"),
        "https://other.example.test"
    );
}

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
            FillOutcome::Filled { username: "u".to_string(), password: "p".to_string() }
        })
    };
    let cache = CredCache::new(fill, Duration::from_secs(60), Duration::from_secs(48));

    // First resolve unwinds inside the filler; catch it so the test can go
    // on to prove the key was un-wedged.
    let c = Arc::clone(&cache);
    let first = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        c.resolve(None, "https://host.com/a", false).into_option()
    }));
    assert!(first.is_err(), "the filler panic must propagate out of resolve");
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    // Must NOT hang: `in_flight` was cleared by the drop-guard, so this does
    // a fresh fill and returns creds.
    let second = cache.resolve(None, "https://host.com/a", false).into_option();
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
    assert!(cache.state.lock().is_err(), "mutex must actually be poisoned");

    // The cache keeps working through the recovering lock helpers.
    let r = cache
        .resolve(None, "https://example.com/repo.git", false)
        .into_option()
        .expect("resolve must survive a poisoned mutex");
    assert_eq!(r.creds, ("u".to_string(), "p".to_string()));
}
