//! §3 wire-shape serde for [`LogRecord`], specifically the THREE-state
//! `changedProps` contract on `render.tally` (§9.2).
//!
//! Absent ⇒ the React call site tracks no props at all; `[]` ⇒ tracked and
//! nothing changed this window; names ⇒ tracked and these changed. The Rust
//! mirror has to keep those three apart in BOTH directions:
//!
//!   * `log_append(records: Vec<LogRecord>)` deserializes the whole batch
//!     BEFORE the command body runs, so a required `changed_props` would reject
//!     an entire batch over one untracked sidebar row — and every aggregate
//!     producer in v1 is a sidebar row;
//!   * `#[serde(default)]` ALONE would be a false fix: absent-in would become
//!     `changedProps: []` out, re-creating on disk exactly the ambiguity the
//!     optional field removes. Hence the `skip_serializing_if` half, pinned by
//!     `an_absent_changed_props_stays_absent_on_re_serialisation` below.

use serde_json::json;

use super::record::{LogPayload, LogRecord, StabilityRegion};

/// A `render.tally` wire object, with `changedProps` set only when `changed` is
/// `Some` — so the "key absent" case is genuinely absent, not `null`.
fn tally_wire(changed: Option<serde_json::Value>) -> serde_json::Value {
    let mut v = json!({
        "seq": 7,
        "ts": 1_700_000_000_000_i64,
        "mono": 120,
        "src": "ui",
        "lvl": "debug",
        "kind": "render.tally",
        "component": "BranchRow",
        "windowMs": 500.0,
        "renders": 12,
        "instances": 6,
        "traces": [],
    });
    if let Some(c) = changed {
        v["changedProps"] = c;
    }
    v
}

/// The tally's `changed_props`, or a panic naming the variant we got instead —
/// the payload kind is fixed by the literal above, so a mismatch is a test bug.
fn changed_props(rec: &LogRecord) -> Option<Vec<String>> {
    match &rec.payload {
        LogPayload::RenderTally { changed_props, .. } => changed_props.clone(),
        other => panic!("expected a render.tally payload, got {}", other.kind()),
    }
}

#[test]
fn a_tally_without_changed_props_deserialises() {
    let rec: LogRecord = serde_json::from_value(tally_wire(None))
        .expect("a tally with no changedProps key must not fail the batch");
    assert_eq!(changed_props(&rec), None, "absent must mean NOT TRACKED");
}

#[test]
fn an_empty_changed_props_round_trips_as_empty() {
    let rec: LogRecord = serde_json::from_value(tally_wire(Some(json!([])))).expect("deserialize");
    assert_eq!(
        changed_props(&rec),
        Some(vec![]),
        "[] must mean TRACKED, nothing changed"
    );
    let out = serde_json::to_value(&rec).expect("serialize");
    assert_eq!(
        out.get("changedProps"),
        Some(&json!([])),
        "tracked-but-unchanged must stay visible on the wire: {out}"
    );
}

#[test]
fn an_absent_changed_props_stays_absent_on_re_serialisation() {
    let rec: LogRecord = serde_json::from_value(tally_wire(None)).expect("deserialize");
    let out = serde_json::to_value(&rec).expect("serialize");
    assert!(
        out.get("changedProps").is_none(),
        "absent in must stay absent out — `serde(default)` alone would emit []: {out}"
    );
}

#[test]
fn named_changed_props_round_trip() {
    let rec: LogRecord =
        serde_json::from_value(tally_wire(Some(json!(["refs", "head"])))).expect("deserialize");
    assert_eq!(
        changed_props(&rec),
        Some(vec!["refs".to_string(), "head".to_string()])
    );
}

// ---- P120 T-R11 — the two stability payloads ------------------------------

const REGIONS: &[&str] = &[
    "sidebar", "graph", "details", "diff", "dock", "toolbar", "other",
];

