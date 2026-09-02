//! P91 §3 — the observability record schema (v1).
//!
//! ONE concern: the wire/on-disk shape of a log record. No IO, no policy, no
//! redaction — those live in `writer.rs` / `sink.rs` / `redact.rs`.
//!
//! **Extensibility rules (the schema grows additively, never breaks).**
//! The concurrently-authored perf-diagnosis addendum adds optional payload
//! fields and, in increment 3, a new `span` kind. Both are cheap here by
//! construction:
//!   * every optional field carries
//!     `#[serde(default, skip_serializing_if = "Option::is_none")]`, so adding
//!     one neither breaks an older reader (serde ignores unknown keys) nor
//!     bloats a line that does not use it;
//!   * [`LogPayload`] is an internally-tagged enum whose variants each rename
//!     to their exact `kind` string, so a new kind is one variant + one
//!     `#[serde(rename = "…")]`.
//!
//! `OBS_SCHEMA_VERSION` stays `1` for additive changes; it is bumped only when
//! an EXISTING field changes shape or meaning.

use serde::{Deserialize, Serialize};

/// Record-schema version, written into the `session` header record (§6).
pub const OBS_SCHEMA_VERSION: u32 = 1;

/// Verbosity level of a record AND the Dev-mode capture threshold
/// (`DevSettings::level`, §10). Ordered most- to least-severe; `trace`
/// additionally force-enables frame capture (§5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    #[default]
    Debug,
    Trace,
}

/// Redaction mode of a whole log FILE (§7). A file never mixes modes — toggling
/// `dev.includeRawNames` starts a new file (§7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RedactionMode {
    /// The default. Safe to hand to a third party without reading it first.
    #[default]
    Strict,
    /// Opt-in: real repo/ref/path names. Tokens are STILL scrubbed (§7.2).
    Raw,
}

impl RedactionMode {
    /// The one-line `redactionNote` embedded in every `session` header so a
    /// reviewer opening the file knows what they hold without external context
    /// (§7.3).
    ///
    /// **Deliberately free of `/` characters.** The header is written through the
    /// same strict-mode enforcement as every other record (`obs/strict.rs` —
    /// which is what stops a producer parking a real path in a fake
    /// `redactionNote`), and that pass ordinalises any slash-bearing run that
    /// looks like a path. Phrasing this sentence as "repo/file/ref/remote names"
    /// or "trace/span/argsHash" would therefore ship every strict file with
    /// `path#N` gibberish in the middle of its own disclosure. Pinned verbatim by
    /// `tests_writer::first_line_is_a_valid_session_header`.
    pub fn note(self) -> &'static str {
        match self {
            RedactionMode::Strict => {
                "strict: argument values, repo, file, ref and remote names are replaced by stable \
                 per-session ordinals (ref#3, path#7). Commit messages, diffs, file contents, \
                 author names and emails are NEVER recorded in any mode. Credentials are always \
                 scrubbed. Ordinals are per-side and per-session (Rust: ref#3; UI: ui:ref#3) — \
                 join records on trace, span or argsHash, never on ordinal equality. Written only to \
                 this computer; Bonsai never uploads logs."
            }
            RedactionMode::Raw => {
                "raw: real repo, file, ref and remote names appear in this file (opt-in). Commit \
                 messages, diffs, file contents, author names and emails are NEVER recorded in \
                 any mode. Credentials are always scrubbed. Ordinals, where present, are per-side \
                 and per-session (Rust: ref#3; UI: ui:ref#3) — join records on trace, span or \
                 argsHash, never on ordinal equality. Written only to this computer; Bonsai \
                 never uploads logs."
            }
        }
    }
}

/// Which side produced the record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogSource {
    Ui,
    Rust,
}

/// Outcome of an IPC call as seen by the frontend proxy (§4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IpcOutcome {
    Ok,
    Err,
    Aborted,
    Superseded,
}

/// Severity of a derived anomaly record (§5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnomalySeverity {
    Info,
    Warn,
    Error,
}

/// Key-names + type + length ONLY, never values (§3 `ArgShape`).
pub type ArgShape = std::collections::BTreeMap<String, String>;

/// One line of a `logs/*.jsonl` file.
///
/// `seq` is assigned by the sink's writer thread (§3: "assigned by the SINK"),
/// which is the only place that sees every record in file order — a producer-side
/// counter could interleave with the channel and make `seq` disagree with the
/// bytes on disk. A record arriving over IPC therefore carries `seq: 0` and is
/// overwritten.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRecord {
    /// Monotonic per-session sequence; ordering is authoritative.
    #[serde(default)]
    pub seq: u64,
    /// Epoch ms, wall clock.
    pub ts: i64,
    /// Ms since session start (jitter-free ordering aid).
    pub mono: u64,
    pub src: LogSource,
    pub lvl: LogLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caused_by: Option<String>,
    /// Carries the `kind` discriminant (internally tagged) plus the per-kind
    /// fields, flattened into the same JSON object as the base fields.
    #[serde(flatten)]
    pub payload: LogPayload,
}

