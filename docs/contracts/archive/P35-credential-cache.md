# P35 — In-process HTTPS credential cache: Implementation Contract

Status: authoritative for P35. Implementer: senior-dev. Builds directly on the M6 credential
chain and its **Addendum 2026-08-04** (`docs/contracts/M6-remotes.md` §A.1–§A.6) — the
`credential_fill` / `acquire_cred` / `CredAttempts` design shipped in
`crates/bonsai-core/src/git/remote.rs`.

**Problem.** `credential_fill` (`remote.rs:145`) shells out to `git credential fill` on EVERY git
network op. On Windows with GCM each call cold-starts `git-credential-manager.exe` (.NET), adding
hundreds of ms to seconds per fetch/pull/push/clone. **Decision (locked, do not re-litigate):** add
an in-process, app-lifetime cache that wraps `credential_fill`, keyed by credential context, with a
TTL backstop, stale-while-revalidate refresh, and invalidation-on-rejection. Credential resolution
stays semantically identical (still resolves through the REAL helper on a miss).

**Invariant recap.** bonsai-core owns all Git logic and MUST NOT depend on tokio; background work
uses `std::thread`. Nothing about the IPC surface changes (§13). The never-prompt policy
(`GIT_TERMINAL_PROMPT=0`) and never-log-credentials discipline from M6 §A.4 carry over verbatim.

---

## 1. Scope & non-goals

In scope:
- New module `crates/bonsai-core/src/git/cred_cache.rs` holding all cache logic.
- Making `remote.rs::credential_fill` reachable by the cache (visibility change only — body byte-for-byte unchanged).
- The `acquire_cred` Helper-arm + `CredAttempts` change for invalidation-on-rejection (§9).

Out of scope / non-goals:
- No new Tauri command, event, or channel; no TS type changes; no `mock.ts` changes (§13).
- No async runtime, no new crate dependency (all `std`).
- No hunk of credential UI; no interactive prompt; no on-disk persistence (in-memory, session-only).
- Zeroize-on-evict is documented as OPTIONAL polish, not implemented in v1 (§12).

## 2. New / changed files

```
crates/bonsai-core/
  src/git/mod.rs          # + `pub mod cred_cache;` (alphabetical: after `conflict`, before `diff`)
  src/git/cred_cache.rs   # NEW — cache data structures, resolve/evict/warm, global instance, tests
  src/git/remote.rs       # credential_fill: fn -> pub(crate) fn (body UNCHANGED);
                          #   CredAttempts + next_cred_method + acquire_cred Helper arm (§9)
```

All 5 credential call sites (`remote.rs` fetch & push, `tags.rs`, `submodule.rs`, `clone.rs`) route
through `acquire_cred`; **none needs editing** — the cache plugs in inside `acquire_cred`'s Helper
arm alone. (Correction to the brief: `clone.rs:87` calls `acquire_cred(None, …)`, NOT
`credential_fill` directly — so it is covered automatically. Flag §16.)

## 3. Module boundary

- `cred_cache.rs` is the ONLY place the cache lives. It imports `credential_fill` from `remote.rs`.
- `remote.rs::acquire_cred` calls the module-level facade (`cred_cache::resolve` / `::evict`) — it
  never touches `CredCache` internals, `GLOBAL`, or `credential_fill` directly anymore.
- The public `cred_cache::warm` is the OPTIONAL warm-on-open entry point (§8, §16); wiring it is a
  ready-to-wire follow-up, not part of this milestone.

## 4. Cache key normalization

libgit2 hands the callback a `url`. Git's own matching is protocol+host (+path only when
`credential.useHttpPath=true`, which we cannot cheaply detect). **Recommended, documented rule:**

```
normalize_key(url) -> String:
    (scheme, rest) = url.split_once("://")   // no "://" -> return url.to_ascii_lowercase() (fallback)
    authority = prefix of rest up to the FIRST of '/', '?', '#'  (or all of rest)
    if authority contains '@':  authority = substring AFTER the LAST '@'   // drop userinfo
    return format!("{}://{}", scheme.to_ascii_lowercase(), authority.to_ascii_lowercase())
```

