//! In-process HTTPS credential cache (P35 contract).
//!
//! Wraps [`crate::git::remote::credential_fill`] with an app-lifetime,
//! in-memory cache keyed by credential context (scheme + authority). On a miss
//! it still resolves through the REAL `git credential fill` helper, so
//! resolution is semantically identical to M6 — it just avoids re-spawning the
//! helper (e.g. Git Credential Manager cold-starting a .NET process) on every
//! network op. Backed by a TTL backstop, stale-while-revalidate refresh, and
//! invalidation-on-rejection driven from `remote.rs::acquire_cred`.
//!
//! Concurrency: single-flight per key via a `Mutex<HashMap>` + `Condvar`; the
//! `state` lock is NEVER held across the blocking `fill` call. Background work
//! (refresh, warm) uses `std::thread::spawn` ONLY — this module has NO tokio and
//! NO async anywhere, per the bonsai-core "no tokio" invariant.
//!
//! Security: the token now lives in process memory for up to [`CRED_TTL`]. This
//! module NEVER logs `url`, `username`, `password`, or subprocess output.
//! Zeroize-on-evict is documented as optional polish and is NOT implemented in
//! v1 (the plaintext already transits process memory per-op today).

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::time::{Duration, Instant};

use crate::git::remote::credential_fill;

/// App-lifetime TTL backstop on a cached credential (staleness bound even
/// though we also invalidate on rejection).
pub(crate) const CRED_TTL: Duration = Duration::from_secs(10 * 60); // 10 min

/// Fraction of TTL after which a READ returns the still-valid cached value AND
/// triggers a background refresh (stale-while-revalidate). 0.8 => refresh at 8 min.
pub(crate) const CRED_REFRESH_FRACTION: f64 = 0.8;

/// A cached credential pair + when it was stored (for TTL/refresh math).
struct CacheEntry {
    username: String,
    password: String,
    stored_at: Instant,
}

/// Inputs needed to (re-)run the filler for a key — kept so a background
/// refresh can re-fill with the original repo cwd + url.
#[derive(Clone)]
struct FillRequest {
    repo_path: Option<PathBuf>,
    url: String,
}

/// Per-key state + single-flight coordination.
struct Slot {
    entry: Option<CacheEntry>,
    in_flight: bool,
    request: FillRequest,
}

/// Freshness of an entry given its age. PURE — unit-testable with no clock,
/// no threads, no git.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Freshness {
    Fresh,
    StaleButValid,
    Expired,
}

fn classify(age: Duration, ttl: Duration, refresh_age: Duration) -> Freshness {
    if age >= ttl {
        Freshness::Expired
    } else if age >= refresh_age {
        Freshness::StaleButValid
    } else {
        Freshness::Fresh
    }
}

/// Result of a resolve. `from_cache` = the creds came from an EXISTING entry
/// (true) vs a fresh fill (false); it drives the invalidation state machine in
/// `acquire_cred` (contract §9).
pub(crate) struct Resolved {
    pub creds: (String, String),
    pub from_cache: bool,
}

/// Injectable filler seam (contract §11). Production = `credential_fill`; tests
/// use a deterministic fake, so cache tests never spawn git and pass on every
/// platform.
type FillFn = Box<dyn Fn(Option<&Path>, &str) -> Option<(String, String)> + Send + Sync>;

pub(crate) struct CredCache {
    state: Mutex<HashMap<String, Slot>>,
    cv: Condvar,
    fill: FillFn,
    ttl: Duration,
    refresh_age: Duration,
}

/// RAII single-flight guard: on drop it re-locks `state`, clears `in_flight`
/// for `key` (if the slot still exists), and wakes all waiters. Held across
/// every blocking `fill` call so that whether the filler returns normally OR
/// PANICS/unwinds, `in_flight` is always cleared and Condvar waiters are always
/// notified. Without it, a panicking filler would leave the slot `in_flight`
/// forever (the mutex is NOT poisoned — it was already released before the
/// fill) and every future `resolve` for that key would block on the Condvar
/// with no notifier: a silent permanent wedge of the auth path for that host.
/// The success-store of a filled entry happens in the guarded region BEFORE the
/// guard drops; the guard only clears the flag + notifies.
struct InFlightGuard<'a> {
    cache: &'a Arc<CredCache>,
    key: &'a str,
}

