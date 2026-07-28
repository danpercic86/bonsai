use tauri::Emitter;

use crate::error::AppError;
use crate::git::branches::{self, BranchesSnapshot};
use crate::git::commit::{create_commit, CommitResult};
use crate::git::conflict::{self, ConflictEntry, ConflictFile, ConflictResolution};
use crate::git::diff::{commit_diff, commit_file_diff, workdir_file_diff, CommitDiff, FileDiff};
use crate::git::merge::{self, MergeOutcome};
use crate::git::opstate::{read_op_state, RepoOpState};
use crate::git::remote::{fetch_all, pull_ff, push_current, FetchResult, PullResult, PushResult};
use crate::git::repo::{read_repo_info, RepoInfo};
use crate::git::stage::{stage_paths, unstage_paths};
use crate::git::status::{read_status, StatusSnapshot};
use crate::graph::{compute_graph, GraphLayout};
use crate::settings::{self, clamp_pane_widths, ListView, PaneWidths, RecentRepo, ThemeChoice};
use crate::state::{AppState, OpenRepo};
use crate::watcher::spawn_watcher;

/// Payload of the `"repo-changed"` event. `reason` is `"fs"` in M1; future
/// reasons (e.g. `"op"` after a commit) reuse this event.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoChangedPayload {
    pub reason: String,
}

/// Opens the folder at `path` as a repository and reports its state.
///
/// Bare repositories are reported (`bare: true`) but NOT stored in state and
/// get no watcher — Bonsai v1 is a working-copy client (M1 contract §3.3).
/// The frontend treats `bare: true` like `isRepo: false`.
///
/// For non-bare repos this (re)starts the file watcher: any previous watcher
/// is dropped first, so re-invoking on the same path is idempotent and
/// self-heals a dead watcher (this is what the refresh button relies on).
///
/// Any `open_repo` call replaces the app's notion of "current repo": an
/// unsuccessful open (non-repo or bare) leaves NO repo open — both the stored
/// repo and the watcher are cleared, so `get_status` returns `NoRepo`.
#[tauri::command]
pub async fn open_repo(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<RepoInfo, AppError> {
    let emit_app = app.clone();
    let info = open_repo_inner(
        state.inner(),
        path,
        Box::new(move || {
            let _ = emit_app.emit(
                "repo-changed",
                RepoChangedPayload {
                    reason: "fs".to_string(),
                },
            );
        }),
    )
    .await?;

    // Recents hook (P1 contract §3.2): record every successful usable open.
    // Uses `info.path` (canonical workdir root), not the raw argument, so
    // "repo root" vs "subfolder" opens dedupe. Save failure is NON-FATAL —
    // the open itself succeeded.
    if info.is_repo && !info.bare {
        match settings::settings_file(&app) {
            Ok(file) => {
                let repo_path = info.path.clone();
                let saved = tauri::async_runtime::spawn_blocking(move || {
                    let mut s = settings::load_from(&file);
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                    settings::record_recent(&mut s, &repo_path, now);
                    settings::save_to(&file, &s)
                })
                .await;
                match saved {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        eprintln!("bonsai: failed to save recent repos (non-fatal): {e}");
                    }
                    Err(e) => {
                        eprintln!("bonsai: recent-repos task join error (non-fatal): {e}");
                    }
                }
            }
            Err(e) => {
                eprintln!("bonsai: cannot resolve settings file (non-fatal): {e}");
            }
        }
    }
    Ok(info)
}

/// Recent successfully-opened repos, most recent first, max 10. Never rejects
/// for a missing/corrupt settings file (`load_from` defaults); only
/// settings-path resolution can error (P1 contract §3.2).
#[tauri::command]
pub async fn get_recent_repos(app: tauri::AppHandle) -> Result<Vec<RecentRepo>, AppError> {
    let file = settings::settings_file(&app)?;
    tauri::async_runtime::spawn_blocking(move || settings::load_from(&file).recent_repos)
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))
}

