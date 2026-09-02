//! Worktree identity + eligibility: resolving the calling worktree's key,
//! its root, and refusing ineligible worktrees or dirty targets.

use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::git::worktree::{canonical, main_workdir};

use super::MAIN_WORKTREE_KEY;

/// Open the repo at `workdir` exactly (no upward search), like
/// `stage::open_workdir_repo` but without the bare check (read-only helper).
pub(super) fn open_repo_at(workdir: &Path) -> Result<git2::Repository, git2::Error> {
    git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )
}

/// The worktree key the D5 wrappers operate under. `"@main"` ONLY when the
/// workdir is not an openable git repo (pure-P24 fallback on a plain folder);
/// for real repos, identity-resolution failures PROPAGATE — a linked worktree
/// whose identity cannot be established must never be silently retargeted to
/// the main worktree's activation slot.
pub(super) fn calling_worktree_key(workdir: &Path) -> Result<String, AppError> {
    if open_repo_at(workdir).is_err() {
        return Ok(MAIN_WORKTREE_KEY.to_string());
    }
    worktree_key_for(workdir)
}

/// P31 D3. `"@main"` for the main worktree, else the linked worktree's NAME.
/// `Err(Git)` when `workdir` is not a git worktree at all or its identity
/// cannot be established.
pub fn worktree_key_for(workdir: &Path) -> Result<String, AppError> {
    let repo = open_repo_at(workdir)
        .map_err(|e| AppError::Git(format!("not a git repository: {}", e.message())))?;
    if !repo.is_worktree() {
        return Ok(MAIN_WORKTREE_KEY.to_string());
    }
    // A linked worktree's gitdir is `<common>/worktrees/<name>` — the basename
    // is the worktree name. Validate it resolves via find_worktree.
    let name = repo
        .path()
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .ok_or_else(|| AppError::Git("cannot determine worktree name".to_string()))?;
    if repo.find_worktree(&name).is_ok() {
        return Ok(name);
    }
    // Fallback: match by canonical workdir path against the worktree list.
    let cur = canonical(workdir);
    for n in repo.worktrees()?.iter().filter_map(Result::ok).flatten() {
        if let Ok(wt) = repo.find_worktree(n) {
            if canonical(wt.path()) == cur {
                return Ok(n.to_string());
            }
        }
    }
    Err(AppError::Git(format!(
        "'{}' matches no worktree of the repository",
        workdir.display()
    )))
}

/// P31 D6 eligibility: a linked worktree must be valid, not prunable, and not
/// locked before we read from or write into its working directory.
fn ensure_eligible(wt: &git2::Worktree, key: &str) -> Result<(), AppError> {
    if wt.validate().is_err() {
        return Err(AppError::Git(format!(
            "worktree '{key}' is invalid (its working directory is missing or broken)"
        )));
    }
    if wt.is_prunable(None).unwrap_or(false) {
        return Err(AppError::Git(format!(
            "worktree '{key}' is stale (prunable); prune or repair it first"
        )));
    }
    if matches!(wt.is_locked()?, git2::WorktreeLockStatus::Locked(_)) {
        return Err(AppError::Git(format!(
            "worktree '{key}' is locked; unlock it first"
        )));
    }
    Ok(())
}

/// Resolve `worktree_key` to the target worktree's ROOT directory, enforcing
/// D6 eligibility for linked worktrees. `"@main"` on a non-repo dir keeps the
/// pure-P24 behavior (`workdir` itself).
pub(super) fn resolve_worktree_root(workdir: &Path, worktree_key: &str) -> Result<PathBuf, AppError> {
    let repo = match open_repo_at(workdir) {
        Ok(r) => r,
        Err(e) if worktree_key == MAIN_WORKTREE_KEY => {
            // Pure-P24 fallback: not a repo → operate on the dir itself.
            let _ = e;
            return Ok(workdir.to_path_buf());
        }
        Err(e) => {
            return Err(AppError::Git(format!(
                "not a git repository: {}",
                e.message()
            )))
        }
    };
    if worktree_key == MAIN_WORKTREE_KEY {
        return main_workdir(&repo);
    }
    let wt = repo.find_worktree(worktree_key).map_err(|e| match e.code() {
        git2::ErrorCode::NotFound => {
            AppError::Git(format!("worktree '{worktree_key}' not found"))
        }
        _ => e.into(),
    })?;
    ensure_eligible(&wt, worktree_key)?;
    Ok(wt.path().to_path_buf())
}

/// P31 D7 dirty-target guard: for each mapped `(rel, proposed)` target in the
/// TARGET worktree, refuse when the file is TRACKED and modified (index or
/// worktree status ≠ CURRENT). Untracked (`WT_NEW`) and clean/missing pass, as
/// does a tracked-modified file whose bytes ALREADY equal the proposed content
/// (the write would be a no-op `Unchanged` — nothing can be lost; this keeps
/// re-activating the same profile idempotent, acceptance §9.3). Checked for
/// ALL targets before ANY write. Pathspec-limited (`status_file`) — never a
/// full-repo scan. Non-repo target dirs (pure-P24) pass trivially.
pub(super) fn ensure_targets_clean(wt_root: &Path, targets: &[(&str, &[u8])]) -> Result<(), AppError> {
    let repo = match open_repo_at(wt_root) {
        Ok(r) => r,
        Err(_) => return Ok(()), // pure-P24 dir: no git safety net to protect
    };
    for (rel, proposed) in targets {
        let status = match repo.status_file(Path::new(rel)) {
            Ok(s) => s,
            // Not found anywhere (HEAD/index/worktree) → nothing to clobber.
            Err(e) if e.code() == git2::ErrorCode::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        // Block only when the file is TRACKED and modified: intersect against
        // the tracked-modified flags rather than exact-equality checks, so
        // untracked files (WT_NEW) and gitignored files (IGNORED) never block —
        // both mean "git has no committed version to protect" — and combined
        // flag sets (e.g. WT_NEW | IGNORED) are handled correctly.
        let tracked_dirty = status.intersects(
            git2::Status::INDEX_NEW
                | git2::Status::INDEX_MODIFIED
                | git2::Status::INDEX_DELETED
                | git2::Status::INDEX_RENAMED
                | git2::Status::INDEX_TYPECHANGE
                | git2::Status::WT_MODIFIED
                | git2::Status::WT_DELETED
                | git2::Status::WT_RENAMED
                | git2::Status::WT_TYPECHANGE
                // A conflicted target (worktree mid-merge) holds unmerged
                // content with no committed safety net — most losable of all.
                | git2::Status::CONFLICTED,
        );
        if !tracked_dirty {
            continue; // clean, untracked, or ignored — nothing of git's to lose
        }
        // Tracked + dirty: allow only the no-op case (bytes already match).
        let full = wt_root.join(rel);
        if full.is_file() && std::fs::read(&full)?.as_slice() == *proposed {
            continue;
        }
        return Err(AppError::Git(format!(
            "worktree has uncommitted changes to {rel}; commit or stash first"
        )));
    }
    Ok(())
}
