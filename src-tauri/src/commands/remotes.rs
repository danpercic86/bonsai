//! `remotes` commands — split from the former monolithic `commands.rs`.

use super::shared::*;
use bonsai_core::git::exec::SpawnGitExec;

/// P85 A3: completion event for the fire-and-forget fetch tag auto-sync. Emitted
/// (alongside a `repo-changed{reason:"tags"}`) ONLY when the pass actually
/// changed local tags (adopted or moved), so the frontend can surface the P84
/// count toast without the fetch response having to wait on the 2nd network
/// round-trip. camelCase on the wire, mirroring `RepoChangedPayload`.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TagAutoSyncEvent {
    repo_id: String,
    report: bonsai_core::git::tag_sync::TagAutoSyncReport,
}

/// Fetches every configured remote, sequentially, fail-fast (M6 contract
/// §2.4/§9). Errors: `noRemote` | `authFailed` | `networkError` | `git`
/// | `noRepo`. Does NOT emit `repo-changed` on the response path — the frontend
/// refetches imperatively. P85 A3: the P84 tag auto-sync runs FIRE-AND-FORGET
/// off the response (its own 2nd network round-trip no longer serializes the
/// fetch); on a real tag change it emits `repo-changed{reason:"tags"}` (refresh)
/// + `tag-auto-sync` (count toast). Its temp-ref churn under
/// `refs/bonsai-tagsync/**` is ignored by the watcher (`watcher.rs::is_relevant`).
#[tauri::command]
pub async fn fetch(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<FetchResult, AppError> {
    let result = fetch_inner(state.inner(), &repo_id).await?;
    // P85 A3: best-effort automatic tag reconciliation, OFF the response path.
    // Mirrors the fire-and-forget commit-graph write below — `spawn_blocking`,
    // never awaited, so `fetch` returns after the single `fetch_all` round-trip.
    // NEVER fails the fetch — `auto_sync_tags` swallows no-remote/auth/network.
    // Only a genuine change (adopted or moved a local tag) emits — a no-op or
    // skipped-only pass is silent (AC-A3c).
    if let Ok(path) = repo_path(state.inner(), &repo_id) {
        let emit_app = app.clone();
        let evt_repo = repo_id.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let Ok(report) = bonsai_core::git::tag_sync::auto_sync_tags(&path, None) else {
                return;
            };
            if report.adopted.is_empty() && report.moved.is_empty() {
                return;
            }
            let _ = emit_app.emit(
                "repo-changed",
                super::repo::RepoChangedPayload {
                    repo_id: evt_repo.clone(),
                    reason: "tags".to_string(),
                },
            );
            let _ = emit_app.emit(
                "tag-auto-sync",
                TagAutoSyncEvent {
                    repo_id: evt_repo,
                    report,
                },
            );
        });
    }
    // P52: when refs actually advanced, (re)write the commit-graph off the
    // response path (fire-and-forget, best-effort, never awaited — the fetch
    // result returns immediately regardless). Gated on `updated_refs > 0` so a
    // no-op fetch does not pay a pointless full rewrite.
    if result.remotes.iter().any(|r| r.updated_refs > 0) {
        if let Ok(path) = repo_path(state.inner(), &repo_id) {
            tauri::async_runtime::spawn_blocking(move || {
                let _ = bonsai_core::git::maintenance::write_commit_graph_best_effort(&path);
            });
        }
    }
    Ok(result)
}

