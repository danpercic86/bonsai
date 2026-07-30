//! The Bonsai MCP server: a thin adapter that exposes `bonsai_core`'s
//! differentiated Git surface (precomputed graph, structured diffs, the
//! conflict trio, stashes) to an AI assistant over stdio JSON-RPC.
//!
//! Every tool wraps a blocking `bonsai_core` call in `spawn_blocking` (git2 is
//! blocking and its handles are `!Send`, so nothing crosses `.await`). Domain
//! errors are surfaced as `CallToolResult { is_error: true }` carrying the
//! `AppError` `{ kind, message }` discriminant so the AI can branch on it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use bonsai_core::error::AppError;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo,
};
use rmcp::{schemars, tool, tool_handler, tool_router, ServerHandler};

/// The immutable server value. Holds only the canonical workdir path (every
/// `bonsai_core` fn opens its own repo from it) plus the write-gate flag.
///
/// `allow_write` is stored now even though P14b registers no mutation tools;
/// P14c composes a write router into `tool_router` when it is `true`.
#[derive(Clone)]
pub struct BonsaiServer {
    /// Canonical workdir path (from `bonsai_core::git::repo::read_repo_info().path`).
    workdir: Arc<PathBuf>,
    /// Mutation tools are unregistered unless `true`. Default `false`.
    allow_write: bool,
    tool_router: ToolRouter<BonsaiServer>,
}

impl BonsaiServer {
    /// Build a server over an already-validated canonical workdir path.
    ///
    /// The read tools (§7.1) are always registered. The write/mutation tools
    /// (§7.3) are merged into `tool_router` **only** when `allow_write` is true,
    /// so `tools/list` truthfully advertises exactly what the server can do.
    pub fn new(workdir: PathBuf, allow_write: bool) -> Self {
        let mut tool_router = Self::tool_router();
        if allow_write {
            tool_router.merge(Self::write_router());
        }
        Self {
            workdir: Arc::new(workdir),
            allow_write,
            tool_router,
        }
    }

    /// Run a blocking `bonsai_core` call on a worker thread, cloning the
    /// `Arc<PathBuf>` into the closure so no `!Send` git2 handle crosses
    /// `.await`. Join failures map to `AppError::Other`.
    async fn run_blocking<T, F>(&self, f: F) -> Result<T, AppError>
    where
        T: Send + 'static,
        F: FnOnce(&Path) -> Result<T, AppError> + Send + 'static,
    {
        let workdir = self.workdir.clone();
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
