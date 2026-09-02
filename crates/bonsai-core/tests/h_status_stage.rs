//! Consolidated `status_stage` integration-test harness (build-time only).
//!
//! Each module below is a former top-level `tests/*.rs` integration binary, moved
//! verbatim into `tests/status_stage/` and included here as its own module. The 78 test
//! binaries, each statically linking vendored libgit2, collapse into 8 — no test
//! was added, removed, renamed, or changed. Giving every former file its own
//! `mod` keeps its helper symbols namespaced, so identically named helpers across
//! files cannot collide.

mod common;

#[path = "status_stage/status_porcelain.rs"]
mod status_porcelain;
#[path = "status_stage/stage_cli.rs"]
mod stage_cli;
#[path = "status_stage/commit_cli.rs"]
mod commit_cli;
#[path = "status_stage/commit_noconfig.rs"]
mod commit_noconfig;
#[path = "status_stage/hooks_commit_cli.rs"]
mod hooks_commit_cli;
#[path = "status_stage/hooks_commit_cli_2.rs"]
mod hooks_commit_cli_2;
#[path = "status_stage/branches_cli.rs"]
mod branches_cli;
#[path = "status_stage/branches_cli_2.rs"]
mod branches_cli_2;
#[path = "status_stage/tags_cli.rs"]
mod tags_cli;
#[path = "status_stage/tags_cli_2.rs"]
mod tags_cli_2;
#[path = "status_stage/history_index_cli.rs"]
mod history_index_cli;
#[path = "status_stage/reflog_cli.rs"]
mod reflog_cli;
