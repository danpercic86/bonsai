//! Shared autostash for operations that need a clean tree (merge / cherry-pick /
//! revert). Stashes TRACKED changes (index + worktree), reset to HEAD, so the
//! subsequent checkout cannot clobber the user's edits. On any failure the stash
//! is RETAINED — never silently dropped (no data loss).
//!
//! Extracted from `merge.rs` (P3c) so cherry-pick, revert AND merge share ONE
//! implementation (P47 §2.1, flag F1). `stash_save` is parameterized on the
//! stash `label`.
//!
//! IDENTITY, NOT POSITION (T2.7 F-A7-6): `stash_save` returns the saved stash
//! commit Oid, and `rollback_and_map` / `pop_after_success` locate that Oid on
//! the stack via `stash_foreach` before applying/dropping. A foreign stash
//! pushed between save and pop (external `git stash`, or a second in-app
//! operation) must never cause the WRONG stash to be applied and destroyed;
//! if the Oid is absent the stash operation errors and nothing is dropped.

use std::path::Path;

use crate::error::AppError;
use crate::git::conflict::list_conflicts;

/// Result of re-applying the autostash after a successful operation.
#[derive(Debug)]
pub enum PopResult {
    /// Clean re-apply; the stash was dropped (equivalent to a clean pop).
    Restored,
    /// Re-apply produced conflict markers; the stash is RETAINED on the stack.
    Conflicted(Vec<String>),
}

/// True iff the working tree has any TRACKED change (staged or unstaged).
/// Untracked and ignored files are excluded (mirrors git's autostash default).
pub fn is_dirty(repo: &git2::Repository) -> Result<bool, AppError> {
    let mut so = git2::StatusOptions::new();
    so.include_untracked(false).include_ignored(false);
    Ok(!repo.statuses(Some(&mut so))?.is_empty())
}

/// Autostash tracked (index + worktree) changes with `label`, resetting the
/// tree to HEAD so a subsequent SAFE checkout cannot conflict with the user's
/// own edits. NOT KEEP_INDEX (would leave the tree dirty), NOT INCLUDE_UNTRACKED
/// (matches git's autostash default).
///
/// Returns the saved stash COMMIT Oid — callers must keep it and hand it back
/// to [`rollback_and_map`] / [`pop_after_success`] so the stash is later
/// addressed by identity, never by stack position (F-A7-6).
pub fn stash_save(
    repo: &mut git2::Repository,
    sig: &git2::Signature,
    label: &str,
) -> Result<git2::Oid, AppError> {
    Ok(repo.stash_save2(sig, Some(label), Some(git2::StashFlags::DEFAULT))?)
}

/// Locate the current stack index of stash commit `oid` via `stash_foreach`.
/// `Ok(None)` when the stash is no longer on the stack (dropped externally).
fn find_stash_index(
    repo: &mut git2::Repository,
    oid: git2::Oid,
) -> Result<Option<usize>, AppError> {
    let mut found: Option<usize> = None;
    repo.stash_foreach(|i, _msg, entry_oid| {
        if found.is_none() && *entry_oid == oid {
            found = Some(i);
        }
        true // never early-abort: an abort surfaces as a spurious Err
    })?;
    Ok(found)
}

