//! Read tools (§7.1) — the always-registered, side-effect-free tool handlers.
//! Split out of `server.rs`; the `#[tool_router]` macro generates
//! `BonsaiServer::tool_router()`, consumed by the constructor in the module
//! root. Behavior unchanged.

use super::*;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};

// ---------------------------------------------------------------------------
// Read tools (§7.1). Always registered; safe. Each wraps one core call.
// ---------------------------------------------------------------------------

#[tool_router(vis = "pub(crate)")]
impl BonsaiServer {
    /// Precomputed commit-graph layout: lane/edge topology, HEAD index, and ref
    /// pills for the whole repo. Seeded from all local branches, remote-tracking
    /// branches, and tags; ordered topologically then by commit date.
    ///
    /// WARNING: returns the ENTIRE layout in one response — for very large
    /// histories (tens of thousands of commits) this can be a multi-MB payload.
    /// Incremental/paged loading is not yet available (P65 deferred); prefer the
    /// narrower diff/status tools when you do not need the whole topology.
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
                    false, // P61a intraline: MCP serves plain typed hunks
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
                    false, // P61a intraline: MCP serves plain typed hunks
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
                    false, // P61a intraline: MCP serves plain typed hunks
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
            // A standalone (`--repo`) server has exactly one, fixed repo, so
            // selection is a client-usage error, not an internal fault — surface
            // it as `invalidName` (matching the unknown-repo rejection) rather
            // than the catch-all `other` (F-A8-d NIT).
            WorkdirSource::Fixed(_) => err_result(AppError::InvalidName(
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
