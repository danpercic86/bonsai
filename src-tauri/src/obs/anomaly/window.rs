//! §5 windowed sink rules, factored out of `anomaly.rs`: `dup-ipc`,
//! `redundant-refresh`, `effect-thrash`, `event-storm`, `watcher-storm`,
//! `unbatched-sink`. Each is a count/gap-within-window rule sharing the
//! [`Sliding`] helper. `render-storm` stays inline in `observe` (single-record,
//! no window state).

use std::collections::HashMap;

use super::AnomalyDetector;
use crate::obs::record::{AnomalySeverity, LogRecord};

/// `dup-ipc` window (§5).
pub(super) const W_DUP_IPC_MS: i64 = 300;
pub(super) const W_REFRESH_MS: i64 = 1_000;
pub(super) const W_EFFECT_MS: i64 = 1_000;
pub(super) const W_EVENT_MS: i64 = 1_000;
pub(super) const W_WATCHER_MS: i64 = 1_000;
pub(super) const W_UNBATCHED_MS: i64 = 1_000;

pub(super) const EFFECT_THRASH_MIN: usize = 5;
pub(super) const EVENT_STORM_MIN: usize = 20;
pub(super) const WATCHER_STORM_MIN: usize = 5;
pub(super) const UNBATCHED_MIN: usize = 10;

/// One windowed observation: its wall-clock `ts`, originating `seq`, and the
/// grouping key the rule counts by.
#[derive(Clone)]
struct Ev {
    ts: i64,
    seq: u64,
    key: String,
}

/// A sliding count-within-window rule with per-key emit debouncing.
#[derive(Default)]
pub(super) struct Sliding {
    events: Vec<Ev>,
    /// Last emit time per grouping key. Pruned to the rule's window by
    /// [`Sliding::prune`] — see there for why that is what keeps §11's "bounded"
    /// claim true for `dup-ipc`, whose key space is NOT a finite catalogue.
    last_fire: HashMap<String, i64>,
}

impl Sliding {
    fn push(&mut self, ts: i64, seq: u64, key: String) {
        self.events.push(Ev { ts, seq, key });
    }

    fn prune(&mut self, now: i64, window: i64) {
        let cutoff = now - window;
        self.events.retain(|e| e.ts >= cutoff);
        // `last_fire` is pruned on the SAME cutoff, and that is a correctness-
        // preserving deletion, not a heuristic — **given one stated premise**:
        // that every later `arm` on this rule sees a `now` no smaller than this
        // one. Under that premise `arm` rejects a key only while
        // `now - prev < window`, so an entry at or before the cutoff can never
        // change an `arm` answer again. (Every rule pairs `prune` and `arm` with
        // the identical window constant, so the two cutoffs cannot disagree.)
        //
        // The premise is NOT enforced anywhere. Record `ts` is merged from two
        // unsynchronised clocks — `Date.now()` in the webview (`src/obs/log.ts`)
        // and `now_ms()` in Rust (`sink.rs`) — into one writer stream via batched
        // `log_append`, with no monotonic clamp on the pipeline. A `ts` that
        // regresses by more than the window can therefore drop a `last_fire`
        // entry that would have debounced the next fire.
        // **Blast radius: one duplicate anomaly record, never a missed one** —
        // `events` above already carries the same exposure, and a duplicate
        // `dup-ipc` warning is strictly less harmful than an unbounded map. We
        // deliberately do NOT clamp `ts` to fix this: clamping would rewrite
        // recorded timestamps (they are diagnostic data), and a monotone cutoff
        // here would retain MORE `events` on a regression, which can turn a
        // count rule's non-fire into a fire — trading a duplicate for a false
        // positive.
        //
        // Why it matters (increment-5 review): every other rule keys on a finite
        // catalogue (refresh scope, event name, a constant), but `dup-ipc` keys on
        // `cmd\0argsHash` — an UNBOUNDED key space (every distinct argument set
        // mints a new key). Without this line the map grew for the whole session
        // and §11's "bounded" claim on `AnomalyDetector` was simply untrue.
        // Bounded now by the keys that actually fired within one window.
        self.last_fire.retain(|_, t| *t >= cutoff);
    }

