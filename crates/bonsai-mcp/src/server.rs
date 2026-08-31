//! The Bonsai MCP server: a thin adapter that exposes `bonsai_core`'s
//! differentiated Git surface (precomputed graph, structured diffs, the
//! conflict trio, stashes) to an AI assistant over stdio JSON-RPC.
//!
//! Every tool wraps a blocking `bonsai_core` call in `spawn_blocking` (git2 is
//! blocking and its handles are `!Send`, so nothing crosses `.await`). Domain
//! errors are surfaced as `CallToolResult { is_error: true }` carrying the
//! `AppError` `{ kind, message }` discriminant so the AI can branch on it.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bonsai_core::error::AppError;
use bonsai_core::git::repo::{read_repo_info, HeadInfo};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo,
};
use rmcp::{schemars, tool_handler, ServerHandler};

mod helpers;
use helpers::*;

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

/// The immutable server value. Holds the workdir source (every `bonsai_core` fn
/// opens its own repo from the resolved path) plus the write-gate flag.
///
/// `allow_write` is stored now even though P14b registers no mutation tools;
/// P14c composes a write router into `tool_router` when it is `true`.
#[derive(Clone)]
pub struct BonsaiServer {
    /// How the target workdir is resolved at each tool call (fixed or per-session).
    workdir: WorkdirSource,
    /// Mutation tools are unregistered unless `true`. Default `false`.
    allow_write: bool,
    tool_router: ToolRouter<BonsaiServer>,
}

impl BonsaiServer {
    /// Build a standalone server over an already-validated canonical workdir
    /// path (the stdio bin). Behavior-identical to the pre-P16a constructor.
    ///
    /// The read tools (§7.1) are always registered. The write/mutation tools
    /// (§7.3) are merged into `tool_router` **only** when `allow_write` is true,
    /// so `tools/list` truthfully advertises exactly what the server can do.
    pub fn new(workdir: PathBuf, allow_write: bool) -> Self {
        Self::with_source(WorkdirSource::Fixed(Arc::new(workdir)), allow_write)
    }

    /// Build an embedded per-session server (called by the embedded server's
    /// service factory, P16b). Resolves the workdir from `repos` at each call.
    pub fn with_session(repos: Arc<SessionRepos>, allow_write: bool) -> Self {
        Self::with_source(WorkdirSource::Session(repos), allow_write)
    }

    /// Shared constructor body: build the read router and merge the write router
    /// when `allow_write`. Both public constructors funnel through here so the
    /// tool-registration behavior is identical for `Fixed` and `Session`.
    fn with_source(workdir: WorkdirSource, allow_write: bool) -> Self {
        let mut tool_router = Self::tool_router();
        if allow_write {
            tool_router.merge(Self::write_router());
        }
        Self {
            workdir,
            allow_write,
            tool_router,
        }
    }

    /// Run a blocking `bonsai_core` call on a worker thread. The workdir is
    /// resolved BEFORE spawning: for `Session` this may fail with `NoRepo`
    /// (nothing selected / tab closed), and the `?` propagates it as the `Err`
    /// of `run_blocking` — each tool body's `Err(e) => err_result(e)` arm then
    /// turns it into a clean `CallToolResult { is_error: true }` (no panic).
    /// The resolved `PathBuf` is moved into the closure so no `!Send` git2
    /// handle crosses `.await`. Join failures map to `AppError::Other`.
    async fn run_blocking<T, F>(&self, f: F) -> Result<T, AppError>
    where
        T: Send + 'static,
        F: FnOnce(&Path) -> Result<T, AppError> + Send + 'static,
    {
        let workdir = self.workdir.resolve()?;
        tokio::task::spawn_blocking(move || f(workdir.as_path()))
            .await
            .map_err(|e| AppError::Other(format!("task join error: {e}")))?
    }

