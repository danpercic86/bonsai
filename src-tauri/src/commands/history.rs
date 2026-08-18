//! `history` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Per-line blame of `path` as of `at_oid` (`null`/omitted -> HEAD, P23
/// contract §9.1/§10). Errors: `other` (bad path) | `git` | `noRepo`. Does NOT
/// emit `repo-changed`.
#[tauri::command]
pub async fn blame_file(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    at_oid: Option<String>,
) -> Result<Vec<BlameLine>, AppError> {
    blame_file_inner(state.inner(), &repo_id, path, at_oid).await
}

/// Runtime-free core of `blame_file` (unit-testable without a Tauri app).
pub(crate) async fn blame_file_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    at_oid: Option<String>,
) -> Result<Vec<BlameLine>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || blame::blame_file(&workdir, &path, at_oid.as_deref()))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Commits that modified `path`, newest-first, best-effort following a single
/// rename (P23 contract §9.2/§10). `limit == 0` -> the built-in `MAX_HISTORY`
/// cap. Errors: `other` (bad path) | `git` | `noRepo`. Does NOT emit
/// `repo-changed`.
#[tauri::command]
pub async fn file_history(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    limit: u32,
) -> Result<Vec<FileHistoryEntry>, AppError> {
    file_history_inner(state.inner(), &repo_id, path, limit).await
}

/// Runtime-free core of `file_history` (unit-testable without a Tauri app).
pub(crate) async fn file_history_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    limit: u32,
) -> Result<Vec<FileHistoryEntry>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || blame::file_history(&workdir, &path, limit))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Reflog for `ref_name` ("HEAD" or a local branch name), newest-first, capped
/// at `MAX_REFLOG_ENTRIES`. A never-updated ref yields `[]` (not an error).
/// Read-only (P38 contract §5.1). Errors: `git` | `noRepo`. Does NOT emit
/// `repo-changed`.
#[tauri::command]
pub async fn read_reflog(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    ref_name: String,
) -> Result<Vec<ReflogEntry>, AppError> {
    read_reflog_inner(state.inner(), &repo_id, ref_name).await
}

