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

use super::record::{LogPayload, LogRecord};

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
