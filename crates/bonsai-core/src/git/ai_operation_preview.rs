//! Read-only PREVIEW builder for a resolved [`SafeOp`] (P55 safety layer L5).
//!
//! Split out of `ai_operation.rs` (file-size discipline, P55b). Given a
//! fully-resolved op, compute exactly what confirming it will change —
//! affected refs, dropped/added commits, worktree impact, danger tier — using
//! ONLY revwalk / revparse / branch-lookup. It **mutates NOTHING** (the
//! `plan_never_mutates` guarantee in `ai_operation` covers this path too).
//!
//! [`build_preview`] is TOTAL over every [`SafeOp`] variant. The reset/revert
//! family gets a generic preview that the resolvers
//! ([`crate::git::ai_operation_resolve`]) refine (title/summary/danger) per
//! intent; the six P55b ops (switch/create/delete/stash/discard/merge) get a
//! complete, display-ready preview here so their resolvers stay thin.

use crate::error::AppError;
use crate::git::ai_operation::{
    current_branch_name, revparse_commit, sanitize_model_text, short7, summary_of, CommitRef,
    DangerLevel, OperationPreview, RefChange, SafeOp, MAX_PREVIEW_DROPPED,
};
use crate::git::reset::ResetMode;

/// Read-only preview for a resolved op (revwalk / revparse / branch-lookup only
/// — NO mutation). For a `Reset`, `dropped` = commits reachable from the old
/// tip but not the target (`target..oldtip`), capped at [`MAX_PREVIEW_DROPPED`].
/// Reset/Revert resolvers refine the display fields (title/summary/danger/
/// confirm_label) afterward; the six P55b ops are returned complete.
pub(crate) fn build_preview(
    repo: &git2::Repository,
    op: &SafeOp,
) -> Result<OperationPreview, AppError> {
    match op {
        SafeOp::Reset {
            target_oid,
            target_short,
            mode,
        } => {
            let head = repo.head()?.peel_to_commit()?;
            let tip_oid = head.id();
            let branch = current_branch_name(repo);
            let target = git2::Oid::from_str(target_oid)
                .map_err(|_| AppError::Git("invalid target oid".to_string()))?;
            let (dropped, total) = dropped_commits(repo, target, tip_oid)?;
            let hard = matches!(mode, ResetMode::Hard);
            let danger = if hard {
                DangerLevel::Destructive
            } else {
                DangerLevel::Caution
            };
            let worktree_warning = if hard {
                Some("This permanently discards any uncommitted changes in your working tree.".to_string())
            } else {
                None
            };
            let more = (total as usize).saturating_sub(dropped.len());
            let more_note = if more > 0 {
                format!(" (+{more} more)")
            } else {
                String::new()
            };
            let summary = format!(
                "Move `{branch}` from {} to {target_short}. {total} commit(s) leave the branch{more_note}.",
                short7(tip_oid)
            );
            Ok(OperationPreview {
                title: "Reset branch".to_string(),
                summary,
                danger,
                ref_changes: vec![RefChange {
                    name: branch,
                    from_short: short7(tip_oid),
                    to_short: target_short.clone(),
                }],
                dropped_commits: dropped,
                added_commits: 0,
                worktree_warning,
                confirm_label: "Reset".to_string(),
            })
        }
        SafeOp::Revert { oid, short } => {
            let branch = current_branch_name(repo);
            let target = revparse_commit(repo, oid)
                .ok_or_else(|| AppError::Git("revert target not found".to_string()))?;
            let subject = summary_of(&target);
            Ok(OperationPreview {
                title: "Revert commit".to_string(),
                summary: format!(
                    "Add a new commit to `{branch}` that undoes {short} (\"{subject}\")."
                ),
                danger: DangerLevel::Caution,
                ref_changes: Vec::new(),
                dropped_commits: Vec::new(),
                added_commits: 1,
                worktree_warning: None,
                confirm_label: "Revert".to_string(),
            })
        }
        SafeOp::SwitchBranch { name, remote } => {
            let extra = if *remote {
                " (creating a local tracking branch)"
            } else {
                ""
            };
            Ok(simple_preview(
                "Switch branch",
                format!(
                    "Switch to `{name}`{extra}, auto-stashing and restoring any uncommitted changes."
                ),
                DangerLevel::Safe,
                "Switch",
            ))
        }
        SafeOp::CreateBranch { name, at_oid } => {
            let at = match at_oid {
                Some(oid) => {
                    let short: String = oid.chars().take(7).collect();
                    format!("starting at {short}")
                }
                None => "at the current commit (HEAD)".to_string(),
            };
            Ok(simple_preview(
                "Create branch",
                format!("Create a new branch `{name}` {at}."),
                DangerLevel::Safe,
                "Create branch",
            ))
        }
        SafeOp::DeleteBranch { name } => Ok(simple_preview(
            "Delete branch",
            format!("Delete the local branch `{name}`."),
            DangerLevel::Caution,
            "Delete branch",
        )),
        SafeOp::Stash {
            message,
            include_untracked,
        } => {
            let untracked = if *include_untracked {
                " (including untracked files)"
            } else {
                ""
            };
            // F-A2-1: the stash message is model text — sanitize before display.
            let named = match message {
                Some(m) if !m.trim().is_empty() => {
                    format!(" as \"{}\"", sanitize_model_text(m.trim()))
                }
                _ => String::new(),
            };
            Ok(simple_preview(
                "Stash changes",
                format!("Stash your uncommitted changes{untracked}{named} for later."),
                DangerLevel::Safe,
                "Stash",
            ))
        }
        SafeOp::Discard { paths } => {
            let n = paths.len();
            let plural = if n == 1 { "" } else { "s" };
            let mut preview = simple_preview(
                "Discard changes",
                format!("Permanently discard your uncommitted changes to {n} file{plural}."),
                DangerLevel::Destructive,
                "Discard changes",
            );
            // F-A2-3: cap the listed paths like MAX_PREVIEW_DROPPED — an
            // unbounded join could balloon the IPC/dialog payload.
            let listed: Vec<&str> = paths
                .iter()
                .take(MAX_PREVIEW_DROPPED)
                .map(String::as_str)
                .collect();
            let more = n.saturating_sub(listed.len());
            let more_note = if more > 0 {
                format!(" (+{more} more)")
            } else {
                String::new()
            };
            preview.worktree_warning = Some(format!(
                "This permanently discards uncommitted changes to {}{more_note}.",
                listed.join(", ")
            ));
            Ok(preview)
        }
        SafeOp::Merge { name } => Ok(simple_preview(
            "Merge branch",
            format!(
                "Merge `{name}` into the current branch. This may create a merge commit or pause on conflicts."
            ),
            DangerLevel::Caution,
            "Merge",
        )),
    }
}

