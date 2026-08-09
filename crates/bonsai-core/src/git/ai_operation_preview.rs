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

    // -------------------- T2 Area 2 (F-A2-4): total coverage over build_preview

    use crate::git::commit::create_commit;
    use crate::git::stage::stage_paths;

    /// Linear repo of `n` commits on the default branch. Returns (dir, oids).
    fn linear(n: usize) -> (tempfile::TempDir, Vec<String>) {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "Test User").expect("name");
            cfg.set_str("user.email", "test@example.com").expect("email");
            cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        }
        let p = dir.path();
        let mut oids = Vec::new();
        for i in 0..n {
            std::fs::write(p.join("a.txt"), format!("v{i}\n")).expect("write");
            stage_paths(p, &["a.txt".to_string()]).expect("stage");
            oids.push(create_commit(p, &format!("c{i}"), None, false).expect("commit").oid);
        }
        (dir, oids)
    }

    /// build_preview is TOTAL over all 8 SafeOp variants: each returns Ok with the
    /// expected danger tier / title (the six P55b simple ops + reset + revert).
    #[test]
    fn build_preview_is_total_over_all_eight_variants() {
        let (dir, oids) = linear(2);
        let repo = git2::Repository::open(dir.path()).expect("open");
        let full = oids[0].clone();
        let short: String = full.chars().take(7).collect();

        let cases: Vec<(SafeOp, &str, DangerLevel)> = vec![
            (
                SafeOp::Reset { target_oid: full.clone(), target_short: short.clone(), mode: ResetMode::Mixed },
                "Reset branch",
                DangerLevel::Caution,
            ),
            (SafeOp::Revert { oid: full.clone(), short: short.clone() }, "Revert commit", DangerLevel::Caution),
            (SafeOp::SwitchBranch { name: "x".into(), remote: false }, "Switch branch", DangerLevel::Safe),
            (SafeOp::CreateBranch { name: "x".into(), at_oid: None }, "Create branch", DangerLevel::Safe),
            (SafeOp::DeleteBranch { name: "x".into() }, "Delete branch", DangerLevel::Caution),
            (SafeOp::Stash { message: None, include_untracked: false }, "Stash changes", DangerLevel::Safe),
            (SafeOp::Discard { paths: vec!["a.txt".into()] }, "Discard changes", DangerLevel::Destructive),
            (SafeOp::Merge { name: "x".into() }, "Merge branch", DangerLevel::Caution),
        ];
        for (op, title, danger) in cases {
            let pv = build_preview(&repo, &op).unwrap_or_else(|e| panic!("{title} previews: {e:?}"));
            assert_eq!(pv.title, title);
            assert_eq!(
                std::mem::discriminant(&pv.danger),
                std::mem::discriminant(&danger),
                "{title} danger tier"
            );
        }
    }

    /// Reset preview: Hard ⇒ Destructive + worktree warning; Mixed ⇒ Caution, no
    /// warning. The moving ref points from the current tip to the target short.
    #[test]
    fn reset_preview_hard_vs_mixed() {
        let (dir, oids) = linear(2);
        let repo = git2::Repository::open(dir.path()).expect("open");
        let target: String = oids[0].clone();
        let target_short: String = target.chars().take(7).collect();

        let mk = |mode| SafeOp::Reset {
            target_oid: target.clone(),
            target_short: target_short.clone(),
            mode,
        };

        let hard = build_preview(&repo, &mk(ResetMode::Hard)).expect("hard");
        assert!(matches!(hard.danger, DangerLevel::Destructive));
        assert!(hard.worktree_warning.is_some(), "hard warns about discard");
        assert_eq!(hard.ref_changes[0].to_short, target_short);
        assert_eq!(hard.dropped_commits.len(), 1, "one commit leaves the branch");

        let mixed = build_preview(&repo, &mk(ResetMode::Mixed)).expect("mixed");
        assert!(matches!(mixed.danger, DangerLevel::Caution));
        assert!(mixed.worktree_warning.is_none(), "mixed keeps changes");
    }

    /// dropped_commits is capped at MAX_PREVIEW_DROPPED with a "(+N more)" note in
    /// the summary; the ref_change still records the real move.
    #[test]
    fn reset_preview_caps_dropped_commits() {
        let n = MAX_PREVIEW_DROPPED + 5; // 25 commits
        let (dir, oids) = linear(n);
        let repo = git2::Repository::open(dir.path()).expect("open");
        let target: String = oids[0].clone(); // reset to the ROOT commit
        let target_short: String = target.chars().take(7).collect();
        let pv = build_preview(
            &repo,
            &SafeOp::Reset { target_oid: target, target_short, mode: ResetMode::Mixed },
        )
        .expect("preview");
        assert_eq!(pv.dropped_commits.len(), MAX_PREVIEW_DROPPED, "listed capped");
        // total dropped = n-1 (everything after the root) → overflow of n-1-20.
        let overflow = (n - 1) - MAX_PREVIEW_DROPPED;
        assert!(pv.summary.contains(&format!("(+{overflow} more)")), "summary: {}", pv.summary);
    }

    /// "Impossible" (fail-safe) states: a Reset with a non-hex target oid and a
    /// Revert whose oid resolves to nothing both return a clean Err — never a
    /// panic, never a bogus preview.
    #[test]
    fn build_preview_impossible_states_error_cleanly() {
        let (dir, _oids) = linear(1);
        let repo = git2::Repository::open(dir.path()).expect("open");

        let bad_reset = build_preview(
            &repo,
            &SafeOp::Reset {
                target_oid: "not-hex".to_string(),
                target_short: "nothex".to_string(),
                mode: ResetMode::Hard,
            },
        );
        assert!(bad_reset.is_err(), "non-hex reset target → Err");

        let bad_revert = build_preview(
            &repo,
            &SafeOp::Revert { oid: "f".repeat(40), short: "fffffff".to_string() },
        );
        assert!(bad_revert.is_err(), "unresolvable revert oid → Err");
    }
}
