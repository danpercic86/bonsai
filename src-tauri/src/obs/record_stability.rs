//! P120 §2.1 — payloads of the two dev-mode stability kinds, `layout.shift`
//! and `focus.lost`.
//!
//! ONE concern: their wire shape — closed enums, booleans and numbers only, so
//! no path or user string can reach disk (AC 13). Only [`StabilityRegion`] is
//! lenient on input (unknown ⇒ `Other`, never a rejected `log_append` batch).

use serde::{Deserialize, Serialize};

/// Closed region vocabulary (P120). NEVER a path or user string.
/// `from = "String"` ⇒ `rename_all` governs Serialize only; the `From<String>`
/// match MUST use the same lowercase strings as `as_str()` (pinned by T-R11's
/// valid-region round trip).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", from = "String")]
pub enum StabilityRegion {
    Sidebar,
    Graph,
    Details,
    Diff,
    Dock,
    Toolbar,
    Other,
}

impl From<String> for StabilityRegion {
    fn from(s: String) -> Self {
        match s.as_str() {
            "sidebar" => StabilityRegion::Sidebar,
            "graph" => StabilityRegion::Graph,
            "details" => StabilityRegion::Details,
            "diff" => StabilityRegion::Diff,
            "dock" => StabilityRegion::Dock,
            "toolbar" => StabilityRegion::Toolbar,
            _ => StabilityRegion::Other,
        }
    }
}

impl StabilityRegion {
    pub fn as_str(self) -> &'static str {
        match self {
            StabilityRegion::Sidebar => "sidebar",
            StabilityRegion::Graph => "graph",
            StabilityRegion::Details => "details",
            StabilityRegion::Diff => "diff",
            StabilityRegion::Dock => "dock",
            StabilityRegion::Toolbar => "toolbar",
            StabilityRegion::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StabilityPhase {
    Boot,
    Interactive,
}

impl StabilityPhase {
    /// The wire string (also `FocusVia::as_str`) — anomaly `detail` text (§2.5).
    pub fn as_str(self) -> &'static str {
        match self {
            StabilityPhase::Boot => "boot",
            StabilityPhase::Interactive => "interactive",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FocusVia {
    Focusout,
    Detached,
}

impl FocusVia {
    pub fn as_str(self) -> &'static str {
        match self {
            FocusVia::Focusout => "focusout",
            FocusVia::Detached => "detached",
        }
    }
}

/// One coalesced bucket: ≤1 record per (region, phase, hadRecentInput, resizing) per 1 s.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutShiftPayload {
    pub region: StabilityRegion,
    pub phase: StabilityPhase,
    pub had_recent_input: bool,
    pub resizing: bool,
    /// Entries coalesced into this record (≥1).
    pub shifts: u32,
    /// Sum of whole-entry scores (A9). Finite, ≥0, 4 dp.
    pub value: f64,
    /// Epoch ms of the first coalesced entry (the record's `ts` is the flush time).
    pub first_at: i64,
    /// last entry − first entry, ms.
    pub span_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusLostPayload {
    /// Region of the element that HAD focus (captured at focusin, A3).
    pub from_region: StabilityRegion,
    pub phase: StabilityPhase,
    pub via: FocusVia,
    /// Epoch ms the loss was observed (record `ts` = classify time, ≈ +50 ms).
    pub lost_at: i64,
}
