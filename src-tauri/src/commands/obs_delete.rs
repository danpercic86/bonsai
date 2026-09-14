//! P91 §6.1 / §F6 — "Delete logs and usage counts": the one destructive
//! observability command, its response type, and the merge that keeps both halves
//! of the result honest.
//!
//! Split out of `commands/obs.rs` because the delete is the only command there
//! that DESTROYS anything: it is the file a reviewer should be able to read whole
//! without also reading the export/session/metrics read paths.
//!
//! On the §2.3 instrumentation EXCLUSION list, like every other command in this
//! pair of files — deleting logs writes no record into the surviving file.

use std::sync::Arc;

use bonsai_core::error::AppError;

use crate::obs::{self, writer, ClearMode, MetricsClearCounts, MetricsState, ObsState};
use crate::perf::PerfState;
use crate::state::AppState;

/// §6.1 — the honest result of "delete all log files".
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsDeleteResult {
    /// Files actually removed (log parts AND export zips — see `deleted_exports`).
    pub deleted_files: u32,
    /// Bytes reclaimed (sizes summed, each measured immediately before removal).
    pub deleted_bytes: u64,
    /// Files that could not be removed (locked, permission denied, ...).
    pub failed_files: u32,
    /// Present only when Dev mode was ON: the fresh, empty file logging continues
    /// into. NAME only, never a path.
    pub active_file: Option<String>,
    /// True when the writer was rolled to a new file as part of this operation.
    pub rolled: bool,
    /// §6.2 — how many of `deleted_files` were export zips. Additive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_exports: Option<u32>,
    /// §F6 — usage-statistics files removed from `metrics/` (`usage.json`,
    /// `.bak`, `.tmp`). ALSO included in `deleted_files`/`deleted_bytes`, like
    /// export zips.
    pub deleted_metrics: u32,
    /// §F6 — the in-memory aggregate was reset AND no usage file remains.
    ///
    /// **True even when `deleted_metrics == 0`:** `init` only marks the file
    /// dirty, so on a launch younger than the first 60 s flush there is nothing on
    /// disk yet while the aggregate is very much live. The copy's "clears your
    /// usage counts" is justified by THIS field, never by the file count. Both
    /// fields are REQUIRED (not `Option`): this is a same-build command response,
    /// never persisted, so the additive-only carve-out for on-disk records does
    /// not apply, and an optional flag would let the UI fall back to a misleading
    /// default.
    pub metrics_cleared: bool,
}