- Keeps `scheme://host[:port]`; **drops** path, query, fragment, and userinfo; lowercases scheme+authority.
- **Tradeoff (documented, accepted):** if a user sets `credential.useHttpPath=true` (rare), two repos
  on the same host that would legitimately have DIFFERENT credentials share one cache key
  (over-matching). Correctness is preserved on the resolution path — a miss still fills with the FULL
  `url` and the server rejects a wrong cred, triggering evict+refill (§9). The only cost is an
  occasional extra rejection round-trip for that rare config. Byte-identical to git's DEFAULT
  matching (protocol+host) for everyone else.

## 5. Constants

```rust
/// App-lifetime TTL backstop on a cached credential (staleness bound even
/// though we also invalidate on rejection).
pub(crate) const CRED_TTL: Duration = Duration::from_secs(10 * 60);   // 10 min

/// Fraction of TTL after which a READ returns the still-valid cached value AND
/// triggers a background refresh (stale-while-revalidate). 0.8 => refresh at 8 min.
pub(crate) const CRED_REFRESH_FRACTION: f64 = 0.8;
```

The `GLOBAL` instance derives `refresh_age = CRED_TTL.mul_f64(CRED_REFRESH_FRACTION)` once at
construction. Both live in `cred_cache.rs`.

## 6. Data structures & public API (`cred_cache.rs`)

Signatures only — senior-dev fills bodies to the algorithms in §7–§9.

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::time::{Duration, Instant};

use crate::git::remote::credential_fill;   // now pub(crate)

/// A cached credential pair + when it was stored (for TTL/refresh math).
struct CacheEntry { username: String, password: String, stored_at: Instant }

/// Inputs needed to (re-)run the filler for a key — kept so a background
/// refresh can re-fill with the original repo cwd + url.
#[derive(Clone)]
struct FillRequest { repo_path: Option<PathBuf>, url: String }

/// Per-key state + single-flight coordination.
struct Slot { entry: Option<CacheEntry>, in_flight: bool, request: FillRequest }

/// Freshness of an entry given its age. PURE — unit-testable with no clock,
/// no threads, no git.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Freshness { Fresh, StaleButValid, Expired }

fn classify(age: Duration, ttl: Duration, refresh_age: Duration) -> Freshness;

/// Result of a resolve. `from_cache` = the creds came from an EXISTING entry
/// (true) vs a fresh fill (false); it drives the invalidation state machine
/// in `acquire_cred` (§9).
pub(crate) struct Resolved { pub creds: (String, String), pub from_cache: bool }

/// Injectable filler seam (§11). Production = `credential_fill`; tests = a
/// deterministic fake, so cache tests never spawn git and pass on all platforms.
type FillFn = Box<dyn Fn(Option<&Path>, &str) -> Option<(String, String)> + Send + Sync>;

pub(crate) struct CredCache {
    state: Mutex<HashMap<String, Slot>>,
    cv: Condvar,
    fill: FillFn,
    ttl: Duration,
    refresh_age: Duration,
}

impl CredCache {
    /// Wrapped in `Arc` so background threads can hold a clone (§8). Tests call
    /// this with a fake filler + short TTLs; production via `GLOBAL`.
    fn new(fill: FillFn, ttl: Duration, refresh_age: Duration) -> Arc<Self>;

    /// Cached-or-fresh resolve. `bypass=true` forces a synchronous fresh fill
    /// (used right after `evict` on rejection). Blocking on a miss; single-flight
    /// per key (§7). `None` == fill failed (same meaning as `credential_fill` None).
    pub(crate) fn resolve(self: &Arc<Self>, repo_path: Option<&Path>, url: &str, bypass: bool)
        -> Option<Resolved>;

    /// Drop the cached entry for `url`'s key (keeps in_flight/request). Called
    /// when a cache-hit credential is rejected by the server.
    pub(crate) fn evict(&self, url: &str);

