//! P91 Amendment A26 — raw-mode `args` enforcement tests
//! (`docs/contracts/P91-raw-args-privacy.md` §H, AC6–AC9).
//!
//! The centrepiece is [`raw_mode_drops_positionally_keyed_args_and_forge_tokens`]
//! (AC6): it drives the writer with EXACTLY what the buggy producer emitted, so
//! it fails on the pre-fix code and passes after. Everything else here is a unit
//! test of the invariant, driven through records a hostile or buggy producer
//! could send rather than through records the emit sites are known to produce.

use std::path::Path;

use serde_json::{json, Value};

use super::raw_args::{enforce, RAW_ARG_MAX_STR};
use super::record::{ArgShape, LogLevel, LogPayload, LogRecord, LogSource, RedactionMode};
use super::writer::{Limits, LogWriter, WriterConfig};

fn shape(pairs: &[(&str, &str)]) -> ArgShape {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn ipc_call(cmd: &str, args: Value, arity: &[(&str, &str)]) -> LogRecord {
    LogRecord {
        seq: 0,
        ts: 1_772_200_991_000,
        mono: 12,
        src: LogSource::Ui,
        lvl: LogLevel::Debug,
        trace: Some("t1".into()),
        span: Some("s1".into()),
        caused_by: None,
        payload: LogPayload::IpcCall {
            cmd: cmd.into(),
            args_hash: "abc123".into(),
            args_shape: Some(shape(arity)),
            args: Some(args),
            args_omitted: None,
        },
    }
}

/// The wire form of one `ipc.call` as the WEBVIEW sends it to `log_append`,
/// carrying whatever `argsOmitted` the producer proposed. Deserialising this
/// into a [`LogRecord`] is the production entry point — a field the payload
/// struct does not declare is dropped here, before the writer ever sees it.
fn ipc_call_wire(args_omitted: Value) -> Value {
    json!({
        "seq": 0, "ts": 1_772_200_991_000_u64, "mono": 12, "src": "ui", "lvl": "debug",
        "kind": "ipc.call", "cmd": "commit", "argsHash": "abc123",
        "args": { "repoId": "r1" },
        "argsOmitted": args_omitted,
    })
}

fn cfg(dir: &Path, redaction: RedactionMode) -> WriterConfig {
    WriterConfig {
        dir: dir.to_path_buf(),
        session_id: "sdeadbeef".into(),
        started_secs: 1_772_200_991,
        app_version: "1.5.0".into(),
        os: "windows".into(),
        level: LogLevel::Debug,
        redaction,
        limits: Limits::default(),
    }
}

fn redactor() -> std::sync::Arc<super::redact::Redactor> {
    std::sync::Arc::new(super::redact::Redactor::with_salt([5; 16]))
}

/// Writes `records` to a fresh session and returns (raw bytes, parsed lines).
fn round_trip(redaction: RedactionMode, records: Vec<LogRecord>) -> (String, Vec<Value>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut w = LogWriter::open(cfg(dir.path(), redaction), redactor()).expect("open");
    let name = w.active_file().to_string();
    for r in records {
        w.write_record(r).expect("write");
    }
    w.flush().expect("flush");
    drop(w);
    let text = std::fs::read_to_string(dir.path().join(&name)).expect("read log file");
    let rows = text
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).expect("each line is valid JSON"))
        .collect();
    (text, rows)
}

const NEG_MESSAGE: &str = "NEGTEST_COMMIT_MESSAGE_ZQX";
/// 40 hex chars: it PASSES the all-hex exemption in `looks_like_opaque_secret`
/// (which exists to protect commit SHAs), so only the key rule can catch it.
const NEG_TOKEN: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678";

