//! `signing` command (P58a) — read-only signing status for the commit-box
//! indicator/toggle. Read-only ⇒ NO `repo-changed` emit. House shape mirrors
//! `search_commits`.

use super::shared::*;
use bonsai_core::git::exec::SpawnGitExec;
use bonsai_core::git::signing::{self, SigningStatus, VerifyResults};

/// Effective signing config for the commit-box indicator/toggle (P58 D6):
/// `commit.gpgsign` (enabled) + `gpg.format` + whether `user.signingkey` is set.
/// Read-only — does NOT emit `repo-changed`. Errors: `git` | `noRepo`.
#[tauri::command]
pub async fn signing_status(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<SigningStatus, AppError> {
    signing_status_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `signing_status` (unit-testable without a Tauri app).
pub(crate) async fn signing_status_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<SigningStatus, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || signing::signing_status(&workdir))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Verify signatures for a bounded set of commit oids (P58b) — the visible graph
/// rows. Read-only — does NOT emit `repo-changed`. ONE `git log` subprocess,
/// capped at `MAX_VERIFY_BATCH`; non-hex oids are dropped and unresolvable ones
/// omitted, and a missing gpg/ssh toolchain degrades to `cannotCheck` rather
/// than failing. Errors: `git` | `noRepo`.
#[tauri::command]
pub async fn verify_commits(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oids: Vec<String>,
) -> Result<VerifyResults, AppError> {
    verify_commits_inner(state.inner(), &repo_id, oids).await
}

/// Runtime-free core of `verify_commits` (unit-testable without a Tauri app).
pub(crate) async fn verify_commits_inner(
    state: &AppState,
    repo_id: &str,
    oids: Vec<String>,
) -> Result<VerifyResults, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        signing::verify_commits(&SpawnGitExec, &workdir, &oids)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
