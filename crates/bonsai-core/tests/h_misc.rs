//! Consolidated `misc` integration-test harness (build-time only).
//!
//! Each module below is a former top-level `tests/*.rs` integration binary, moved
//! verbatim into `tests/misc/` and included here as its own module. The 78 test
//! binaries, each statically linking vendored libgit2, collapse into 8 — no test
//! was added, removed, renamed, or changed. Giving every former file its own
//! `mod` keeps its helper symbols namespaced, so identically named helpers across
//! files cannot collide.

mod common;

#[path = "misc/cli_crosscheck.rs"]
mod cli_crosscheck;
#[path = "misc/graph_adversarial.rs"]
mod graph_adversarial;
#[path = "misc/perf_gate.rs"]
mod perf_gate;
#[path = "misc/stream_perf.rs"]
mod stream_perf;
#[path = "misc/external_spawn.rs"]
mod external_spawn;
#[path = "misc/lifecycle_cli.rs"]
mod lifecycle_cli;
#[path = "misc/m3_adversarial.rs"]
mod m3_adversarial;
#[path = "misc/m5_adversarial.rs"]
mod m5_adversarial;
#[path = "misc/m6_adversarial.rs"]
mod m6_adversarial;
#[path = "misc/essentials_cli.rs"]
mod essentials_cli;
#[path = "misc/essentials_error_paths.rs"]
mod essentials_error_paths;