/// A preview with no ref move, no dropped/added commits, and no worktree
/// warning — the shape the six P55b ops share (each varies only in
/// title/summary/danger/confirm-label; `Discard` sets its own warning after).
fn simple_preview(
    title: &str,
    summary: String,
    danger: DangerLevel,
    confirm_label: &str,
) -> OperationPreview {
    OperationPreview {
        title: title.to_string(),
        summary,
        danger,
        ref_changes: Vec::new(),
        dropped_commits: Vec::new(),
        added_commits: 0,
        worktree_warning: None,
        confirm_label: confirm_label.to_string(),
    }
}

/// Commits reachable from `tip` but not `target` (`target..tip`), newest first,
/// capped at [`MAX_PREVIEW_DROPPED`]. Returns `(listed, total)` — `total` is the
/// pre-cap count for the "(+N more)" note.
fn dropped_commits(
    repo: &git2::Repository,
    target: git2::Oid,
    tip: git2::Oid,
) -> Result<(Vec<CommitRef>, u32), AppError> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
    walk.push(tip)?;
    walk.hide(target)?;
    let mut listed = Vec::new();
    let mut total = 0u32;
    for oid in walk {
        let oid = oid?;
        total = total.saturating_add(1);
        if listed.len() < MAX_PREVIEW_DROPPED {
            let commit = repo.find_commit(oid)?;
            listed.push(CommitRef {
                short: short7(oid),
                summary: summary_of(&commit),
            });
        }
    }
    Ok((listed, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bare-bones repo with one commit (build_preview's Discard arm never
    /// touches the repo, but the signature requires one).
    fn tiny_repo() -> tempfile::TempDir {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        dir
    }

    /// F-A2-3: the Discard worktree warning lists at most MAX_PREVIEW_DROPPED
    /// paths and collapses the rest into a "(+N more)" note; at or under the
    /// cap there is no note.
    #[test]
    fn discard_warning_caps_listed_paths() {
        let dir = tiny_repo();
        let repo = git2::Repository::open(dir.path()).expect("open");

        // 25 paths → 20 listed + "(+5 more)".
        let paths: Vec<String> = (0..25).map(|i| format!("f{i:02}.txt")).collect();
        let preview =
            build_preview(&repo, &SafeOp::Discard { paths: paths.clone() }).expect("preview");
        let warn = preview.worktree_warning.expect("warning present");
        assert!(warn.contains("f00.txt"), "first path listed: {warn}");
        assert!(warn.contains("f19.txt"), "20th path listed: {warn}");
        assert!(!warn.contains("f20.txt"), "21st path NOT listed: {warn}");
        assert!(warn.contains("(+5 more)"), "overflow note: {warn}");
        assert!(preview.summary.contains("25 file"), "summary keeps the real count");

        // Exactly at the cap → all listed, no note.
        let paths: Vec<String> = (0..MAX_PREVIEW_DROPPED).map(|i| format!("g{i}.txt")).collect();
        let preview = build_preview(&repo, &SafeOp::Discard { paths }).expect("preview");
        let warn = preview.worktree_warning.expect("warning present");
        assert!(warn.contains("g19.txt"), "all paths listed: {warn}");
        assert!(!warn.contains("more)"), "no overflow note at the cap: {warn}");
    }

    /// F-A2-1: the stash message (free model text) is sanitized in the preview
    /// summary — bidi/control chars stripped, length capped.
    #[test]
    fn stash_message_is_sanitized_in_preview() {
        let dir = tiny_repo();
        let repo = git2::Repository::open(dir.path()).expect("open");
        let preview = build_preview(
            &repo,
            &SafeOp::Stash {
                message: Some(format!("wip\u{202e}\x1b[0m {}", "x".repeat(400))),
                include_untracked: false,
            },
        )
        .expect("preview");
        assert!(!preview.summary.contains('\u{202e}'), "bidi stripped");
        assert!(!preview.summary.contains('\x1b'), "ESC stripped");
        assert!(preview.summary.contains('…'), "long message truncated");
    }
}