/// **AC6 — the negative test.** Both records are exactly what `ipcProxy.ts`
/// emitted before A26: positional keys, and a cleartext PAT. Neither substring
/// may reach the file, in the mode the user opted into.
#[test]
fn raw_mode_drops_positionally_keyed_args_and_forge_tokens() {
    let (text, rows) = round_trip(
        RedactionMode::Raw,
        vec![
            ipc_call(
                "commit",
                json!({ "0": "r1", "1": NEG_MESSAGE, "2": false }),
                &[("0", "str"), ("1", "str:26"), ("2", "bool")],
            ),
            ipc_call(
                "forgeSetToken",
                json!({ "repoId": "r1", "token": NEG_TOKEN }),
                &[("0", "str"), ("1", "str:40")],
            ),
            // The form the buggy producer ACTUALLY emitted for the same call.
            // Nothing but the key rule can catch this one: `is_sensitive_key`
            // sees `"1"`, and `looks_like_opaque_secret` exempts all-hex words.
            ipc_call(
                "forgeSetToken",
                json!({ "0": "r1", "1": NEG_TOKEN }),
                &[("0", "str"), ("1", "str:40")],
            ),
        ],
    );

    assert!(
        !text.contains(NEG_MESSAGE),
        "commit message reached disk: {text}"
    );
    assert!(!text.contains(NEG_TOKEN), "forge PAT reached disk: {text}");

    let calls: Vec<&Value> = rows.iter().filter(|r| r["kind"] == "ipc.call").collect();
    assert_eq!(calls.len(), 3, "{rows:?}");
    for c in calls {
        assert_eq!(c["argsPolicyViolation"], json!(true), "{c}");
        assert!(c.get("args").is_none(), "{c}");
        // The record stays USEFUL: the reviewer still gets cmd + hash + shape.
        assert!(c["cmd"].is_string(), "{c}");
        assert_eq!(c["argsHash"], json!("abc123"), "{c}");
        assert!(c["argsShape"].is_object(), "{c}");
    }
}

/// AC6 companion: a CONFORMING raw record keeps its allow-listed scalars and is
/// NOT flagged.
#[test]
fn raw_mode_keeps_a_conforming_name_keyed_args_object() {
    let (_, rows) = round_trip(
        RedactionMode::Raw,
        vec![ipc_call(
            "commit",
            json!({ "repoId": "r1", "sign": false, "skipHooks": true }),
            &[("0", "str"), ("1", "str:9"), ("2", "bool"), ("3", "bool")],
        )],
    );
    let call = rows.iter().find(|r| r["kind"] == "ipc.call").expect("call");
    assert_eq!(
        call["args"],
        json!({ "repoId": "r1", "sign": false, "skipHooks": true })
    );
    assert!(call.get("argsPolicyViolation").is_none(), "{call}");
}

/// AC8 — strict mode still removes `args` outright (`strict.rs` unregressed), and
/// because it removes it BEFORE `raw_args`, a strict file carries no violation
/// flag either.
#[test]
fn strict_mode_still_removes_args_entirely() {
    let (text, rows) = round_trip(
        RedactionMode::Strict,
        vec![ipc_call(
            "commit",
            json!({ "0": "r1", "1": NEG_MESSAGE }),
            &[("0", "str"), ("1", "str:26")],
        )],
    );
    assert!(!text.contains(NEG_MESSAGE), "{text}");
    let call = rows.iter().find(|r| r["kind"] == "ipc.call").expect("call");
    assert!(call.get("args").is_none(), "{call}");
    assert!(call.get("argsPolicyViolation").is_none(), "{call}");
    assert!(call["argsShape"].is_object(), "{call}");
}

// ---------------------------------------------------------------------------
// AC7 — `enforce` unit tests: each rule independently.
// ---------------------------------------------------------------------------

fn call_value(args: Value) -> Value {
    json!({ "kind": "ipc.call", "cmd": "commit", "argsHash": "h", "args": args })
}

fn assert_dropped(mut v: Value) {
    assert!(enforce(&mut v), "expected a drop: {v}");
    assert!(v.get("args").is_none(), "{v}");
    assert_eq!(v["argsPolicyViolation"], json!(true), "{v}");
}