    /// Non-blocking background pre-fill. No-op if already Fresh or a fill is in
    /// flight. Warm-on-open (§8).
    pub(crate) fn warm(self: &Arc<Self>, repo_path: Option<&Path>, url: &str);
}

fn normalize_key(url: &str) -> String;   // §4

// ---- process-global instance + thin facade the Helper arm calls ----

static GLOBAL: LazyLock<Arc<CredCache>> = LazyLock::new(|| {
    CredCache::new(
        Box::new(|repo_path, url| credential_fill(repo_path, url)),
        CRED_TTL,
        CRED_TTL.mul_f64(CRED_REFRESH_FRACTION),
    )
});

pub(crate) fn resolve(repo_path: Option<&Path>, url: &str, bypass: bool) -> Option<Resolved>;
pub(crate) fn evict(url: &str);

/// OPTIONAL warm-on-open (§8, §16). Public so a command-layer repo-open path
/// MAY call it; ready-to-wire, unwired in this milestone.
pub fn warm(repo_path: Option<&Path>, url: &str);
```

`LazyLock` is stable (Rust ≥ 1.80; toolchain pinned at 1.97 in `rust-toolchain.toml`) — use it, no
`once_cell` needed.

## 7. Resolve algorithm — stale-while-revalidate + single-flight (pseudocode)

`classify` (pure):

```
classify(age, ttl, refresh_age):
    if age >= ttl:           Expired
    elif age >= refresh_age: StaleButValid
    else:                    Fresh
```

`resolve` — the lock is NEVER held across the blocking fill:

```
resolve(self, repo_path, url, bypass):
    key = normalize_key(url)
    req = FillRequest{ repo_path.map(to_path_buf), url.to_string() }

    // ---- fast path + refresh scheduling, one lock, no blocking ----
    lock g = self.state:
        slot = g.entry(key).or_insert(Slot{ None, in_flight:false, request:req })
        slot.request = req                       // remember latest inputs for refresh
        if not bypass and slot.entry is Some(e):
            match classify(e.stored_at.elapsed(), ttl, refresh_age):
                Fresh          -> return Some(Resolved{ (e.user,e.pass), from_cache:true })
                StaleButValid  -> creds = (e.user,e.pass)
                                  trigger_fill_locked(&mut g, key)   // background; sets in_flight if idle
                                  return Some(Resolved{ creds, from_cache:true })
                Expired        -> {}                                 // fall through to synchronous fill

        // ---- synchronous single-flight fill: miss / expired / bypass ----
        loop:
            slot = g[key]
            if slot.entry is Some(e) and classify(e.age, ttl, refresh_age) == Fresh:
                return Some(Resolved{ (e.user,e.pass), from_cache:true })   // a concurrent fill landed
            if slot.in_flight:
                g = self.cv.wait(g); continue        // another thread owns the fill — wait (single-flight)
            slot.in_flight = true; break             // we own the fill
        // g dropped here -> lock released BEFORE the blocking call

    filled = (self.fill)(req.repo_path.as_deref(), &req.url)   // BLOCKING, no lock held

    lock g = self.state:
        slot = g[key]
        slot.in_flight = false
        if filled is Some((u,p)): slot.entry = Some(CacheEntry{ u, p, stored_at: Instant::now() })
        self.cv.notify_all()

    return filled.map(|creds| Resolved{ creds, from_cache:false })
