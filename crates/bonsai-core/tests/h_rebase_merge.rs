//! Consolidated `rebase_merge` integration-test harness (build-time only).
//!
//! Each module below is a former top-level `tests/*.rs` integration binary, moved
//! verbatim into `tests/rebase_merge/` and included here as its own module. The 78 test
//! binaries, each statically linking vendored libgit2, collapse into 8 — no test
//! was added, removed, renamed, or changed. Giving every former file its own
//! `mod` keeps its helper symbols namespaced, so identically named helpers across
//! files cannot collide.

mod common;

#[path = "rebase_merge/rebase_cli.rs"]
mod rebase_cli;
#[path = "rebase_merge/rebase_interactive_cli.rs"]
mod rebase_interactive_cli;
#[path = "rebase_merge/merge_cli.rs"]
mod merge_cli;
#[path = "rebase_merge/conflict_cli.rs"]
mod conflict_cli;
#[path = "rebase_merge/sequencer_salvage_cli.rs"]
mod sequencer_salvage_cli;
#[path = "rebase_merge/autostash_cli.rs"]
mod autostash_cli;
#[path = "rebase_merge/essentials_autostash_cli.rs"]
mod essentials_autostash_cli;
#[path = "rebase_merge/opstate_cli.rs"]
mod opstate_cli;
#[path = "rebase_merge/bisect_cli.rs"]
mod bisect_cli;
