//! The **testable** half of `pick_external_tool` (P112 §5.4 / AC18): everything
//! after the native dialog has produced a path.
//!
//! Split from `commands::tools` on purpose. The dialog itself needs a real
//! window and is a USER CHECKPOINT; validate-then-write is pure logic over a
//! settings file, and putting it behind this seam is what makes the write
//! semantics — one `settings::update` cycle, nothing written on a refusal —
//! assertable without one.

use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::external::TargetOs;
use bonsai_core::tools::{self, DetectedTool, ToolKind};

use crate::settings;

/// Validate `chosen` and, only if it passes, write BOTH `custom_<kind>_path` and
/// `<kind>_tool = "custom"` in ONE [`settings::update`] cycle.
///
/// **The single cycle is the point.** `update` holds the process-wide
/// `SETTINGS_IO` mutex across load→mutate→save; outside it a pick racing a
/// pane-width drag-save would lose one of the two writes to the last rename —
/// the precise failure that mutex exists for. Two cycles could also leave
/// `*_tool == "custom"` with an empty path, which `coerce_tool_id` would scrub
/// on the next unrelated patch.
///
/// A refusal writes **nothing** — not the path, not the selection — because the
/// validation runs before `update` is ever entered. The error is category-only
/// and never echoes the path.
///
/// BLOCKING (the validation stats the filesystem; the write is a temp-write +
/// atomic rename). Callers run it under `spawn_blocking`.
pub(crate) fn commit_browsed_tool(
    file: &Path,
    kind: ToolKind,
    chosen: &Path,
    os: TargetOs,
) -> Result<DetectedTool, AppError> {
    // Validate FIRST: `browsed_tool_row` is the refusal, and it is also the one
    // place the picker row's shape is decided, so the row returned here is
    // byte-identical to the one the next scan lists for this path.
    let row = tools::browsed_tool_row(kind, chosen, os)?;
    let stored = chosen.to_string_lossy().to_string();
    settings::update(file, |s| settings::set_browsed_tool(s, kind, &stored))?;
    Ok(row)
}

#[cfg(test)]
#[path = "tests_tools_pick.rs"]
mod tests;
