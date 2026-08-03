pub mod ai_commit;
pub mod ai_explain;
pub mod ai_resolve;
pub mod ai_summary;
pub mod blame;
pub mod branches;
pub mod cherrypick;
pub mod clone;
pub mod commit;
pub mod conflict;
pub mod diff;
pub mod discard;
pub mod discard_partial;
pub mod merge;
pub mod opstate;
pub mod rebase;
pub mod rebase_interactive;
pub mod remote;
pub mod reset;
pub mod repo;
pub mod revert;
pub mod stage;
pub mod stage_partial;
pub mod stale;
pub mod stash;
pub mod status;
pub mod submodule;
pub mod tags;
pub mod worktree;

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
