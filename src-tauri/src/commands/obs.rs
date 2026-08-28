//! P91 §6 — the observability commands (increment 1 subset): `log_append`,
//! `log_session_info`, `log_reveal_dir`, `log_export_session`.
//!
//! `logs_delete_all` (§6.1) and the metrics commands (§8) belong to later
//! increments and are deliberately absent.
//!
//! Every one of these is on the §2.3 instrumentation EXCLUSION list — logging
//! about logging self-amplifies — which is why nothing here mints a record.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use bonsai_core::error::AppError;

use crate::obs::metrics::MetricsFile;
use crate::obs::record::LogPayload;
use crate::obs::{self, record::LogRecord, record::RedactionMode, writer, MetricsState, ObsState};
use crate::perf::PerfState;
use crate::state::AppState;

/// §6 — what the Dev page shows about the current log session.
///
/// `salt` is NOT in the contract's §6 TS interface but IS required by §7.2 ("the
/// frontend gets the same salt at boot via `log_session_info`"): without it
/// `src/obs/redact.ts` cannot produce the same `ref#3` as Rust and the two halves
/// of a file would disagree. It is safe to hand out — the salt exists precisely so
/// that ordinals cannot be correlated ACROSS sessions, and this command is on the
/// instrumentation exclusion list, so the salt is never itself logged.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSessionInfo {
    /// Empty while Dev mode is off (no session exists).
    pub session_id: String,
    /// Absolute path of the logs directory (shown in the UI, used by "Reveal").
    pub dir: String,
    /// File NAMES of the CURRENT session's parts (empty when Dev mode is off).
    pub files: Vec<String>,
    /// Bytes of `files`.
    pub bytes: u64,
    /// Records accepted by the sink this session.
    pub records: u64,
    /// Anomaly records the detector has emitted this session (§5).
    pub anomalies: u64,
    /// Records discarded by backpressure this session (§6).
    pub dropped: u64,
    pub redaction: RedactionMode,
    /// Hex of the 16-byte session salt (§7.2). Empty while Dev mode is off.
    pub salt: String,
    /// ALL log files on disk, not just this session — the §6.1 confirm copy
    /// needs it.
    pub total_files: u32,
    pub total_bytes: u64,
    /// §6.2 — export zips inside the purge scope, so the delete-confirm copy can
    /// name them. `None` when the exports directory does not exist yet, which is
    /// distinct from "exists and is empty" (`Some(0)`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_files: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_bytes: Option<u64>,
}

/// Appends a batch of frontend records (§6 "Frontend → file").
///
/// A no-op when Dev mode is off: the frontend's own gate should have prevented
/// the call, but a racing toggle must not error. Enqueue never blocks — a full
/// queue drops and is accounted for by a `drop` record — so this command does no
/// IO and needs no `spawn_blocking`.
#[tauri::command]
pub async fn log_append(
    obs_state: tauri::State<'_, ObsState>,
    metrics: tauri::State<'_, Arc<MetricsState>>,
    records: Vec<LogRecord>,
) -> Result<(), AppError> {
    let Some(sink) = obs_state.sink() else {
        return Ok(());
    };
    // §8: fold every `ipc.result` into the durable metrics store as it passes
    // through. In-memory ONLY — `observe_ipc_result` never touches disk, so this
    // command keeps its no-IO / never-blocks guarantee.
    let today = writer::utc_date(writer::now_secs());
    // Signal the batch boundary to the detector (§5 `unbatched-sink`) BEFORE the
    // records, so its refs point at the first record of this batch.
    sink.note_batch();
    for rec in records {
        if let LogPayload::IpcResult {
            cmd, ms, err_code, ..
        } = &rec.payload
        {
            metrics.observe_ipc_result(cmd, *ms, err_code.as_deref(), &today);
        }
        sink.enqueue(rec);
    }
    Ok(())
}

