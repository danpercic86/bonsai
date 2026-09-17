//! The process-wide probe cache, and the rule that **one probe runs at a time**.
//!
//! Split out of `tools/mod.rs` (2026-09-15) for two reasons: `mod.rs` was at the
//! ~500-line limit, and the coalescing below is only testable if the cache is a
//! *value* a test can own rather than a bare `static` every test would share.
//! [`ScanCell`] is that value; the `static` is one instance of it.
//!
//! ## Why coalescing
//!
//! `refresh_tool_scan` (the picker's Rescan) bypasses the cache unconditionally,
//! and the probe it runs stats the filesystem and spawns `reg.exe` under
//! `detect::SCAN_REG_BUDGET`. Before this, N concurrent `listExternalTools(true)`
//! calls ran **N concurrent probes**, each pinning a `spawn_blocking` thread
//! (audit LOW-2, 2026-09-15). Not an escalation — absolute program, fixed argv,
//! nothing renderer-supplied reaches the child — but it is local resource
//! consumption a UI can trigger by repeating one click.
//!
//! The fix is an in-flight flag **inside the existing `RwLock`**: the first
//! caller leads, the rest wait for its result instead of duplicating it. No new
//! lock and no `Condvar` (which would need a `Mutex`, i.e. exactly that).

use std::sync::RwLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::detect;
use super::{Resolution, ToolEntry};

/// What a probe produces: the catalog entries that resolved, with their
/// resolutions.
pub(super) type Rows = Vec<(&'static ToolEntry, Resolution)>;

/// How long a follower waits for the leader's probe before giving up on it.
/// Twice the leader's own registry budget: that budget bounds the slow half of a
/// probe, and the rest is `stat` calls.
const PROBE_WAIT: Duration = detect::SCAN_REG_BUDGET.saturating_mul(2);

/// Follower poll interval. Coarse on purpose — the thread is a blocking one and
/// the thing it waits for takes hundreds of milliseconds.
const PROBE_POLL: Duration = Duration::from_millis(25);

struct CachedScan {
    at_ms: u64,
    found: Rows,
}

struct ScanState {
    cached: Option<CachedScan>,
    /// A probe is running right now, so a second caller must wait for it rather
    /// than start another. Normally cleared by the publish in
    /// [`ScanCell::probe`], in the same acquisition; [`Lease`]'s `Drop` clears it
    /// only on an **unwind**, so a probe that panics cannot wedge every later
    /// Rescan into waiting forever.
    probing: bool,
    /// Bumped by every publish, and ONLY by a publish. This is how a follower
    /// tells "the leader finished" from "the leader died on the way out": both
    /// leave `probing == false`, but only the first advances this. Wrapping is
    /// harmless — a follower compares one snapshot for *inequality*, so fooling
    /// it would take 2^64 publishes inside one [`PROBE_WAIT`].
    generation: u64,
}

/// Clone the published rows out of a guard the caller already holds, so a check
/// and the value that check justifies can happen under ONE acquisition.
fn snapshot(state: &ScanState) -> Option<(u64, Rows)> {
    state.cached.as_ref().map(|c| (c.at_ms, c.found.clone()))
}

/// One probe cache. The process has exactly one ([`SCAN`]); tests make their own
/// so nothing they do is visible to `tool_scan` / `picked`.
pub(super) struct ScanCell {
    state: RwLock<ScanState>,
}

/// Held by the thread that is probing. Clearing the flag on an **unwind** is its
/// whole job; the happy path clears it inside the publish instead.
struct Lease<'a> {
    cell: &'a ScanCell,
    /// `true` until the publish clears `probing` under its own acquisition —
    /// still `true` on a panic, which is the only case `Drop` has work to do.
    ///
    /// **Not "idempotent, so just clear twice".** A second clear after the
    /// publish's acquisition would be a TOCTOU against [`ScanCell::claim`]: a
    /// caller that claims in the gap between the publish and this `Drop` sets
    /// `probing = true`, and the redundant clear would erase THAT leader's flag,
    /// letting a third caller lead concurrently — reopening the very
    /// double-probe (audit LOW-2) this module exists to close. No deterministic
    /// test pins that window without scheduling hooks; it is closed by
    /// construction, and by reading these two lines.
    armed: bool,
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.cell.write().probing = false;
        }
    }
}