/// On a mutation failure AFTER `stash_save` but BEFORE the terminal outcome,
/// try to restore the user's original dirty state, then return the original
/// error. Drops the stash ONLY on a genuinely clean restore; if the restore
/// conflicts or fails, the stash is left on the stack and the error message is
/// augmented to say so — never a silent success or a silent drop.
///
/// `stash_oid: None` (nothing was stashed) → passthrough. `Some(oid)` → the
/// stash is located BY IDENTITY first (F-A7-6): a foreign stash pushed on top
/// in the meantime must not make us apply/drop the wrong entry.
pub fn rollback_and_map(
    repo: &mut git2::Repository,
    stash_oid: Option<git2::Oid>,
    err: AppError,
) -> AppError {
    let Some(oid) = stash_oid else {
        return err;
    };
    let base = match &err {
        AppError::CheckoutConflict(m) | AppError::Git(m) => m.clone(),
        other => other.to_string(),
    };
    // Locate our stash by identity. Absent (dropped externally) → nothing we
    // could safely restore or drop; tell the user where to look.
    let index = match find_stash_index(repo, oid) {
        Ok(Some(i)) => i,
        Ok(None) => {
            return AppError::Git(format!(
                "{base} (your autostashed changes were not found; check `git stash list`)"
            ));
        }
        Err(_) => {
            return AppError::Git(format!(
                "{base} (your changes are safe at stash@{{0}})"
            ));
        }
    };
    // Attempt to restore the user's original dirty state. Most callers reach
    // here with a clean tree (just-stashed, or an error path that reset the
    // index), so the apply is clean. A caller that already checked out an
    // incoming tree could conflict on restore.
    //
    // Use stash_apply (NOT stash_pop): this libgit2 applies a *content*
    // conflict as Ok(()) with markers and stash_pop would then silently DROP
    // the stash (data loss). We inspect the index after Ok and only drop on a
    // genuinely clean restore; otherwise the stash is RETAINED and we augment
    // the error to say so — never a silent success.
    let augment = || -> AppError {
        AppError::Git(format!(
            "{base} (your changes are safe at stash@{{{index}}})"
        ))
    };
    match repo.stash_apply(index, Some(&mut git2::StashApplyOptions::new())) {
        Ok(()) => match repo.index() {
            Ok(idx) if !idx.has_conflicts() => {
                // Clean restore → drop the now-redundant stash and return the
                // original error (state is as if nothing happened).
                let _ = repo.stash_drop(index);
                err
            }
            // Conflicted (or unreadable) restore: LEAVE the stash on the stack
            // (never drop) and tell the user where their changes are.
            _ => augment(),
        },
        // Could not auto-restore: stash_apply never drops → stash retained.
        Err(_) => augment(),
    }
}