    /// Test-only view of the debounce map's size — the §11 boundedness assertion.
    #[cfg(test)]
    pub(super) fn last_fire_len(&self) -> usize {
        self.last_fire.len()
    }

    /// The `seq`s of same-key events currently inside the window (self included,
    /// already pushed by the caller).
    fn refs_for(&self, key: &str) -> Vec<u64> {
        self.events
            .iter()
            .filter(|e| e.key == key)
            .map(|e| e.seq)
            .collect()
    }

    /// True (and stamps `last_fire`) when `key` may fire now: the previous fire
    /// for this key is at least `window` ms old. Debounces a rule to at most once
    /// per window per key.
    fn arm(&mut self, key: &str, now: i64, window: i64) -> bool {
        match self.last_fire.get(key) {
            Some(&prev) if now - prev < window => false,
            _ => {
                self.last_fire.insert(key.to_string(), now);
                true
            }
        }
    }
}

impl AnomalyDetector {
    pub(super) fn detect_dup_ipc(
        &mut self,
        ts: i64,
        seq: u64,
        cmd: &str,
        args_hash: &str,
        out: &mut Vec<LogRecord>,
    ) {
        let key = format!("{cmd}\u{0}{args_hash}");
        self.ipc_calls.prune(ts, W_DUP_IPC_MS);
        // Find the most-recent prior call with the same cmd+argsHash inside the
        // window and with NO mutation command between it and now.
        let prior = self
            .ipc_calls
            .events
            .iter()
            .rev()
            .find(|e| e.key == key)
            .map(|e| (e.seq, e.ts));
        self.ipc_calls.push(ts, seq, key.clone());
        if let Some((prior_seq, prior_ts)) = prior {
            // P117 §2.5 — `dup-ipc` is UNCHANGED: its key is `cmd\0argsHash` and
            // `argsHash` already hashes the whole args object (which carries
            // `repoId` for every repo-scoped command), so it is repo-discriminating
            // already. It adopts the attributed `mutations` tuple and deliberately
            // IGNORES the attribution — any mutation still intervenes.
            let mutation_between = self
                .mutations
                .iter()
                .any(|(m, _)| *m > prior_ts && *m <= ts);
            if !mutation_between && self.ipc_calls.arm(&key, ts, W_DUP_IPC_MS) {
                out.push(self.anomaly(
                    "dup-ipc",
                    AnomalySeverity::Warn,
                    format!(
                        "{cmd}: duplicate call within {W_DUP_IPC_MS}ms, no intervening mutation"
                    ),
                    vec![prior_seq, seq],
                    Vec::new(),
                    ts,
                ));
            }
        }
    }

    /// §5 `redundant-refresh`, P117 §2.3 — keyed on `(repo, scope)`, not `scope`
    /// alone. With 5 repos open, 17 of 26 firings in the measured session were
    /// cross-repo pairs sharing a scope and nothing else.
    ///
    /// `repo: None` gets its own bucket (`""`), i.e. today's repo-blind behaviour
    /// for any unattributed producer (§2.5) — never merged into a repo's bucket.
    /// The composite key goes through ALL FOUR uses — the `find` lookup, `push`,
    /// `arm` and `refs_for`. Missing any one reintroduces the bug elsewhere:
    /// `arm` keyed on `scope` alone would rate-limit repo B out of a genuine
    /// finding because repo A had just fired.
    pub(super) fn detect_redundant_refresh(
        &mut self,
        ts: i64,
        seq: u64,
        repo: Option<&str>,
        scope: &str,
        out: &mut Vec<LogRecord>,
    ) {
        let key = format!("{}\u{0}{scope}", repo.unwrap_or(""));
        self.refreshes.prune(ts, W_REFRESH_MS);
        let prior_ts = self
            .refreshes
            .events
            .iter()
            .rev()
            .find(|e| e.key == key)
            .map(|e| e.ts);
        self.refreshes.push(ts, seq, key.clone());
        if let Some(prior_ts) = prior_ts {
            let mutation_between = self.mutations.iter().any(|(m, m_repo)| {
                *m > prior_ts && *m <= ts && super::mutation_attributed_to(m_repo.as_deref(), repo)
            });
            if !mutation_between && self.refreshes.arm(&key, ts, W_REFRESH_MS) {
                let refs = self.refreshes.refs_for(&key);
                // §2.2 point 8 — the detail names the SCOPE only. Interpolating
                // the repo value would leak a path in raw mode and break the
                // strict/raw byte-identity of the anomaly stream; `refs` already
                // points at the records that carry the dimension.
                out.push(self.anomaly(
                    "redundant-refresh",
                    AnomalySeverity::Warn,
                    format!(
                        "{scope}: repeated refresh for the same repo within \
                         {W_REFRESH_MS}ms, no intervening mutation"
                    ),
                    refs,
                    Vec::new(),
                    ts,
                ));
            }
        }
    }

