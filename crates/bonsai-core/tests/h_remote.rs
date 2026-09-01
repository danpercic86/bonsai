//! Consolidated `remote` integration-test harness (build-time only).
//!
//! Each module below is a former top-level `tests/*.rs` integration binary, moved
//! verbatim into `tests/remote/` and included here as its own module. The 78 test
//! binaries, each statically linking vendored libgit2, collapse into 8 — no test
//! was added, removed, renamed, or changed. Giving every former file its own
//! `mod` keeps its helper symbols namespaced, so identically named helpers across
//! files cannot collide.

mod common;

#[path = "remote/remote_cli.rs"]
mod remote_cli;
#[path = "remote/remote_mgmt_cli.rs"]
mod remote_mgmt_cli;
#[path = "remote/force_push_cli.rs"]
mod force_push_cli;
#[path = "remote/bundle_cli.rs"]
mod bundle_cli;
#[path = "remote/stale_cli.rs"]
mod stale_cli;
#[path = "remote/stale_cli_2.rs"]
mod stale_cli_2;
#[path = "remote/signing_cli.rs"]
mod signing_cli;
#[path = "remote/config_cli.rs"]
mod config_cli;
#[path = "remote/health_cli.rs"]
mod health_cli;
