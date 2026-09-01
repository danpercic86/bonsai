//! Consolidated `prop` integration-test harness (build-time only).
//!
//! Each module below is a former top-level `tests/*.rs` integration binary, moved
//! verbatim into `tests/prop/` and included here as its own module. The 78 test
//! binaries, each statically linking vendored libgit2, collapse into 8 — no test
//! was added, removed, renamed, or changed. Giving every former file its own
//! `mod` keeps its helper symbols namespaced, so identically named helpers across
//! files cannot collide.

#[path = "prop_common/mod.rs"]
mod prop_common;

#[path = "prop/prop_graph_layout.rs"]
mod prop_graph_layout;
#[path = "prop/prop_history_index.rs"]
mod prop_history_index;
#[path = "prop/prop_intraline.rs"]
mod prop_intraline;
#[path = "prop/prop_stash_roundtrip.rs"]
mod prop_stash_roundtrip;
#[path = "prop/prop_status.rs"]
mod prop_status;
#[path = "prop/corrupt_repo_cli.rs"]
mod corrupt_repo_cli;
#[path = "prop/race_lifecycle_cli.rs"]
mod race_lifecycle_cli;
