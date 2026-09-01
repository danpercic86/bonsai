//! Consolidated `ai` integration-test harness (build-time only).
//!
//! Each module below is a former top-level `tests/*.rs` integration binary, moved
//! verbatim into `tests/ai/` and included here as its own module. The 78 test
//! binaries, each statically linking vendored libgit2, collapse into 8 — no test
//! was added, removed, renamed, or changed. Giving every former file its own
//! `mod` keeps its helper symbols namespaced, so identically named helpers across
//! files cannot collide.

mod common;

#[path = "ai/ai_changelog_cli.rs"]
mod ai_changelog_cli;
#[path = "ai/ai_commit_cli.rs"]
mod ai_commit_cli;
#[path = "ai/ai_compose_cli.rs"]
mod ai_compose_cli;
#[path = "ai/ai_digest_cli.rs"]
mod ai_digest_cli;
#[path = "ai/ai_explain_cli.rs"]
mod ai_explain_cli;
#[path = "ai/ai_history_cli.rs"]
mod ai_history_cli;
#[path = "ai/ai_operation_cli.rs"]
mod ai_operation_cli;
#[path = "ai/ai_operation_safety_cli.rs"]
mod ai_operation_safety_cli;
#[path = "ai/ai_pr_description_cli.rs"]
mod ai_pr_description_cli;
#[path = "ai/ai_resolve_cli.rs"]
mod ai_resolve_cli;
#[path = "ai/ai_stream_bulk_cli.rs"]
mod ai_stream_bulk_cli;
#[path = "ai/ai_summary_cli.rs"]
mod ai_summary_cli;
