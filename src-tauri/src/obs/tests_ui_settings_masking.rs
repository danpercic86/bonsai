//! `getUiSettings` / `setUiSettings` stay captured, but no path they carry may
//! reach disk un-masked (user ruling 2026-09-28). `pickExternalTool` returns a
//! browsed path and `tool_scan` a `DetectedTool.detail`, both of which can land
//! in `UiSettings` (e.g. a custom editor path).
//!
//! VERIFIED COVERED by the existing pipeline — this file is the regression
//! guard, not a fix. Three layers, all on the writer thread (`writer.rs`):
//!   1. neither command is in `src/obs/rawArgPolicy.json`, so the producer
//!      sends no argument VALUES in raw mode either (DENY ⇒ hash + shape only);
//!   2. `raw_args::enforce` drops a whole non-scalar `args` object, so the nested
//!      `{ settings: {…} }` a buggy producer might send never lands;
//!   3. `scrub_value` home-masks EVERY string in every mode — so even a flat
//!      scalar path a forged/buggy producer slips past layers 1–2, or a path in
//!      an `error` message, is written as `<home>…`.
//!
//! Each test asserts the `<home>` token IS present as well as the account name
//! being absent, so a record silently dropped cannot pass for a masked one.

use std::path::Path;

use serde_json::{json, Value};

use super::record::{
    ArgShape, IpcOutcome, LogLevel, LogPayload, LogRecord, LogSource, RedactionMode,
};
use super::writer::{Limits, LogWriter, WriterConfig};

/// Distinctive so an absence check cannot pass by coincidence.
const ACCOUNT: &str = "jzqdoe";
const HOME_FOLDED: &str = "c:/users/jzqdoe";
const EDITOR_PATH: &str = r"C:\Users\jzqdoe\tools\code.exe";
const EDITOR_MASKED: &str = r"<home>\tools\code.exe";

fn shape(pairs: &[(&str, &str)]) -> ArgShape {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn base(payload: LogPayload) -> LogRecord {
    LogRecord {
        seq: 0,
        ts: 1_772_200_991_000,
        mono: 12,
        src: LogSource::Ui,
        lvl: LogLevel::Debug,
        trace: Some("t1".into()),
        span: None,
        caused_by: None,
        repo: None,
        payload,
    }
}

fn set_ui_settings_call(args: Value) -> LogRecord {
    base(LogPayload::IpcCall {
        cmd: "setUiSettings".into(),
        args_hash: "abc123".into(),
        args_shape: Some(shape(&[("0", "obj:1")])),
        args: Some(args),
        args_omitted: None,
    })
}

fn get_ui_settings_result() -> LogRecord {
    base(LogPayload::IpcResult {
        cmd: "getUiSettings".into(),
        args_hash: "abc123".into(),
        ms: 1.5,
        outcome: IpcOutcome::Ok,
        err_code: None,
        result_shape: Some(shape(&[("customEditorPath", "str"), ("theme", "str")])),
    })
}

fn set_ui_settings_error() -> LogRecord {
    base(LogPayload::Error {
        location: "setUiSettings".into(),
        code: None,
        message: format!("editor not found: {EDITOR_PATH}"),
        stack_hash: None,
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
        home_mask: Some(HOME_FOLDED.to_string()),
        limits: Limits::default(),
    }
}

/// Writes `records` to a fresh session; returns (raw file text, parsed lines).
fn round_trip(redaction: RedactionMode, records: Vec<LogRecord>) -> (String, Vec<Value>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let redactor = std::sync::Arc::new(super::redact::Redactor::with_salt([5; 16]));
    let mut w = LogWriter::open(cfg(dir.path(), redaction), redactor).expect("open");
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

/// The ruling's test: a raw-mode `setUiSettings` record carrying a path under
/// the configured home lands as `<home>…` and never as the account name.
#[test]
fn raw_set_ui_settings_flat_path_arg_is_home_masked_on_disk() {
    let (text, rows) = round_trip(
        RedactionMode::Raw,
        vec![set_ui_settings_call(
            json!({ "customEditorPath": EDITOR_PATH }),
        )],
    );
    assert!(!text.contains(ACCOUNT), "account name reached disk: {text}");
    assert_eq!(rows[0]["homeMasking"], true);
    assert_eq!(rows[1]["kind"], "ipc.call");
    assert_eq!(rows[1]["args"]["customEditorPath"], EDITOR_MASKED);
}

/// The shape the real call actually has (`setUiSettings(patch)`): a nested
/// object. The writer drops the whole `args` (non-scalar) and flags it.
#[test]
fn raw_set_ui_settings_nested_patch_is_dropped_whole() {
    let (text, rows) = round_trip(
        RedactionMode::Raw,
        vec![set_ui_settings_call(
            json!({ "settings": { "customEditorPath": EDITOR_PATH } }),
        )],
    );
    assert!(!text.contains(ACCOUNT), "account name reached disk: {text}");
    assert!(
        rows[1].get("args").is_none(),
        "nested args survived: {}",
        rows[1]
    );
    assert_eq!(rows[1]["argsPolicyViolation"], true);
}

/// `getUiSettings`' result is logged as a SHAPE (type tokens), and an `error`
/// naming the path is home-masked — in raw mode, the widest one.
#[test]
fn raw_get_ui_settings_result_and_error_message_carry_no_account_name() {
    let (text, rows) = round_trip(
        RedactionMode::Raw,
        vec![get_ui_settings_result(), set_ui_settings_error()],
    );
    assert!(!text.contains(ACCOUNT), "account name reached disk: {text}");
    assert_eq!(rows[1]["resultShape"]["customEditorPath"], "str");
    assert_eq!(
        rows[2]["message"],
        format!("editor not found: {EDITOR_MASKED}")
    );
}

/// Strict mode (the default) removes `args` outright; the account name still
/// never reaches disk from any of the three records.
#[test]
fn strict_mode_drops_ui_settings_args_and_masks_the_rest() {
    let (text, rows) = round_trip(
        RedactionMode::Strict,
        vec![
            set_ui_settings_call(json!({ "customEditorPath": EDITOR_PATH })),
            get_ui_settings_result(),
            set_ui_settings_error(),
        ],
    );
    assert!(!text.contains(ACCOUNT), "account name reached disk: {text}");
    assert!(
        rows[1].get("args").is_none(),
        "strict kept args: {}",
        rows[1]
    );
    assert_eq!(rows[1]["cmd"], "setUiSettings");
    assert_eq!(rows[2]["cmd"], "getUiSettings");
}
