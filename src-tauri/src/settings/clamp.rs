//! Range-clamping helpers + their bound/default constants for
//! [`crate::settings`].
//!
//! Split out of `settings.rs` to keep whole-file reads cheap (CLAUDE.md
//! file-size discipline). Each helper is called by both `load_from` (defend a
//! hand-edited file) and the setter commands (defend a future UI bug).
//! Re-exported from the `settings` module so external call sites are unchanged.

use super::{AutoFetch, GraphPrefs, HealthRefresh, PaneWidths, Settings};

pub const SIDEBAR_MIN: u32 = 180;
pub const SIDEBAR_MAX: u32 = 480;
pub const RIGHT_PANEL_MIN: u32 = 280;
pub const RIGHT_PANEL_MAX: u32 = 640;

/// Clamps to the documented ranges; called by both `load_from` (defend
/// against a hand-edited file) and the setter commands (defend against a
/// future UI bug).
pub fn clamp_pane_widths(w: PaneWidths) -> PaneWidths {
    PaneWidths {
        sidebar: w.sidebar.clamp(SIDEBAR_MIN, SIDEBAR_MAX),
        right_panel: w.right_panel.clamp(RIGHT_PANEL_MIN, RIGHT_PANEL_MAX),
    }
}

pub const HEALTH_REFRESH_INTERVAL_MIN: u32 = 1;
pub const HEALTH_REFRESH_INTERVAL_MAX: u32 = 240;

/// Clamps the health-refresh interval to its documented range; called by both
/// `load_from` (defend a hand-edited file) and the setter command (P30 D12,
/// mirrors [`clamp_auto_fetch`]).
pub fn clamp_health_refresh(h: HealthRefresh) -> HealthRefresh {
    HealthRefresh {
        enabled: h.enabled,
        interval_minutes: h
            .interval_minutes
            .clamp(HEALTH_REFRESH_INTERVAL_MIN, HEALTH_REFRESH_INTERVAL_MAX),
    }
}

pub const AUTO_FETCH_INTERVAL_MIN: u32 = 1;
pub const AUTO_FETCH_INTERVAL_MAX: u32 = 120;
pub const AVATAR_RADIUS_MIN: u32 = 6;
pub const AVATAR_RADIUS_MAX: u32 = 16;
pub const ROW_HEIGHT_MIN: u32 = 24;
pub const ROW_HEIGHT_MAX: u32 = 48;
pub const LANE_WIDTH_MIN: u32 = 10;
pub const LANE_WIDTH_MAX: u32 = 28;

/// Clamps the auto-fetch interval to its documented range; called by both
/// `load_from` (defend a hand-edited file) and the setter command.
pub fn clamp_auto_fetch(a: AutoFetch) -> AutoFetch {
    AutoFetch {
        enabled: a.enabled,
        interval_minutes: a
            .interval_minutes
            .clamp(AUTO_FETCH_INTERVAL_MIN, AUTO_FETCH_INTERVAL_MAX),
    }
}

/// Clamps each graph geometry knob to its documented range; called by both
/// `load_from` (defend a hand-edited file) and the setter command. The P51
/// detail toggles + `date_basis` have no numeric range — they pass through
/// unclamped via struct-update (`..g`). Keep the `..g`: dropping it would
/// silently reset every toggle to its field default on every load/save.
pub fn clamp_graph_prefs(g: GraphPrefs) -> GraphPrefs {
    GraphPrefs {
        avatar_radius: g.avatar_radius.clamp(AVATAR_RADIUS_MIN, AVATAR_RADIUS_MAX),
        row_height: g.row_height.clamp(ROW_HEIGHT_MIN, ROW_HEIGHT_MAX),
        lane_width: g.lane_width.clamp(LANE_WIDTH_MIN, LANE_WIDTH_MAX),
        ..g // toggles + date_basis pass through unclamped
    }
}

// ---- P68 §8.3: streaming AI-run ranges. `0` is a documented SENTINEL for the
// first two (watchdog disabled / no hard cap) and for the budget (no flag), so the
// clamps are "0 or in range", never a plain `clamp` that would silently turn an
// intentional 0 into a minimum.
pub const AI_IDLE_TIMEOUT_DEFAULT: u32 = 300;
pub const AI_IDLE_TIMEOUT_MIN: u32 = 30;
pub const AI_IDLE_TIMEOUT_MAX: u32 = 3600;
pub const AI_HARD_CAP_MIN: u32 = 60;
pub const AI_HARD_CAP_MAX: u32 = 86_400;
pub const AI_MAX_TURNS_MIN: u32 = 1;
pub const AI_MAX_TURNS_MAX: u32 = 20;
pub const AI_BULK_MAX_BYTES_DEFAULT: u32 = 400_000;
pub const AI_BULK_MAX_BYTES_MIN: u32 = 20_000;
pub const AI_BULK_MAX_BYTES_MAX: u32 = 4_000_000;
pub const AI_MAX_BUDGET_USD_MAX: f64 = 100.0;
pub const AI_DOCK_HEIGHT_DEFAULT: u32 = 180;
pub const AI_DOCK_HEIGHT_MIN: u32 = 120;
pub const AI_DOCK_HEIGHT_MAX: u32 = 600;

/// Clamps the P68 streaming-AI knobs to their documented ranges; called by both
/// `load_from` (defend a hand-edited file) and `apply_patch` (defend a future UI
/// bug), exactly like `clamp_pane_widths` / `clamp_graph_prefs`.
///
/// Mutates in place rather than returning a struct, because these are ten
/// TOP-LEVEL fields (A8) rather than one nested preference object.
pub fn clamp_ai_settings(s: &mut Settings) {
    if s.ai_idle_timeout_secs != 0 {
        s.ai_idle_timeout_secs =
            s.ai_idle_timeout_secs.clamp(AI_IDLE_TIMEOUT_MIN, AI_IDLE_TIMEOUT_MAX);
    }
    if s.ai_hard_cap_secs != 0 {
        s.ai_hard_cap_secs = s.ai_hard_cap_secs.clamp(AI_HARD_CAP_MIN, AI_HARD_CAP_MAX);
    }
    s.ai_max_turns = s.ai_max_turns.clamp(AI_MAX_TURNS_MIN, AI_MAX_TURNS_MAX);
    s.ai_bulk_max_bytes =
        s.ai_bulk_max_bytes.clamp(AI_BULK_MAX_BYTES_MIN, AI_BULK_MAX_BYTES_MAX);
    // NaN/inf would poison the `{:.4}` argv formatting, and a negative budget is
    // meaningless — both collapse to "no cap".
    if !s.ai_max_budget_usd.is_finite() || s.ai_max_budget_usd < 0.0 {
        s.ai_max_budget_usd = 0.0;
    } else if s.ai_max_budget_usd > AI_MAX_BUDGET_USD_MAX {
        s.ai_max_budget_usd = AI_MAX_BUDGET_USD_MAX;
    }
    s.ai_dock_height = s.ai_dock_height.clamp(AI_DOCK_HEIGHT_MIN, AI_DOCK_HEIGHT_MAX);
}
