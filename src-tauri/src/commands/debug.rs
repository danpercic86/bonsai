//! Debug/diagnostics commands (P86 instrumentation).
//!
//! `debug_perf_counters` exposes the backend cache/scan counters so the tester
//! (and the browser harness) can assert — from the outside — that an
//! unchanged-topology refresh served the layout cache instead of re-walking.
//! Read-only except `debug_reset_perf_counters`, which only zeroes the tallies.

use crate::perf::PerfCounters;
use crate::state::AppState;
use bonsai_core::error::AppError;

/// Current backend perf counters (graph walks / cache hits / redecorates /
/// status scans / repo opens).
#[tauri::command]
pub async fn debug_perf_counters(
    state: tauri::State<'_, AppState>,
) -> Result<PerfCounters, AppError> {
    Ok(state.perf.snapshot())
}

/// Zero every perf counter (harness/test reset before a measured scenario).
#[tauri::command]
pub async fn debug_reset_perf_counters(
    state: tauri::State<'_, AppState>,
) -> Result<(), AppError> {
    state.perf.reset();
    Ok(())
}
