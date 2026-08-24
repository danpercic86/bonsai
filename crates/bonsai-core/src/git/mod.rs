pub mod ai_branch_name;
pub mod ai_changelog;
pub mod ai_commit;
pub mod ai_compose;
pub mod ai_explain;
pub mod ai_history;
pub mod ai_line;
pub mod ai_operation;
pub mod ai_operation_grounding;
pub mod ai_operation_preview;
pub mod ai_operation_resolve;
pub mod ai_pr_description;
pub mod ai_resolve;
/// PURE bulk payload/attribution rules for a streaming resolve (P68 §6).
pub mod ai_resolve_bulk;
/// Streaming + bulk conflict resolve orchestration (P68 §D).
pub mod ai_resolve_stream;
/// The run-level event funnel `ai_resolve_stream` sequences its batches through
/// (P68 §6.3). Private: only that module may fabricate run events.
mod ai_resolve_stream_events;
pub mod ai_summary;
/// P87 git-activity observability: the event model + recorder/emitter.
pub mod activity;
pub mod autostash;
pub mod bisect;
pub mod blame;
pub mod branches;
pub mod cherrypick;
pub mod clone;
pub mod commit;
/// P87 activity-recording commit/amend cores (split from `commit.rs` for size).
pub mod commit_activity;
pub mod compose_apply;
pub mod config;
pub mod conflict;
pub mod cred;
pub mod cred_cache;
pub mod diff;
pub mod discard;
pub mod discard_partial;
pub mod exec;
/// P87 `SpawnGitExec::exec_streaming` reader (split from `exec.rs` for size).
pub mod exec_stream;
pub mod history_index;
pub mod hooks;
/// P87 streaming hook variants (split from `hooks.rs` for size).
pub mod hooks_stream;
pub mod image_diff;
pub mod intraline;
pub mod maintenance;
pub mod merge;
/// P87 activity-recording merge-commit core (split from `merge.rs` for size).
pub mod merge_activity;
pub mod opstate;
/// Cross-module symlink-escape (path-traversal) guard tests for `ensure_within_workdir`.
#[cfg(test)]
mod path_traversal_tests;
pub mod rebase;
pub mod rebase_interactive;
pub mod reflog;
pub mod remote;
/// P87 activity-recording fetch/pull cores (split from `remote.rs` for size).
pub mod remote_activity;
/// P87 activity-recording push/force-push cores (split from `remote.rs`).
pub mod remote_push_activity;
pub mod reset;
pub mod repo;
pub mod revert;
pub mod search;
pub mod signing;
pub mod stage;
pub mod stage_partial;
pub mod stale;
pub mod stash;
pub mod status;
pub mod submodule;
/// P73 reconnect/salvage machinery for `submodule` (private to `git`).
mod submodule_reconnect;
/// P73 clone-path rollback for `submodule` (private to `git`).
mod submodule_rollback;
/// P82 deinit/remove force machinery for `submodule` (private to `git`).
mod submodule_teardown;
pub mod tags;
pub mod tag_sync;
pub mod timefmt;
pub mod timeout;
pub mod undo;
pub mod worktree;
/// P88b/B2a handle-reusing twins of the worktree readers (crate-internal).
pub(crate) mod worktree_reuse;
pub mod worktree_copy;

/// libgit2 re-hashes every object it inflates to verify its id (strict
/// hash verification, on by default). Over a 31k-commit walk that is
/// ~25-30% of the runtime for zero benefit on a repo we just read from
/// disk — Cargo disables it for its git operations too. Process-global,
/// one-time; called at app init (`run()`) and from the perf-fixture setup
/// so benches/gates measure the app's real configuration.
pub fn relax_odb_hash_verification() {
    static RELAX_HASH_VERIFICATION: std::sync::Once = std::sync::Once::new();
    RELAX_HASH_VERIFICATION.call_once(|| {
        git2::opts::strict_hash_verification(false);
    });
}