/// §8 read API for the future Statistics page. Folds the latest `perf.*` deltas
/// first so the snapshot reflects current counters, then returns the aggregates
/// with DERIVED `p50Ms`/`p95Ms` filled — those are NEVER persisted to disk.
///
/// On the §2.3 instrumentation exclusion list (the Dev/Stats page polls it).
#[tauri::command]
pub async fn metrics_snapshot(
    app_state: tauri::State<'_, AppState>,
    metrics: tauri::State<'_, Arc<MetricsState>>,
) -> Result<MetricsFile, AppError> {
    let perf: Arc<PerfState> = app_state.perf.clone();
    let metrics = Arc::clone(&metrics);
    tauri::async_runtime::spawn_blocking(move || {
        let today = writer::utc_date(writer::now_secs());
        metrics.fold_perf(&perf.snapshot(), &today);
        metrics.snapshot()
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))
}

/// §8 `metrics_reset` — clears every local aggregate and persists the empty file.
///
/// **Headless by design:** this command exists but appears in NO settings catalog
/// row — there is no UI affordance for it in P91 (a Statistics page is future
/// work). Blocking (writes the file), so it runs on the blocking pool.
#[tauri::command]
pub async fn metrics_reset(
    metrics: tauri::State<'_, Arc<MetricsState>>,
) -> Result<(), AppError> {
    let metrics = Arc::clone(&metrics);
    tauri::async_runtime::spawn_blocking(move || metrics.reset(writer::now_secs()))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// §6 — current session summary + the totals the delete-confirm copy needs.
///
/// Scans the logs directory, so it runs on `spawn_blocking`.
#[tauri::command]
pub async fn log_session_info(
    app: tauri::AppHandle,
    obs_state: tauri::State<'_, ObsState>,
) -> Result<LogSessionInfo, AppError> {
    let dir = obs::logs_dir(&app)?;
    let exports = obs::exports_dir(&app)?;
    let session = obs_state.sink().map(|s| {
        // REQUESTS a flush (non-blocking `try_send`) so the byte counts are as
        // fresh as they can be without stalling the command. They can therefore
        // lag the buffer by up to one flush window (1 s); the next poll corrects
        // it, and the delete-confirm dialog re-reads this immediately before
        // asking, so the number the user consents to is never stale by more than
        // that window.
        s.request_flush();
        (
            s.session_id().to_string(),
            s.redactor().salt_hex(),
            s.redaction(),
            s.accepted(),
            s.dropped(),
            s.anomalies(),
        )
    });
    tauri::async_runtime::spawn_blocking(move || {
        let all = writer::list_log_files(&dir);
        let total_files = all.len() as u32;
        let total_bytes = all.iter().map(|(_, b)| *b).sum();
        let (session_id, salt, redaction, records, dropped, anomalies) =
            session.unwrap_or_else(|| {
                (String::new(), String::new(), RedactionMode::Strict, 0, 0, 0)
            });
        let (files, bytes) = if session_id.is_empty() {
            (Vec::new(), 0)
        } else {
            let mine: Vec<(String, u64)> = all
                .into_iter()
                .filter(|(n, _)| n.contains(&session_id))
                .collect();
            let bytes = mine.iter().map(|(_, b)| *b).sum();
            (mine.into_iter().map(|(n, _)| n).collect(), bytes)
        };
        // §6.2: export zips are in the purge scope, so their count/size belong in
        // the same summary the confirm copy is built from.
        let (export_files, export_bytes) = count_exports(&exports);
        LogSessionInfo {
            session_id,
            dir: dir.to_string_lossy().to_string(),
            files,
            bytes,
            records,
            anomalies,
            dropped,
            redaction,
            salt,
            total_files,
            total_bytes,
            export_files,
            export_bytes,
        }
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))
}

/// Counts the `*.zip` exports Bonsai created in `<app_config_dir>/exports`
/// (§6.2). Returns `(None, None)` when the directory does not exist yet — the UI
/// distinguishes "no exports directory" from "an empty one".
pub(crate) fn count_exports(exports_dir: &Path) -> (Option<u32>, Option<u64>) {
    let Ok(rd) = std::fs::read_dir(exports_dir) else {
        return (None, None);
    };
    let mut files = 0u32;
    let mut bytes = 0u64;
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !name.ends_with(".zip") {
            continue;
        }
        files += 1;
        bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
    }
    (Some(files), Some(bytes))
}