    /// Sorted names of the always-registered read tools (§7.1), read from the
    /// live read router — the single source of truth so `src-tauri`'s status
    /// counts and the test catalogs cannot silently drift (F-A8-b).
    pub fn read_tool_names() -> Vec<String> {
        let mut names: Vec<String> = Self::tool_router()
            .list_all()
            .iter()
            .map(|t| t.name.to_string())
            .collect();
        names.sort();
        names
    }

    /// Sorted names of the mutation tools (§7.3), read from the live write
    /// router. See [`read_tool_names`](Self::read_tool_names).
    pub fn write_tool_names() -> Vec<String> {
        let mut names: Vec<String> = Self::write_router()
            .list_all()
            .iter()
            .map(|t| t.name.to_string())
            .collect();
        names.sort();
        names
    }

    /// Count of always-registered read tools, derived from the live router.
    pub fn read_tool_count() -> usize {
        Self::tool_router().list_all().len()
    }

    /// Count of mutation tools, derived from the live write router.
    pub fn write_tool_count() -> usize {
        Self::write_router().list_all().len()
    }
}


// ---------------------------------------------------------------------------
// Input param structs (§7.4). Field docs become the JSON-Schema descriptions
// the AI reads. All use camelCase to match the frontend's JSON convention.
// ---------------------------------------------------------------------------

/// A single commit object id.
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct OidArgs {
    /// Full 40-char hex object id of the target commit.
    oid: String,
}

/// A single conflicted-file path.
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct PathArgs {
    /// Repo-relative path (forward slashes) of a currently-conflicted file.
    path: String,
}

/// One file of a commit-vs-first-parent diff.
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct CommitFileDiffArgs {
    /// Full 40-char hex object id of the commit.
    oid: String,
    /// Repo-relative path (forward slashes) of the file within the commit.
    path: String,
    /// Optional pre-rename path when the file was renamed in this commit.
    orig_path: Option<String>,
}

/// One file of a working-directory diff.
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct WorkdirFileDiffArgs {
    /// Repo-relative path (forward slashes) of the file to diff.
    path: String,
    /// Optional pre-rename path when the file was renamed.
    orig_path: Option<String>,
    /// `false`: index vs working-dir (unstaged). `true`: HEAD vs index (staged).
    staged: bool,
}

/// One file of a HEAD -> oid comparison.
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct CompareFileDiffArgs {
    /// Full 40-char hex object id of the commit to compare HEAD against.
    oid: String,
    /// Repo-relative path (forward slashes) of the file within the comparison.
    path: String,
    /// Optional pre-rename path when the file was renamed.
    orig_path: Option<String>,
}

// ---------------------------------------------------------------------------
// Mutation param structs (§7.4). Only used by the write tools (registered when
// `--allow-write`). camelCase to match the frontend's JSON convention.
// ---------------------------------------------------------------------------

/// A batch of repo-relative paths to stage or unstage.
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct PathsArgs {
    /// Repo-relative paths (forward slashes) to operate on, staged atomically.
    paths: Vec<String>,
}

/// A commit / merge-commit message.
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct MessageArgs {
    /// The commit message. Must be non-empty (else an `emptyMessage` error).
    message: String,
}

/// AI-authored final content for a conflicted file.
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ResolveConflictTextArgs {
    /// Repo-relative path of the conflicted file to resolve.
    path: String,
    /// Full final file content (no conflict markers). Written to the worktree and staged.
    content: String,
}

/// A take-ours / take-theirs / mark-resolved shortcut for a conflicted file.
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ResolveConflictArgs {
    /// Repo-relative path of the conflicted file to resolve.
    path: String,
    /// One of: `"ours"` | `"theirs"` | `"markResolved"`.
    resolution: String,
}

/// A branch (or ref) name.
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct NameArgs {
    /// The branch name (short form, e.g. `feature/x`).
    name: String,
}

/// The target ref a rebase replays onto.
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct OntoArgs {
    /// The branch/ref name to rebase the current branch onto.
    onto: String,
}

