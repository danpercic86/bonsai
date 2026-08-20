//! Local-branch deletion (M5 contract §2).

use std::path::Path;

use crate::error::AppError;

use super::open_repo_at;

/// Blocking. Deletes LOCAL branch `name`. Safety gates in order:
/// not-found → `BranchNotFound`; currently checked out → `Git` (race-only
/// backstop, the UI never offers it); not fully merged into HEAD →
/// `UnmergedBranch` (libgit2's `Branch::delete` has `git branch -D`
/// semantics, so the `-d` merged-check is implemented here).
pub fn delete_branch(workdir: &Path, name: &str) -> Result<(), AppError> {
    let repo = open_repo_at(workdir)?;

    let mut branch = match repo.find_branch(name, git2::BranchType::Local) {
        Ok(b) => b,
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(AppError::BranchNotFound(format!("branch '{name}' not found")));
        }
        Err(e) => return Err(e.into()),
    };

    if branch.is_head() {
        return Err(AppError::Git(format!(
            "cannot delete '{name}': it is the currently checked-out branch"
        )));
    }

    let tip = branch
        .get()
        .target()
        .ok_or_else(|| AppError::Git(format!("branch '{name}' has no target commit")))?;

    // Merged = tip reachable from HEAD (strict `git branch -d`-style check
    // against HEAD only). Detached HEAD: the detached commit; unborn HEAD:
    // treat as unmerged.
    let head_oid = match repo.head() {
        Ok(head) => head.target(),
        Err(e)
            if matches!(
                e.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            None
        }
        Err(e) => return Err(e.into()),
    };
    let merged = match head_oid {
        Some(head) => tip == head || repo.graph_descendant_of(head, tip)?,
        None => false,
    };
    if !merged {
        let tip_hex = tip.to_string();
        let short_tip = tip_hex.get(..7).unwrap_or(&tip_hex);
        return Err(AppError::UnmergedBranch(format!(
            "branch '{name}' is not fully merged into HEAD (tip {short_tip}). \
             Bonsai v1 does not force-delete; use `git branch -D {name}` if you are sure."
        )));
    }

    branch.delete()?;
    Ok(())
}
