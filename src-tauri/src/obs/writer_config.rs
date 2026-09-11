//! P91 §6 — the §6 CAPS and the writer's start-up configuration: the byte/part
//! limits and [`WriterConfig`]. Split out of `writer.rs` to keep that file on its
//! one stateful concern ([`super::writer::LogWriter`]); both types are
//! re-exported from `writer`, so every existing `writer::…` call site is
//! unchanged.

use std::path::PathBuf;

use super::record::{LogLevel, RedactionMode};

/// Size cap of one rotation part (§6).
pub const PART_BYTES: u64 = 16 * 1024 * 1024;
/// Max rotation parts per session (§6).
pub const MAX_PARTS: u32 = 8;
/// Session files kept at session start (§6).
pub const KEEP_SESSIONS: usize = 10;
/// Total bytes kept across all sessions at session start (§6).
pub const TOTAL_BYTES: u64 = 256 * 1024 * 1024;
/// Buffered bytes that force a flush (§6).
pub const FLUSH_BYTES: u64 = 64 * 1024;

/// The §6 caps, as data so tests can drive rotation/pruning without writing
/// hundreds of megabytes. Production always uses [`Limits::default`].
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub part_bytes: u64,
    pub max_parts: u32,
    pub keep_sessions: usize,
    pub total_bytes: u64,
    pub flush_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            part_bytes: PART_BYTES,
            max_parts: MAX_PARTS,
            keep_sessions: KEEP_SESSIONS,
            total_bytes: TOTAL_BYTES,
            flush_bytes: FLUSH_BYTES,
        }
    }
}

/// Everything the writer needs to open a session's first file.
#[derive(Debug, Clone)]
pub struct WriterConfig {
    pub dir: PathBuf,
    pub session_id: String,
    /// Epoch SECONDS of session start — the file-name stamp.
    pub started_secs: i64,
    pub app_version: String,
    pub os: String,
    pub level: LogLevel,
    pub redaction: RedactionMode,
    /// §7.2 — the FOLDED home directory masked out of every string field
    /// (`obs::home_resolve::resolve_home_mask`), or `None` when it could not be
    /// resolved. Carried per-writer instead of in a process global so the wiring
    /// is unit-testable and the session header can state it: `homeMasking` is
    /// exactly `home_mask.is_some()`.
    pub home_mask: Option<String>,
    pub limits: Limits,
}
