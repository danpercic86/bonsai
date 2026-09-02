//! Consolidated `worktree_submodule` integration-test harness (build-time only).
//!
//! Each module below is a former top-level `tests/*.rs` integration binary, moved
//! verbatim into `tests/worktree_submodule/` and included here as its own module. The 78 test
//! binaries, each statically linking vendored libgit2, collapse into 8 — no test
//! was added, removed, renamed, or changed. Giving every former file its own
//! `mod` keeps its helper symbols namespaced, so identically named helpers across
//! files cannot collide.

mod common;

#[path = "worktree_submodule/worktree_cli.rs"]
mod worktree_cli;
#[path = "worktree_submodule/worktree_context_cli.rs"]
mod worktree_context_cli;
#[path = "worktree_submodule/worktree_copy_cli.rs"]
mod worktree_copy_cli;
#[path = "worktree_submodule/submodule_cli.rs"]
mod submodule_cli;
#[path = "worktree_submodule/submodule_cli_2.rs"]
mod submodule_cli_2;
#[path = "worktree_submodule/submodule_reconnect_cli.rs"]
mod submodule_reconnect_cli;
#[path = "worktree_submodule/submodule_wedge_cli.rs"]
mod submodule_wedge_cli;
#[path = "worktree_submodule/stash_cli.rs"]
mod stash_cli;
#[path = "worktree_submodule/stash_cli_conflicts.rs"]
mod stash_cli_conflicts;