/// Runtime-free core of `fetch` (unit-testable without a Tauri app).
pub(crate) async fn fetch_inner(state: &AppState, repo_id: &str) -> Result<FetchResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || fetch_all(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Fetches the upstream's remote + fast-forwards ONLY (M6 contract §2.5).
/// Errors: `noUpstream` | `authFailed` | `networkError` | `checkoutConflict`
/// | `git` | `noRepo`.
#[tauri::command]
pub async fn pull(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<PullResult, AppError> {
    let result = pull_inner(state.inner(), &repo_id).await?;
    // P52: pull is user-initiated + low frequency, so (re)write the
    // commit-graph unconditionally on success (a no-op rewrite is harmless).
    // Fire-and-forget, best-effort, never awaited — the pull result returns
    // immediately regardless.
    if let Ok(path) = repo_path(state.inner(), &repo_id) {
        tauri::async_runtime::spawn_blocking(move || {
            let _ = bonsai_core::git::maintenance::write_commit_graph_best_effort(&path);
        });
    }
    Ok(result)
}

/// Runtime-free core of `pull` (unit-testable without a Tauri app).
pub(crate) async fn pull_inner(state: &AppState, repo_id: &str) -> Result<PullResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || pull_ff(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Pushes the current branch to its upstream — or origin/<branch> + set
/// upstream when none (M6 contract §2.6). Never force. `skip_hooks` (P59a-2):
/// `true` ≡ `git push --no-verify` — otherwise the `pre-push` hook runs before
/// the push. Errors: `noRemote` | `authFailed` | `networkError` |
/// `pushRejected` | `hookRejected` | `git` | `noRepo`.
#[tauri::command]
pub async fn push(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    skip_hooks: Option<bool>,
) -> Result<PushResult, AppError> {
    push_inner(state.inner(), &repo_id, skip_hooks).await
}

/// Runtime-free core of `push` (unit-testable without a Tauri app).
pub(crate) async fn push_inner(
    state: &AppState,
    repo_id: &str,
    skip_hooks: Option<bool>,
) -> Result<PushResult, AppError> {
    let path = repo_path(state, repo_id)?;
    let skip = skip_hooks.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || push_current(&path, &SpawnGitExec, skip))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Force-push the current branch to its upstream WITH A LEASE (P37 + P59b). The
/// push runs through the git binary so git performs its atomic
/// `--force-with-lease` server-side check (closes P37's client-side TOCTOU);
/// refuses if the remote moved since the last fetch. `skip_hooks` (P59a-2):
/// `true` ≡ `--no-verify` — otherwise the `pre-push` hook runs before the push.
/// Errors: `noUpstream` | `authFailed` | `networkError` | `pushRejected` |
/// `hookRejected` | `git` | `noRepo`.
#[tauri::command]
pub async fn force_push(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    skip_hooks: Option<bool>,
) -> Result<PushResult, AppError> {
    force_push_inner(state.inner(), &repo_id, skip_hooks).await
}

/// Runtime-free core of `force_push` (unit-testable without a Tauri app).
pub(crate) async fn force_push_inner(
    state: &AppState,
    repo_id: &str,
    skip_hooks: Option<bool>,
) -> Result<PushResult, AppError> {
    let path = repo_path(state, repo_id)?;
    let skip = skip_hooks.unwrap_or(false);
    // P59b: the push runs through the git binary for git's atomic
    // `--force-with-lease` (closes P37's client-side TOCTOU). P59a-2: the
    // pre-push hook (also via the git binary) runs first unless skipped.
    tauri::async_runtime::spawn_blocking(move || force_push_with_lease(&path, &SpawnGitExec, skip))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Lists configured remotes (name + fetch URL, P22 contract §3.2). Errors:
/// `noRepo` | `git`.
#[tauri::command]
pub async fn list_remotes(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<RemoteInfo>, AppError> {
    list_remotes_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_remotes` (unit-testable without a Tauri app).
pub(crate) async fn list_remotes_inner(state: &AppState, repo_id: &str) -> Result<Vec<RemoteInfo>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || list_remotes_core(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Adds a remote (P22 contract §3.2). Errors: `noRepo` | `invalidName` | `git`.
/// Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn add_remote(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
    url: String,
) -> Result<(), AppError> {
    add_remote_inner(state.inner(), &repo_id, name, url).await
}

/// Runtime-free core of `add_remote` (unit-testable without a Tauri app).
pub(crate) async fn add_remote_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
    url: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || add_remote_core(&path, &name, &url))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Removes a remote and its remote-tracking refs (P22 contract §3.2). Errors:
/// `noRepo` | `noRemote` | `git`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn remove_remote(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    remove_remote_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `remove_remote` (unit-testable without a Tauri app).
pub(crate) async fn remove_remote_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || remove_remote_core(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Renames a remote (P22 contract §3.2). Errors:
/// `noRepo` | `noRemote` | `invalidName` | `git`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn rename_remote(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
    new_name: String,
) -> Result<(), AppError> {
    rename_remote_inner(state.inner(), &repo_id, name, new_name).await
}

/// Runtime-free core of `rename_remote` (unit-testable without a Tauri app).
pub(crate) async fn rename_remote_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
    new_name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || rename_remote_core(&path, &name, &new_name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Sets a remote's fetch URL (P22 contract §3.2). Errors:
/// `noRepo` | `noRemote` | `git`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn set_remote_url(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
    url: String,
) -> Result<(), AppError> {
    set_remote_url_inner(state.inner(), &repo_id, name, url).await
}

/// Runtime-free core of `set_remote_url` (unit-testable without a Tauri app).
pub(crate) async fn set_remote_url_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
    url: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || set_remote_url_core(&path, &name, &url))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