impl Drop for InFlightGuard<'_> {
    fn drop(&mut self) {
        let mut g = self.cache.lock();
        if let Some(slot) = g.get_mut(self.key) {
            slot.in_flight = false;
        }
        self.cache.cv.notify_all();
    }
}

impl CredCache {
    /// Wrapped in `Arc` so background threads can hold a clone. Tests call this
    /// with a fake filler + short TTLs; production via [`GLOBAL`].
    fn new(fill: FillFn, ttl: Duration, refresh_age: Duration) -> Arc<Self> {
        Arc::new(CredCache {
            state: Mutex::new(HashMap::new()),
            cv: Condvar::new(),
            fill,
            ttl,
            refresh_age,
        })
    }

    /// Cached-or-fresh resolve. `bypass=true` forces a synchronous fresh fill
    /// (used right after `evict` on rejection). Blocking on a miss; single-flight
    /// per key (contract §7). `None` == fill failed (same meaning as
    /// `credential_fill` returning `None`).
    ///
    /// The `state` lock is NEVER held across `self.fill`: the fast-path/loop
    /// runs under one lock, we mark `in_flight` and drop the lock, THEN call the
    /// blocking filler, then re-lock to store.
    pub(crate) fn resolve(
        self: &Arc<Self>,
        repo_path: Option<&Path>,
        url: &str,
        bypass: bool,
    ) -> Option<Resolved> {
        let key = normalize_key(url);
        let req = FillRequest {
            repo_path: repo_path.map(Path::to_path_buf),
            url: url.to_string(),
        };

        // ---- fast path + refresh scheduling: one lock, no blocking ----
        {
            let mut g = self.lock();
            self.upsert_request(&mut g, &key, &req);

            if !bypass {
                if let Some((freshness, user, pass)) = self.peek(&g, &key) {
                    match freshness {
                        Freshness::Fresh => {
                            return Some(Resolved {
                                creds: (user, pass),
                                from_cache: true,
                            });
                        }
                        Freshness::StaleButValid => {
                            // Return the still-valid creds NOW; refresh in the
                            // background so the NEXT read is warm.
                            self.trigger_fill_locked(&mut g, &key);
                            return Some(Resolved {
                                creds: (user, pass),
                                from_cache: true,
                            });
                        }
                        Freshness::Expired => {} // fall through to synchronous fill
                    }
                }
            }

            // ---- synchronous single-flight fill: miss / expired / bypass ----
            loop {
                // A concurrent fill may have landed a Fresh entry while we waited.
                if let Some((Freshness::Fresh, user, pass)) = self.peek(&g, &key) {
                    return Some(Resolved {
                        creds: (user, pass),
                        from_cache: true,
                    });
                }
                let in_flight = g.get(&key).map(|s| s.in_flight).unwrap_or(false);
                if in_flight {
                    // Another thread owns the fill — wait (single-flight).
                    // Predicate re-check loop => spurious-wakeup safe.
                    g = self.wait(g);
                    continue;
                }
                if let Some(slot) = g.get_mut(&key) {
                    slot.in_flight = true;
                }
                break; // we own the fill
            }
            // `g` dropped here -> lock released BEFORE the blocking call.
        }

        // The drop-guard clears `in_flight` + notifies on EVERY exit (normal
        // return OR panic), so a panicking filler can never permanently wedge
        // this key. The success-store stays a distinct step done before the
        // guard drops.
        let filled = {
            let _guard = InFlightGuard {
                cache: self,
                key: &key,
            };
            let filled = (self.fill)(req.repo_path.as_deref(), &req.url); // BLOCKING, no lock held
            if let Some((u, p)) = &filled {
                let mut g = self.lock();
                if let Some(slot) = g.get_mut(&key) {
                    slot.entry = Some(CacheEntry {
                        username: u.clone(),
                        password: p.clone(),
                        stored_at: Instant::now(),
                    });
                }
            }
            filled
            // `_guard` drops here -> clears in_flight + notify_all.
        };

        filled.map(|creds| Resolved {
            creds,
            from_cache: false,
        })
    }

