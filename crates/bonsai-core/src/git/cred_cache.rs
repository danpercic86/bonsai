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

use crate::git::remote::{credential_fill, FillOutcome};

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
type FillFn = Box<dyn Fn(Option<&Path>, &str) -> FillOutcome + Send + Sync>;

/// Result of a cache resolve (P70 §3.1). Replaces the old `Option<Resolved>`:
/// ONLY `Resolved` is ever cached — `NoCredentials` / `GitUnavailable` are
/// never stored, so a transient launch failure can never poison the cache.
pub(crate) enum CredResolve {
    Resolved(Resolved),
    NoCredentials,
    GitUnavailable(String),
}

impl CredResolve {
    /// Test-only compatibility view: `Some` iff the fill actually resolved.
    #[cfg(test)]
    fn into_option(self) -> Option<Resolved> {
        match self {
            CredResolve::Resolved(r) => Some(r),
            _ => None,
        }
    }
}

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
    /// per key (contract §7). A non-`Resolved` outcome mirrors the filler's own
    /// verdict verbatim (P70) and is never cached.
    ///
    /// The `state` lock is NEVER held across `self.fill`: the fast-path/loop
    /// runs under one lock, we mark `in_flight` and drop the lock, THEN call the
    /// blocking filler, then re-lock to store.
    pub(crate) fn resolve(
        self: &Arc<Self>,
        repo_path: Option<&Path>,
        url: &str,
        bypass: bool,
    ) -> CredResolve {
        let key = key_for(repo_path, url);
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
                            return CredResolve::Resolved(Resolved {
                                creds: (user, pass),
                                from_cache: true,
                            });
                        }
                        Freshness::StaleButValid => {
                            // Return the still-valid creds NOW; refresh in the
                            // background so the NEXT read is warm.
                            self.trigger_fill_locked(&mut g, &key);
                            return CredResolve::Resolved(Resolved {
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
                    return CredResolve::Resolved(Resolved {
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
            // ONLY a real fill is stored: a NoCredentials / GitUnavailable
            // outcome must never be cached (P70) — the next op re-asks.
            if let FillOutcome::Filled { username, password } = &filled {
                let mut g = self.lock();
                if let Some(slot) = g.get_mut(&key) {
                    slot.entry = Some(CacheEntry {
                        username: username.clone(),
                        password: password.clone(),
                        stored_at: Instant::now(),
                    });
                }
            }
            filled
            // `_guard` drops here -> clears in_flight + notify_all.
        };

        match filled {
            FillOutcome::Filled { username, password } => CredResolve::Resolved(Resolved {
                creds: (username, password),
                from_cache: false,
            }),
            FillOutcome::NoCredentials => CredResolve::NoCredentials,
            FillOutcome::GitUnavailable(e) => CredResolve::GitUnavailable(e),
        }
    }

    /// Drop the cached entry for `url`'s key (keeps `in_flight`/`request`).
    /// Called when a cache-hit credential is rejected by the server. `repo_path`
    /// must match the one used to `resolve`/`warm` so the SAME key is computed
    /// under `credential.useHttpPath` (F-A5-a).
    pub(crate) fn evict(&self, repo_path: Option<&Path>, url: &str) {
        let key = key_for(repo_path, url);
        let mut g = self.lock();
        if let Some(slot) = g.get_mut(&key) {
            slot.entry = None;
        }
    }

    /// Non-blocking background pre-fill. No-op if already Fresh or a fill is in
    /// flight. Warm-on-open (contract §8).
    pub(crate) fn warm(self: &Arc<Self>, repo_path: Option<&Path>, url: &str) {
        let key = key_for(repo_path, url);
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
            // Fire-and-forget: a non-`Filled` outcome (no creds, or git not
            // launchable) is simply ignored — nothing is cached (P70).
            if let FillOutcome::Filled { username, password } = filled {
                let mut g2 = this.lock();
                if let Some(slot) = g2.get_mut(&key) {
                    slot.entry = Some(CacheEntry {
                        username,
                        password,
                        stored_at: Instant::now(),
                    });
                }
            }
            // `_guard` drops here -> clears in_flight + notify_all.
        });
    }

    // ---- small lock helpers (§10.5: RECOVER from poison, never panic) ----
    // These run inside libgit2's credentials C-callback trampoline: a panic
    // here would permanently kill every fetch/pull/push until app restart.
    // Poison is benign for this map — every slot is left consistent between
    // statements — so recover the inner value instead of propagating.

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Slot>> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn wait<'a>(
        &self,
        guard: std::sync::MutexGuard<'a, HashMap<String, Slot>>,
    ) -> std::sync::MutexGuard<'a, HashMap<String, Slot>> {
        self.cv.wait(guard).unwrap_or_else(|p| p.into_inner())
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

