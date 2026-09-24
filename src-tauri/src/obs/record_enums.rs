//! P120 §2.3 — the small closed enums of the observability record schema,
//! split out of `record.rs` verbatim and re-exported from there so every
//! `record::X` path still resolves.

use serde::{Deserialize, Serialize};

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

/// §9.3 — the dimension a `frame` record reports. The two frame recorders
/// measure different quantities (paint duration vs scroll inter-frame gap) and
/// must never be averaged together, so each record names its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrameDim {
    Paint,
    Gap,
}
