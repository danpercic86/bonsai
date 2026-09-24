//! Spec-008 AC 7/8: the one-shot layout walk's operation counters are
//! deterministic and never regress past the checked-in baseline.
//!
//! Fixture: a small synthetic history (~410 commits: main + overlapping
//! feature branches with merges, long-lived unmerged branches, tags) built by
//! `bonsai_core::fixture::generate_fixture`, whose signatures and timestamps
//! are fixed — NOT the 31k cached gate fixture (too slow for the rust tier).
//!
//! Baseline: `tests/fixtures/layout-counts-baseline.json`. A counter above its
//! baseline fails. With `BONSAI_UPDATE_PERF_BASELINE=1` the file is rewritten
//! with any LOWERED values (it is never raised — raising needs a hand edit);
//! a missing file is created from the current run in that mode.

use std::path::PathBuf;

use bonsai_core::fixture::{generate_fixture, FixtureSpec};
use bonsai_core::graph::{compute_graph_counted, GraphFilter, LayoutCounters};
use serde_json::{Map, Value};

const UPDATE_ENV: &str = "BONSAI_UPDATE_PERF_BASELINE";
const COMMENT: &str = "Spec-008 layout operation counters for the small fixture in \
tests/graph_layout_counts.rs. Values may only go down: run with \
BONSAI_UPDATE_PERF_BASELINE=1 to lower them; raising one requires a hand edit and review.";

fn small_spec() -> FixtureSpec {
    FixtureSpec {
        main_len: 240,
        branch_every: 12,
        branch_len: 6,
        // > branch_every: several feature lanes are open at once.
        merge_after: 30,
        long_branches: 2,
        long_branch_len: 25,
        tag_every: 30,
        keep_branch_ref_every: 4,
    }
}

fn baseline_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("layout-counts-baseline.json")
}

fn counters_map(c: &LayoutCounters) -> Map<String, Value> {
    match serde_json::to_value(c).expect("serialize counters") {
        Value::Object(m) => m,
        other => panic!("counters serialized to a non-object: {other}"),
    }
}

/// Per-counter comparison. Returns the lowered-only merged baseline and a
/// message for every counter that exceeds (or is missing from) the baseline.
fn compare(
    baseline: &Map<String, Value>,
    new: &Map<String, Value>,
) -> (Map<String, Value>, Vec<String>) {
    let mut merged = baseline.clone();
    let mut failures = Vec::new();
    for (name, value) in new {
        let now = value.as_u64().expect("counter is u64");
        match baseline.get(name).and_then(Value::as_u64) {
            Some(base) if now > base => failures.push(format!(
                "layout counter `{name}` regressed: baseline {base}, new {now}"
            )),
            Some(base) if now < base => {
                merged.insert(name.clone(), Value::from(now));
            }
            Some(_) => {}
            None => failures.push(format!(
                "layout counter `{name}` has no baseline (new {now}); add it by hand"
            )),
        }
    }
    (merged, failures)
}

fn write_baseline(counters: Map<String, Value>) {
    let mut doc = Map::new();
    doc.insert("_comment".into(), Value::from(COMMENT));
    doc.insert("counters".into(), Value::Object(counters));
    let mut text = serde_json::to_string_pretty(&Value::Object(doc)).expect("serialize baseline");
    text.push('\n');
    std::fs::write(baseline_path(), text).expect("write baseline");
}

#[test]
fn layout_counters_are_deterministic_and_within_baseline() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    generate_fixture(dir.path(), &small_spec()).expect("generate fixture");
    let filter = GraphFilter::default();

    let (layout_a, counters_a) = compute_graph_counted(dir.path(), &filter).expect("layout run 1");
    let (layout_b, counters_b) = compute_graph_counted(dir.path(), &filter).expect("layout run 2");
    assert_eq!(counters_a, counters_b, "AC 7: counters differ between runs");
    assert_eq!(layout_a, layout_b, "layouts differ between runs");
    assert_eq!(layout_a.nodes.len(), small_spec().total_commits());
    assert_eq!(counters_a.lane_steps, layout_a.nodes.len() as u64);
    assert_eq!(counters_a.edges_emitted, layout_a.edges.len() as u64);

    let new = counters_map(&counters_a);
    let update = std::env::var(UPDATE_ENV).is_ok_and(|v| v == "1");
    let path = baseline_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        if update {
            write_baseline(new);
            return;
        }
        panic!(
            "layout counts baseline missing at {}; seed it with \
             `{UPDATE_ENV}=1 cargo nextest run -p bonsai-core --test graph_layout_counts` \
             (current counters: {counters_a:?})",
            path.display()
        );
    };
    let doc: Value = serde_json::from_str(&text).expect("baseline is valid JSON");
    let baseline = doc
        .get("counters")
        .and_then(Value::as_object)
        .expect("baseline has a `counters` object");

    let (merged, failures) = compare(baseline, &new);
    if update && &merged != baseline {
        write_baseline(merged);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn compare_lowers_only_and_reports_regressions() {
    let base: Map<String, Value> =
        serde_json::from_str(r#"{"a": 10, "b": 10, "c": 10}"#).expect("json");
    let new: Map<String, Value> =
        serde_json::from_str(r#"{"a": 7, "b": 10, "c": 12, "d": 1}"#).expect("json");
    let (merged, failures) = compare(&base, &new);
    assert_eq!(merged.get("a"), Some(&Value::from(7)));
    assert_eq!(merged.get("b"), Some(&Value::from(10)));
    assert_eq!(merged.get("c"), Some(&Value::from(10)), "never raised");
    assert!(
        !merged.contains_key("d"),
        "new counters are never auto-added"
    );
    assert_eq!(failures.len(), 2);
    assert!(failures[0].contains("`c`") && failures[0].contains("baseline 10, new 12"));
    assert!(failures[1].contains("`d`"));
}
