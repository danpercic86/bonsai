//! Consolidated `diff` integration-test harness (build-time only).
//!
//! Each module below is a former top-level `tests/*.rs` integration binary, moved
//! verbatim into `tests/diff/` and included here as its own module. The 78 test
//! binaries, each statically linking vendored libgit2, collapse into 8 — no test
//! was added, removed, renamed, or changed. Giving every former file its own
//! `mod` keeps its helper symbols namespaced, so identically named helpers across
//! files cannot collide.

mod common;

#[path = "diff/diff_cli.rs"]
mod diff_cli;
#[path = "diff/image_diff_cli.rs"]
mod image_diff_cli;
#[path = "diff/image_diff_cli_2.rs"]
mod image_diff_cli_2;
#[path = "diff/blame_cli.rs"]
mod blame_cli;
#[path = "diff/stage_partial_cli.rs"]
mod stage_partial_cli;
#[path = "diff/discard_partial_cli.rs"]
mod discard_partial_cli;
#[path = "diff/search_cli.rs"]
mod search_cli;
#[path = "diff/search_gitbin_cli.rs"]
mod search_gitbin_cli;
#[path = "diff/assets_cli.rs"]
mod assets_cli;