impl ScanCell {
    pub(super) const fn new() -> Self {
        Self {
            state: RwLock::new(ScanState {
                cached: None,
                probing: false,
                generation: 0,
            }),
        }
    }

    /// Poison-recovering, for the `gitbin::GIT_BIN` reason: a panicked probe must
    /// not make "install the editor, press Rescan" require an app restart.
    fn write(&self) -> std::sync::RwLockWriteGuard<'_, ScanState> {
        self.state.write().unwrap_or_else(|p| p.into_inner())
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, ScanState> {
        self.state.read().unwrap_or_else(|p| p.into_inner())
    }

    /// The cached rows, if a probe has ever published any.
    pub(super) fn cached(&self) -> Option<(u64, Rows)> {
        snapshot(&self.read())
    }

    /// Run `probe` — **or**, if one is already running, wait for its result.
    ///
    /// A follower normally gets the leader's freshly published rows and the
    /// leader's `at_ms`, so `scanned_at_ms` still advances for every caller that
    /// asked for a refresh. (No frontend consumer reads that value yet; it is
    /// carried so one will be able to tell that a Rescan landed.)
    ///
    /// **Two paths on which a follower does NOT get the leader's rows, stated
    /// rather than buried. They differ, and that difference is the whole reason
    /// [`ScanState::generation`] exists:**
    ///
    /// 1. **The leader overruns [`PROBE_WAIT`].** With something cached, the
    ///    follower returns those STALE rows with the OLD `at_ms` — so that
    ///    caller's Rescan looks like it did not land. Before coalescing, every
    ///    caller always got a fresh probe. The trade is deliberate: a bounded
    ///    wait plus an occasional unchanged `scanned_at_ms` beats N concurrent
    ///    `reg.exe` sweeps, and the leader is still *running*, so its own result
    ///    lands in the cache moments later and the next read sees it.
    /// 2. **The leader panics before publishing.** [`Lease`]'s `Drop` clears
    ///    `probing` on the unwind path — that is its whole purpose — so a
    ///    follower sees "done" with nothing published. Here the stale answer
    ///    would be wrong twice over, because nothing is coming to replace those
    ///    rows: the *next* read would return them too. So this follower probes
    ///    itself instead. A publish bumps `generation` and a panic does not,
    ///    which is exactly how it tells this case from case 1's "done".
    ///
    /// With nothing cached the follower probes on both paths — an empty picker
    /// would be worse than the duplicated work.
    pub(super) fn probe(&self, probe: impl FnOnce() -> Rows) -> (u64, Rows) {
        let mut lease = match self.claim() {
            Ok(lease) => Some(lease),
            Err(generation) => {
                if let Some(fresh) = self.await_leader(generation) {
                    return fresh;
                }
                // The leader overran [`PROBE_WAIT`] with nothing cached, or it
                // died before publishing. Probe anyway — an empty or a stale
                // picker is worse than the duplicated work — but WITHOUT a
                // lease, and this stays the only path on which two probes can
                // overlap.
                None
            }
        };
        let found = probe();
        let at_ms = now_ms();
        {
            let mut state = self.write();
            state.cached = Some(CachedScan {
                at_ms,
                found: found.clone(),
            });
            state.generation = state.generation.wrapping_add(1);
            // ONE acquisition for "these rows are published" and "no probe is in
            // flight", so a follower can never observe the second without the
            // first. Two things are load-bearing here:
            //   * the `Some` test — the LEASE-LESS publisher above must not
            //     clear a flag it does not own; the real leader may still run;
            //   * disarming — with `probing` cleared here, a `Drop` that cleared
            //     it again could erase a claim that landed in between (see
            //     [`Lease::armed`]).
            if let Some(held) = lease.as_mut() {
                state.probing = false;
                held.armed = false;
            }
        }
        // A no-op now by construction, and kept explicit because the ORDER is
        // the invariant: nothing may clear `probing` before those rows are
        // published.
        drop(lease);
        (at_ms, found)
    }

