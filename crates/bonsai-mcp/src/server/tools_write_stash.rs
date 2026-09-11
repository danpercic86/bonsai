//! Stash mutation tools (§7.3), split out of `tools_write.rs` so neither file
//! crosses the ~500-line limit (the write descriptions grew with the audit
//! 2026-09-11 corrections). Same gate, same shape: the
//! `#[tool_router(router = stash_router)]` macro generates
//! `BonsaiServer::stash_router()`, which the constructor merges alongside
//! `write_router()` whenever write access is granted — so the registered tool
//! set is exactly what it was before the split.
//!
//! Nothing here needs a [`super::write_guards`] precondition: stash create /
//! apply / pop / drop take no caller-supplied path (only an index) and the
//! primitives validate the index themselves.

use super::*;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};

#[tool_router(router = stash_router, vis = "pub(crate)")]
impl BonsaiServer {
    /// Create a stash. `created=false` means nothing to stash (not an error).
    ///
    /// Check `created` before assuming a stash exists - a clean worktree yields
    /// `created=false` with no error and no new entry. Set `includeUntracked` to sweep
    /// untracked files in as well; without it they stay in the worktree. `message` is
    /// optional. Stashing reverts the worktree to HEAD, so this is how work is set
    /// aside before a checkout. New entries land at index 0 and shift every existing
    /// index. Requires write access.
    #[tool]
    async fn bonsai_create_stash(
        &self,
        Parameters(args): Parameters<CreateStashArgs>,
    ) -> CallToolResult {
        match self
            .run_blocking(move |wd| {
                let scope = if args.include_untracked {
                    bonsai_core::git::stash::StashScope::AllWithUntracked
                } else {
                    bonsai_core::git::stash::StashScope::All
                };
                bonsai_core::git::stash::create_stash(wd, args.message.as_deref(), scope)
            })
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Apply a stash without dropping it; conflicts reported as typed paths.
    ///
    /// The stash survives regardless of outcome, so this is the recoverable option -
    /// `bonsai_pop_stash` also removes it. Branch on the outcome tag: `applied`,
    /// `conflicts` (typed paths to resolve), `appliedPartially`, `notApplied`, or the
    /// reserved-path cases below. Take `index` from `bonsai_list_stashes`; indices
    /// shift after any stash mutation. Requires write access.
    ///
    /// If the stash contains Windows-reserved paths (e.g. `NUL`) that cannot be
    /// checked out, the first attempt (`skipReserved` false/omitted) applies
    /// nothing and returns a `reservedPaths` outcome listing them; retry with
    /// `skipReserved: true` to apply the rest, yielding `appliedSkippingReserved`.
    #[tool]
    async fn bonsai_apply_stash(
        &self,
        Parameters(args): Parameters<StashApplyArgs>,
    ) -> CallToolResult {
        match self
            .run_blocking(move |wd| {
                bonsai_core::git::stash::apply_stash(wd, args.index, args.skip_reserved, None)
            })
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Apply a stash and drop it on clean success only.
    ///
    /// Same typed outcomes as `bonsai_apply_stash` - `applied`, `conflicts`,
    /// `appliedPartially`, `notApplied`, plus the reserved-path cases - but the entry
    /// is removed on a clean apply, which shifts every remaining index. On anything
    /// less than clean the stash is KEPT, so nothing is lost; re-read
    /// `bonsai_list_stashes` afterwards instead of reusing the old index. Requires
    /// write access.
    ///
    /// If the stash contains Windows-reserved paths (e.g. `NUL`) that cannot be
    /// checked out, the first attempt (`skipReserved` false/omitted) applies
    /// nothing and returns a `reservedPaths` outcome listing them; retry with
    /// `skipReserved: true` to apply the rest (`appliedSkippingReserved`). When
    /// any path is skipped the stash is KEPT (not dropped) so nothing is lost.
    #[tool]
    async fn bonsai_pop_stash(
        &self,
        Parameters(args): Parameters<StashApplyArgs>,
    ) -> CallToolResult {
        match self
            .run_blocking(move |wd| {
                bonsai_core::git::stash::pop_stash(wd, args.index, args.skip_reserved, None)
            })
            .await
        {
            Ok(v) => ok_json(&v),
            Err(e) => err_result(e),
        }
    }

    /// Permanently delete one stash entry by index, without applying it.
    ///
    /// This cannot be undone and the stashed changes are unrecoverable - inspect the
    /// entry via `bonsai_list_stashes`, or apply it first, before dropping.
    /// Indices are positional and shift after every drop, so re-read the stack rather
    /// than dropping a second index from the same listing. Requires write access.
    /// Use `bonsai_pop_stash` to apply and remove in one step.
    #[tool]
    async fn bonsai_drop_stash(
        &self,
        Parameters(args): Parameters<StashIndexArgs>,
    ) -> CallToolResult {
        match self
            .run_blocking(move |wd| bonsai_core::git::stash::drop_stash(wd, args.index, None))
            .await
        {
            Ok(()) => ok_null(),
            Err(e) => err_result(e),
        }
    }
}