#[test]
fn w1_args_are_only_allowed_on_ipc_call() {
    let mut v = json!({ "kind": "ipc.result", "cmd": "commit", "args": { "repoId": "r1" } });
    assert!(enforce(&mut v));
    assert!(v.get("args").is_none(), "{v}");
}

#[test]
fn w2_args_must_be_an_object() {
    assert_dropped(call_value(json!(["r1", "hello"])));
    assert_dropped(call_value(json!("r1")));
}

#[test]
fn w3_numeric_keys_are_rejected() {
    // The single rule that kills the original leak at the writer.
    assert_dropped(call_value(json!({ "0": "r1", "1": "msg" })));
    assert_dropped(call_value(json!({ "repoId": "r1", "2": false })));
    assert_dropped(call_value(json!({ "Repo": "r1" })));
    assert_dropped(call_value(json!({ "repo_id": "r1" })));
    assert_dropped(call_value(json!({ "__proto__": "r1" })));
}

#[test]
fn w4_denied_param_names_are_rejected() {
    for key in [
        "message",
        "msg",
        "query",
        "searchText",
        "body",
        "prompt",
        "description",
        "noteBody",
        "content",
        "commentTitle",
        "subject",
        "summary",
        "patch",
        "diff",
        "blurb",
        "input",
        "reason",
        "token",
        "accessToken",
        "secret",
        "password",
        "passphrase",
        "authHeader",
        "credential",
        "apikey",
        "privatekey",
        "sshkey",
        "pat",
    ] {
        assert_dropped(call_value(json!({ "repoId": "r1", key: "x" })));
    }
}

#[test]
fn w4_identifier_param_names_are_kept() {
    let mut v = call_value(json!({
        "repoId": "r1",
        "path": "src/App.tsx",
        "origPath": "src/Old.tsx",
        "oid": "a1b2c3",
        "refName": "refs/heads/main",
        "url": "https://example.test/r.git",
        "topK": 5,
        "force": true,
        "atOid": Value::Null,
    }));
    assert!(!enforce(&mut v), "{v}");
    assert!(v["args"].is_object(), "{v}");
    assert!(v.get("argsPolicyViolation").is_none(), "{v}");
}

#[test]
fn w5_non_scalar_and_oversized_values_are_rejected() {
    assert_dropped(call_value(json!({ "repoId": { "token": "ghp_x" } })));
    assert_dropped(call_value(json!({ "repoId": ["r1"] })));
    assert_dropped(call_value(
        json!({ "repoId": "x".repeat(RAW_ARG_MAX_STR + 1) }),
    ));
    assert_dropped(call_value(json!({ "repoId": "line1\nline2" })));
    assert_dropped(call_value(json!({ "repoId": "line1\rline2" })));
    // Exactly at the cap is fine.
    let mut ok = call_value(json!({ "repoId": "x".repeat(RAW_ARG_MAX_STR) }));
    assert!(!enforce(&mut ok), "{ok}");
}

/// W6 on the PRODUCTION path — wire JSON → `LogRecord` (what `log_append` does)
/// → `LogWriter::write_record` → file, like W1–W5.
///
/// This is the test that would have caught `argsOmitted` being absent from
/// `LogPayload::IpcCall`: with the field missing, serde drops it as an unknown
/// field at deserialisation and it never reaches disk, exactly as the forged
/// `argsPolicyViolation` does in
/// [`a_producer_supplied_violation_flag_does_not_survive`].
#[test]
fn w6_args_omitted_reaches_disk_on_a_real_record() {
    let rec: LogRecord = serde_json::from_value(ipc_call_wire(json!(2))).expect("deserialize");
    let (text, rows) = round_trip(RedactionMode::Raw, vec![rec]);
    let call = rows.iter().find(|r| r["kind"] == "ipc.call").expect("call");
    assert_eq!(call["argsOmitted"], json!(2), "{text}");
    // The conforming `args` still rides along and is not flagged.
    assert_eq!(call["args"], json!({ "repoId": "r1" }), "{call}");
    assert!(call.get("argsPolicyViolation").is_none(), "{call}");

    // A fully-included call stays byte-identical to before the amendment: the
    // producer omits the field and `skip_serializing_if` keeps it off disk.
    let mut absent = ipc_call_wire(json!(0));
    absent
        .as_object_mut()
        .expect("wire object")
        .remove("argsOmitted");
    let rec: LogRecord = serde_json::from_value(absent).expect("deserialize");
    let (_, rows) = round_trip(RedactionMode::Raw, vec![rec]);
    let call = rows.iter().find(|r| r["kind"] == "ipc.call").expect("call");
    assert!(call.get("argsOmitted").is_none(), "{call}");
}