    /// Drop the cached entry for `url`'s key (keeps `in_flight`/`request`).
    /// Called when a cache-hit credential is rejected by the server.
    pub(crate) fn evict(&self, url: &str) {
        let key = normalize_key(url);
        let mut g = self.lock();
        if let Some(slot) = g.get_mut(&key) {
            slot.entry = None;
        }
    }

    /// Non-blocking background pre-fill. No-op if already Fresh or a fill is in
    /// flight. Warm-on-open (contract §8).
    pub(crate) fn warm(self: &Arc<Self>, repo_path: Option<&Path>, url: &str) {
        let key = normalize_key(url);
        let req = FillRequest {
            repo_path: repo_path.map(Path::to_path_buf),
            url: url.to_string(),
        };
        let mut g = self.lock();
        self.upsert_request(&mut g, &key, &req);
        if let Some((Freshness::Fresh, _, _)) = self.peek(&g, &key) {
            return; // already warm
        }
        self.trigger_fill_locked(&mut g, &key);
    }

    /// Spawns a background fill for `key` (called WITH the map lock held). The
    /// child captures its inputs before spawning, so it never needs the lock to
    /// start; single-flight via the `in_flight` flag. No-op if a fill is
    /// already running.
    fn trigger_fill_locked(self: &Arc<Self>, g: &mut HashMap<String, Slot>, key: &str) {
        let req = match g.get_mut(key) {
            Some(slot) if !slot.in_flight => {
                slot.in_flight = true;
                slot.request.clone()
            }
            _ => return, // single-flight — a fill is already running (or no slot)
        };
        let this = Arc::clone(self);
        let key = key.to_string();
        std::thread::spawn(move || {
            // Same panic-safe drop-guard as the synchronous path: clear
            // in_flight + notify on any exit, so a panicking background filler
            // never wedges the key.
            let _guard = InFlightGuard {
                cache: &this,
                key: &key,
            };
            let filled = (this.fill)(req.repo_path.as_deref(), &req.url); // BLOCKING, no lock held
            if let Some((u, p)) = filled {
                let mut g2 = this.lock();
                if let Some(slot) = g2.get_mut(&key) {
                    slot.entry = Some(CacheEntry {
                        username: u,
                        password: p,
                        stored_at: Instant::now(),
                    });
                }
            }
            // `_guard` drops here -> clears in_flight + notify_all.
        });
    }

    // ---- small lock helpers (poisoning is an unrecoverable bug, §10.5) ----

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Slot>> {
        self.state.lock().expect("cred cache mutex poisoned")
    }

    fn wait<'a>(
        &self,
        guard: std::sync::MutexGuard<'a, HashMap<String, Slot>>,
    ) -> std::sync::MutexGuard<'a, HashMap<String, Slot>> {
        self.cv.wait(guard).expect("cred cache mutex poisoned")
    }

    /// Insert the slot if absent, and remember the latest fill inputs so a
    /// background refresh re-fills with the current cwd + url.
    fn upsert_request(&self, g: &mut HashMap<String, Slot>, key: &str, req: &FillRequest) {
        match g.entry(key.to_string()) {
            Entry::Occupied(mut o) => o.get_mut().request = req.clone(),
            Entry::Vacant(v) => {
                v.insert(Slot {
                    entry: None,
                    in_flight: false,
                    request: req.clone(),
                });
            }
        }
    }

    /// Freshness + a clone of the cached creds for `key`, or `None` if there is
    /// no stored entry. Cloning lets us drop the map borrow before scheduling a
    /// background fill.
    fn peek(&self, g: &HashMap<String, Slot>, key: &str) -> Option<(Freshness, String, String)> {
        g.get(key).and_then(|slot| slot.entry.as_ref()).map(|e| {
            (
                classify(e.stored_at.elapsed(), self.ttl, self.refresh_age),
                e.username.clone(),
                e.password.clone(),
            )
        })
    }
}