    /// Become the leader, or `Err(generation)` if someone already is: the
    /// generation the follower must watch advance before it will trust the
    /// cache. Snapshotted HERE, under the same write acquisition that observes
    /// `probing`, because a snapshot taken any later could miss a publish that
    /// happened in between and send that follower off to re-probe for nothing.
    /// Check-and-set in ONE acquisition, so two callers cannot both lead.
    fn claim(&self) -> Result<Lease<'_>, u64> {
        let mut state = self.write();
        if state.probing {
            return Err(state.generation);
        }
        state.probing = true;
        drop(state);
        Ok(Lease {
            cell: self,
            armed: true,
        })
    }

    /// Wait for the in-flight probe to publish, from the `generation`
    /// [`claim`](Self::claim) observed. `None` ⇒ the leader died without
    /// publishing, which is the caller's cue to probe itself rather than serve
    /// rows that nothing is coming to replace.
    fn await_leader(&self, generation: u64) -> Option<(u64, Rows)> {
        let deadline = Instant::now() + PROBE_WAIT;
        loop {
            {
                let state = self.read();
                // Checked BEFORE the first sleep: the leader may have published
                // microseconds ago, and no follower should pay a [`PROBE_POLL`]
                // floor to find that out.
                if state.generation != generation {
                    // Only a publish bumps it, and a publish always sets
                    // `cached` — so this is the leader's own scan. (`None` is
                    // unreachable rather than unwrapped: were it ever to happen,
                    // "probe yourself" is the safe answer, and that is exactly
                    // what `None` means to the caller.)
                    return snapshot(&state);
                }
                if !state.probing {
                    // Done, yet nothing published: [`Lease`]'s `Drop` cleared
                    // the flag on an unwind. Case 2 above — serving `cached`
                    // here would hand this caller the PREVIOUS probe's rows and
                    // `at_ms`, with no leader left to correct them.
                    return None;
                }
                if Instant::now() >= deadline {
                    // Case 1 above: the leader is alive and slow, so its result
                    // WILL land; this caller takes stale rows over a second
                    // concurrent probe.
                    return snapshot(&state);
                }
            }
            std::thread::sleep(PROBE_POLL);
        }
    }
}

/// Probe results for the host, cached for the process lifetime.
///
/// `RwLock<…>` rather than `OnceLock`, and poison-recovering, for the
/// `gitbin::GIT_BIN` reason: "install the editor, press Rescan" must work
/// without restarting the app. Only *probe* results are cached — the custom row
/// is one stat and the label maps are static data, so both are derived per call.
static SCAN: ScanCell = ScanCell::new();

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Poison-recovering read of the cache; a miss probes the host once.
///
/// Two threads that both miss no longer run two full probes: the second waits
/// for the first (see [`ScanCell::probe`]). A probe only READS (filesystem +
/// `reg.exe`), so serving one probe's rows to both callers is exactly as valid
/// as the two-probe behaviour this replaces — and holding the write lock across
/// the probe, the other way to serialise it, would park every cache HIT behind
/// `detect::SCAN_REG_BUDGET` too.
pub(super) fn cached_rows() -> (u64, Rows) {
    match SCAN.cached() {
        Some(hit) => hit,
        None => probe_host(),
    }
}

/// Probe the host and replace the cache. The ONLY place production code scans.
pub(super) fn probe_host() -> (u64, Rows) {
    SCAN.probe(|| {
        detect::scan_for(
            &detect::HostToolEnv::new(),
            crate::external::TargetOs::host(),
        )
    })
}

#[cfg(test)]
#[path = "scan_cache_tests.rs"]
mod tests;