```

Notes:
- The in-loop "a concurrent Fresh entry landed → reuse it" check applies to `bypass` too: after
  `evict`, the entry is `None`, so any Fresh entry present post-wait came from a fill that completed
  AFTER we entered — reusing it is correct and preserves single-flight (no second subprocess).
- `None` from the fill leaves any prior entry untouched only if it was still there; on the
  miss/expired/bypass path the entry is absent/stale, so a `None` result yields a miss (same
  fall-through as today's `credential_fill` → `None`).

## 8. Background refresh & warm — `std::thread` design

`trigger_fill_locked` (called WITH the map lock held; spawns AFTER capturing inputs so the child
never needs the lock to start):

```
trigger_fill_locked(self: &Arc<Self>, g: &mut Guard, key):
    slot = g[key]
    if slot.in_flight: return          // single-flight — a fill is already running
    slot.in_flight = true
    req  = slot.request.clone()
    this = Arc::clone(self)
    std::thread::spawn(move ||:
        filled = (this.fill)(req.repo_path.as_deref(), &req.url)   // BLOCKING, no lock held
        lock g2 = this.state:
            if g2 has key:
                slot.in_flight = false
                if filled is Some((u,p)): slot.entry = CacheEntry{ u, p, stored_at: Instant::now() }
            this.cv.notify_all()
    )
```

`warm`:

```
warm(self, repo_path, url):
    key = normalize_key(url)
    lock g:
        slot = g.entry(key).or_insert(...); slot.request = FillRequest{...}
        if slot.entry is Some(e) and classify(e.age,...) == Fresh: return   // already warm
        trigger_fill_locked(&mut g, key)
```

- Runtime-agnostic: `std::thread::spawn` only — NO tokio, NO async. State this in the module doc comment.
- The background thread holds a cloned `Arc<CredCache>`; `GLOBAL` is `'static`, so its threads
  outliving a caller is fine (they self-terminate after storing).
- Single-flight covers the refresh, the warm, AND concurrent blocking misses via the shared `in_flight`
  flag + `Condvar`: at most ONE `git credential fill` subprocess per key at a time.

Optional warm wiring (follow-up, see §16): a command-layer repo-open / remote-list inner COULD call
`cred_cache::warm(Some(&workdir), &remote_url)` for each HTTPS remote after opening a repo, so the
first fetch/pull/push is already warm. Do NOT wire it in this milestone unless the seam is clean;
document it as ready-to-wire.

## 9. `acquire_cred` / `CredAttempts` change — invalidation on rejection (§ correctness subtlety)

**Problem restated.** Today Helper is attempted exactly once per op. With a cache, a STALE cached
cred the server REJECTS would (under the old one-shot guard) fall through to SshAgent/Default and
fail — even though a FRESH `git credential fill` would have succeeded, making caching LESS reliable
than today. Fix: allow Helper a SECOND attempt **only when the first was a cache HIT**, and make that
second attempt a cache-bypassing fresh fill (evict + `resolve(bypass=true)`). A fresh fill (cache
miss or bypass) is NEVER retried — no loops.

Replace the `bool helper` field with a small state machine:

```rust
/// Helper-arm state across libgit2 callback re-invocations within ONE operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum HelperState {
    #[default]
    Untried,
    /// First Helper attempt returned a CACHED entry; ONE cache-bypassing re-fill
    /// is still permitted (the invalidation-on-rejection path).
    RetryAllowed,
    /// Helper exhausted: a fresh fill was attempted (miss or bypass), a cache-hit
    /// cred failed to construct, or resolve returned None.
    Done,
}

#[derive(Debug, Default)]
pub(crate) struct CredAttempts { helper: HelperState, agent: bool, default_: bool }
```

`next_cred_method` — helper eligibility is now driven by `HelperState`; it does NOT mutate `helper`
(acquire_cred owns those transitions). `agent`/`default_` behavior is UNCHANGED:

```
next_cred_method(attempts, allowed):
    if allowed.USER_PASS_PLAINTEXT and attempts.helper != Done:
        return Some(Helper)
        // INVARIANT: acquire_cred MUST set attempts.helper (RetryAllowed on a first
        // cache-hit, else Done) after every Helper attempt, or this loops.
    if allowed.SSH_KEY and not attempts.agent:  attempts.agent = true;   return Some(SshAgent)
    if allowed.DEFAULT  and not attempts.default_: attempts.default_ = true; return Some(Default)
    return None
```

`acquire_cred` Helper arm (SshAgent, Default, and exhaustion arms BYTE-FOR-BYTE UNCHANGED from M6
§A.2; `repo_path`/`url` params unchanged):

