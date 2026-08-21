//! P82 (F-A7-7): submodule deinit/remove FORCE machinery — the two outcome
//! enums, the dirty-worktree check, and the conditional-`-f` argv builders.
//! Split out of `submodule.rs` to keep that file under the ~500-line limit; the
//! `deinit_submodule` / `remove_submodule` ops themselves stay in `submodule`
//! and call into here. Pure git2 + argv logic, no Tauri types.

use crate::error::AppError;

/// Result of `deinit_submodule` (P82, F-A7-7). Wire: tagged "kind", camelCase
/// (same recipe as [`crate::git::stash::ApplyStashOutcome`]). `DirtyNeedsForce`
/// is returned WITHOUT mutating anything when `force == false` and the submodule
/// worktree is dirty.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SubmoduleDeinitOutcome {
    /// Plain (`force=false`, clean) or forced (`force=true`) deinit succeeded.
    Deinitialized,
    /// `force=false` and the worktree is dirty; nothing was changed. The UI
    /// re-invokes with `force=true` after an explicit danger confirm.
    DirtyNeedsForce,
}

/// Result of `remove_submodule` (P82, F-A7-7). Wire: tagged "kind", camelCase.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SubmoduleRemoveOutcome {
    /// Full teardown succeeded (deinit → `git rm` → drop `.git/modules/<name>`).
    Removed,
    /// `force=false` and the worktree is dirty; nothing was changed.
    DirtyNeedsForce,
}

/// True when submodule `name`'s own worktree/index holds uncommitted work that a
/// force deinit/rm would destroy: staged (`WD_INDEX_MODIFIED`), unstaged
/// (`WD_WD_MODIFIED`), or untracked (`WD_UNTRACKED`) changes inside it. Uses the
/// same status path as `list_submodules` (`submodule_status(name, Ignore::None)`).
/// NOT dirty: uninitialized, absent workdir, or merely out-of-sync (a different
/// but committed pinned commit — no uncommitted work is lost).
pub(crate) fn is_submodule_dirty(
    repo: &git2::Repository,
    name: &str,
) -> Result<bool, AppError> {
    use git2::SubmoduleStatus as S;
    let flags = repo.submodule_status(name, git2::SubmoduleIgnore::None)?;
    Ok(flags.intersects(S::WD_INDEX_MODIFIED | S::WD_WD_MODIFIED | S::WD_UNTRACKED))
}

/// Pure argv for `git submodule deinit [-f] -- <path>`. `-f` is added only when
/// `force` (P82). `path` is ALWAYS the final token, after `--` — never
/// interpolated into a flag — so a space/`;` in it stays one token and can never
/// become a second command.
pub(crate) fn deinit_args(path: &str, force: bool) -> Vec<String> {
    let mut v = vec!["submodule".to_string(), "deinit".to_string()];
    if force {
        v.push("-f".to_string());
    }
    v.push("--".to_string());
    v.push(path.to_string());
    v
}

/// Pure argv for `git rm [-f] -- <path>` (drops the gitlink + .gitmodules entry
/// and stages the removal). `-f` only when `force` (P82). `path` is the final
/// token, after `--`.
pub(crate) fn rm_args(path: &str, force: bool) -> Vec<String> {
    let mut v = vec!["rm".to_string()];
    if force {
        v.push("-f".to_string());
    }
    v.push("--".to_string());
    v.push(path.to_string());
    v
}