/// Cache key normalization (contract §4): `scheme://host[:port]`, lowercased,
/// dropping userinfo/path/query/fragment. A non-`://` input (e.g. an SCP-like
/// SSH url) falls back to the lowercased input.
fn normalize_key(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_ascii_lowercase();
    };
    let authority_end = rest
        .find(['/', '?', '#'])
        .unwrap_or(rest.len());
    let mut authority = &rest[..authority_end];
    if let Some(at) = authority.rfind('@') {
        authority = &authority[at + 1..]; // drop userinfo
    }
    format!(
        "{}://{}",
        scheme.to_ascii_lowercase(),
        authority.to_ascii_lowercase()
    )
}

// ---- process-global instance + thin facade the Helper arm calls ----

static GLOBAL: LazyLock<Arc<CredCache>> = LazyLock::new(|| {
    CredCache::new(
        Box::new(credential_fill),
        CRED_TTL,
        CRED_TTL.mul_f64(CRED_REFRESH_FRACTION),
    )
});

pub(crate) fn resolve(repo_path: Option<&Path>, url: &str, bypass: bool) -> Option<Resolved> {
    GLOBAL.resolve(repo_path, url, bypass)
}

pub(crate) fn evict(url: &str) {
    GLOBAL.evict(url);
}

/// Warm-on-open entry point (contract §8, §16). Public so the command layer MAY
/// pre-fill HTTPS remotes after opening a repo. Fire-and-forget / best-effort.
pub fn warm(repo_path: Option<&Path>, url: &str) {
    GLOBAL.warm(repo_path, url);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // ---- injectable fake fillers (NO git spawn, cross-platform) ----

    /// Returns fixed creds and bumps `counter` on every call.
    fn counting_fill(counter: Arc<AtomicUsize>) -> FillFn {
        Box::new(move |_repo, _url| {
            counter.fetch_add(1, Ordering::SeqCst);
            Some(("u".to_string(), "p".to_string()))
        })
    }

    /// Returns a distinguishable password `p{n}` per call, so tests can prove a
    /// value swap.
    fn versioned_fill(counter: Arc<AtomicUsize>) -> FillFn {
        Box::new(move |_repo, _url| {
            let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
            Some(("u".to_string(), format!("p{n}")))
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
            if let Some(r) = cache.resolve(None, url, false) {
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
        let r1 = cache.resolve(None, "https://host.com/a", false).expect("some");
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert!(!r1.from_cache);
        assert_eq!(r1.creds, ("u".to_string(), "p".to_string()));

        let r2 = cache.resolve(None, "https://host.com/a", false).expect("some");
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
        cache.resolve(None, "https://host.com/a", false).expect("some");
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        std::thread::sleep(Duration::from_millis(140)); // past ttl
        let r = cache.resolve(None, "https://host.com/a", false).expect("some");
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
        let r1 = cache.resolve(None, "https://host.com/a", false).expect("some");
        assert_eq!(r1.creds.1, "p1");
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        std::thread::sleep(Duration::from_millis(120)); // into stale-but-valid window
        let r2 = cache.resolve(None, "https://host.com/a", false).expect("some");
        assert!(r2.from_cache);
        assert_eq!(r2.creds.1, "p1", "stale read returns the OLD value immediately");

        // Wait on store COMPLETION (the refreshed value becoming observable),
        // not the call-counter — the counter bumps at fill START, before the
        // background thread re-locks to store, so keying off it would race.
        assert!(
            poll_until_value(&cache, "https://host.com/a", "p2"),
            "background refresh did not store the new value in time"
        );

        let r3 = cache.resolve(None, "https://host.com/a", false).expect("some");
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
                Some(("u".to_string(), "p".to_string()))
            })
        };
        let cache = CredCache::new(fill, Duration::from_secs(60), Duration::from_secs(48));

        let mut handles = Vec::new();
        for _ in 0..8 {
            let c = Arc::clone(&cache);
            handles.push(std::thread::spawn(move || {
                c.resolve(None, "https://host.com/a", false).map(|r| r.creds)
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
        cache.resolve(None, "https://host-a.com/x", false).expect("some");
        cache.resolve(None, "https://host-b.com/x", false).expect("some");
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
        let r1 = cache.resolve(None, "https://host.com/a", false).expect("some");
        assert_eq!(r1.creds.1, "p1");
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        cache.evict("https://host.com/a");
        let r2 = cache.resolve(None, "https://host.com/a", true).expect("some");
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
                    None // first call fails
                } else {
                    Some(("u".to_string(), "p".to_string()))
                }
            })
        };
        let cache = CredCache::new(fill, Duration::from_secs(60), Duration::from_secs(48));

        assert!(cache.resolve(None, "https://host.com/a", false).is_none());
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // No entry stored + in_flight cleared -> a following resolve still works.
        let r = cache.resolve(None, "https://host.com/a", false).expect("some");
        assert_eq!(counter.load(Ordering::SeqCst), 2);
        assert!(!r.from_cache);
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

        let r = cache.resolve(None, "https://host.com/a", false).expect("some");
        assert!(r.from_cache);
        assert_eq!(counter.load(Ordering::SeqCst), 1, "resolve finds it warm");

        cache.warm(None, "https://host.com/a"); // already Fresh -> no-op
        std::thread::sleep(Duration::from_millis(40));
        assert_eq!(counter.load(Ordering::SeqCst), 1, "warm on Fresh key does not re-fill");
    }

    // 9. key normalization table.
    #[test]
    fn normalize_key_table() {
        assert_eq!(normalize_key("https://Host.COM/a/b.git?x=1#f"), "https://host.com");
        assert_eq!(normalize_key("https://user:pw@host.com/a"), "https://host.com");
        assert_eq!(normalize_key("https://host.com/other"), "https://host.com");
        assert_eq!(normalize_key("https://host.com:8443/a"), "https://host.com:8443");
        assert_eq!(normalize_key("HTTPS://HOST.com"), "https://host.com");
        // non-`://` fallback -> lowercased input.
        assert_eq!(
            normalize_key("Git@GitHub.com:owner/repo.git"),
            "git@github.com:owner/repo.git"
        );

        // The first three collapse to one shared key (host+scheme only).
        let a = normalize_key("https://Host.COM/a/b.git?x=1#f");
        let b = normalize_key("https://user:pw@host.com/a");
        let c = normalize_key("https://host.com/other");
        assert_eq!(a, b);
        assert_eq!(b, c);
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
                Some(("u".to_string(), "p".to_string()))
            })
        };
        let cache = CredCache::new(fill, Duration::from_secs(60), Duration::from_secs(48));

        // First resolve unwinds inside the filler; catch it so the test can go
        // on to prove the key was un-wedged.
        let c = Arc::clone(&cache);
        let first = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            c.resolve(None, "https://host.com/a", false)
        }));
        assert!(first.is_err(), "the filler panic must propagate out of resolve");
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Must NOT hang: `in_flight` was cleared by the drop-guard, so this does
        // a fresh fill and returns creds.
        let second = cache.resolve(None, "https://host.com/a", false);
        assert_eq!(
            second.map(|r| r.creds),
            Some(("u".to_string(), "p".to_string())),
            "key un-wedged: a subsequent fresh fill succeeds"
        );
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
