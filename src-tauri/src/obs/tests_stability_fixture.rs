//! P120 T-F10 cross-check — the SAME `stabilityAnomalyCases.json` the TS mock
//! test (`src/ipc/mock/obsAnomalyStability.test.ts`) consumes, fed through the
//! real [`AnomalyDetector`]. If either side's rules drift, one of the two tests
//! fails against the one shared `expect` array.
//!
//! Loading mirrors the TS `toRecords`: each wire record gets `mono = ts`,
//! `src = rust` for `watcher` (else `ui`) and `lvl = debug`, then goes through
//! the real `LogRecord` deserializer. `expect` is the full dump output, so the
//! detector's session-end drain is included.

use serde_json::{json, Map, Value};

use super::anomaly::AnomalyDetector;
use super::record::LogRecord;

const FIXTURE: &str = include_str!("../../../src/ipc/mock/fixtures/stabilityAnomalyCases.json");

/// The six fields the TS test projects each anomaly onto.
const PROJECTED: [&str; 6] = ["rule", "severity", "refs", "traces", "ts", "detail"];

fn to_record(wire: &Value) -> Result<LogRecord, String> {
    let mut obj: Map<String, Value> = wire
        .as_object()
        .cloned()
        .ok_or_else(|| format!("record is not an object: {wire}"))?;
    let ts = obj.get("ts").cloned().unwrap_or(Value::Null);
    let src = if obj.get("kind") == Some(&json!("watcher")) {
        "rust"
    } else {
        "ui"
    };
    obj.insert("mono".into(), ts);
    obj.insert("src".into(), json!(src));
    obj.insert("lvl".into(), json!("debug"));
    serde_json::from_value(Value::Object(obj)).map_err(|e| format!("{e}: {wire}"))
}

fn project(anomaly: &LogRecord) -> Result<Value, String> {
    let full = serde_json::to_value(anomaly).map_err(|e| e.to_string())?;
    if full.get("kind") != Some(&json!("anomaly")) {
        return Err(format!("detector emitted a non-anomaly: {full}"));
    }
    let mut out = Map::new();
    for key in PROJECTED {
        let v = full.get(key).cloned().unwrap_or(Value::Null);
        out.insert(key.into(), v);
    }
    Ok(Value::Object(out))
}

fn run_case(case: &Value) -> Result<(), String> {
    let name = case["name"].as_str().unwrap_or("<unnamed>");
    let records = case["records"]
        .as_array()
        .ok_or_else(|| format!("{name}: `records` is not an array"))?;
    let mut det = AnomalyDetector::new(None);
    let mut out: Vec<LogRecord> = Vec::new();
    for wire in records {
        let rec = to_record(wire).map_err(|e| format!("{name}: {e}"))?;
        let seq = rec.seq;
        out.extend(det.observe(&rec, seq));
    }
    out.extend(det.on_session_end());
    let got = out.iter().map(project).collect::<Result<Vec<_>, _>>()?;
    let want = case["expect"]
        .as_array()
        .ok_or_else(|| format!("{name}: `expect` is not an array"))?;
    if &got != want {
        return Err(format!(
            "{name}:\n  got  {}\n  want {}",
            Value::Array(got),
            Value::Array(want.clone())
        ));
    }
    Ok(())
}

#[test]
fn stability_shared_fixture_matches_the_real_detector() {
    let doc: Value = serde_json::from_str(FIXTURE).expect("fixture is valid JSON");
    let cases = doc["cases"]
        .as_array()
        .expect("fixture has a `cases` array");
    assert!(cases.len() >= 10, "fixture shrank to {} cases", cases.len());
    let failures: Vec<String> = cases.iter().filter_map(|c| run_case(c).err()).collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The comparison has teeth: a case whose `expect` is tampered with fails.
#[test]
fn stability_shared_fixture_rejects_a_tampered_expectation() {
    let doc: Value = serde_json::from_str(FIXTURE).expect("fixture is valid JSON");
    let mut case = doc["cases"][0].clone();
    case["expect"][0]["severity"] = json!("info");
    assert!(run_case(&case).is_err());
    case["expect"] = json!([]);
    assert!(run_case(&case).is_err());
}
