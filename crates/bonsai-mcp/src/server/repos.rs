//! Repo identity, per-session selection, and the repo-selection tool types —
//! split out of `server.rs` so neither file crosses the ~500-line limit.
//!
//! `OpenRepo` / `SessionRepos` / `WorkdirSource` answer one question ("which
//! workdir does this tool call operate on?") for both deployments: the
//! standalone stdio server's single `--repo`, and the embedded server's
//! per-session selection over the app's open tabs. Behaviour unchanged by the
//! move; the module root re-exports everything here.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bonsai_core::error::AppError;
use bonsai_core::git::repo::{read_repo_info, HeadInfo};

use rmcp::schemars;

use super::helpers::pois;

/// One open repo as the embedded server sees it (repoId + canonical workdir).
///
/// The `repo_id` is the canonical workdir path string (the same value the
/// embedded `bonsai_select_repo` tool accepts); `path` is that workdir as a
/// `PathBuf` for the git tools.
#[derive(Clone)]
pub struct OpenRepo {
    /// Stable identifier for the open tab (canonical workdir path string).
    pub repo_id: String,
    /// Canonical workdir path the git tools operate on.
    pub path: PathBuf,
}

/// Per-SESSION repo state for the embedded server. Each MCP session gets its own
/// instance (built by the embedded server's service factory), so `selected` is
/// private to that session and never disturbs other sessions or the app's
/// focused tab.
pub struct SessionRepos {
    /// This session's currently-selected repoId. Seeded at session open and
    /// mutated only by `select` (the embedded `bonsai_select_repo` tool, P16b).
    selected: Mutex<Option<String>>,
    /// Snapshot the app's currently-open tabs. Cheap (no git2); called at every
    /// workdir resolve / list / select.
    list_open: Box<dyn Fn() -> Vec<OpenRepo> + Send + Sync>,
}

impl SessionRepos {
    /// Build a per-session selection state seeded with `seed` (the focused
    /// tab's repoId, or `None`), reading open tabs via `list_open`.
    pub fn new(seed: Option<String>, list_open: Box<dyn Fn() -> Vec<OpenRepo> + Send + Sync>) -> Self {
        SessionRepos {
            selected: Mutex::new(seed),
            list_open,
        }
    }

    /// Snapshot of open tabs (for `bonsai_list_repos`, P16b).
    pub(crate) fn open(&self) -> Vec<OpenRepo> {
        (self.list_open)()
    }

    /// The session's selected repoId, if any.
    pub(crate) fn selected_id(&self) -> Result<Option<String>, AppError> {
        Ok(self.selected.lock().map_err(pois)?.clone())
    }

    /// Resolve the selected repo -> workdir at git-tool call time.
    ///
    /// `None` selected -> `NoRepo` ("call bonsai_select_repo"); selected but the
    /// tab was closed since selection -> `NoRepo`.
    pub(crate) fn resolve_workdir(&self) -> Result<PathBuf, AppError> {
        let id = self.selected_id()?.ok_or(AppError::NoRepo)?;
        (self.list_open)()
            .into_iter()
            .find(|r| r.repo_id == id)
            .map(|r| r.path)
            .ok_or(AppError::NoRepo)
    }

    /// Validate `repo_id` is a currently-open tab, then select it for this
    /// session. Unknown / closed id -> `InvalidName`.
    pub(crate) fn select(&self, repo_id: &str) -> Result<(), AppError> {
        if !(self.list_open)().iter().any(|r| r.repo_id == repo_id) {
            return Err(AppError::InvalidName(format!(
                "repo '{repo_id}' is not an open tab"
            )));
        }
        *self.selected.lock().map_err(pois)? = Some(repo_id.to_string());
        Ok(())
    }
}

/// Resolves the target repo workdir at each git-tool call. Two variants share
/// the identical tool bodies: the standalone stdio server's fixed workdir and
/// the embedded server's per-session selection over the app's open tabs.
#[derive(Clone)]
pub enum WorkdirSource {
    /// Standalone stdio server: one fixed, pre-validated canonical workdir.
    Fixed(Arc<PathBuf>),
    /// Embedded server: per-session selection over the app's open tabs.
    Session(Arc<SessionRepos>),
}

impl WorkdirSource {
    /// Workdir for the git tools (locks a mutex + clones a `PathBuf` — no git2,
    /// no `.await`). `Session` may surface `NoRepo` (nothing selected / closed
    /// tab), which propagates through `run_blocking` into a clean error result.
    pub fn resolve(&self) -> Result<PathBuf, AppError> {
        match self {
            WorkdirSource::Fixed(p) => Ok((**p).clone()),
            WorkdirSource::Session(s) => s.resolve_workdir(),
        }
    }
}

// ---------------------------------------------------------------------------
// Repo-selection tool types (P16 §4b, D-2). Output/param types for the two
// always-registered repo-management read tools.
// ---------------------------------------------------------------------------

/// One open repo as the AI sees it via `bonsai_list_repos` / `bonsai_select_repo`.
///
/// Output-only (serialized into a `CallToolResult`), so it needs `Serialize` but
/// not `JsonSchema` — the latter would force `HeadInfo: JsonSchema` on
/// `bonsai-core` for no benefit.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenRepoSummary {
    /// Canonical workdir path string = the repoId used by `bonsai_select_repo`.
    repo_id: String,
    /// Canonical workdir path (same value; explicit for readability).
    path: String,
    /// HEAD summary (branch name / detached / unborn); `None` if unreadable.
    head: Option<HeadInfo>,
    /// True for the repo THIS session currently has selected.
    selected: bool,
}

/// Argument for `bonsai_select_repo`: the repoId of an open Bonsai tab.
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelectRepoArgs {
    /// The repoId (canonical workdir path) of an open Bonsai tab, from
    /// `bonsai_list_repos`.
    pub(crate) repo_id: String,
}

/// Build an [`OpenRepoSummary`] for `repo`, reading its HEAD via `read_repo_info`
/// (blocking git2 — call inside `spawn_blocking`). An unreadable HEAD yields
/// `head: None` rather than failing the whole listing.
pub(crate) fn summarize_repo(repo: &OpenRepo, selected: bool) -> OpenRepoSummary {
    let head = read_repo_info(&repo.path).ok().and_then(|info| info.head);
    OpenRepoSummary {
        repo_id: repo.repo_id.clone(),
        path: repo.path.to_string_lossy().into_owned(),
        head,
        selected,
    }
}
