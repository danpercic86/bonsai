//! P91 §2.3 — the invoke-shim extraction, tested runtime-free (constructing a
//! real `tauri::ipc::Invoke` needs the tauri `test` feature, banned on this
//! machine). The shim closure is thin; the logic under test is
//! [`super::invoke_shim::extract_trace_ids`] plus the `ipc.recv` payload shape.

use serde_json::json;

use crate::obs::invoke_shim::{extract_trace_ids, SPAN_KEY, TRACE_KEY};
use crate::obs::record::{LogPayload, LogRecord, LogSource};

/// §2.4 migration invariant: every backend `app.emit(...)` goes through
/// [`crate::obs::trace::emit_logged`], which is the ONLY site allowed to bring the
/// `tauri::Emitter` trait into scope. Calling `AppHandle::emit` requires that
/// trait, so a new unmigrated emit would have to import it — and this test would
/// catch it. (The activity `Channel` path is not an `Emitter` call — channels
/// get open/close records, not `emit_logged`, per §2.4.)
#[test]
fn no_unmigrated_app_emit_site() {
    fn imports_tauri_emitter(line: &str) -> bool {
        let l = line.trim_start();
        !l.starts_with("//")
            && l.contains("use")
            && l.contains("tauri")
            && l.contains("Emitter")
            && !l.contains("ActivityEmitter")
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read_dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let rel = path.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
            // The sole legitimate importer is the helper itself.
            if rel == "obs/trace.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read src file");
            if text.lines().any(imports_tauri_emitter) {
                offenders.push(rel);
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "unmigrated app.emit site(s) importing tauri::Emitter: {offenders:?}"
    );
}

/// §12 row-3 (f): `crates/bonsai-core` gains NO dependency on `obs`. All phase
/// timing lives at the src-tauri caller layer (§3.1.2). Inspection test: no core
/// source line references the `obs` module.
#[test]
fn bonsai_core_has_no_obs_reference() {
    let core = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("crates")
        .join("bonsai-core")
        .join("src");
    let mut offenders = Vec::new();
    let mut stack = vec![core.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read core src").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read core file");
            if text.contains("crate::obs") || text.contains("obs::phase") || text.contains(" obs::") {
                offenders.push(path.to_string_lossy().to_string());
            }
        }
    }
    assert!(offenders.is_empty(), "bonsai-core references obs: {offenders:?}");
}

#[test]
fn extracts_reserved_keys_from_payload_map() {
    let payload = json!({
        "repoId": "/some/path",
        TRACE_KEY: "abc123-x9",
        SPAN_KEY: "s0f1a2",
    });
    let (trace, span) = extract_trace_ids(&payload);
    assert_eq!(trace.as_deref(), Some("abc123-x9"));
    assert_eq!(span.as_deref(), Some("s0f1a2"));
}

#[test]
fn untraced_dispatch_yields_no_ids() {
    // §2.5: the transport stamps `__trace` ONLY when an ambient trace exists, so
    // an untraced dispatch legitimately carries neither key — that is correct,
    // not a bug.
    let payload = json!({ "repoId": "/x", "staged": true });
    assert_eq!(extract_trace_ids(&payload), (None, None));
    // Non-object bodies (arrays, scalars) also yield nothing.
    assert_eq!(extract_trace_ids(&json!([1, 2, 3])), (None, None));
    assert_eq!(extract_trace_ids(&json!("raw")), (None, None));
}

/// PROHIBITION (§7.2 / §12 row 3): an emitted `ipc.recv` carries `cmd` + trace
/// ids ONLY — never an `argsHash`/`argsShape`. Adding one would introduce a
/// second canonical form and silently break `dup-ipc`.
#[test]
fn ipc_recv_json_has_cmd_and_no_args_hash() {
    let rec = LogRecord {
        seq: 0,
        ts: 1,
        mono: 0,
        src: LogSource::Rust,
        lvl: crate::obs::record::LogLevel::Debug,
        trace: Some("abc123-x9".into()),
        span: Some("s0f1a2".into()),
        caused_by: None,
        payload: LogPayload::IpcRecv {
            cmd: "get_graph".into(),
        },
    };
    let v = serde_json::to_value(&rec).expect("serialize");
    assert_eq!(v["kind"], "ipc.recv");
    assert_eq!(v["cmd"], "get_graph");
    assert_eq!(v["trace"], "abc123-x9");
    assert!(v.get("argsHash").is_none(), "ipc.recv must have no argsHash");
    assert!(v.get("argsShape").is_none(), "ipc.recv must have no argsShape");
    assert!(v.get("args").is_none(), "ipc.recv must have no args");
}