/// Removes one recents entry (case-insensitive path match) and returns the
/// updated list (P1 contract §3.2).
#[tauri::command]
pub async fn remove_recent_repo(
    app: tauri::AppHandle,
    path: String,
) -> Result<Vec<RecentRepo>, AppError> {
    let file = settings::settings_file(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut s = settings::load_from(&file);
        s.recent_repos
            .retain(|r| !r.path.eq_ignore_ascii_case(&path));
        settings::save_to(&file, &s)?;
        Ok(s.recent_repos)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Combined UI settings surfaced to the frontend (P2 contract §2.2).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSettings {
    pub theme: ThemeChoice,
    pub pane_widths: PaneWidths,
    pub list_view: ListView,
}

/// Partial patch for `set_ui_settings` — only `Some(..)` fields are applied
/// (P2 contract §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSettingsPatch {
    pub theme: Option<ThemeChoice>,
    pub pane_widths: Option<PaneWidths>,
    pub list_view: Option<ListView>,
}

/// Pure patch application: only `Some(..)` fields of `patch` mutate `s`; pane
/// widths are clamped on write. Extracted from `set_ui_settings` so its
/// partial-update semantics are unit-testable without a Tauri app
/// (P2a contract §3.4.3).
fn apply_patch(s: &mut settings::Settings, patch: UiSettingsPatch) {
    if let Some(theme) = patch.theme {
        s.theme = theme;
    }
    if let Some(pane_widths) = patch.pane_widths {
        s.pane_widths = clamp_pane_widths(pane_widths);
    }
    if let Some(list_view) = patch.list_view {
        s.list_view = list_view;
    }
}

/// Current UI settings (theme + pane widths). Never rejects for a
/// missing/corrupt settings file (same as `get_recent_repos`); only
/// settings-path resolution can error.
#[tauri::command]
pub async fn get_ui_settings(app: tauri::AppHandle) -> Result<UiSettings, AppError> {
    let file = settings::settings_file(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let s = settings::load_from(&file);
        UiSettings {
            theme: s.theme,
            pane_widths: s.pane_widths,
            list_view: s.list_view,
        }
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))
}

/// Applies a partial patch (only `Some(..)` fields) to the persisted UI
/// settings and returns the resulting `UiSettings`. Save failure surfaces as
/// `AppError::Io` (NOT silently swallowed like the recents hook — the user
/// just took an explicit action, e.g. finished a drag or toggled the theme,
/// and silently losing it would be surprising).
#[tauri::command]
pub async fn set_ui_settings(
    app: tauri::AppHandle,
    patch: UiSettingsPatch,
) -> Result<UiSettings, AppError> {
    let file = settings::settings_file(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut s = settings::load_from(&file);
        apply_patch(&mut s, patch);
        settings::save_to(&file, &s)?;
        Ok(UiSettings {
            theme: s.theme,
            pane_widths: s.pane_widths,
            list_view: s.list_view,
        })
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Runtime-free core of `open_repo` (unit-testable without a Tauri app).
/// `on_change` is what the watcher fires on debounced filesystem changes; the
/// command wires it to an app-wide `"repo-changed"` emit.
async fn open_repo_inner(
    state: &AppState,
    path: String,
    on_change: Box<dyn Fn() + Send + 'static>,
) -> Result<RepoInfo, AppError> {
    let path_buf = std::path::PathBuf::from(&path);
    let info = tauri::async_runtime::spawn_blocking(move || read_repo_info(&path_buf))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))??;

    if info.is_repo && !info.bare {
        let workdir = std::path::PathBuf::from(&info.path);

        // Stop any previous watcher BEFORE storing the new repo path: the old
        // handle drops here, its debounce thread joins.
        {
            let mut watcher = state
                .watcher
                .lock()
                .map_err(|_| AppError::Other("state lock poisoned".to_string()))?;
            *watcher = None;
        }

        {
            let mut repo = state
                .repo
                .lock()
                .map_err(|_| AppError::Other("state lock poisoned".to_string()))?;
            *repo = Some(OpenRepo {
                path: workdir.clone(),
            });
        }

        // Watch failure is non-fatal (M1 contract §4): manual refresh + focus
        // rescan keep the app correct even without filesystem events.
        match spawn_watcher(&workdir, on_change) {
            Ok(handle) => {
                let mut watcher = state
                    .watcher
                    .lock()
                    .map_err(|_| AppError::Other("state lock poisoned".to_string()))?;
                *watcher = Some(handle);
            }
            Err(e) => {
                eprintln!("bonsai: file watcher failed to start (falling back to manual refresh): {e}");
            }
        }
    } else {
        // Unsuccessful open (non-repo or bare): the previous repo is no longer
        // "current". Drop the watcher first (its debounce thread joins), then
        // clear the stored repo so get_status returns NoRepo.
        {
            let mut watcher = state
                .watcher
                .lock()
                .map_err(|_| AppError::Other("state lock poisoned".to_string()))?;
            *watcher = None;
        }
        {
            let mut repo = state
                .repo
                .lock()
                .map_err(|_| AppError::Other("state lock poisoned".to_string()))?;
            *repo = None;
        }
    }
    Ok(info)
}