```
Some(CredMethod::Helper) =>
    bypass = (attempts.helper == RetryAllowed)
    if bypass: cred_cache::evict(url)
    match cred_cache::resolve(repo_path, url, bypass):
        Some(Resolved{ creds:(user,pass), from_cache }) =>
            match git2::Cred::userpass_plaintext(&user, &pass):
                Ok(cred) =>
                    // Only a FIRST-attempt cache hit earns a retry; a fresh fill
                    // (miss or bypass) is terminal.
                    attempts.helper = if from_cache and not bypass { RetryAllowed } else { Done }
                    return Ok(cred)
                Err(_) => attempts.helper = Done      // theoretical; treat as construction failure
        None => attempts.helper = Done                // no cached creds / fill failed -> fall through
    // fall through (continue loop): helper is now RetryAllowed only on a return path,
    // never on a continue path -> the within-invocation loop always terminates.
```

Resulting per-operation bounds (assert in tests):
- Helper attempted **at most twice**, and the second time ONLY when the first was a cache hit;
- a fresh fill (miss or bypass) is **never** retried;
- SshAgent and Default each still attempted **at most once** afterwards, in order;
- exhaustion still returns the `CRED_EXHAUSTED_MSG` error (M6 §2.2) unchanged.

The `RetryAllowed` state is set only on a `return Ok(cred)` (which exits `acquire_cred`'s internal
loop), so the bypass path is reached ONLY on a fresh libgit2 re-invocation after a real rejection —
never within a single invocation's fall-through loop.

## 10. Concurrency & lock discipline (reviewer MUST-FIX if violated)

1. **Never hold `state` lock across a blocking fill.** Both `resolve` and `trigger_fill_locked`
   capture the `FillRequest`, release the lock, THEN call `self.fill`, then re-lock to store.
2. **Single-flight:** at most one `git credential fill` subprocess per key at a time, enforced by the
   per-key `in_flight` flag + shared `Condvar`. Concurrent blocking misses wait; background
   refresh/warm no-op when a fill is already in flight.
3. `Condvar::wait` is used in a predicate re-check loop (spurious-wakeup safe): after waking, re-read
   the slot and either reuse a Fresh entry or become the filler.
4. `std::thread::spawn` only — NO tokio/async anywhere in `cred_cache.rs`.
5. Mutex poisoning: `.lock().unwrap()`/`.expect(...)` is acceptable (a poisoned cred cache is a bug,
   not a recoverable state); do NOT swallow it silently in a way that returns wrong creds.

## 11. Testability seam

- `CredCache::new(fill, ttl, refresh_age)` takes an **injectable filler** and **injectable timings**.
  Tests construct their own `Arc<CredCache>` with a deterministic fake `FillFn` (e.g. a closure over
  an `AtomicUsize` call-counter returning canned `Some(("u".into(),"p".into()))` or `None`) and
  SHORT TTLs (e.g. `ttl=200ms`, `refresh_age=100ms`) — **no git process is ever spawned**, so the
  tests pass on Windows/macOS/Linux. This deliberately avoids the M6 `#!/bin/sh`-fixture trap
  (those no-op on Windows).
- `classify(age, ttl, refresh_age)` is PURE — unit-test its three boundaries with zero clock/threads.
- Time control: prefer short real TTLs + `std::thread::sleep`/`Instant` for the behavior tests
  (deterministic enough, no `Instant`-construction hacks). A `now_fn` injection is an acceptable
  alternative but NOT required.
- The existing M6 §6.5/§6.6 `next_cred_method` / `map_remote_err` tests in `remote.rs` must be
  UPDATED for the new `HelperState` (the pure `next_cred_method` sequence test now drives the helper
  state manually between calls, mirroring what `acquire_cred` does). `map_remote_err` tests are
  unaffected.

## 12. Security note