/// §10 `dev.reveal-logs` — open the logs folder in the OS file manager.
///
/// Creates the directory first: revealing a path that does not exist yet fails
/// on every platform, and "I turned Dev mode on but never logged" is a normal
/// state.
#[tauri::command]
pub async fn log_reveal_dir(app: tauri::AppHandle) -> Result<(), AppError> {
    let dir = obs::logs_dir(&app)?;
    let for_create = dir.clone();
    tauri::async_runtime::spawn_blocking(move || std::fs::create_dir_all(&for_create))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
        .map_err(|e| AppError::Io(format!("cannot create log dir: {e}")))?;
    super::external::reveal_in_file_manager(dir.to_string_lossy().to_string()).await
}

/// §10 `dev.export-session` — zip the current session's parts, return the path.
///
/// **Falls back to the most recent session on disk when Dev mode is off.** That
/// is not a liberty: the UI contract's user story is *enable → reproduce → turn
/// Dev mode OFF → find the file → send it*, so refusing to export without a live
/// session would break the milestone's own workflow at its second-to-last step.
#[tauri::command]
pub async fn log_export_session(
    app: tauri::AppHandle,
    obs_state: tauri::State<'_, ObsState>,
    dest: Option<String>,
) -> Result<String, AppError> {
    let dir = obs::logs_dir(&app)?;
    let exports = obs::exports_dir(&app)?;
    if let Some(s) = obs_state.sink() {
        // Ask the writer to push its buffer before we read the files back. This
        // is a non-blocking `try_send`, so a record written microseconds ago can
        // still be inside the 1 s flush window and miss the zip; the alternative
        // — a blocking round-trip inside a command the UI awaits — is worse, and
        // the user can simply export again.
        s.request_flush();
    }
    let session_id = obs_state.sink().map(|s| s.session_id().to_string());
    tauri::async_runtime::spawn_blocking(move || export_session(&dir, &exports, session_id, dest))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// The blocking half of [`log_export_session`], separated so it is testable
/// without a Tauri app.
pub(crate) fn export_session(
    dir: &Path,
    exports_dir: &Path,
    session_id: Option<String>,
    dest: Option<String>,
) -> Result<String, AppError> {
    let all = writer::list_log_files(dir);
    if all.is_empty() {
        return Err(AppError::Other("there are no log files to export".into()));
    }
    let group = match &session_id {
        Some(id) => all
            .iter()
            .find(|(n, _)| n.contains(id.as_str()))
            .map(|(n, _)| writer::session_group(n)),
        // Names begin with a sortable UTC stamp, so the last entry is newest.
        None => all.last().map(|(n, _)| writer::session_group(n)),
    }
    .ok_or_else(|| AppError::Other("there are no log files to export".into()))?;

    let parts: Vec<String> = all
        .into_iter()
        .filter(|(n, _)| writer::session_group(n) == group)
        .map(|(n, _)| n)
        .collect();

    // §6.2: the default destination is `exports/`, NEVER `logs/`. A zip left in
    // `logs/` would survive `logs_delete_all` (whose scope is `*.jsonl`), so a
    // user could "delete all log files" and still hold a raw-names archive —
    // exactly the failure decision 7 exists to prevent. A caller-supplied `dest`
    // is a privileged write and is taken verbatim: it is the result of the OS
    // save dialog, i.e. a path the user chose explicitly.
    let out: PathBuf = match dest {
        Some(d) => PathBuf::from(d),
        None => exports_dir.join(format!("{group}.zip")),
    };
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Io(format!("cannot create export folder: {e}")))?;
    }

    let file = std::fs::File::create(&out)
        .map_err(|e| AppError::Io(format!("cannot create export file: {e}")))?;
    let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
    let opts: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for name in parts {
        let bytes = match std::fs::read(dir.join(&name)) {
            Ok(b) => b,
            // A part that vanished mid-export (pruned, or deleted by the user)
            // must not fail the whole export of the parts that DO exist.
            Err(_) => continue,
        };
        zip.start_file(name, opts)
            .map_err(|e| AppError::Io(format!("cannot add log part to the zip: {e}")))?;
        std::io::Write::write_all(&mut zip, &bytes)
            .map_err(|e| AppError::Io(format!("cannot write log part to the zip: {e}")))?;
    }
    zip.finish()
        .map_err(|e| AppError::Io(format!("cannot finalize the zip: {e}")))?;
    Ok(out.to_string_lossy().to_string())
}
