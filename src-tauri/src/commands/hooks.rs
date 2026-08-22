//! First-time per-repo git-hook execution disclosure (backend half).
//!
//! `bonsai.runHooks` defaults **true**, so opening a pre-existing on-disk repo
//! and committing / merging / pushing silently runs whatever lives in
//! `.git/hooks`. This module supplies the two commands behind a one-time,
//! per-repo UX disclosure: [`get_repo_hooks_disclosure`] reports whether the
//! repo has runnable hooks and whether the user already acknowledged them, and
//! [`ack_repo_hooks`] records the acknowledgement durably. The gate itself lives
//! in the frontend (`useHookDisclosure`) — hooks do nothing git itself would not
//! do, so no backend refusal of the commit is added.
//!
//! Hosted in its own module (SRP) rather than grown into `merge.rs`. Detection
//! is `bonsai_core::git::hooks::repo_has_runnable_hooks`; persistence mirrors the
//! per-repo forge overrides (`settings::hooks_ack_repos`, keyed by canonical
//! workdir path). Every blocking call (git2 + fs + settings I/O) runs under
//! `spawn_blocking`.

use std::path::Path;

use super::shared::*;

/// Whether a repo has runnable git hooks and whether the user has been shown the
/// one-time disclosure. Both fields are computed by [`get_repo_hooks_disclosure`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoHooksDisclosure {
    /// `repo_has_runnable_hooks(workdir)` — the repo has ≥1 hook Bonsai would run.
    pub has_hooks: bool,
    /// The user already acknowledged this repo's hook disclosure (persisted).
    pub acknowledged: bool,
}

/// Whether this repo has runnable git hooks and whether the user has already
/// acknowledged the one-time disclosure. NO mutation. Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn get_repo_hooks_disclosure(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<RepoHooksDisclosure, AppError> {
    let file = settings::settings_file(&app)?;
    get_repo_hooks_disclosure_inner(state.inner(), &file, &repo_id).await
}

/// Runtime-free core of [`get_repo_hooks_disclosure`] (mirrors
/// `forge_set_repo_account_inner`): resolve the workdir, then compute detection +
/// the persisted ack inside one blocking task.
pub(crate) async fn get_repo_hooks_disclosure_inner(
    state: &AppState,
    settings_file: &Path,
    repo_id: &str,
) -> Result<RepoHooksDisclosure, AppError> {
    let workdir = repo_path(state, repo_id)?;
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let has_hooks = bonsai_core::git::hooks::repo_has_runnable_hooks(&workdir);
        let s = settings::load_from(&file);
        let workdir_str = workdir.to_string_lossy().to_string();
        let acknowledged = settings::hooks_ack_contains(&s, &workdir_str);
        Ok(RepoHooksDisclosure { has_hooks, acknowledged })
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Record that the user acknowledged this repo's hook disclosure (persisted,
/// per-repo). Idempotent. Errors: `noRepo` | `git` | `other`.
#[tauri::command]
pub async fn ack_repo_hooks(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    let file = settings::settings_file(&app)?;
    ack_repo_hooks_inner(state.inner(), &file, &repo_id).await
}

/// Runtime-free core of [`ack_repo_hooks`].
pub(crate) async fn ack_repo_hooks_inner(
    state: &AppState,
    settings_file: &Path,
    repo_id: &str,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let workdir_str = workdir.to_string_lossy().to_string();
        let _ = settings::update(&file, |s| settings::set_hooks_ack(s, &workdir_str));
        Ok(())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::tests_support::*;

    fn settings_file(dir: &tempfile::TempDir) -> std::path::PathBuf {
        dir.path().join("settings.json")
    }

    /// Writes an executable `pre-commit` into the repo's `.git/hooks`.
    fn write_exec_pre_commit(workdir: &Path) {
        let hooks = workdir.join(".git").join("hooks");
        std::fs::create_dir_all(&hooks).expect("mkdir hooks");
        let path = hooks.join("pre-commit");
        std::fs::write(&path, "#!/bin/sh\nexit 0\n").expect("write hook");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).expect("meta").permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).expect("chmod");
        }
    }

    /// A hookless repo reports `hasHooks:false`; ack flips `acknowledged` and
    /// persists (both fields are exercised across the round-trip).
    #[test]
    fn disclosure_reports_both_fields_and_ack_persists() {
        let state = AppState::default();
        let sdir = tempfile::TempDir::new().expect("settings dir");
        let sfile = settings_file(&sdir);
        let (_dir, id, _c0) = fixture_repo(&state);

        let before =
            tauri::async_runtime::block_on(get_repo_hooks_disclosure_inner(&state, &sfile, &id))
                .expect("disclosure");
        assert!(!before.has_hooks, "fresh fixture repo has no hooks");
        assert!(!before.acknowledged, "not acknowledged yet");

        tauri::async_runtime::block_on(ack_repo_hooks_inner(&state, &sfile, &id)).expect("ack");

        let after =
            tauri::async_runtime::block_on(get_repo_hooks_disclosure_inner(&state, &sfile, &id))
                .expect("disclosure");
        assert!(after.acknowledged, "ack must persist through settings.json");
        // Independent proof the ack landed on disk keyed by the workdir.
        let s = settings::load_from(&sfile);
        assert_eq!(s.hooks_ack_repos.len(), 1);
    }

    /// An executable pre-commit ⇒ `hasHooks:true`.
    #[test]
    fn disclosure_detects_runnable_hook() {
        let state = AppState::default();
        let sdir = tempfile::TempDir::new().expect("settings dir");
        let sfile = settings_file(&sdir);
        let (dir, id, _c0) = fixture_repo(&state);
        write_exec_pre_commit(dir.path());

        let d =
            tauri::async_runtime::block_on(get_repo_hooks_disclosure_inner(&state, &sfile, &id))
                .expect("disclosure");
        assert!(d.has_hooks, "an executable pre-commit must be detected");
    }

    /// Both commands reject an unknown repo id with `NoRepo`.
    #[test]
    fn unknown_repo_is_no_repo() {
        let state = AppState::default();
        let sdir = tempfile::TempDir::new().expect("settings dir");
        let sfile = settings_file(&sdir);

        let got =
            tauri::async_runtime::block_on(get_repo_hooks_disclosure_inner(&state, &sfile, MISSING_ID));
        assert!(matches!(got, Err(AppError::NoRepo)));

        let acked = tauri::async_runtime::block_on(ack_repo_hooks_inner(&state, &sfile, MISSING_ID));
        assert!(matches!(acked, Err(AppError::NoRepo)));
    }
}