The token now lives in process memory for up to `CRED_TTL` (and is refreshed) rather than only for
the duration of one op. This is a deliberate, accepted tradeoff for performance. Requirements:
- **Nothing logged.** No `eprintln!`/`println!`/`tracing::*`/`dbg!` in `cred_cache.rs` (or the Helper
  arm) ever prints `url`, `username`, `password`, or subprocess output. Same discipline as M6 §A.4.3.
- **Zeroize on eviction is OPTIONAL polish, NOT implemented in v1.** Documented as a ready follow-up
  (would need the `zeroize` crate — a new dependency; not warranted now, since the plaintext already
  transits process memory per-op today). Recommendation: skip for v1.
- No on-disk persistence; cache dies with the process.

## 13. IPC surface

**None.** P35 is entirely internal to bonsai-core credential resolution:
- No new Tauri command, event, or channel; the `fetch`/`pull`/`push`/`clone` command surfaces are
  unchanged.
- No `src/ipc/types.ts`, `tauri.ts`, or `mock.ts` changes. The browser harness never invokes
  credential logic (M6 §5: "Mock never invokes credential logic"), so the mock stays trivially
  correct — no fixture work needed. Note this explicitly in the PR so the reviewer does not look for
  a missing mock update.
- The optional `warm` entry point, IF later wired, is called Rust-side from within an EXISTING
  command inner — still no new IPC surface.

## 14. Testing (contract for tester)

All in `cred_cache.rs` `#[cfg(test)]`, using the injectable fake filler (NO git spawn, NO scratch
repo, cross-platform). `TMP`/`TEMP=D:\Temp` per project rule (though these tests touch no disk).

1. **classify boundaries.** `age < refresh_age` → Fresh; `refresh_age ≤ age < ttl` → StaleButValid;
   `age ≥ ttl` → Expired (test at and around each boundary).
2. **Miss then hit.** First `resolve` → fills (fake filler call count == 1, `from_cache=false`);
   immediate second `resolve` → cached (count still 1, `from_cache=true`).
3. **TTL expiry.** With tiny `ttl`, sleep past it → next `resolve` re-fills (count increments,
   `from_cache=false`).
4. **Stale-while-revalidate.** With `refresh_age < ttl`, sleep into the stale-but-valid window →
   `resolve` returns the OLD creds immediately with `from_cache=true` AND schedules a background
   refill; after a short join/poll the fake filler count has incremented and a subsequent `resolve`
   returns the REFRESHED value. (Fake filler returns distinguishable values across calls to prove the
   swap.)
5. **Single-flight.** Fake filler blocks on a barrier/sleep; spawn N threads calling `resolve` for the
   same key concurrently → filler invoked exactly ONCE; all N get the same creds. Different keys →
   independent fills.
6. **Bypass evict+refill.** Seed an entry; `evict(url)` then `resolve(bypass=true)` → filler invoked
   again, `from_cache=false`, returns the fresh value.
7. **Fill failure.** Fake filler returns `None` → `resolve` returns `None`, no entry stored,
   `in_flight` cleared (a following successful `resolve` still works).
8. **`warm`.** On an empty key → schedules a background fill; a following `resolve` finds it warm
   (`from_cache=true`, no additional filler call). `warm` on an already-Fresh key → no filler call.
9. **key normalization.** `normalize_key` table: `https://Host.COM/a/b.git?x=1#f` and
   `https://user:pw@host.com/a` and `https://host.com/other` collapse per §4 (host+scheme only,
   lowercased, userinfo/path/query/fragment dropped); a non-`://` fallback returns lowercased input.
10. **CredAttempts state machine (in `remote.rs` tests).** Drive `next_cred_method` + the helper
    transitions to assert: (a) hit-then-reject → Helper twice (2nd bypass) → then SshAgent → Default;
    (b) miss (fresh) → reject → Helper NOT retried → SshAgent → Default; (c) exhaustion returns
    `CRED_EXHAUSTED_MSG`. Update the existing §6.5 sequence test for `HelperState`.
