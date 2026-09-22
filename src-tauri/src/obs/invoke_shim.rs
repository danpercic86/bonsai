//! P91 §2.3 — the Rust dispatch choke point.
//!
//! [`instrumented_handler`] wraps the closure returned by `generate_handler![…]`
//! in `lib.rs`. It reads the command name and the payload map NON-consumingly to
//! extract the reserved [`TRACE_KEY`]/[`SPAN_KEY`] the frontend proxy stamps
//! (§2.2), emits one `ipc.recv` arrival stamp carrying **`cmd` + trace ids
//! ONLY** (§7.2 — no `argsHash`/`argsShape`, ever), then delegates.
//!
//! **The shim cannot observe completion** — the resolver is consumed downstream —
//! so command duration is measured on the frontend `ipc.call`/`ipc.result` pair
//! (§2.3). This is an arrival stamp, not a second description of the call.
//!
//! ## §2.3.1 Contingency — dropping the shim without re-deriving it
//!
//! `tauri::ipc::Invoke` is documented upstream as "explicitly NOT stable". **If a
//! Tauri upgrade breaks [`instrumented_handler`], delete this file and its
//! `lib.rs` wiring (revert to a bare `generate_handler!`). Do NOT attempt to
//! repair it and do NOT add `TraceMeta` parameters to command signatures.**
//!
//! Exactly what is lost: only the `ipc.recv` record — the Rust-side arrival
//! stamp. Retained in full: per-command traces/spans/durations/outcomes
//! (frontend `ipc.call`/`ipc.result`), every event/channel/watcher record
//! (`emit_logged` takes its `TraceMeta` explicitly and never depended on the
//! shim), the `span` phase records (emitted by explicit call sites), and every
//! anomaly rule except distinguishing "never sent" from "sent but never
//! answered". `dup-ipc` is unaffected (it keys on frontend `ipc.call`). No
//! increment other than 3 is affected.

use tauri::{Manager, Runtime};

use super::record::{LogLevel, LogPayload, LogRecord, LogSource};
use super::ObsState;

/// Reserved payload keys the frontend proxy stamps at the payload top level
/// (`src/ipc/tauri/invoke.ts`). Kept as constants so both sides pin the exact
/// strings.
pub const TRACE_KEY: &str = "__trace";
pub const SPAN_KEY: &str = "__span";

/// Commands excluded from instrumentation by name — logging about logging (and
/// the perf/metrics reads the Dev page polls) self-amplifies (§2.3).
const EXCLUDED: &[&str] = &[
    "log_append",
    "log_session_info",
    "logs_delete_all",
    "metrics_snapshot",
    "metrics_reset",
    "debug_perf_counters",
];

fn is_excluded(cmd: &str) -> bool {
    EXCLUDED.contains(&cmd)
}

/// Runtime-free extraction of the reserved trace ids from a decoded payload map.
///
/// Factored out precisely because unit-testing the full shim would need a
/// `tauri::ipc::Invoke` (which needs the tauri `test` feature — banned on this
/// machine). A `Raw` (non-JSON) body carries no map and yields `(None, None)`.
pub fn extract_trace_ids(payload: &serde_json::Value) -> (Option<String>, Option<String>) {
    let obj = payload.as_object();
    let get = |k: &str| {
        obj.and_then(|m| m.get(k))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };
    (get(TRACE_KEY), get(SPAN_KEY))
}

/// Wraps the generated invoke handler, stamping an `ipc.recv` before delegating.
pub fn instrumented_handler<R: Runtime>(
    inner: impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static,
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    move |invoke| {
        let cmd = invoke.message.command().to_string();
        if !is_excluded(&cmd) {
            if let Some(sink) = invoke
                .message
                .webview_ref()
                .try_state::<ObsState>()
                .and_then(|s| s.sink())
            {
                let (trace, span) = match invoke.message.payload() {
                    tauri::ipc::InvokeBody::Json(v) => extract_trace_ids(v),
                    tauri::ipc::InvokeBody::Raw(_) => (None, None),
                };
                let rec = LogRecord {
                    seq: 0,
                    ts: super::writer::now_ms(),
                    mono: sink.mono(),
                    src: LogSource::Rust,
                    lvl: LogLevel::Debug,
                    trace,
                    span,
                    caused_by: None,
                    repo: None,
                    payload: LogPayload::IpcRecv { cmd },
                };
                sink.enqueue(rec);
            }
        }
        inner(invoke)
    }
}

#[cfg(test)]
#[path = "tests_shim.rs"]
mod tests_shim;
