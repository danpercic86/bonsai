//! `search` command (P50a) — commit/content search. Read-only ⇒ NO
//! `repo-changed` emit. Split from the former monolithic `commands.rs`.

use super::shared::*;
use bonsai_core::git::search::{self, SearchQuery, SearchResults, SpawnGitRunner};

/// Commit/content search (P50 §4). Dispatches by `query.field`: message/author/
/// all via a header-only git2 revwalk; path/content via `git log` (`-- <path>`
/// / `-S`/`-G`). Capped at `MAX_SEARCH_RESULTS` with `truncated` set when more
/// may exist. Empty/whitespace `text` resolves to empty. Read-only — does NOT
/// emit `repo-changed`. Errors: `git` (bad pathspec / invalid `-G` regex) |
/// `noRepo`.
#[tauri::command]
pub async fn search_commits(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    query: SearchQuery,
) -> Result<SearchResults, AppError> {
    search_commits_inner(state.inner(), &repo_id, query).await
}

/// Runtime-free core of `search_commits` (unit-testable without a Tauri app).
pub(crate) async fn search_commits_inner(
    state: &AppState,
    repo_id: &str,
    query: SearchQuery,
) -> Result<SearchResults, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        search::search_commits(&workdir, &SpawnGitRunner, &query)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