/// W6, the rejection half — also on the production path. A producer value that
/// is not a non-negative integer never becomes a record: `log_append`'s
/// deserialisation fails the whole batch (`null` is the one benign case, which
/// serde folds to "absent").
#[test]
fn w6_a_malformed_args_omitted_never_becomes_a_record() {
    for bad in [json!(-1), json!("2"), json!(1.5), json!(u64::from(u32::MAX) + 1)] {
        let wire = ipc_call_wire(bad.clone());
        assert!(
            serde_json::from_value::<LogRecord>(wire).is_err(),
            "argsOmitted {bad} was accepted as a record"
        );
    }
    let rec: LogRecord = serde_json::from_value(ipc_call_wire(json!(null))).expect("deserialize");
    let (_, rows) = round_trip(RedactionMode::Raw, vec![rec]);
    let call = rows.iter().find(|r| r["kind"] == "ipc.call").expect("call");
    assert!(call.get("argsOmitted").is_none(), "{call}");
}

/// W6's writer-side backstop, at the layer `enforce` actually operates on: a
/// `Value` whose `argsOmitted` is malformed loses that field. Unreachable via
/// the typed path above (serde rejects those shapes first) — this is the
/// defence-in-depth check for a future payload struct that widens the field.
#[test]
fn w6_enforce_strips_a_malformed_args_omitted() {
    let mut good = json!({ "kind": "ipc.call", "cmd": "commit", "argsOmitted": 2 });
    assert!(!enforce(&mut good));
    assert_eq!(good["argsOmitted"], json!(2));

    for bad in [json!(-1), json!("2"), json!(1.5), json!(null)] {
        let mut v = json!({ "kind": "ipc.call", "cmd": "commit", "argsOmitted": bad });
        assert!(!enforce(&mut v));
        assert!(v.get("argsOmitted").is_none(), "{v}");
    }
}

#[test]
fn enforce_is_a_no_op_on_records_without_args() {
    let mut v = json!({ "kind": "render", "component": "Sidebar", "count": 1 });
    let before = v.clone();
    assert!(!enforce(&mut v));
    assert_eq!(v, before);
}

/// AC7 — the violation flag cannot be forged: it is not a field of the payload
/// struct, so it is dropped at deserialisation, and `enforce` strips any
/// leftover before deciding.
#[test]
fn a_producer_supplied_violation_flag_does_not_survive() {
    let wire = json!({
        "seq": 0, "ts": 1_772_200_991_000_u64, "mono": 12, "src": "ui", "lvl": "debug",
        "kind": "ipc.call", "cmd": "commit", "argsHash": "h",
        "args": { "repoId": "r1" },
        "argsPolicyViolation": true
    });
    let rec: LogRecord = serde_json::from_value(wire).expect("deserialize");
    let mut v = serde_json::to_value(&rec).expect("serialize");
    assert!(
        v.get("argsPolicyViolation").is_none(),
        "forged flag survived deserialisation: {v}"
    );
    assert!(!enforce(&mut v), "{v}");
    assert!(v.get("argsPolicyViolation").is_none(), "{v}");

    // Belt and braces: even if some future struct accepted it, `enforce` strips
    // it from an otherwise-clean record.
    let mut injected = call_value(json!({ "repoId": "r1" }));
    injected["argsPolicyViolation"] = json!(true);
    assert!(!enforce(&mut injected), "{injected}");
    assert!(injected.get("argsPolicyViolation").is_none(), "{injected}");
}