/// §6.1 / §F6 — "Delete logs and usage counts" (roll-then-purge, then clear).
///
/// Dev mode ON: sends `RollAndPurge` to the writer thread, which rolls to a fresh
/// file (header `afterPurge: true`) then deletes every other in-scope file — so
/// logging continues with no lost record. Dev mode OFF: no writer exists, so the
/// in-scope files are deleted directly. Either way the scan/delete runs on
/// `spawn_blocking`, so the UI never blocks.
///
/// §F6: the scope now also covers `<app_config_dir>/metrics/`. The metrics step
/// runs in BOTH branches, on this command's OWN `spawn_blocking` — never on the
/// sink writer thread, which has no access to `MetricsState` — and always AFTER
/// the log purge, so a `clear` failure cannot discard the log counts we already
/// earned. `metrics/` is the only addition; `settings.json` and everything
/// outside `logs/`, `exports/` and `metrics/` stay untouched.
///
/// On the §2.3 instrumentation exclusion list — deleting logs writes no record
/// into the surviving file.
#[tauri::command]
pub async fn logs_delete_all(
    app: tauri::AppHandle,
    app_state: tauri::State<'_, AppState>,
    obs_state: tauri::State<'_, ObsState>,
    metrics: tauri::State<'_, Arc<MetricsState>>,
) -> Result<LogsDeleteResult, AppError> {
    let dir = obs::logs_dir(&app)?;
    let exports = obs::exports_dir(&app)?;
    let perf: Arc<PerfState> = app_state.perf.clone();
    let metrics = Arc::clone(&metrics);
    let mut result = if let Some(sink) = obs_state.sink() {
        let exports_for_purge = exports.clone();
        // ALL-OR-NOTHING, deliberately (review 2026-09-14, SHOULD-FIX 5). The
        // second `?` returns before the metrics clear below, so a failed log purge
        // leaves the usage counts alone even though the row copy promises both.
        // That is the honest outcome, not an oversight:
        //  * every failure mode here means ZERO files were removed — `send`/`recv`
        //    fail only when the writer thread is gone, and the thread's own only
        //    `Err` is `writer.roll(true)`, which runs BEFORE `purge_scope`
        //    (`sink.rs`). So `deleteErrorText`'s "Couldn't delete the logs and
        //    usage counts." and the "Nothing was deleted." announcement are
        //    literally true, and a retry is available and idempotent;
        //  * clearing the counts anyway would perform the IRREVERSIBLE half of a
        //    destructive action under a message that says nothing happened — the
        //    same class of lie §6.4 forbids, pointing the other way. Usage counts
        //    are never exported, so nothing recovers them;
        //  * fabricating a partial `LogsDeleteResult` to keep going would have to
        //    invent `failed_files: 1`, whose signed copy blames another program
        //    holding the file — a cause that is false here.
        // The reverse asymmetry (a `clear` failure must not discard log counts) is
        // the contract's §6 rule and lives in `merge_metrics_counts`.
        let reply = tauri::async_runtime::spawn_blocking(move || {
            sink.roll_and_purge(exports_for_purge)
        })
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))??;
        LogsDeleteResult {
            deleted_files: reply.deleted_files,
            deleted_bytes: reply.deleted_bytes,
            failed_files: reply.failed_files,
            active_file: Some(reply.active_file),
            rolled: true,
            deleted_exports: Some(reply.deleted_exports),
            deleted_metrics: 0,
            metrics_cleared: false,
        }
    } else {
        let counts = tauri::async_runtime::spawn_blocking(move || {
            writer::purge_scope(&dir, &exports, None)
        })
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?;
        LogsDeleteResult {
            deleted_files: counts.deleted_files,
            deleted_bytes: counts.deleted_bytes,
            failed_files: counts.failed_files,
            active_file: None,
            rolled: false,
            deleted_exports: Some(counts.deleted_exports),
            deleted_metrics: 0,
            metrics_cleared: false,
        }
    };
    let cleared = tauri::async_runtime::spawn_blocking(move || {
        metrics.clear(&perf.snapshot(), writer::now_secs(), ClearMode::DeleteFiles)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?;
    merge_metrics_counts(&mut result, cleared.ok());
    Ok(result)
}

/// §F6 — folds the metrics half of a delete into the log half. Extracted from the
/// command so the honesty rules below are unit-testable without a `tauri::State`.
///
/// Three rules, each of which has a way of being got wrong:
/// * the metrics files are ALSO counted in `deleted_files`/`deleted_bytes`, like
///   export zips — `deleted_metrics` is a BREAKDOWN, not an addition the UI has
///   to sum itself;
/// * `metrics_cleared` tracks `dir_removed`, not the file count: on a launch
///   younger than the first 60 s flush nothing is on disk yet while the aggregate
///   is live, so `deleted_metrics == 0` is a full success;
/// * `None` (the clear itself failed) must NOT discard the log-purge counts we
///   already earned — it reports one more failure and leaves `metrics_cleared`
///   false. The whole command never fails for it.
pub(super) fn merge_metrics_counts(
    result: &mut LogsDeleteResult,
    cleared: Option<MetricsClearCounts>,
) {
    let Some(c) = cleared else {
        result.failed_files = result.failed_files.saturating_add(1);
        result.metrics_cleared = false;
        return;
    };
    result.deleted_files = result.deleted_files.saturating_add(c.deleted_files);
    result.deleted_bytes = result.deleted_bytes.saturating_add(c.deleted_bytes);
    result.failed_files = result.failed_files.saturating_add(c.failed_files);
    result.deleted_metrics = c.deleted_files;
    result.metrics_cleared = c.dir_removed;
}