/// Runtime-free core of `read_reflog` (unit-testable without a Tauri app).
pub(crate) async fn read_reflog_inner(
    state: &AppState,
    repo_id: &str,
    ref_name: String,
) -> Result<Vec<ReflogEntry>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || reflog::read_reflog(&workdir, &ref_name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Build/refresh the per-commit semantic-search INDEX (BM25 over message+diff),
/// streaming `IndexProgress` over `on_progress` (channel command, mirrors
/// `clone_repo`). CPU-heavy diff walk ⇒ `spawn_blocking`. Incremental. Writes to
/// the app data dir keyed by repo — NOT the repo — so it does NOT emit
/// `repo-changed`, and is NOT AI-gated (P57a contract §4). Rejects git | io | noRepo.
#[tauri::command]
pub async fn history_index_build(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    on_progress: tauri::ipc::Channel<IndexProgress>,
) -> Result<IndexStatus, AppError> {
    let base = app_data_root(&app)?;
    history_index_build_inner(state.inner(), &base, &repo_id, move |p| {
        // A send failure means the frontend dropped the channel — ignore it,
        // the build completes and the final IndexStatus still resolves.
        let _ = on_progress.send(p);
    })
    .await
}

/// Runtime-free core of `history_index_build` (unit-testable without a Tauri
/// app): the AppHandle-derived data dir arrives as `base` and the Channel is
/// abstracted to a plain progress callback (mirrors `ai_search_history_inner`).
pub(crate) async fn history_index_build_inner(
    state: &AppState,
    base: &std::path::Path,
    repo_id: &str,
    on_progress: impl Fn(IndexProgress) + Send + 'static,
) -> Result<IndexStatus, AppError> {
    let workdir = repo_path(state, repo_id)?;
    let base = base.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        // F-T5-4 (audit #2 §3.2): a truncated loose object hangs the extraction
        // walk forever, wedging the indexer thread permanently. The inactivity-
        // deadline wrapper (each IndexProgress event ticks liveness) turns that
        // into a clean error; the wedged worker is abandoned. If an abandoned
        // worker later resumes it may still `store::save` — benign: the store
        // uses unique-tmp + atomic rename (F-A9-1), so the worst case is a
        // briefly stale last-writer-wins index corrected by the next build.
        bonsai_core::git::timeout::run_with_git_timeout("history_index_build", move |progress| {
            let dir = history_index::index_dir_for_repo(&base, &workdir);
            history_index::build_index(&workdir, &dir, move |p| {
                progress.tick();
                on_progress(p);
            })
        })
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Cheap status of the persisted index (built?, count, staleness vs current
/// refs). Read-only; NOT AI-gated; does NOT emit `repo-changed` (P57a §4).
/// Rejects git | noRepo.
#[tauri::command]
pub async fn history_index_status(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<IndexStatus, AppError> {
    let base = app_data_root(&app)?;
    history_index_status_inner(state.inner(), &base, &repo_id).await
}

/// Runtime-free core of `history_index_status` (unit-testable without a Tauri
/// app); `base` is the AppHandle-derived app-data dir (mirrors
/// `ai_search_history_inner`).
pub(crate) async fn history_index_status_inner(
    state: &AppState,
    base: &std::path::Path,
    repo_id: &str,
) -> Result<IndexStatus, AppError> {
    let workdir = repo_path(state, repo_id)?;
    let base = base.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let dir = history_index::index_dir_for_repo(&base, &workdir);
        history_index::index_status(&workdir, &dir)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Relevance-ranked retrieval over the persisted index (BM25). Pure IR — read-
/// only, touches NO git objects, does NOT emit `repo-changed`, and is NOT
/// AI-gated (P57b contract §4 / OQ5). Empty/whitespace `text` ⇒ empty hits; a
/// missing index ⇒ `{ hits: [], indexStale: true, indexedCommits: 0 }` (the UI
/// offers Build). Mirrors `history_index_status`'s shape. Rejects io | noRepo.
#[tauri::command]
pub async fn history_search(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    query: HistoryQuery,
) -> Result<HistorySearchResults, AppError> {
    let base = app_data_root(&app)?;
    history_search_inner(state.inner(), &base, &repo_id, query).await
}

/// Runtime-free core of `history_search` (unit-testable without a Tauri app);
/// `base` is the AppHandle-derived app-data dir (mirrors
/// `ai_search_history_inner`).
pub(crate) async fn history_search_inner(
    state: &AppState,
    base: &std::path::Path,
    repo_id: &str,
    query: HistoryQuery,
) -> Result<HistorySearchResults, AppError> {
    let workdir = repo_path(state, repo_id)?;
    let base = base.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let dir = history_index::index_dir_for_repo(&base, &workdir);
        history_index::search_history(&workdir, &dir, &query)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// AI semantic-history answer: retrieves the top-K relevant commits from the
/// persisted index, then synthesizes an NL answer grounded in the REAL diffs of
/// those commits via the local `claude` CLI (P57c contract §4). The C3 consent-
/// gate TRIPLE — loads settings and REFUSES with `AiUnavailable` unless
/// `ai_enabled && ai_consented` (enforced in `_inner`, BEFORE `repo_path`).
/// Read-only; WRITES NOTHING; does NOT emit `repo-changed`. Errors:
/// `aiUnavailable` | `aiFailed` (no index / no relevant commits / CLI error) |
/// `git` (bad oid) | `noRepo`.
#[tauri::command]
pub async fn ai_search_history(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    question: String,
    top_k: u32,
) -> Result<HistoryAnswer, AppError> {
    // Resolve the settings-file path + app-data base at the AppHandle boundary so
    // the inner stays runtime-free and unit-testable (mirrors the `ai_*` triple),
    // then delegate.
    let file = settings::settings_file(&app)?;
    let base = app_data_root(&app)?;
    ai_search_history_inner(state.inner(), &file, &base, &repo_id, question, top_k).await
}

/// Runtime-free core of `ai_search_history` (unit-testable without a Tauri app).
/// The consent gate is enforced HERE, BEFORE `repo_path` (C3). `top_k` is capped
/// at `MAX_TOP_K`; the `0` sentinel flows through UNCHANGED so the core resolves
/// it to `DEFAULT_TOP_K` (the "default depth" contract honored by
/// `search_history`/`answer_history`), NOT to a single commit.
pub(crate) async fn ai_search_history_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    base: &std::path::Path,
    repo_id: &str,
    question: String,
    top_k: u32,
) -> Result<HistoryAnswer, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled or not yet consented to".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    let base = base.to_path_buf();
    let top_k = resolve_ai_top_k(top_k);
    tauri::async_runtime::spawn_blocking(move || {
        let dir = history_index::index_dir_for_repo(&base, &workdir);
        ai_history::answer_history(&workdir, &dir, &question, top_k, RunOpts::default())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Resolve the wire `top_k` for the AI history path: cap at
/// [`history_index::MAX_TOP_K`] and let the `0` "default depth" sentinel flow
/// through UNCHANGED (the core then resolves `0` ⇒ `DEFAULT_TOP_K` via
/// `effective_top_k`). Deliberately NOT a `clamp(1, MAX)`: a min-1 lower bound
/// would turn the UI's `topK: 0` into `1` and ground the AI answer on only the
/// single top commit instead of the top ~`DEFAULT_TOP_K` (P57c reviewer MUST-FIX).
fn resolve_ai_top_k(top_k: u32) -> usize {
    top_k.min(history_index::MAX_TOP_K) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P57c regression: `resolve_ai_top_k` must pass the `0` "default depth"
    /// sentinel through UNCHANGED (⇒ `DEFAULT_TOP_K` in the core), NOT collapse it
    /// to `1`. This FAILS against the prior `clamp(1, MAX_TOP_K)` (which returned
    /// `1` for `0`, grounding the AI answer on a single commit); it passes once the
    /// lower bound is dropped.
    #[test]
    fn resolve_ai_top_k_zero_is_default_sentinel_not_one() {
        assert_eq!(
            resolve_ai_top_k(0),
            0,
            "the 0 sentinel must flow through (⇒ DEFAULT_TOP_K in the core), not clamp to 1"
        );
        assert_eq!(resolve_ai_top_k(5), 5, "an explicit depth is preserved");
        assert_eq!(
            resolve_ai_top_k(9_999),
            history_index::MAX_TOP_K as usize,
            "an oversized depth is capped at MAX_TOP_K"
        );
    }
}
