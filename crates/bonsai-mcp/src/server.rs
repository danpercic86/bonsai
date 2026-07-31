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
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo,
};
use rmcp::{schemars, tool, tool_handler, tool_router, ServerHandler};

/// Map a poisoned lock to a domain error rather than panicking.
fn pois<T>(_: std::sync::PoisonError<T>) -> AppError {
    AppError::Other("state lock poisoned".into())
}

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
}

/// Success result: structured JSON content (serde of the core type) plus a
/// compact text echo of the same JSON.
fn ok_json<T: serde::Serialize>(v: &T) -> CallToolResult {
    match serde_json::to_value(v) {
        Ok(value) => CallToolResult::structured(value),
        Err(e) => err_result(AppError::Other(format!("serialization error: {e}"))),
    }
}

/// Success result for a mutation that returns no data (`() -> null`).
fn ok_null() -> CallToolResult {
    CallToolResult::structured(serde_json::Value::Null)
}

/// Domain-error result: preserves `AppError`'s `{ kind, message }` in structured
/// content (via its custom `Serialize`) plus a human `"<kind>: <message>"` text.
/// `is_error = true`.
fn err_result(e: AppError) -> CallToolResult {
    let value = serde_json::to_value(&e).unwrap_or_else(|_| {
        serde_json::json!({ "kind": "other", "message": "unserializable error" })
    });
    let kind = value.get("kind").and_then(|v| v.as_str()).unwrap_or("other");
    let message = value.get("message").and_then(|v| v.as_str()).unwrap_or("");
    let text = format!("{kind}: {message}");
    let mut result = CallToolResult::structured_error(value);
    result.content = vec![ContentBlock::text(text)];
    result
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
// Read tools (§7.1). Always registered; safe. Each wraps one core call.
// ---------------------------------------------------------------------------

#[tool_router]
impl BonsaiServer {
    /// Precomputed commit-graph layout: lane/edge topology, HEAD index, and ref
    /// pills for the whole repo. Seeded from all local branches, remote-tracking
    /// branches, and tags; ordered topologically then by commit date.
    #[tool]
    async fn bonsai_get_graph(&self) -> CallToolResult {
        match self.run_blocking(bonsai_core::graph::compute_graph).await {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Structured working-directory status: staged / unstaged / untracked /
    /// conflicted split lists with rename detection (no porcelain parsing).
    #[tool]
    async fn bonsai_get_status(&self) -> CallToolResult {
        match self.run_blocking(bonsai_core::git::status::read_status).await {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// All refs in one call: local branches, remote-tracking branches, and tags,
    /// each with upstream + ahead/behind + tip, plus HEAD.
    #[tool]
    async fn bonsai_list_branches(&self) -> CallToolResult {
        match self.run_blocking(bonsai_core::git::branches::list_refs).await {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Commit details + per-file headers (adds/dels/status) for a commit vs its
    /// first parent, structured.
    #[tool]
    async fn bonsai_get_commit_diff(&self, Parameters(args): Parameters<OidArgs>) -> CallToolResult {
        match self
            .run_blocking(move |wd| bonsai_core::git::diff::commit_diff(wd, &args.oid))
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Typed hunks/lines (with old/new line numbers) for one file of the
    /// commit-vs-first-parent diff. No `@@` parsing.
    #[tool]
    async fn bonsai_get_commit_file_diff(
        &self,
        Parameters(args): Parameters<CommitFileDiffArgs>,
    ) -> CallToolResult {
        match self
            .run_blocking(move |wd| {
                bonsai_core::git::diff::commit_file_diff(
                    wd,
                    &args.oid,
                    &args.path,
                    args.orig_path.as_deref(),
                    false,
                )
            })
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Structured working-dir diff for one file. `staged=false`: index vs
    /// working-dir. `staged=true`: HEAD vs index.
    #[tool]
    async fn bonsai_get_workdir_file_diff(
        &self,
        Parameters(args): Parameters<WorkdirFileDiffArgs>,
    ) -> CallToolResult {
        match self
            .run_blocking(move |wd| {
                bonsai_core::git::diff::workdir_file_diff(
                    wd,
                    &args.path,
                    args.orig_path.as_deref(),
                    args.staged,
                    false,
                )
            })
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Tree-vs-tree HEAD -> oid per-file headers, structured.
    #[tool]
    async fn bonsai_compare_with_head(
        &self,
        Parameters(args): Parameters<OidArgs>,
    ) -> CallToolResult {
        match self
            .run_blocking(move |wd| bonsai_core::git::diff::compare_head_diff(wd, &args.oid))
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Per-file hunks of the HEAD -> oid comparison.
    #[tool]
    async fn bonsai_compare_with_head_file_diff(
        &self,
        Parameters(args): Parameters<CompareFileDiffArgs>,
    ) -> CallToolResult {
        match self
            .run_blocking(move |wd| {
                bonsai_core::git::diff::compare_head_file_diff(
                    wd,
                    &args.oid,
                    &args.path,
                    args.orig_path.as_deref(),
                    false,
                )
            })
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Whether a merge/rebase/cherry-pick/revert is mid-flight plus its step
    /// counters — drives the conflict-resolution loop.
    #[tool]
    async fn bonsai_get_op_state(&self) -> CallToolResult {
        match self.run_blocking(bonsai_core::git::opstate::read_op_state).await {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Structured conflict inventory with a `ConflictKind` per path.
    #[tool]
    async fn bonsai_list_conflicts(&self) -> CallToolResult {
        match self
            .run_blocking(bonsai_core::git::conflict::list_conflicts)
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// The crown conflict tool: separated ours/theirs blob text + marker text +
    /// kind + binary/tooLarge/missing flags — everything an AI needs to author a
    /// resolution.
    #[tool]
    async fn bonsai_get_conflict(
        &self,
        Parameters(args): Parameters<PathArgs>,
    ) -> CallToolResult {
        match self
            .run_blocking(move |wd| bonsai_core::git::conflict::get_conflict(wd, &args.path))
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Structured stash stack (index / message / oid / base / timestamp).
    #[tool]
    async fn bonsai_list_stashes(&self) -> CallToolResult {
        match self.run_blocking(bonsai_core::git::stash::list_stashes).await {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Enumerate the repos the user has OPEN in Bonsai (P16 §4b, D-2). Each
    /// summary carries a HEAD summary and a `selected` flag marking this
    /// session's currently-selected repo. Standalone (`Fixed`) servers report
    /// their single `--repo`; embedded (`Session`) servers snapshot the app's
    /// open tabs.
    #[tool]
    async fn bonsai_list_repos(&self) -> CallToolResult {
        match &self.workdir {
            WorkdirSource::Fixed(p) => {
                let repo = OpenRepo {
                    repo_id: p.to_string_lossy().into_owned(),
                    path: (**p).clone(),
                };
                match tokio::task::spawn_blocking(move || vec![summarize_repo(&repo, true)]).await {
                    Ok(v) => ok_json(&v),
                    Err(e) => err_result(AppError::Other(format!("task join error: {e}"))),
                }
            }
            WorkdirSource::Session(s) => {
                let selected = match s.selected_id() {
                    Ok(v) => v,
                    Err(e) => return err_result(e),
                };
                let open = s.open();
                match tokio::task::spawn_blocking(move || {
                    open.iter()
                        .map(|r| {
                            let is_sel = selected.as_deref() == Some(r.repo_id.as_str());
                            summarize_repo(r, is_sel)
                        })
                        .collect::<Vec<_>>()
                })
                .await
                {
                    Ok(v) => ok_json(&v),
                    Err(e) => err_result(AppError::Other(format!("task join error: {e}"))),
                }
            }
        }
    }

    /// Set the CALLING SESSION's selected repo to `repoId` (P16 §4b, D-2).
    /// Validates `repoId` against the currently-open set (unknown/closed →
    /// `invalidName`); `Fixed` (standalone) servers reject selection. Returns
    /// the now-selected repo's summary. Never disturbs other sessions or the
    /// app's focused tab.
    #[tool]
    async fn bonsai_select_repo(
        &self,
        Parameters(args): Parameters<SelectRepoArgs>,
    ) -> CallToolResult {
        match &self.workdir {
            WorkdirSource::Fixed(_) => err_result(AppError::Other(
                "single-repo (standalone) server; repo selection unavailable".to_string(),
            )),
            WorkdirSource::Session(s) => {
                if let Err(e) = s.select(&args.repo_id) {
                    return err_result(e);
                }
                let repo = match s.open().into_iter().find(|r| r.repo_id == args.repo_id) {
                    Some(r) => r,
                    // Closed between select() and here — treat as no-repo.
                    None => return err_result(AppError::NoRepo),
                };
                match tokio::task::spawn_blocking(move || summarize_repo(&repo, true)).await {
                    Ok(v) => ok_json(&v),
                    Err(e) => err_result(AppError::Other(format!("task join error: {e}"))),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Mutation tools (§7.3). Registered into `tool_router` only when `--allow-write`
// (see `new()`); when off, this router is never merged so the tools do not
// appear in `tools/list`. `Self::write_router()` is generated by the macro.
// ---------------------------------------------------------------------------

#[tool_router(router = write_router)]
impl BonsaiServer {
    /// Atomically stage a batch of repo-relative paths (worktree untouched).
    #[tool]
    async fn bonsai_stage(&self, Parameters(args): Parameters<PathsArgs>) -> CallToolResult {
        match self
            .run_blocking(move |wd| bonsai_core::git::stage::stage_paths(wd, &args.paths))
            .await
        {
            Ok(()) => ok_null(),
            Err(e) => err_result(e),
        }
    }

    /// Unstage a batch of repo-relative paths (never touches the worktree).
    #[tool]
    async fn bonsai_unstage(&self, Parameters(args): Parameters<PathsArgs>) -> CallToolResult {
        match self
            .run_blocking(move |wd| bonsai_core::git::stage::unstage_paths(wd, &args.paths))
            .await
        {
            Ok(()) => ok_null(),
            Err(e) => err_result(e),
        }
    }

    /// Create a commit from the staged index. Errors clearly on empty message,
    /// missing git identity, or nothing-to-commit (preserved `kind`).
    #[tool]
    async fn bonsai_commit(&self, Parameters(args): Parameters<MessageArgs>) -> CallToolResult {
        match self
            .run_blocking(move |wd| bonsai_core::git::commit::create_commit(wd, &args.message))
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Resolve a conflicted file by writing AI-authored final content to the
    /// worktree and staging it (the primary AI resolution path).
    #[tool]
    async fn bonsai_resolve_conflict_text(
        &self,
        Parameters(args): Parameters<ResolveConflictTextArgs>,
    ) -> CallToolResult {
        match self
            .run_blocking(move |wd| {
                bonsai_core::git::conflict::resolve_conflict_text(wd, &args.path, &args.content)
            })
            .await
        {
            Ok(()) => ok_null(),
            Err(e) => err_result(e),
        }
    }

    /// Resolve a conflicted file via take-ours / take-theirs / mark-resolved.
    #[tool]
    async fn bonsai_resolve_conflict(
        &self,
        Parameters(args): Parameters<ResolveConflictArgs>,
    ) -> CallToolResult {
        let resolution = match parse_resolution(&args.resolution) {
            Ok(r) => r,
            Err(e) => return err_result(e),
        };
        match self
            .run_blocking(move |wd| {
                bonsai_core::git::conflict::resolve_conflict(wd, &args.path, resolution)
            })
            .await
        {
            Ok(()) => ok_null(),
            Err(e) => err_result(e),
        }
    }

    /// Merge a branch into the current branch (FF / clean-merge / conflicts are
    /// distinguished in the typed outcome; autostash handled; never force).
    #[tool]
    async fn bonsai_merge_branch(&self, Parameters(args): Parameters<NameArgs>) -> CallToolResult {
        match self
            .run_blocking(move |wd| bonsai_core::git::merge::merge_branch(wd, &args.name))
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Finalize a paused merge (refuses on `unresolvedConflicts`).
    #[tool]
    async fn bonsai_commit_merge(
        &self,
        Parameters(args): Parameters<MessageArgs>,
    ) -> CallToolResult {
        match self
            .run_blocking(move |wd| bonsai_core::git::merge::commit_merge(wd, &args.message))
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Abort an in-progress merge (worktree-destructive; gated by `--allow-write`).
    #[tool]
    async fn bonsai_abort_merge(&self) -> CallToolResult {
        match self.run_blocking(bonsai_core::git::merge::abort_merge).await {
            Ok(()) => ok_null(),
            Err(e) => err_result(e),
        }
    }

    /// Rebase the current branch onto another ref (typed FF/rebased/conflicts
    /// with step counters).
    #[tool]
    async fn bonsai_rebase_branch(&self, Parameters(args): Parameters<OntoArgs>) -> CallToolResult {
        match self
            .run_blocking(move |wd| bonsai_core::git::rebase::rebase_branch(wd, &args.onto))
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Resume a paused rebase after resolving the current step's conflicts.
    #[tool]
    async fn bonsai_rebase_continue(&self) -> CallToolResult {
        match self
            .run_blocking(bonsai_core::git::rebase::rebase_continue)
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Skip the current step of a paused rebase.
    #[tool]
    async fn bonsai_rebase_skip(&self) -> CallToolResult {
        match self
            .run_blocking(bonsai_core::git::rebase::rebase_skip)
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Abort an in-progress rebase (worktree-destructive; gated).
    #[tool]
    async fn bonsai_rebase_abort(&self) -> CallToolResult {
        match self
            .run_blocking(bonsai_core::git::rebase::rebase_abort)
            .await
        {
            Ok(()) => ok_null(),
            Err(e) => err_result(e),
        }
    }

    /// Create a branch at HEAD (no checkout).
    #[tool]
    async fn bonsai_create_branch(&self, Parameters(args): Parameters<NameArgs>) -> CallToolResult {
        match self
            .run_blocking(move |wd| bonsai_core::git::branches::create_branch(wd, &args.name))
            .await
        {
            Ok(()) => ok_null(),
            Err(e) => err_result(e),
        }
    }

    /// Create a branch at a specific commit (autostash across the checkout).
    #[tool]
    async fn bonsai_create_branch_here(
        &self,
        Parameters(args): Parameters<CreateBranchHereArgs>,
    ) -> CallToolResult {
        match self
            .run_blocking(move |wd| {
                bonsai_core::git::branches::create_branch_here(wd, &args.name, &args.oid)
            })
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Safely checkout a branch — never force; `checkoutConflict` surfaces
    /// instead of clobbering the worktree.
    #[tool]
    async fn bonsai_checkout_branch(
        &self,
        Parameters(args): Parameters<NameArgs>,
    ) -> CallToolResult {
        match self
            .run_blocking(move |wd| bonsai_core::git::branches::checkout_branch(wd, &args.name))
            .await
        {
            Ok(()) => ok_null(),
            Err(e) => err_result(e),
        }
    }

    /// Delete a branch — blocks unmerged deletion (`unmergedBranch`); no force.
    #[tool]
    async fn bonsai_delete_branch(&self, Parameters(args): Parameters<NameArgs>) -> CallToolResult {
        match self
            .run_blocking(move |wd| bonsai_core::git::branches::delete_branch(wd, &args.name))
            .await
        {
            Ok(()) => ok_null(),
            Err(e) => err_result(e),
        }
    }

    /// Create a stash. `created=false` means nothing to stash (not an error).
    #[tool]
    async fn bonsai_create_stash(
        &self,
        Parameters(args): Parameters<CreateStashArgs>,
    ) -> CallToolResult {
        match self
            .run_blocking(move |wd| {
                bonsai_core::git::stash::create_stash(
                    wd,
                    args.message.as_deref(),
                    args.include_untracked,
                )
            })
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Apply a stash without dropping it; conflicts reported as typed paths.
    #[tool]
    async fn bonsai_apply_stash(
        &self,
        Parameters(args): Parameters<StashIndexArgs>,
    ) -> CallToolResult {
        match self
            .run_blocking(move |wd| bonsai_core::git::stash::apply_stash(wd, args.index))
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Apply a stash and drop it on clean success only.
    #[tool]
    async fn bonsai_pop_stash(
        &self,
        Parameters(args): Parameters<StashIndexArgs>,
    ) -> CallToolResult {
        match self
            .run_blocking(move |wd| bonsai_core::git::stash::pop_stash(wd, args.index))
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Permanently drop a stash (gated by `--allow-write`).
    #[tool]
    async fn bonsai_drop_stash(
        &self,
        Parameters(args): Parameters<StashIndexArgs>,
    ) -> CallToolResult {
        match self
            .run_blocking(move |wd| bonsai_core::git::stash::drop_stash(wd, args.index))
            .await
        {
            Ok(()) => ok_null(),
            Err(e) => err_result(e),
        }
    }
}

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

// ---------------------------------------------------------------------------
// Unit tests for the per-session selection state (P16a). Pure: no rmcp, no
// git2, no CLI — `list_open` is a closure over a fixed `Vec<OpenRepo>`.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn open(id: &str) -> OpenRepo {
        OpenRepo {
            repo_id: id.to_string(),
            path: PathBuf::from(format!("/repo/{id}")),
        }
    }

    /// A session over open tabs `a` and `b`, seeded with `seed`.
    fn session(seed: Option<&str>) -> SessionRepos {
        let repos = vec![open("a"), open("b")];
        SessionRepos::new(
            seed.map(str::to_string),
            Box::new(move || repos.clone()),
        )
    }

    #[test]
    fn resolve_workdir_none_selected_is_no_repo() {
        let s = session(None);
        assert!(matches!(s.resolve_workdir(), Err(AppError::NoRepo)));
    }

    #[test]
    fn resolve_workdir_selected_present_returns_that_path() {
        let s = session(Some("b"));
        assert_eq!(
            s.resolve_workdir().expect("selected tab is open"),
            PathBuf::from("/repo/b")
        );
    }

    #[test]
    fn resolve_workdir_selected_but_closed_is_no_repo() {
        // Seed selects `b`, but the open set no longer contains it (tab closed).
        let repos = vec![open("a")];
        let s = SessionRepos::new(Some("b".to_string()), Box::new(move || repos.clone()));
        assert!(matches!(s.resolve_workdir(), Err(AppError::NoRepo)));
    }

    #[test]
    fn select_unknown_id_is_invalid_name() {
        let s = session(None);
        assert!(matches!(s.select("nope"), Err(AppError::InvalidName(_))));
    }

    #[test]
    fn select_known_id_then_resolve_returns_right_path() {
        let s = session(None);
        s.select("a").expect("`a` is an open tab");
        assert_eq!(
            s.resolve_workdir().expect("just-selected tab resolves"),
            PathBuf::from("/repo/a")
        );
    }
}