/// Computes the working-directory status of the currently open repository.
#[tauri::command]
pub async fn get_status(state: tauri::State<'_, AppState>) -> Result<StatusSnapshot, AppError> {
    get_status_inner(state.inner()).await
}

/// Runtime-free core of `get_status` (unit-testable without a Tauri app).
async fn get_status_inner(state: &AppState) -> Result<StatusSnapshot, AppError> {
    let path = {
        let repo = state
            .repo
            .lock()
            .map_err(|_| AppError::Other("state lock poisoned".to_string()))?;
        repo.as_ref().ok_or(AppError::NoRepo)?.path.clone()
    };

    tauri::async_runtime::spawn_blocking(move || read_status(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Computes the full commit-graph layout of the currently open repository.
///
/// Unborn-HEAD / zero-ref repos yield an empty layout (M2 contract §2.1),
/// not an error; `NoRepo` when nothing is open.
#[tauri::command]
pub async fn get_graph(state: tauri::State<'_, AppState>) -> Result<GraphLayout, AppError> {
    get_graph_inner(state.inner()).await
}

/// Runtime-free core of `get_graph` (unit-testable without a Tauri app).
async fn get_graph_inner(state: &AppState) -> Result<GraphLayout, AppError> {
    let path = {
        let repo = state
            .repo
            .lock()
            .map_err(|_| AppError::Other("state lock poisoned".to_string()))?;
        repo.as_ref().ok_or(AppError::NoRepo)?.path.clone()
    };

    tauri::async_runtime::spawn_blocking(move || compute_graph(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Path of the currently open repo, or `NoRepo`.
fn current_repo_path(state: &AppState) -> Result<std::path::PathBuf, AppError> {
    let repo = state
        .repo
        .lock()
        .map_err(|_| AppError::Other("state lock poisoned".to_string()))?;
    Ok(repo.as_ref().ok_or(AppError::NoRepo)?.path.clone())
}

/// Stages the given worktree-relative paths (atomic batch, M3 contract §2.2).
/// Does NOT emit `repo-changed` — the frontend refetches imperatively after
/// every successful mutation (§2.7).
#[tauri::command]
pub async fn stage(state: tauri::State<'_, AppState>, paths: Vec<String>) -> Result<(), AppError> {
    stage_inner(state.inner(), paths).await
}

/// Runtime-free core of `stage` (unit-testable without a Tauri app).
async fn stage_inner(state: &AppState, paths: Vec<String>) -> Result<(), AppError> {
    let path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || stage_paths(&path, &paths))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Unstages the given worktree-relative paths (atomic batch). Safe: the
/// worktree is never touched.
#[tauri::command]
pub async fn unstage(
    state: tauri::State<'_, AppState>,
    paths: Vec<String>,
) -> Result<(), AppError> {
    unstage_inner(state.inner(), paths).await
}

/// Runtime-free core of `unstage` (unit-testable without a Tauri app).
async fn unstage_inner(state: &AppState, paths: Vec<String>) -> Result<(), AppError> {
    let path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || unstage_paths(&path, &paths))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Creates a commit from the current index. Errors:
/// `emptyMessage` | `configMissing` | `nothingToCommit` | `git` | `noRepo`.
#[tauri::command]
pub async fn commit(
    state: tauri::State<'_, AppState>,
    message: String,
) -> Result<CommitResult, AppError> {
    commit_inner(state.inner(), message).await
}

/// Runtime-free core of `commit` (unit-testable without a Tauri app).
async fn commit_inner(state: &AppState, message: String) -> Result<CommitResult, AppError> {
    let path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || create_commit(&path, &message))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Diff of one working-dir file (M4 contract §2.2/§2.8).
/// `staged == false`: index vs workdir; `staged == true`: HEAD vs index.
/// `orig_path`: pass `StatusEntry.origPath` for renames.
#[tauri::command]
pub async fn get_workdir_file_diff(
    state: tauri::State<'_, AppState>,
    path: String,
    orig_path: Option<String>,
    staged: bool,
) -> Result<FileDiff, AppError> {
    get_workdir_file_diff_inner(state.inner(), path, orig_path, staged).await
}

/// Runtime-free core of `get_workdir_file_diff` (unit-testable without a Tauri app).
async fn get_workdir_file_diff_inner(
    state: &AppState,
    path: String,
    orig_path: Option<String>,
    staged: bool,
) -> Result<FileDiff, AppError> {
    let repo_path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || {
        workdir_file_diff(&repo_path, &path, orig_path.as_deref(), staged)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Commit details + per-file headers for `oid` vs its first parent
/// (M4 contract §2.2/§2.8). Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn get_commit_diff(
    state: tauri::State<'_, AppState>,
    oid: String,
) -> Result<CommitDiff, AppError> {
    get_commit_diff_inner(state.inner(), oid).await
}

/// Runtime-free core of `get_commit_diff` (unit-testable without a Tauri app).
async fn get_commit_diff_inner(state: &AppState, oid: String) -> Result<CommitDiff, AppError> {
    let repo_path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || commit_diff(&repo_path, &oid))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Hunks for ONE file of a commit's first-parent diff (M4 contract §2.2/§2.8).
#[tauri::command]
pub async fn get_commit_file_diff(
    state: tauri::State<'_, AppState>,
    oid: String,
    path: String,
    orig_path: Option<String>,
) -> Result<FileDiff, AppError> {
    get_commit_file_diff_inner(state.inner(), oid, path, orig_path).await
}

/// Runtime-free core of `get_commit_file_diff` (unit-testable without a Tauri app).
async fn get_commit_file_diff_inner(
    state: &AppState,
    oid: String,
    path: String,
    orig_path: Option<String>,
) -> Result<FileDiff, AppError> {
    let repo_path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || {
        commit_file_diff(&repo_path, &oid, &path, orig_path.as_deref())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// One snapshot of local branches + remote-tracking branches + tags + HEAD
/// (M5 contract §2.2/§2.8). Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn list_branches(
    state: tauri::State<'_, AppState>,
) -> Result<BranchesSnapshot, AppError> {
    list_branches_inner(state.inner()).await
}

/// Runtime-free core of `list_branches` (unit-testable without a Tauri app).
async fn list_branches_inner(state: &AppState) -> Result<BranchesSnapshot, AppError> {
    let path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || branches::list_refs(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Creates a local branch at the current HEAD commit — does NOT check out
/// (M5 contract §2.4). Errors: `invalidName` | `branchExists` | `git` | `noRepo`.
/// Does NOT emit `repo-changed` — the frontend refetches imperatively.
#[tauri::command]
pub async fn create_branch(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<(), AppError> {
    create_branch_inner(state.inner(), name).await
}

/// Runtime-free core of `create_branch` (unit-testable without a Tauri app).
async fn create_branch_inner(state: &AppState, name: String) -> Result<(), AppError> {
    let path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || branches::create_branch(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Safe checkout of a LOCAL branch (M5 contract §2.5 — never force).
/// Errors: `branchNotFound` | `checkoutConflict` | `git` | `noRepo`.
#[tauri::command]
pub async fn checkout_branch(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<(), AppError> {
    checkout_branch_inner(state.inner(), name).await
}

/// Runtime-free core of `checkout_branch` (unit-testable without a Tauri app).
async fn checkout_branch_inner(state: &AppState, name: String) -> Result<(), AppError> {
    let path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || branches::checkout_branch(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Deletes a LOCAL, fully merged, non-current branch (M5 contract §2.6 —
/// unmerged deletion is blocked; no force-delete in v1).
/// Errors: `branchNotFound` | `unmergedBranch` | `git` | `noRepo`.
#[tauri::command]
pub async fn delete_branch(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<(), AppError> {
    delete_branch_inner(state.inner(), name).await
}

/// Runtime-free core of `delete_branch` (unit-testable without a Tauri app).
async fn delete_branch_inner(state: &AppState, name: String) -> Result<(), AppError> {
    let path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || branches::delete_branch(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Fetches every configured remote, sequentially, fail-fast (M6 contract
/// §2.4/§9). Errors: `noRemote` | `authFailed` | `networkError` | `git`
/// | `noRepo`. Does NOT emit `repo-changed` — the frontend refetches
/// imperatively (the watcher also fires and is absorbed by request-id guards).
#[tauri::command]
pub async fn fetch(state: tauri::State<'_, AppState>) -> Result<FetchResult, AppError> {
    fetch_inner(state.inner()).await
}

/// Runtime-free core of `fetch` (unit-testable without a Tauri app).
async fn fetch_inner(state: &AppState) -> Result<FetchResult, AppError> {
    let path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || fetch_all(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Fetches the upstream's remote + fast-forwards ONLY (M6 contract §2.5).
/// Errors: `noUpstream` | `authFailed` | `networkError` | `checkoutConflict`
/// | `git` | `noRepo`.
#[tauri::command]
pub async fn pull(state: tauri::State<'_, AppState>) -> Result<PullResult, AppError> {
    pull_inner(state.inner()).await
}

/// Runtime-free core of `pull` (unit-testable without a Tauri app).
async fn pull_inner(state: &AppState) -> Result<PullResult, AppError> {
    let path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || pull_ff(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Pushes the current branch to its upstream — or origin/<branch> + set
/// upstream when none (M6 contract §2.6). Never force. Errors: `noRemote`
/// | `authFailed` | `networkError` | `pushRejected` | `git` | `noRepo`.
#[tauri::command]
pub async fn push(state: tauri::State<'_, AppState>) -> Result<PushResult, AppError> {
    push_inner(state.inner()).await
}

/// Runtime-free core of `push` (unit-testable without a Tauri app).
async fn push_inner(state: &AppState) -> Result<PushResult, AppError> {
    let path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || push_current(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Current operation state (merge / rebase / cherry-pick / revert / none).
/// Part of the frontend refresh batch (P3c contract §6). Errors: `noRepo`
/// | `git`.
#[tauri::command]
pub async fn get_op_state(state: tauri::State<'_, AppState>) -> Result<RepoOpState, AppError> {
    get_op_state_inner(state.inner()).await
}

/// Runtime-free core of `get_op_state` (unit-testable without a Tauri app).
async fn get_op_state_inner(state: &AppState) -> Result<RepoOpState, AppError> {
    let path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || read_op_state(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Merges a local or remote-tracking branch into the current branch (P3c
/// contract §4). Errors: `operationInProgress` | `branchNotFound`
/// | `checkoutConflict` | `configMissing` | `git` | `noRepo`. Does NOT emit
/// `repo-changed` — the frontend refetches imperatively.
#[tauri::command]
pub async fn merge_branch(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<MergeOutcome, AppError> {
    merge_branch_inner(state.inner(), name).await
}

/// Runtime-free core of `merge_branch` (unit-testable without a Tauri app).
async fn merge_branch_inner(state: &AppState, name: String) -> Result<MergeOutcome, AppError> {
    let path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || merge::merge_branch(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Finalizes a paused merge as a 2(+)-parent commit (P3c contract §4.4).
/// Errors: `noOperationInProgress` | `unresolvedConflicts` | `emptyMessage`
/// | `configMissing` | `git` | `noRepo`.
#[tauri::command]
pub async fn commit_merge(
    state: tauri::State<'_, AppState>,
    message: String,
) -> Result<CommitResult, AppError> {
    commit_merge_inner(state.inner(), message).await
}

/// Runtime-free core of `commit_merge` (unit-testable without a Tauri app).
async fn commit_merge_inner(state: &AppState, message: String) -> Result<CommitResult, AppError> {
    let path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || merge::commit_merge(&path, &message))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Aborts a paused merge (worktree-destructive for merge-touched files —
/// the UI confirms first; P3c contract §4.5). Errors: `noOperationInProgress`
/// | `git` | `noRepo`.
#[tauri::command]
pub async fn abort_merge(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    abort_merge_inner(state.inner()).await
}

/// Runtime-free core of `abort_merge` (unit-testable without a Tauri app).
async fn abort_merge_inner(state: &AppState) -> Result<(), AppError> {
    let path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || merge::abort_merge(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// All current index conflicts, path-ascending (P3c contract §3).
/// Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn list_conflicts(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ConflictEntry>, AppError> {
    list_conflicts_inner(state.inner()).await
}

/// Runtime-free core of `list_conflicts` (unit-testable without a Tauri app).
async fn list_conflicts_inner(state: &AppState) -> Result<Vec<ConflictEntry>, AppError> {
    let path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || conflict::list_conflicts(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Read-only marker view of one conflicted file (P3c contract §3).
/// Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn get_conflict(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<ConflictFile, AppError> {
    get_conflict_inner(state.inner(), path).await
}

/// Runtime-free core of `get_conflict` (unit-testable without a Tauri app).
async fn get_conflict_inner(state: &AppState, path: String) -> Result<ConflictFile, AppError> {
    let repo_path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || conflict::get_conflict(&repo_path, &path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Resolves one conflicted path per the P3c contract §3.2 matrix.
/// Errors: `noRepo` | `git` | `invalidName` (validate_rel_path).
#[tauri::command]
pub async fn resolve_conflict(
    state: tauri::State<'_, AppState>,
    path: String,
    resolution: ConflictResolution,
) -> Result<(), AppError> {
    resolve_conflict_inner(state.inner(), path, resolution).await
}

/// Runtime-free core of `resolve_conflict` (unit-testable without a Tauri app).
async fn resolve_conflict_inner(
    state: &AppState,
    path: String,
    resolution: ConflictResolution,
) -> Result<(), AppError> {
    let repo_path = current_repo_path(state)?;
    tauri::async_runtime::spawn_blocking(move || {
        conflict::resolve_conflict(&repo_path, &path, resolution)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path_string(p: &std::path::Path) -> String {
        p.to_string_lossy().into_owned()
    }

    fn open(state: &AppState, path: &std::path::Path) -> Result<RepoInfo, AppError> {
        tauri::async_runtime::block_on(open_repo_inner(
            state,
            path_string(path),
            Box::new(|| {}),
        ))
    }

    /// Opening a non-repo path replaces the current repo with "none open":
    /// both the stored repo and the watcher slot are cleared.
    #[test]
    fn failed_open_clears_previous_repo_and_watcher() {
        let state = AppState::default();

        // Open a real (empty, unborn-HEAD) repo first.
        let repo_dir = tempfile::TempDir::new().expect("create temp dir");
        git2::Repository::init(repo_dir.path()).expect("init repo");
        let info = open(&state, repo_dir.path()).expect("open repo A");
        assert!(info.is_repo && !info.bare);
        tauri::async_runtime::block_on(get_status_inner(&state)).expect("status of repo A");

        // Now open a plain directory: not a repo.
        let non_repo_dir = tempfile::TempDir::new().expect("create temp dir");
        let info = open(&state, non_repo_dir.path()).expect("open non-repo dir");
        assert!(!info.is_repo);

        let err = tauri::async_runtime::block_on(get_status_inner(&state))
            .expect_err("no repo must be open after a failed open");
        assert!(matches!(err, AppError::NoRepo));

        assert!(state.repo.lock().expect("repo lock").is_none());
        assert!(state.watcher.lock().expect("watcher lock").is_none());
    }

    /// `get_graph` with nothing open returns `NoRepo`; after opening an
    /// unborn-HEAD repo it returns an empty layout (not an error).
    #[test]
    fn get_graph_no_repo_and_unborn() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(get_graph_inner(&state))
            .expect_err("no repo open must be NoRepo");
        assert!(matches!(err, AppError::NoRepo));

        let repo_dir = tempfile::TempDir::new().expect("create temp dir");
        git2::Repository::init(repo_dir.path()).expect("init repo");
        open(&state, repo_dir.path()).expect("open unborn repo");

        let layout = tauri::async_runtime::block_on(get_graph_inner(&state))
            .expect("empty layout for unborn repo");
        assert!(layout.nodes.is_empty());
        assert_eq!(layout.head_index, None);
    }

    /// Same semantics for bare repos: reported but not kept open.
    #[test]
    fn bare_open_clears_previous_repo_and_watcher() {
        let state = AppState::default();

        let repo_dir = tempfile::TempDir::new().expect("create temp dir");
        git2::Repository::init(repo_dir.path()).expect("init repo");
        open(&state, repo_dir.path()).expect("open repo A");

        let bare_dir = tempfile::TempDir::new().expect("create temp dir");
        git2::Repository::init_bare(bare_dir.path()).expect("init bare repo");
        let info = open(&state, bare_dir.path()).expect("open bare repo");
        assert!(info.is_repo && info.bare);

        let err = tauri::async_runtime::block_on(get_status_inner(&state))
            .expect_err("no repo must be open after opening a bare repo");
        assert!(matches!(err, AppError::NoRepo));

        assert!(state.repo.lock().expect("repo lock").is_none());
        assert!(state.watcher.lock().expect("watcher lock").is_none());
    }

    /// The M3 mutation commands all return `NoRepo` when nothing is open.
    #[test]
    fn mutation_commands_require_an_open_repo() {
        let state = AppState::default();
        let paths = vec!["file.txt".to_string()];

        let err = tauri::async_runtime::block_on(stage_inner(&state, paths.clone()))
            .expect_err("stage with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(unstage_inner(&state, paths))
            .expect_err("unstage with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(commit_inner(&state, "msg".to_string()))
            .expect_err("commit with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The M4 diff commands all return `NoRepo` when nothing is open
    /// (contract §6.2 scenario 17).
    #[test]
    fn diff_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(get_workdir_file_diff_inner(
            &state,
            "file.txt".to_string(),
            None,
            false,
        ))
        .expect_err("get_workdir_file_diff with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let oid = "0123456789abcdef0123456789abcdef01234567".to_string();
        let err = tauri::async_runtime::block_on(get_commit_diff_inner(&state, oid.clone()))
            .expect_err("get_commit_diff with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(get_commit_file_diff_inner(
            &state,
            oid,
            "file.txt".to_string(),
            None,
        ))
        .expect_err("get_commit_file_diff with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The M5 branch commands all return `NoRepo` when nothing is open
    /// (contract §6.5).
    #[test]
    fn branch_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(list_branches_inner(&state))
            .expect_err("list_branches with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err =
            tauri::async_runtime::block_on(create_branch_inner(&state, "topic".to_string()))
                .expect_err("create_branch with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err =
            tauri::async_runtime::block_on(checkout_branch_inner(&state, "topic".to_string()))
                .expect_err("checkout_branch with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err =
            tauri::async_runtime::block_on(delete_branch_inner(&state, "topic".to_string()))
                .expect_err("delete_branch with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The M6 remote commands all return `NoRepo` when nothing is open
    /// (contract §6.7).
    #[test]
    fn remote_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(fetch_inner(&state))
            .expect_err("fetch with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(pull_inner(&state))
            .expect_err("pull with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(push_inner(&state))
            .expect_err("push with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P3c merge/conflict commands all return `NoRepo` when nothing is
    /// open (contract §6).
    #[test]
    fn merge_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(get_op_state_inner(&state))
            .expect_err("get_op_state with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err =
            tauri::async_runtime::block_on(merge_branch_inner(&state, "topic".to_string()))
                .expect_err("merge_branch with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err =
            tauri::async_runtime::block_on(commit_merge_inner(&state, "msg".to_string()))
                .expect_err("commit_merge with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(abort_merge_inner(&state))
            .expect_err("abort_merge with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(list_conflicts_inner(&state))
            .expect_err("list_conflicts with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err =
            tauri::async_runtime::block_on(get_conflict_inner(&state, "file.txt".to_string()))
                .expect_err("get_conflict with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(resolve_conflict_inner(
            &state,
            "file.txt".to_string(),
            ConflictResolution::Ours,
        ))
        .expect_err("resolve_conflict with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// Patching only `theme` leaves `pane_widths`/`list_view` untouched, and
    /// each other single-field patch is equally partial (P2a contract §3.4.3;
    /// P3b contract §2.1).
    #[test]
    fn set_ui_settings_patch_is_partial() {
        let mut s = settings::Settings::default();
        let original_widths = s.pane_widths;

        apply_patch(
            &mut s,
            UiSettingsPatch {
                theme: Some(ThemeChoice::Light),
                pane_widths: None,
                list_view: None,
            },
        );
        assert_eq!(s.theme, ThemeChoice::Light);
        assert_eq!(s.pane_widths, original_widths);
        assert_eq!(s.list_view, settings::ListView::Tree);

        apply_patch(
            &mut s,
            UiSettingsPatch {
                theme: None,
                pane_widths: Some(PaneWidths {
                    sidebar: 300,
                    right_panel: 400,
                }),
                list_view: None,
            },
        );
        assert_eq!(s.theme, ThemeChoice::Light); // untouched by the second patch
        assert_eq!(s.list_view, settings::ListView::Tree);
        assert_eq!(
            s.pane_widths,
            PaneWidths {
                sidebar: 300,
                right_panel: 400,
            }
        );

        // Patching only `list_view` leaves theme + pane widths untouched.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                theme: None,
                pane_widths: None,
                list_view: Some(settings::ListView::Flat),
            },
        );
        assert_eq!(s.list_view, settings::ListView::Flat);
        assert_eq!(s.theme, ThemeChoice::Light);
        assert_eq!(
            s.pane_widths,
            PaneWidths {
                sidebar: 300,
                right_panel: 400,
            }
        );

        // Out-of-range pane widths in a patch get clamped on write.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                theme: None,
                pane_widths: Some(PaneWidths {
                    sidebar: 5,
                    right_panel: 5000,
                }),
                list_view: None,
            },
        );
        assert_eq!(s.pane_widths.sidebar, settings::SIDEBAR_MIN);
        assert_eq!(s.pane_widths.right_panel, settings::RIGHT_PANEL_MAX);
    }
}
