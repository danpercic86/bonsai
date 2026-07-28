use tauri::Emitter;

use crate::error::AppError;
use crate::git::commit::{create_commit, CommitResult};
use crate::git::repo::{read_repo_info, RepoInfo};
use crate::git::stage::{stage_paths, unstage_paths};
use crate::git::status::{read_status, StatusSnapshot};
use crate::graph::{compute_graph, GraphLayout};
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
    open_repo_inner(
        state.inner(),
        path,
        Box::new(move || {
            let _ = app.emit(
                "repo-changed",
                RepoChangedPayload {
                    reason: "fs".to_string(),
                },
            );
        }),
    )
    .await
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
}