/// Re-apply the autostash after a SUCCESSFUL finalize. `stash_oid` is the Oid
/// returned by [`stash_save`]; the entry is located BY IDENTITY via
/// `stash_foreach` (F-A7-6) — if it is no longer on the stack (a foreign
/// drop), we error WITHOUT touching any other stash.
///
/// Uses `stash_apply` (NOT `stash_pop`) so WE control dropping. This libgit2
/// version applies a *content* conflict as `Ok(())` — writing conflict markers
/// into the worktree and conflict entries into the index — rather than
/// returning `GIT_ECONFLICT`, and `stash_pop` would then DROP the stash on that
/// silent conflict (data loss). So we must inspect the index after `Ok`, not
/// trust the return code, before deciding to drop. No REINSTATE_INDEX: staged
/// changes return as unstaged.
pub fn pop_after_success(
    repo: &mut git2::Repository,
    workdir: &Path,
    stash_oid: git2::Oid,
) -> Result<PopResult, AppError> {
    let Some(index) = find_stash_index(repo, stash_oid)? else {
        return Err(AppError::Git(
            "operation succeeded, but your autostashed changes were not found; \
             check `git stash list`"
                .to_string(),
        ));
    };
    let mut opts = git2::StashApplyOptions::new();
    match repo.stash_apply(index, Some(&mut opts)) {
        Ok(()) => {
            if repo.index()?.has_conflicts() {
                // Conflicted re-apply: LEAVE the stash on the stack (do NOT
                // drop) → retained for the user to resolve. A failure LISTING
                // the conflicts must not lose the "your changes are safe"
                // message (T2.7 NIT) — fall back to a plain message instead of
                // `?`-propagating a bare error.
                match list_conflicts(workdir) {
                    Ok(list) => Ok(PopResult::Conflicted(
                        list.into_iter().map(|c| c.path).collect(),
                    )),
                    Err(e) => Err(AppError::Git(format!(
                        "operation succeeded, but re-applying your stashed changes \
                         produced conflicts (listing them failed: {e}). Your changes \
                         are safe at stash@{{{index}}}."
                    ))),
                }
            } else {
                // Clean apply → now drop, equivalent to a clean pop.
                repo.stash_drop(index)?;
                Ok(PopResult::Restored)
            }
        }
        // A checkout-level conflict (rare) means nothing droppable was applied
        // → stash retained. When the index holds no conflict entries (nothing
        // was applied at all), an empty Conflicted list would render as
        // "conflicts" with no paths — surface libgit2's message instead (it
        // names the blocking file); the stash stays safe on the stack.
        Err(e) if e.code() == git2::ErrorCode::Conflict => {
            // Best-effort listing (same NIT as above): an error here degrades
            // to the plain "safe at stash@{N}" message below, never a bare `?`.
            let paths: Vec<String> = list_conflicts(workdir)
                .map(|v| v.into_iter().map(|c| c.path).collect())
                .unwrap_or_default();
            if paths.is_empty() {
                return Err(AppError::Git(format!(
                    "operation succeeded, but re-applying your stashed changes was \
                     blocked at checkout: {}. Your changes are safe at stash@{{{index}}}.",
                    e.message()
                )));
            }
            Ok(PopResult::Conflicted(paths))
        }
        // Rare non-conflict failure: the operation HAS ALREADY landed and
        // stash_apply never drops, so the stash is retained. Report success and
        // point the user at their safe stash.
        Err(e) => Err(AppError::Git(format!(
            "operation succeeded, but re-applying your stashed changes failed: {e}. \
             Your changes are safe at stash@{{{index}}}."
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scratch repo with identity + a base commit on `f.txt`.
    fn init_with_base() -> (tempfile::TempDir, git2::Repository) {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = git2::Repository::init(d).expect("init");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "Test User").expect("name");
            cfg.set_str("user.email", "test@example.com").expect("email");
            cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        }
        std::fs::write(d.join("f.txt"), "base\n").expect("write");
        crate::git::stage::stage_paths(d, &["f.txt".to_string()]).expect("stage");
        crate::git::commit::create_commit(d, "base", None, false).expect("commit");
        (dir, repo)
    }

    fn sig() -> git2::Signature<'static> {
        git2::Signature::now("Test User", "test@example.com").expect("sig")
    }

    fn read(d: &Path, name: &str) -> String {
        std::fs::read_to_string(d.join(name)).expect("read")
    }

    /// F-A7-6 regression: a FOREIGN stash pushed between save and pop must not
    /// be the one applied/dropped — the autostash is located by Oid.
    #[test]
    fn pop_after_success_locates_stash_by_oid_not_position() {
        let (dir, mut repo) = init_with_base();
        let d = dir.path();

        // Our autostash (edit A).
        std::fs::write(d.join("f.txt"), "ours\n").expect("edit");
        let oid = stash_save(&mut repo, &sig(), "bonsai: autostash test").expect("save");
        assert_eq!(read(d, "f.txt"), "base\n", "tree clean after autostash");

        // Foreign stash pushed on top (edit B) → ours shifts to stash@{1}.
        std::fs::write(d.join("f.txt"), "foreign\n").expect("edit 2");
        repo.stash_save2(&sig(), Some("foreign"), Some(git2::StashFlags::DEFAULT))
            .expect("foreign stash");

        match pop_after_success(&mut repo, d, oid).expect("pop") {
            PopResult::Restored => {}
            PopResult::Conflicted(p) => panic!("expected clean restore, got conflicts {p:?}"),
        }
        assert_eq!(read(d, "f.txt"), "ours\n", "OUR edit restored, not the foreign one");

        // Exactly the foreign stash survives.
        let list = crate::git::stash::list_stashes(d).expect("list");
        assert_eq!(list.len(), 1, "only the foreign stash remains");
        assert!(
            list[0].message.contains("foreign"),
            "survivor must be the foreign stash, got {:?}",
            list[0].message
        );
    }

    /// F-A7-6: the autostash was dropped externally → error-with-retain, and
    /// the foreign stash on the stack is NOT touched.
    #[test]
    fn pop_after_success_missing_oid_errors_and_touches_nothing() {
        let (dir, mut repo) = init_with_base();
        let d = dir.path();

        std::fs::write(d.join("f.txt"), "ours\n").expect("edit");
        let oid = stash_save(&mut repo, &sig(), "bonsai: autostash test").expect("save");

        // Externally drop ours, then push an unrelated foreign stash.
        repo.stash_drop(0).expect("external drop");
        std::fs::write(d.join("f.txt"), "foreign\n").expect("edit 2");
        repo.stash_save2(&sig(), Some("foreign"), Some(git2::StashFlags::DEFAULT))
            .expect("foreign stash");

        let err = pop_after_success(&mut repo, d, oid).expect_err("must error");
        assert!(
            matches!(&err, AppError::Git(m) if m.contains("were not found")
                && m.contains("git stash list")),
            "got {err:?}"
        );
        let list = crate::git::stash::list_stashes(d).expect("list");
        assert_eq!(list.len(), 1, "foreign stash retained untouched");
        assert!(list[0].message.contains("foreign"));
        assert_eq!(read(d, "f.txt"), "base\n", "worktree untouched");
    }

    /// F-A7-6: rollback restores OUR stash by identity under a foreign stash
    /// pushed on top, returning the ORIGINAL error.
    #[test]
    fn rollback_and_map_restores_by_oid_under_foreign_stash() {
        let (dir, mut repo) = init_with_base();
        let d = dir.path();

        std::fs::write(d.join("f.txt"), "ours\n").expect("edit");
        let oid = stash_save(&mut repo, &sig(), "bonsai: autostash test").expect("save");
        std::fs::write(d.join("f.txt"), "foreign\n").expect("edit 2");
        repo.stash_save2(&sig(), Some("foreign"), Some(git2::StashFlags::DEFAULT))
            .expect("foreign stash");

        let out = rollback_and_map(&mut repo, Some(oid), AppError::Git("boom".to_string()));
        assert!(
            matches!(&out, AppError::Git(m) if m == "boom"),
            "clean restore must return the original error, got {out:?}"
        );
        assert_eq!(read(d, "f.txt"), "ours\n", "OUR edit restored");
        let list = crate::git::stash::list_stashes(d).expect("list");
        assert_eq!(list.len(), 1, "ours dropped after clean restore; foreign retained");
        assert!(list[0].message.contains("foreign"));
    }

    /// rollback with the autostash dropped externally: error names the miss.
    #[test]
    fn rollback_and_map_missing_oid_augments_error() {
        let (dir, mut repo) = init_with_base();
        let d = dir.path();

        std::fs::write(d.join("f.txt"), "ours\n").expect("edit");
        let oid = stash_save(&mut repo, &sig(), "bonsai: autostash test").expect("save");
        repo.stash_drop(0).expect("external drop");

        let out = rollback_and_map(&mut repo, Some(oid), AppError::Git("boom".to_string()));
        assert!(
            matches!(&out, AppError::Git(m) if m.contains("boom")
                && m.contains("were not found") && m.contains("git stash list")),
            "got {out:?}"
        );
    }

    /// `None` passthrough: no stash was made, the error flows through untouched.
    #[test]
    fn rollback_and_map_none_is_passthrough() {
        let (_dir, mut repo) = init_with_base();
        let out = rollback_and_map(&mut repo, None, AppError::Git("boom".to_string()));
        assert!(matches!(&out, AppError::Git(m) if m == "boom"), "got {out:?}");
    }
}