/// A new branch at a specific commit.
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct CreateBranchHereArgs {
    /// The new branch name.
    name: String,
    /// Full 40-char hex object id of the commit the branch should point at.
    oid: String,
}

/// Options for creating a stash.
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct CreateStashArgs {
    /// Optional stash message.
    message: Option<String>,
    /// Whether to include untracked files in the stash.
    include_untracked: bool,
}

/// A stash-stack index.
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct StashIndexArgs {
    /// Zero-based index into the stash stack (0 = most recent).
    index: usize,
}

/// A stash-stack index plus the reserved-path skip flag for apply/pop.
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct StashApplyArgs {
    /// Zero-based index into the stash stack (0 = most recent).
    index: usize,
    /// When true, apply everything except Windows-reserved paths (e.g. `NUL`)
    /// that cannot be written to the working tree. Defaults to false; the first
    /// attempt (false) returns a `reservedPaths` outcome listing the offending
    /// paths so the caller can retry with `skipReserved: true`.
    #[serde(default)]
    skip_reserved: bool,
}

/// Map the string `resolution` tool argument to the core `ConflictResolution`
/// enum without adding a `schemars` dependency to `bonsai-core`.
fn parse_resolution(s: &str) -> Result<bonsai_core::git::conflict::ConflictResolution, AppError> {
    use bonsai_core::git::conflict::ConflictResolution;
    match s {
        "ours" => Ok(ConflictResolution::Ours),
        "theirs" => Ok(ConflictResolution::Theirs),
        "markResolved" => Ok(ConflictResolution::MarkResolved),
        other => Err(AppError::InvalidName(format!(
            "invalid resolution '{other}' (expected 'ours' | 'theirs' | 'markResolved')"
        ))),
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
struct OpenRepoSummary {
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
struct SelectRepoArgs {
    /// The repoId (canonical workdir path) of an open Bonsai tab, from
    /// `bonsai_list_repos`.
    repo_id: String,
}

/// Build an [`OpenRepoSummary`] for `repo`, reading its HEAD via `read_repo_info`
/// (blocking git2 — call inside `spawn_blocking`). An unreadable HEAD yields
/// `head: None` rather than failing the whole listing.
fn summarize_repo(repo: &OpenRepo, selected: bool) -> OpenRepoSummary {
    let head = read_repo_info(&repo.path).ok().and_then(|info| info.head);
    OpenRepoSummary {
        repo_id: repo.repo_id.clone(),
        path: repo.path.to_string_lossy().into_owned(),
        head,
        selected,
    }
}

// ---------------------------------------------------------------------------
// Read tools (§7.1) live in `tools_read`; mutation tools (§7.3) in
// `tools_write`. Each is a `#[tool_router]` impl block whose generated router
// (`tool_router()` / `write_router()`) the constructor above consumes.
// ---------------------------------------------------------------------------
mod tools_read;
mod tools_write;

#[tool_handler(router = self.tool_router)]
impl ServerHandler for BonsaiServer {
    fn get_info(&self) -> ServerInfo {
        let write_note = if self.allow_write {
            " Mutation tools (stage/commit, conflict resolution, merge/rebase, branches, \
             stashes) are ENABLED (--allow-write)."
        } else {
            " This server is READ-ONLY; mutation tools are not registered (start with \
             --allow-write to enable them)."
        };
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("bonsai", env!("CARGO_PKG_VERSION")))
            .with_instructions(format!(
                "Bonsai exposes structured Git data for the repository passed via --repo: a \
                 precomputed commit-graph layout (lanes/edges/refs), typed diffs, working-dir \
                 status, and the ours/theirs/base conflict trio. Prefer these tools over parsing \
                 `git` output for graph topology, structured diffs, and conflict contents.{write_note}"
            ))
    }
}

#[cfg(test)]
mod tests;