fn shift_wire(region: &str) -> serde_json::Value {
    json!({
        "seq": 3, "ts": 1_700_000_000_500_i64, "mono": 40, "src": "ui", "lvl": "info",
        "kind": "layout.shift", "region": region, "phase": "interactive",
        "hadRecentInput": false, "resizing": false, "shifts": 3, "value": 0.0412,
        "firstAt": 1_700_000_000_000_i64, "spanMs": 412,
    })
}

fn focus_wire(region: &str) -> serde_json::Value {
    json!({
        "seq": 4, "ts": 1_700_000_000_550_i64, "mono": 41, "src": "ui", "lvl": "info",
        "kind": "focus.lost", "fromRegion": region, "phase": "interactive",
        "via": "detached", "lostAt": 1_700_000_000_500_i64,
    })
}

/// §2.2 wire JSON round-trips exactly, for every valid region (which also pins
/// the `From<String>` match to the same strings as `as_str()`).
#[test]
fn stability_payloads_round_trip_the_wire_shape_for_every_region() {
    for region in REGIONS {
        for wire in [shift_wire(region), focus_wire(region)] {
            let rec: LogRecord = serde_json::from_value(wire.clone()).expect("deserialize");
            let out = serde_json::to_value(&rec).expect("serialize");
            assert_eq!(
                out, wire,
                "round trip must be lossless for region {region:?}"
            );
        }
    }
}

/// A forged region (a path, an unknown word) becomes `other` — and does not
/// reject a batch that also holds valid records (A14 / AC 13).
#[test]
fn an_unknown_region_deserialises_to_other_without_rejecting_the_batch() {
    let batch = json!([shift_wire(r"C:\x"), focus_wire("foo"), shift_wire("graph")]);
    let recs: Vec<LogRecord> = serde_json::from_value(batch).expect("batch must not be rejected");
    let regions: Vec<StabilityRegion> = recs
        .iter()
        .map(|r| match &r.payload {
            LogPayload::LayoutShift(p) => p.region,
            LogPayload::FocusLost(p) => p.from_region,
            other => panic!("unexpected kind {}", other.kind()),
        })
        .collect();
    assert_eq!(
        regions,
        vec![
            StabilityRegion::Other,
            StabilityRegion::Other,
            StabilityRegion::Graph
        ]
    );
    let out = serde_json::to_string(&recs[0]).expect("serialize");
    assert!(
        !out.contains("C:"),
        "a forged region must never reach disk: {out}"
    );
}

/// A `watcher` payload with the pre-P113d optionals set and the five P113d
/// fields all `new` (`None` or `Some(n)`).
fn watcher_payload(new: Option<u32>) -> LogPayload {
    LogPayload::Watcher {
        paths: 5,
        relevant: 3,
        debounce_ms: 300,
        fired: true,
        suppressed: true,
        suppress_reason: Some("ignored".into()),
        burst_class: Some("worktree".into()),
        worktree_paths: Some(3),
        ignore_checked: Some(3),
        ignored: Some(3),
        batches: new,
        burst_ms: new,
        errors: new,
        skipped_batches: new,
        skipped_paths: new,
    }
}

fn key_set(p: &LogPayload) -> std::collections::BTreeSet<String> {
    let v = serde_json::to_value(p).expect("serialize");
    v.as_object().expect("object").keys().cloned().collect()
}

/// P113d §3: the five new fields are additive — absent, the key set is the
/// pre-P113d one byte-for-byte; present, they serialize camelCase.
#[test]
fn watcher_record_key_set_unchanged_when_new_fields_absent() {
    let legacy: std::collections::BTreeSet<String> = [
        "kind",
        "paths",
        "relevant",
        "debounceMs",
        "fired",
        "suppressed",
        "suppressReason",
        "burstClass",
        "worktreePaths",
        "ignoreChecked",
        "ignored",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(key_set(&watcher_payload(None)), legacy);

    let with_new = key_set(&watcher_payload(Some(2)));
    let added: Vec<&str> = with_new
        .iter()
        .filter(|k| !legacy.contains(*k))
        .map(String::as_str)
        .collect();
    assert_eq!(
        added,
        vec![
            "batches",
            "burstMs",
            "errors",
            "skippedBatches",
            "skippedPaths"
        ]
    );
}
