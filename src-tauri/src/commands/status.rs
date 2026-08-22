//! `status` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Computes the working-directory status of `repo_id`.
#[tauri::command]
pub async fn get_status(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<StatusSnapshot, AppError> {
    get_status_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `get_status` (unit-testable without a Tauri app).
pub(crate) async fn get_status_inner(state: &AppState, repo_id: &str) -> Result<StatusSnapshot, AppError> {
    let path = repo_path(state, repo_id)?;
    // P86 instrumentation: this is the O(worktree) scan seam. `repo_opens` too —
    // `read_status` opens the repo from `path`.
    state.perf.inc_status_scans();
    state.perf.inc_repo_opens();
    // F-T5-4 (audit #2 §3.2): the HEAD peel inside `read_status` spins forever
    // on a truncated loose commit — the wrapper converts that into a clean error.
    tauri::async_runtime::spawn_blocking(move || {
        bonsai_core::git::timeout::run_with_git_timeout("read_status", move |_progress| {
            read_status(&path)
        })
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Computes the full commit-graph layout of `repo_id`.
///
/// Unborn-HEAD / zero-ref repos yield an empty layout (M2 contract §2.1),
/// not an error; `NoRepo` when nothing is open under that id.
#[tauri::command]
pub async fn get_graph(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<GraphLayout, AppError> {
    get_graph_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `get_graph` (unit-testable without a Tauri app).
///
/// P86 note: `get_graph` is intentionally NOT layout-cached. The cache stores
/// the STREAMING chunk stream (capped at `STREAM_MAX_COMMITS` = 1_000_000),
/// whereas the one-shot layout is capped at `MAX_COMMITS` = 100_000; a single
/// shared chunk cache cannot serve both caps without a stale/over-long result,
/// and `get_graph` is off the refresh hot path (the frontend uses `streamGraph`
/// — `get_graph` is retained for small-repo/tests/mock reuse). So it always
/// walks. See `graph_cache.rs` and the P86 report.
pub(crate) async fn get_graph_inner(state: &AppState, repo_id: &str) -> Result<GraphLayout, AppError> {
    let path = repo_path(state, repo_id)?;
    // P86 instrumentation: uncached ⇒ every call is a real walk + open.
    state.perf.inc_graph_walks();
    state.perf.inc_repo_opens();
    // F-T5-4 (audit #2 §3.2): the one-shot walk spins forever on a truncated
    // loose commit — the wrapper converts that into a clean error. No tick seam
    // (single-shot): the deadline bounds the WHOLE walk, generous for the
    // MAX_COMMITS-capped layout and overridable via BONSAI_GIT_TIMEOUT_MS.
    tauri::async_runtime::spawn_blocking(move || {
        bonsai_core::git::timeout::run_with_git_timeout("compute_graph", move |_progress| {
            compute_graph(&path)
        })
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Streams the commit-graph layout of `repo_id` as ordered [`GraphChunk`]
/// batches over `on_chunk` (channel command; mirrors `history_index_build` /
/// `clone_repo`). The heavy git2 walk runs in `spawn_blocking`.
///
/// Wire order: exactly one `Meta`, then N `Batch`, then exactly one `Done`.
/// Unborn / zero-ref repos yield a `Meta` + `Done` pair, NOT an error (parity
/// with `get_graph`). `get_graph` is retained (small-repo / tests / mock reuse).
/// Rejects `NoRepo` when nothing is open under that id; git errors surface as
/// `AppError` (the command rejects instead of sending `Done`).
#[tauri::command]
pub async fn stream_graph(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    on_chunk: tauri::ipc::Channel<GraphChunk>,
) -> Result<(), AppError> {
    // P86 B1: clone the workdir path AND the per-repo layout-cache handle out
    // together under one brief map lock, then hand both into the blocking pool.
    let (path, cache) = repo_path_and_graph_cache(state.inner(), &repo_id)?;
    let perf = state.inner().perf.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // F-T5-4 (audit #2 §3.2): a truncated loose object makes libgit2 spin
        // forever inside the walk, so the channel would never send `Done` and
        // the frontend would wait on a partial graph forever. The inactivity-
        // deadline wrapper turns that into a clean reject (each emitted chunk
        // ticks liveness; a wedged walk stops ticking and times out).
        bonsai_core::git::timeout::run_with_git_timeout("stream_graph", move |progress| {
            // Cache-aware: an unchanged-topology refresh replays (HitVerbatim) or
            // re-pills (HitRedecorate) the cached chunks with no revwalk; a real
            // topology change falls through to a full walk that repopulates the
            // cache. `Channel::send` errs once the frontend drops the channel
            // (unmount / repo switch / `close_repo`); `is_ok() == false` stops
            // the pass promptly with `Ok` (contract §6 cancellation).
            crate::graph_cache::stream_graph_cached(&path, &cache, &perf, |chunk| {
                progress.tick();
                on_chunk.send(chunk).is_ok()
            })
        })
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