    pub(super) fn detect_effect_thrash(
        &mut self,
        ts: i64,
        seq: u64,
        component: &str,
        effect: &str,
        out: &mut Vec<LogRecord>,
    ) {
        let key = format!("{component}\u{0}{effect}");
        self.effects.prune(ts, W_EFFECT_MS);
        self.effects.push(ts, seq, key.clone());
        let refs = self.effects.refs_for(&key);
        if refs.len() >= EFFECT_THRASH_MIN && self.effects.arm(&key, ts, W_EFFECT_MS) {
            out.push(self.anomaly(
                "effect-thrash",
                AnomalySeverity::Warn,
                format!(
                    "{component}.{effect}: ran {}× within {W_EFFECT_MS}ms",
                    refs.len()
                ),
                refs,
                Vec::new(),
                ts,
            ));
        }
    }

    pub(super) fn detect_event_storm(
        &mut self,
        ts: i64,
        seq: u64,
        name: &str,
        out: &mut Vec<LogRecord>,
    ) {
        self.events.prune(ts, W_EVENT_MS);
        self.events.push(ts, seq, name.to_string());
        let refs = self.events.refs_for(name);
        if refs.len() >= EVENT_STORM_MIN && self.events.arm(name, ts, W_EVENT_MS) {
            out.push(self.anomaly(
                "event-storm",
                AnomalySeverity::Warn,
                format!("{name}: {} deliveries within {W_EVENT_MS}ms", refs.len()),
                refs,
                Vec::new(),
                ts,
            ));
        }
    }

    pub(super) fn detect_watcher_storm(&mut self, ts: i64, seq: u64, out: &mut Vec<LogRecord>) {
        const KEY: &str = "watcher";
        self.watchers.prune(ts, W_WATCHER_MS);
        self.watchers.push(ts, seq, KEY.to_string());
        let refs = self.watchers.refs_for(KEY);
        if refs.len() >= WATCHER_STORM_MIN && self.watchers.arm(KEY, ts, W_WATCHER_MS) {
            out.push(self.anomaly(
                "watcher-storm",
                AnomalySeverity::Warn,
                format!("{} debounce firings within {W_WATCHER_MS}ms", refs.len()),
                refs,
                Vec::new(),
                ts,
            ));
        }
    }

    /// Fed a `BatchMark` per `log_append` call (§5 `unbatched-sink`): ≥10 in 1 s.
    pub fn on_batch_mark(&mut self, ts: i64, seq: u64) -> Vec<LogRecord> {
        const KEY: &str = "log_append";
        let mut out = Vec::new();
        self.unbatched.prune(ts, W_UNBATCHED_MS);
        self.unbatched.push(ts, seq, KEY.to_string());
        let refs = self.unbatched.refs_for(KEY);
        if refs.len() >= UNBATCHED_MIN && self.unbatched.arm(KEY, ts, W_UNBATCHED_MS) {
            out.push(self.anomaly(
                "unbatched-sink",
                AnomalySeverity::Info,
                format!("{} log_append calls within {W_UNBATCHED_MS}ms", refs.len()),
                refs,
                Vec::new(),
                ts,
            ));
        }
        self.bump_anomaly_count(out.len());
        out
    }
}