/// §9.3 — the dimension a `frame` record reports. The two frame recorders
/// measure different quantities (paint duration vs scroll inter-frame gap) and
/// must never be averaged together, so each record names its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrameDim {
    Paint,
    Gap,
}

/// The per-`kind` payload union (§3). Internally tagged on `kind`; each variant
/// renames to its exact wire string (the dotted kinds cannot be derived by
/// `rename_all`).
#[derive(Debug, Clone, Serialize, Deserialize)]
// `rename_all` covers the VARIANT names, `rename_all_fields` the fields inside
// them — both are needed: the wire is camelCase throughout, and every dotted
// kind (`ipc.call`, `render.tally`) additionally overrides its variant name.
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LogPayload {
    /// First line of every file (§6).
    #[serde(rename = "session")]
    Session {
        schema: u32,
        app: String,
        os: String,
        session_id: String,
        /// Always `true` — a session record only exists while Dev mode is on.
        dev_mode: bool,
        level: LogLevel,
        redaction: RedactionMode,
        /// Human-readable one-liner restating §7 for the reviewer.
        redaction_note: String,
        /// True when this header opens a file created by a purge roll (§6.1).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after_purge: Option<bool>,
        /// §6.3 — true once one or more EARLIER parts of this session have been
        /// evicted at the part cap: the file's beginning is gone.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        truncated: Option<bool>,
        /// §6.3 — count of earlier parts deleted so far for this session.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dropped_parts: Option<u32>,
    },
    #[serde(rename = "gesture")]
    Gesture { origin: String, gesture: String },
    #[serde(rename = "ipc.call")]
    IpcCall {
        cmd: String,
        args_hash: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        args_shape: Option<ArgShape>,
        /// Only present when the file's redaction mode is `raw` (§7.1).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        args: Option<serde_json::Value>,
    },
    #[serde(rename = "ipc.result")]
    IpcResult {
        cmd: String,
        args_hash: String,
        ms: f64,
        outcome: IpcOutcome,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        err_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        result_shape: Option<ArgShape>,
    },
    /// Rust-side dispatch stamp (§2.3; increment 3).
    #[serde(rename = "ipc.recv")]
    IpcRecv { cmd: String },
    #[serde(rename = "event")]
    Event {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        delivered: bool,
        listeners: u32,
    },
    #[serde(rename = "channel")]
    Channel {
        name: String,
        phase: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        chunks: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bytes: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ms: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        outcome: Option<String>,
    },
    #[serde(rename = "watcher")]
    Watcher {
        paths: u32,
        relevant: u32,
        debounce_ms: u32,
        fired: bool,
        suppressed: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        suppress_reason: Option<String>,
    },
    #[serde(rename = "refresh")]
    Refresh {
        round: u64,
        scope: String,
        origins: Vec<String>,
        contributing_traces: Vec<String>,
        collapsed: u32,
        ms: f64,
    },
    #[serde(rename = "render")]
    Render {
        component: String,
        count: u64,
        since_ms: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        changed_props: Option<Vec<String>>,
    },
    /// §9.2 aggregate mode — ONE record per component per window.
    #[serde(rename = "render.tally")]
    RenderTally {
        component: String,
        window_ms: f64,
        renders: u64,
        instances: u64,
        changed_props: Vec<String>,
        traces: Vec<String>,
    },
    #[serde(rename = "effect")]
    Effect {
        component: String,
        effect: String,
        run: u64,
        /// `[]` ⇒ ran with no semantic change.
        changed_deps: Vec<String>,
        dep_count: u32,
    },
    #[serde(rename = "state")]
    State {
        store: String,
        field: String,
        from: String,
        to: String,
    },
    #[serde(rename = "frame")]
    Frame {
        /// §9.3 — WHICH dimension this window measured. Paint duration and scroll
        /// inter-frame gap come from two separate recorders, so exactly one of
        /// `paint_ms`/`gap_ms` is a measurement and the other is a filler `0.0`.
        /// Without this discriminator a consumer reads `gapMs: 0` on a paint
        /// record as "zero gap measured" — a fabricated datum. REQUIRED, not
        /// optional: the only producer is the frontend's own `graphObs.ts`, log
        /// records are never re-parsed from disk by Rust, so there is no older
        /// writer to stay compatible with, and a `frame` record without a
        /// dimension is a bug worth rejecting rather than mislabelling.
        dim: FrameDim,
        paint_ms: f64,
        gap_ms: f64,
        over33: u32,
        over100: u32,
        worst_ms: f64,
    },
    #[serde(rename = "error")]
    Error {
        #[serde(rename = "where")]
        location: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<String>,
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stack_hash: Option<String>,
    },
    #[serde(rename = "anomaly")]
    Anomaly {
        rule: String,
        severity: AnomalySeverity,
        detail: String,
        /// `seq` numbers of the implicated records.
        refs: Vec<u64>,
        traces: Vec<String>,
    },
    /// Backend operation span (§3.1; increment 3). ONE record per completed
    /// operation, carrying its phase breakdown. Every field beyond `op`/`ms` is
    /// optional and `skip_serializing_if`, so a span that measured nothing extra
    /// still serialises to a compact `{op, ms}`.
    ///
    /// **Carries NO `argsHash`/`argsShape`** — same reason as `ipc.recv` (§7.2):
    /// a second canonical form for a call would silently break `dup-ipc`.
    #[serde(rename = "span")]
    Span {
        /// Allow-listed `<domain>.<action>`: `graph.get` | `status.scan` | `diff.compute`.
        op: String,
        /// Total wall time of the operation, measured at the src-tauri call site.
        ms: f64,
        /// Ordered, ≤16 entries. Sum may be < `ms`; the remainder is unattributed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        phases: Option<Vec<PhaseTiming>>,
        /// Ms spent QUEUED before the `spawn_blocking` closure started (§3.1.2).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        queued_ms: Option<u32>,
        /// Instrumented ops in flight when this one started, and the pool cap.
        /// Counts only span-emitting ops holding a `PoolGuard`, not the true tokio
        /// blocking-pool depth (see `obs::phase::POOL_INFLIGHT`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pool_inflight: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pool_max: Option<u32>,
        /// elapsed / git-timeout deadline, 0..1+ — watchdog pressure (§3.1.3).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        deadline_frac: Option<f32>,
        /// Graph-cache outcome, emitted only by `graph_cache.rs` (§5.1).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache: Option<String>,
        /// Primary unit count for the whole op (commits, files).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        items: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        outcome: Option<String>,
    },
    /// Sink backpressure (§6): records the bounded channel refused.
    #[serde(rename = "drop")]
    Drop { dropped: u64, since_seq: u64 },
    /// §6.3 — on-disk loss: an earlier part of this session was deleted to honour
    /// the part cap. Distinct from `drop` (in-memory backpressure). Emitted into
    /// the SURVIVING newest part immediately after the eviction.
    #[serde(rename = "truncate")]
    Truncate {
        /// Only cause in v1: the `max_parts` cap.
        reason: String,
        /// How many parts have now been deleted for this session.
        dropped_parts: u32,
        /// Redacted part label, e.g. `part#0` — never a path.
        dropped_part: String,
        /// Bytes of the evicted part, measured immediately before removal.
        bytes: u64,
        /// Lowest `seq` still present on disk, when known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        first_retained_seq: Option<u64>,
    },
}

