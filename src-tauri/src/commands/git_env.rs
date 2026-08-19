//! `git_env` command (P70): the one-shot git preflight behind the
//! "Git is not available" notice bar.
//!
//! Git-state-free like the P49 external commands — no `repo_path`, no managed
//! state, no `opActive` gating — so the banner's **Re-check** button works even
//! with no repository open and never contends with a running git operation.

use super::shared::*;
use bonsai_core::gitbin::{self, GitAvailability};

/// Resolve the `git` executable and report availability (P70 §4.2).
///
/// NEVER rejects for git state: a missing or unrunnable git is
/// `{ found: false, .. }`, mirroring `check_ai_availability`. The only possible
/// rejection is a task-join failure. Safe to re-invoke — it re-runs the resolver
/// ladder, which is exactly how "install Git, press Re-check" recovers without
/// an app restart.
#[tauri::command]
pub async fn check_git_availability() -> Result<GitAvailability, AppError> {
    // spawn_blocking: the ladder touches the filesystem and (when git resolves)
    // spawns `git --version`, so it must never run on the UI thread.
    tauri::async_runtime::spawn_blocking(gitbin::check_availability)
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))
}
