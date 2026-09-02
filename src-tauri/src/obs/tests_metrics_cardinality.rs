//! REGRESSION (audit F3) — `usage.json` key cardinality is bounded.
//!
//! `log_append` hands `ipc.result` records from the WEBVIEW straight to
//! `observe_ipc_result`, so both halves of the defence are tested here:
//! membership in the real `IpcApi` command set (a well-shaped unknown name is
//! dropped) and the hard per-map cap with its overflow bucket (nothing can grow
//! a map without limit, including the 400-day → `lifetime` roll-up).

use crate::obs::histogram::Histogram;
use crate::obs::metrics::MetricsState;
use crate::obs::metrics_map::{bump, merge_histogram, observe, MAX_KEYS_PER_MAP, OVERFLOW_KEY};
use std::collections::BTreeMap;

const DAY: &str = "2026-08-27";

// ------------------------------------------------------ command membership

/// A name that PASSES the shape predicate but is not an `IpcApi` method must not
/// mint a key: shape alone bounds nothing, since a hostile or buggy frontend can
/// emit unlimited well-shaped names.
#[test]
fn well_shaped_but_unknown_command_names_are_dropped() {
    let store = MetricsState::default();
    for cmd in [
        "getStatus",   // real — kept
        "getGraph",    // real — kept
        "notACommand", // shape-valid, not in the command set
        "aaaaaaaa",
        "get_status", // the snake_case Tauri name is NOT what the proxy sends
        "x1",
    ] {
        store.observe_ipc_result(cmd, 4.0, None, DAY);
    }
    let snap = store.snapshot();
    let keys: Vec<String> = snap.days[0].totals.durations.keys().cloned().collect();
    assert_eq!(
        keys,
        vec!["cmd.getGraph".to_string(), "cmd.getStatus".to_string()],
        "only real IpcApi method names may become durable keys"
    );
}

/// The end-to-end shape the frontend actually produces still records — the
/// membership check must not silently kill the `cmd.*` family.
#[test]
fn real_command_names_still_record_durations() {
    let store = MetricsState::default();
    store.observe_ipc_result("openRepo", 12.0, None, DAY);
    store.observe_ipc_result("openRepo", 18.0, Some("git"), DAY);
    let snap = store.snapshot();
    let h = snap.days[0]
        .totals
        .durations
        .get("cmd.openRepo")
        .expect("cmd.openRepo histogram");
    assert_eq!(h.count, 2);
    assert_eq!(snap.days[0].totals.errors.get("git"), Some(&1));
}

// ------------------------------------------------------------ hard cap

#[test]
fn counter_map_stops_minting_keys_at_the_cap() {
    let mut map: BTreeMap<String, u64> = BTreeMap::new();
    for i in 0..(MAX_KEYS_PER_MAP + 50) {
        bump(&mut map, &format!("k.{i}"), 1);
    }
    assert_eq!(map.len(), MAX_KEYS_PER_MAP + 1, "cap + the overflow bucket");
    assert_eq!(
        map.get(OVERFLOW_KEY),
        Some(&50),
        "observations past the cap are counted, not lost"
    );
    // An ALREADY-KNOWN key keeps accumulating after the cap is reached.
    bump(&mut map, "k.0", 7);
    assert_eq!(map.get("k.0"), Some(&8));
    assert_eq!(map.len(), MAX_KEYS_PER_MAP + 1);
}

#[test]
fn duration_map_stops_minting_keys_at_the_cap() {
    let mut map: BTreeMap<String, Histogram> = BTreeMap::new();
    for i in 0..(MAX_KEYS_PER_MAP + 10) {
        observe(&mut map, &format!("cmd.k{i}"), 5);
    }
    assert_eq!(map.len(), MAX_KEYS_PER_MAP + 1);
    assert_eq!(
        map.get(OVERFLOW_KEY).map(|h| h.count),
        Some(10),
        "capped observations fold into the overflow histogram"
    );
}

/// The 400-day → `lifetime` roll-up accumulates every retained bucket's key set,
/// so it is the other unbounded-growth path and goes through the same cap.
#[test]
fn merging_histograms_respects_the_cap() {
    let mut map: BTreeMap<String, Histogram> = BTreeMap::new();
    let mut src = Histogram::default();
    src.observe(3);
    for i in 0..(MAX_KEYS_PER_MAP + 5) {
        merge_histogram(&mut map, &format!("op.k{i}"), &src);
    }
    assert_eq!(map.len(), MAX_KEYS_PER_MAP + 1);
    assert_eq!(map.get(OVERFLOW_KEY).map(|h| h.count), Some(5));
}

/// The overflow key must itself be a legal metric key, or the cap would create
/// exactly the kind of key the §8 predicates exist to keep out.
#[test]
fn the_overflow_key_is_a_domain_action_literal() {
    assert!(!OVERFLOW_KEY.is_empty());
    assert!(OVERFLOW_KEY
        .chars()
        .all(|c| c.is_ascii_lowercase() || c == '.'));
    assert!(OVERFLOW_KEY.contains('.'));
}
