//! P91 §3 — the wire `kind` string of a [`LogPayload`].
//!
//! ONE concern: the payload-variant → `kind` mapping the capture filters use
//! without re-serialising. Its own file because [`super::record`] sits at the
//! ~500-line soft limit and this is the one piece of it that is a lookup rather
//! than a schema declaration — the same reasoning that gave
//! `tests_schema_parity.rs` its own file.
//!
//! Adding a variant to [`LogPayload`] therefore touches two files, which is the
//! point: the `match` is exhaustive, so a forgotten arm is a compile error here
//! rather than a wrong `kind` string on disk.

use super::record::LogPayload;

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