/// Cache key normalization (contract §4). Base key is `scheme://host[:port]`,
/// lowercased, dropping userinfo/query/fragment. When `use_http_path` is set
/// (mirroring git's `credential.useHttpPath`), the URL PATH is appended so a
/// token filled for `host/orgA` is NOT replayed to `host/orgB` on the same
/// host (F-A5-a — the dev.azure.com case). git lowercases scheme+host but NOT
/// the path, so the path is kept verbatim. A non-`://` input (e.g. an SCP-like
/// SSH url) falls back to the lowercased input (no path concept there).
fn normalize_key(url: &str, use_http_path: bool) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_ascii_lowercase();
    };
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let mut authority = &rest[..authority_end];
    if let Some(at) = authority.rfind('@') {
        authority = &authority[at + 1..]; // drop userinfo
    }
    let base = format!(
        "{}://{}",
        scheme.to_ascii_lowercase(),
        authority.to_ascii_lowercase()
    );
    if !use_http_path {
        return base;
    }
    // Append the path only (strip query/fragment), case-preserved.
    let path_end = rest.find(['?', '#']).unwrap_or(rest.len());
    let path = &rest[authority_end..path_end];
    format!("{base}{path}")
}

/// Resolve the cache key for `url`, honoring `credential.useHttpPath` read from
/// the repo (or global) git config. When set, the key includes the URL path so
/// per-org tokens on a shared host don't cross-contaminate (F-A5-a). Reading
/// config per call is cheap relative to a network auth op and keeps `resolve` /
/// `evict` / `warm` computing the SAME key for the same inputs.
fn key_for(repo_path: Option<&Path>, url: &str) -> String {
    normalize_key(url, use_http_path(repo_path, url))
}

/// Whether git's `credential.useHttpPath` is enabled for `url`. Checks the
/// URL-host-scoped key (`credential.<scheme://host>.useHttpPath`, how Azure
/// DevOps setups typically enable it) first, then the unscoped
/// `credential.useHttpPath`. Any config-open/lookup failure ⇒ `false` (the
/// host-only key, unchanged prior behavior). NEVER panics.
fn use_http_path(repo_path: Option<&Path>, url: &str) -> bool {
    let cfg = match repo_path {
        Some(p) => git2::Repository::open(p).ok().and_then(|r| r.config().ok()),
        None => git2::Config::open_default().ok(),
    };
    let Some(cfg) = cfg else {
        return false;
    };
    // Most-specific-wins (a subset of git's URL matching): scheme://host scope.
    let host_scope = normalize_key(url, false);
    if let Ok(b) = cfg.get_bool(&format!("credential.{host_scope}.useHttpPath")) {
        return b;
    }
    cfg.get_bool("credential.useHttpPath").unwrap_or(false)
}

// ---- process-global instance + thin facade the Helper arm calls ----

static GLOBAL: LazyLock<Arc<CredCache>> = LazyLock::new(|| {
    CredCache::new(
        Box::new(credential_fill),
        CRED_TTL,
        CRED_TTL.mul_f64(CRED_REFRESH_FRACTION),
    )
});

pub(crate) fn resolve(repo_path: Option<&Path>, url: &str, bypass: bool) -> CredResolve {
    GLOBAL.resolve(repo_path, url, bypass)
}

pub(crate) fn evict(repo_path: Option<&Path>, url: &str) {
    GLOBAL.evict(repo_path, url);
}

/// Warm-on-open entry point (contract §8, §16). Public so the command layer MAY
/// pre-fill HTTPS remotes after opening a repo. Fire-and-forget / best-effort.
pub fn warm(repo_path: Option<&Path>, url: &str) {
    GLOBAL.warm(repo_path, url);
}

#[cfg(test)]
mod tests;