/// One phase of a backend operation span (§3.1). `name` is an allow-listed
/// `&'static str` on the producing side (`obs/phase.rs`), so no user-derived
/// string can reach a span record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseTiming {
    /// Allow-listed, dotted for nesting: `revwalk`, `decorate`, `lane`, `serialize`.
    pub name: String,
    pub ms: f64,
    /// Optional unit count for the phase (commits walked, files scanned).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n: Option<u64>,
}

impl LogPayload {
    /// The wire `kind` string of this payload — used by the capture filters
    /// without re-serialising.
    pub fn kind(&self) -> &'static str {
        match self {
            LogPayload::Session { .. } => "session",
            LogPayload::Gesture { .. } => "gesture",
            LogPayload::IpcCall { .. } => "ipc.call",
            LogPayload::IpcResult { .. } => "ipc.result",
            LogPayload::IpcRecv { .. } => "ipc.recv",
            LogPayload::Event { .. } => "event",
            LogPayload::Channel { .. } => "channel",
            LogPayload::Watcher { .. } => "watcher",
            LogPayload::Refresh { .. } => "refresh",
            LogPayload::Render { .. } => "render",
            LogPayload::RenderTally { .. } => "render.tally",
            LogPayload::Effect { .. } => "effect",
            LogPayload::State { .. } => "state",
            LogPayload::Frame { .. } => "frame",
            LogPayload::Error { .. } => "error",
            LogPayload::Anomaly { .. } => "anomaly",
            LogPayload::Span { .. } => "span",
            LogPayload::Drop { .. } => "drop",
            LogPayload::Truncate { .. } => "truncate",
        }
    }
}