11. **Regression.** All existing `remote.rs` cred/error unit tests compile and pass (updated only for
    `HelperState` per §11); `credential_fill` behavior (M6 §A.6) is unchanged.

## 15. Acceptance criteria

AI gate:
1. `crates/bonsai-core/src/git/cred_cache.rs` exists; `git/mod.rs` registers `pub mod cred_cache;`.
2. `remote.rs::credential_fill` is `pub(crate)`, **body byte-for-byte unchanged** (only visibility).
3. `acquire_cred` Helper arm calls `cred_cache::resolve`/`evict` per §9; SshAgent/Default/exhaustion
   arms and `credential_fill` are otherwise untouched.
4. `CredAttempts.helper` is the `HelperState` state machine; `next_cred_method` matches §9 and no
   longer self-mutates `helper`.
5. Invalidation-on-rejection: a cache-HIT cred that is rejected triggers exactly ONE cache-bypassing
   fresh re-fill within the same op before falling through (test 10a). A fresh fill is never retried
   (test 10b). Helper ≤ 2 attempts/op; SshAgent, Default ≤ 1 each.
6. Cache key normalization matches §4 exactly (test 9), with the useHttpPath tradeoff documented.
7. `CRED_TTL` (10 min) and `CRED_REFRESH_FRACTION` (0.8) are named consts; `refresh_age` derived once.
8. `classify` is pure and correct at all three boundaries (test 1).
9. Hit/miss, TTL expiry, and stale-while-revalidate (return-stale + background refill so the NEXT read
   is warm) behave per §7 (tests 2–4).
10. Single-flight: concurrent resolves + background refresh/warm for one key spawn AT MOST one filler
    invocation at a time (test 5); `warm` is non-blocking and no-ops when Fresh/in-flight (test 8).
11. The `state` lock is never held across a `fill` call; background work uses `std::thread::spawn`
    only — **no tokio, no async, no new crate dependency** (grep the diff).
12. Injectable-filler seam: all cache tests run WITHOUT spawning git and pass on all platforms
    (no `#!/bin/sh` fixtures in the new tests).
13. Security: no credential/url/subprocess-output is logged anywhere in `cred_cache.rs` or the Helper
    arm; zeroize-on-evict documented as optional-not-done.
14. No IPC/TS/mock changes; `fetch`/`pull`/`push`/`clone` command surfaces unchanged; browser harness
    unaffected.
15. `cargo test -p bonsai-core` green; `cargo clippy -p bonsai-core -- -D warnings` clean; existing
    M6 cred/error tests still pass (updated only for `HelperState`).

USER CHECKPOINT (never self-declared): against a REAL HTTPS remote configured with GCM (or
osxkeychain/libsecret) — a cold first fetch/pull/push resolves via the helper as before; the SECOND
and subsequent ops in the same session are visibly faster (no helper process spawn — observe via
Task Manager/`ps` that `git-credential-manager` does not relaunch on the warm op). Rotate/expire the
stored credential externally and confirm the next op recovers (evict+refill) rather than failing.

## 16. Ambiguities / flags for the orchestrator

- **Brief correction (non-blocking):** `clone.rs` does NOT call `credential_fill(None, url)` directly
  — it calls `acquire_cred(None, …)` (`clone.rs:87`). All 5 sites route through `acquire_cred`, so the
  Helper-arm change is the ONLY edit; zero call-site changes. Contract designed accordingly.
- **Warm-on-open is left UNWIRED** (public `warm` provided, ready to wire). Recommendation: keep this
  milestone focused; wire warm in a follow-up once a clean repo-open/remote-list seam is chosen
  (candidate: the repo-open command inner, iterating HTTPS remotes). Flag if the orchestrator wants it
  wired now.
- **TTL = 10 min, refresh at 80%** are my picks within the "modest default" the user specified. Easy
  to tune via the two consts if the orchestrator prefers different values.
- **Zeroize-on-evict deferred** (would add the `zeroize` dependency). Recommend v1 without it; flag if
  the security posture requires it now.
```