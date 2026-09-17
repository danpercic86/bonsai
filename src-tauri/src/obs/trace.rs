//! P91 §2.1/§2.4 — the backend trace model and the `emit_logged` event helper.
//!
//! **There is deliberately no ambient backend trace** (§2.2): a task-local would
//! be empty exactly where heavy work runs (`spawn_blocking`), so every site that
//! wants causality threads a [`TraceMeta`] EXPLICITLY. `emit_logged` takes its
//! meta by reference at the call site — a command that already carries a caller
//! trace passes [`TraceMeta::child_of`]; everywhere else [`TraceMeta::root`],
//! which is honest rather than guessed.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{Emitter, Manager};

use super::record::{LogLevel, LogPayload, LogRecord, LogSource};
use super::sink::Sink;
use super::ObsState;

/// 12-char base36, monotonic-prefixed (`{t36}-{rand4}`), matching the TS shape.
pub type TraceId = String;

/// Causal metadata attached to a backend-produced record (§2.1).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceMeta {
    pub trace: TraceId,
    /// Set when this backend work was CAUSED by a prior trace (event fan-out, a
    /// scheduler job seeded by a user action).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caused_by: Option<TraceId>,
}

impl TraceMeta {
    /// Mints a fresh backend-origin trace. `origin` is a call-site label
    /// (`"backend"`, `"watcher"`) documenting intent; §2.1's `TraceMeta` has no
    /// origin field, so it is deliberately NOT stored — only the minted trace and
    /// its (absent) cause are recorded.
    pub fn root(_origin: &str) -> Self {
        TraceMeta {
            trace: mint_trace(),
            caused_by: None,
        }
    }

    /// A fresh trace whose `caused_by` points at `parent` — used when backend
    /// work fans out from a known caller trace.
    pub fn child_of(parent: &TraceId) -> Self {
        TraceMeta {
            trace: mint_trace(),
            caused_by: Some(parent.clone()),
        }
    }
}

/// Monotonic-prefixed, collision-resistant trace id (`{t36}-{rand4}`).
fn mint_trace() -> TraceId {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ms = super::writer::now_ms().max(0) as u64;
    let t36 = to_base36(ms.wrapping_add(n));
    let rand4 = to_base36(rand::random::<u32>() as u64 & 0x1F_FFFF);
    let mut id = format!("{t36}-{rand4}");
    id.truncate(12);
    id
}

fn to_base36(mut n: u64) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".into();
    }
    let mut buf = Vec::new();
    while n > 0 {
        buf.push(DIGITS[(n % 36) as usize]);
        n /= 36;
    }
    buf.reverse();
    String::from_utf8(buf).unwrap_or_default()
}

// --------------------------------------------------------------- active sink

/// Process-wide handle to the live sink, used ONLY by producers that cannot
/// reach an `AppHandle` — the [`super::phase::PhaseRecorder`] on the blocking
/// pool. Consistent with the panic hook's global weak (`obs/mod.rs`): the sink
/// already outlives any single call, and threading an `AppHandle` into the
/// `spawn_blocking` git closures would force a command-signature change (§2.2
/// forbids that).
static ACTIVE_SINK: Mutex<Option<Arc<Sink>>> = Mutex::new(None);

/// Set/cleared by `apply_dev_settings`/`stop` alongside the writer lifecycle.
pub fn set_active_sink(sink: Option<Arc<Sink>>) {
    *ACTIVE_SINK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = sink;
}

/// Serializes tests that mutate the process-wide [`ACTIVE_SINK`] global, so a
/// span-emitting test in one module can't have its sink yanked by another.
#[cfg(test)]
pub(crate) fn test_sink_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The live sink, if Dev mode is on. `None` ⇒ zero-cost no-op for producers.
pub fn active_sink() -> Option<Arc<Sink>> {
    ACTIVE_SINK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

// ------------------------------------------------------------- record helper

/// Builds a Rust-side [`LogRecord`] (seq assigned later by the sink) carrying the
/// trace/causal ids from `meta` and the given payload.
pub fn make_record(sink: &Sink, lvl: LogLevel, meta: &TraceMeta, payload: LogPayload) -> LogRecord {
    LogRecord {
        seq: 0,
        ts: super::writer::now_ms(),
        mono: sink.mono(),
        src: LogSource::Rust,
        lvl,
        trace: Some(meta.trace.clone()),
        span: None,
        caused_by: meta.caused_by.clone(),
        payload,
    }
}

/// Enqueues a Rust-side record iff Dev mode is on. Never blocks.
pub fn log_with(app: &tauri::AppHandle, lvl: LogLevel, meta: &TraceMeta, payload: LogPayload) {
    if let Some(sink) = app.try_state::<ObsState>().and_then(|s| s.sink()) {
        sink.enqueue(make_record(&sink, lvl, meta, payload));
    }
}

// -------------------------------------------------------------- emit_logged

/// Emits a Tauri event AND logs an `event` record with explicit causality
/// (§2.4). Every migrated `app.emit(...)` site calls this instead.
///
/// The logged record describes the EMISSION (event name, delivered) — never the
/// payload contents, which stay on the wire to the frontend only. `listeners` is
/// best-effort `0`: Tauri v2 does not expose a per-event listener count to the
/// emitter, and the frontend proxy records delivery on the receiving side (§4).
pub fn emit_logged<P: serde::Serialize + Clone>(
    app: &tauri::AppHandle,
    event: &str,
    payload: P,
    meta: &TraceMeta,
) {
    let delivered = app.emit(event, payload).is_ok();
    log_with(
        app,
        LogLevel::Debug,
        meta,
        LogPayload::Event {
            name: event.to_string(),
            reason: None,
            delivered,
            listeners: 0,
        },
    );
}
