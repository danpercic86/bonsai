//! P91 §8 — the CARDINALITY cap on `usage.json`'s key maps.
//!
//! The key predicates in `metrics_keys.rs` answer "may this string be a key?".
//! This module answers the second, independent question: "how MANY keys may
//! there be?". Both are needed. `usage.json` is durable, uncovered by
//! `logs_delete_all` and unredacted, and its maps grow from `log_append`, whose
//! records come straight from the webview — so a shape-valid but unbounded key
//! stream (audit F3) would still grow the file without limit across 400 day
//! buckets.
//!
//! Past [`MAX_KEYS_PER_MAP`] a map stops minting keys and folds every further
//! observation into the single [`OVERFLOW_KEY`] bucket, so the count is never
//! LOST — an operator reading the file can tell that a cap was hit — while the
//! key set stays bounded at `MAX_KEYS_PER_MAP + 1` per map, per day bucket.

use std::collections::BTreeMap;

use crate::obs::histogram::Histogram;

/// Maximum distinct keys per map (`counters` / `durations` / `errors`) in one
/// day bucket or in `lifetime`.
///
/// Sized well above the real key set — ~200 `cmd.*` names, 3 `op.*` + their
/// allow-listed phases, `queue.blocking`, a handful of `perf.*` counters and the
/// `AppError` codes — so a healthy build never reaches it, while a runaway
/// producer is stopped long before the file matters.
pub const MAX_KEYS_PER_MAP: usize = 512;

/// Where observations go once a map is full. `<domain>.<action>` shaped, so it
/// passes every predicate in `metrics_keys.rs` and cannot be confused with a
/// real key.
pub const OVERFLOW_KEY: &str = "meta.overflow";

/// The key `map` should actually record under: `key` itself while there is room
/// (or when it already exists), otherwise [`OVERFLOW_KEY`].
fn capped<'a, V>(map: &BTreeMap<String, V>, key: &'a str) -> &'a str {
    if map.contains_key(key) || map.len() < MAX_KEYS_PER_MAP {
        key
    } else {
        OVERFLOW_KEY
    }
}

/// Adds `n` to `key`'s counter, honouring the cap.
pub fn bump(map: &mut BTreeMap<String, u64>, key: &str, n: u64) {
    let key = capped(map, key);
    let slot = map.entry(key.to_string()).or_insert(0);
    *slot = slot.saturating_add(n);
}

/// Records one duration observation under `key`, honouring the cap.
pub fn observe(map: &mut BTreeMap<String, Histogram>, key: &str, ms: u64) {
    let key = capped(map, key);
    map.entry(key.to_string()).or_default().observe(ms);
}

/// Merges one histogram into `key`, honouring the cap. Used by the 400-day →
/// `lifetime` roll-up, which is the other way a map can grow without bound.
pub fn merge_histogram(map: &mut BTreeMap<String, Histogram>, key: &str, other: &Histogram) {
    let key = capped(map, key);
    map.entry(key.to_string()).or_default().merge(other);
}

#[cfg(test)]
#[path = "tests_metrics_cardinality.rs"]
mod tests_metrics_cardinality;
