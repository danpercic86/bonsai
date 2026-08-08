//! Reset core (P20 contract §3).
//!
//! Pure git2 logic, no Tauri types. Moves the CURRENT branch (HEAD) to a target
//! commit in one of three modes, mirroring `git reset --soft/--mixed/--hard`.
//! Hard is destructive (worktree discarded) — the UI confirms first.

use std::path::Path;

use crate::error::AppError;
use crate::git::bisect::require_no_bisect;
use crate::git::stage::open_workdir_repo;

/// Reset MODE. Wire: "soft" | "mixed" | "hard".
///
/// `Serialize` (added P55a): the AI safe-operation planner embeds a `ResetMode`
/// inside a `SafeOp::Reset` it serializes back to the frontend, so the same
/// camelCase wire strings must round-trip out as well as in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

/// Blocking. Moves the CURRENT branch (HEAD) to `target_oid`.
/// Soft: move ref only. Mixed: ref + index. Hard: ref + index + worktree
/// (destructive — the UI confirms first). Mirrors
/// `git reset --soft/--mixed/--hard <oid>` (P20 contract §3.1).
pub fn reset_branch(workdir: &Path, target_oid: &str, mode: ResetMode) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;

    // A clean detached-HEAD bisect is invisible to `state()` below — refuse.
    require_no_bisect(&repo)?;

    if repo.state() != git2::RepositoryState::Clean {
        return Err(AppError::OperationInProgress(
            "an operation is in progress — finish or abort it first".to_string(),
        ));
    }

    // HEAD must be born (detached HEAD is allowed; the UI gates it).
    match repo.head() {
        Ok(_) => {}
        Err(e)
            if matches!(
                e.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            return Err(AppError::Git(
                "nothing to reset: the repository has no commits yet".to_string(),
            ));
        }
        Err(e) => return Err(e.into()),
    }

    let oid = git2::Oid::from_str(target_oid).map_err(|_| AppError::Git("invalid commit id".to_string()))?;
    let obj = repo.find_object(oid, None)?;
    // Reject non-commit targets by peeling to a commit.
    let commit = obj
        .peel_to_commit()
        .map_err(|_| AppError::Git("not a commit".to_string()))?;

    let kind = match mode {
        ResetMode::Soft => git2::ResetType::Soft,
        ResetMode::Mixed => git2::ResetType::Mixed,
        ResetMode::Hard => git2::ResetType::Hard,
    };
    repo.reset(commit.as_object(), kind, None)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `ResetMode` enum deserializes the exact camelCase wire strings.
    #[test]
    fn reset_mode_deserializes_wire_strings() {
        assert_eq!(
            serde_json::from_str::<ResetMode>("\"soft\"").expect("soft"),
            ResetMode::Soft
        );
        assert_eq!(
            serde_json::from_str::<ResetMode>("\"mixed\"").expect("mixed"),
            ResetMode::Mixed
        );
        assert_eq!(
            serde_json::from_str::<ResetMode>("\"hard\"").expect("hard"),
            ResetMode::Hard
        );
        assert!(serde_json::from_str::<ResetMode>("\"bogus\"").is_err());
    }

    /// Resetting an unborn HEAD refuses with a Git error before any mutation.
    #[test]
    fn reset_on_unborn_head_errors() {
        let dir = crate::testutil::scratch_dir();
        git2::Repository::init(dir.path()).expect("init repo");
        let err = reset_branch(dir.path(), &"0".repeat(40), ResetMode::Soft).expect_err("unborn");
        match err {
            AppError::Git(m) => assert!(m.contains("no commits yet"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }
    }

    /// An invalid oid string surfaces as a Git error.
    #[test]
    fn reset_invalid_oid_errors() {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "Test User").expect("name");
            cfg.set_str("user.email", "test@example.com").expect("email");
        }
        std::fs::write(dir.path().join("a.txt"), "one\n").expect("write");
        crate::git::stage::stage_paths(dir.path(), &["a.txt".to_string()]).expect("stage");
        crate::git::commit::create_commit(dir.path(), "base", None, false).expect("commit");

        let err = reset_branch(dir.path(), "not-a-hex-oid", ResetMode::Mixed).expect_err("bad oid");
        match err {
            AppError::Git(m) => assert!(m.contains("invalid commit id"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }
    }
}
