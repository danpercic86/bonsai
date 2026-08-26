//! P55b resolution happy-path + sanitization unit tests. Extracted verbatim
//! from the former inline `mod tests`; shared fixtures live in `test_support`.

use super::test_support::{expect_proposed, expect_unsupported, linear_repo, oid};
use super::*;

// ----------------------------------------------- switch/create/delete/stash/merge

/// The remaining P55b happy paths (complement to §11.6/§11.7): createBranch
/// validates + resolves, deleteBranch rejects current/non-local, stash needs
/// a dirty tree, merge needs a resolvable branch.
#[test]
fn create_delete_stash_merge_resolution() {
    let (dir, a, b) = linear_repo();
    let p = dir.path();
    let repo = git2::Repository::open(p).expect("open");
    let head_branch = repo.head().expect("head").shorthand().expect("sh").to_string();

    // createBranch at HEAD (at_oid = None), Safe.
    let op = expect_proposed(
        resolve_intent(
            &repo,
            AiOpIntent::CreateBranch {
                name: "feat/x".to_string(),
                at_commit: None,
            },
            None,
        )
        .expect("Ok"),
    );
    match &op.op {
        SafeOp::CreateBranch { name, at_oid } => {
            assert_eq!(name, "feat/x");
            assert_eq!(at_oid, &None, "no atCommit → create at HEAD");
        }
        other => panic!("expected CreateBranch, got {other:?}"),
    }
    assert!(matches!(op.preview.danger, DangerLevel::Safe));

    // createBranch atCommit resolves to a full oid.
    let short_a: String = a.chars().take(7).collect();
    let op = expect_proposed(
        resolve_intent(
            &repo,
            AiOpIntent::CreateBranch {
                name: "feat/at-a".to_string(),
                at_commit: Some(short_a),
            },
            None,
        )
        .expect("Ok"),
    );
    match &op.op {
        SafeOp::CreateBranch { at_oid, .. } => assert_eq!(at_oid.as_deref(), Some(a.as_str())),
        other => panic!("expected CreateBranch, got {other:?}"),
    }

    // Invalid branch name → Unsupported.
    let bad = resolve_intent(
        &repo,
        AiOpIntent::CreateBranch {
            name: "bad name~with^junk".to_string(),
            at_commit: None,
        },
        None,
    )
    .expect("Ok");
    assert!(expect_unsupported(bad).contains("isn't a valid branch name"));

    // deleteBranch: the CURRENT branch → Unsupported.
    let cur = resolve_intent(
        &repo,
        AiOpIntent::DeleteBranch {
            branch: head_branch.clone(),
        },
        None,
    )
    .expect("Ok");
    assert!(expect_unsupported(cur).contains("current branch"));

    // deleteBranch: a non-local name → Unsupported.
    let missing = resolve_intent(
        &repo,
        AiOpIntent::DeleteBranch {
            branch: "ghost".to_string(),
        },
        None,
    )
    .expect("Ok");
    assert!(expect_unsupported(missing).contains("no local branch"));

    // deleteBranch: a real local, non-current branch → Caution DeleteBranch.
    let head_c = repo.find_commit(oid(&b)).expect("B");
    repo.branch("stale", &head_c, false).expect("branch");
    let op = expect_proposed(
        resolve_intent(
            &repo,
            AiOpIntent::DeleteBranch {
                branch: "stale".to_string(),
            },
            None,
        )
        .expect("Ok"),
    );
    match &op.op {
        SafeOp::DeleteBranch { name } => assert_eq!(name, "stale"),
        other => panic!("expected DeleteBranch, got {other:?}"),
    }
    assert!(matches!(op.preview.danger, DangerLevel::Caution));

    // stashChanges on a CLEAN tree → Unsupported.
    let clean = resolve_intent(
        &repo,
        AiOpIntent::StashChanges {
            message: None,
            include_untracked: false,
        },
        None,
    )
    .expect("Ok");
    assert!(expect_unsupported(clean).contains("no changes to stash"));

    // Dirty the tree → stashChanges proposes a Safe stash.
    std::fs::write(p.join("a.txt"), "dirty\n").expect("edit");
    let op = expect_proposed(
        resolve_intent(
            &repo,
            AiOpIntent::StashChanges {
                message: Some("wip".to_string()),
                include_untracked: false,
            },
            None,
        )
        .expect("Ok"),
    );
    match &op.op {
        SafeOp::Stash {
            message,
            include_untracked,
        } => {
            assert_eq!(message.as_deref(), Some("wip"));
            assert!(!include_untracked);
        }
        other => panic!("expected Stash, got {other:?}"),
    }
    assert!(matches!(op.preview.danger, DangerLevel::Safe));

    // mergeBranch: unresolvable → Unsupported; resolvable local → Caution Merge.
    let bad = resolve_intent(
        &repo,
        AiOpIntent::MergeBranch {
            branch: "ghost".to_string(),
        },
        None,
    )
    .expect("Ok");
    assert!(expect_unsupported(bad).contains("couldn't find a branch"));

    let op = expect_proposed(
        resolve_intent(
            &repo,
            AiOpIntent::MergeBranch {
                branch: "stale".to_string(),
            },
            None,
        )
        .expect("Ok"),
    );
    match &op.op {
        SafeOp::Merge { name } => assert_eq!(name, "stale"),
        other => panic!("expected Merge, got {other:?}"),
    }
    assert!(matches!(op.preview.danger, DangerLevel::Caution));
}

// ------------------------------------------- F-A2-1 model-echo sanitization

/// F-A2-1: model-derived text surfaced to the UI is sanitized — the
/// Unsupported.reason passthrough and the branch/commit echoes in resolver
/// messages strip control/bidi chars and are length-capped.
#[test]
fn model_echoes_are_sanitized() {
    let (dir, _a, _b) = linear_repo();
    let repo = git2::Repository::open(dir.path()).expect("open");

    // Unsupported.reason passthrough: controls + bidi stripped, capped.
    let evil = format!(
        "run\u{202e}\x1b[31m rm -rf\n{}",
        "A".repeat(500)
    );
    let reason = expect_unsupported(
        resolve_intent(&repo, AiOpIntent::Unsupported { reason: evil }, None).expect("Ok"),
    );
    assert!(!reason.contains('\u{202e}'), "bidi stripped: {reason:?}");
    assert!(!reason.contains('\x1b'), "ESC stripped: {reason:?}");
    assert!(!reason.contains('\n'), "newline replaced: {reason:?}");
    assert!(reason.chars().count() <= 201, "capped: {}", reason.chars().count());
    assert!(reason.ends_with('…'), "truncation marker present");

    // Branch echo in an Unsupported message: bidi/control chars removed.
    let reason = expect_unsupported(
        resolve_intent(
            &repo,
            AiOpIntent::SwitchBranch {
                branch: "gh\u{202e}\x07ost".to_string(),
            },
            None,
        )
        .expect("Ok"),
    );
    assert!(reason.contains("'ghost'"), "sanitized echo, got: {reason:?}");

    // Commit echo: a non-hex spec is gated (F-A2-2) and echoed sanitized.
    let reason = expect_unsupported(
        resolve_intent(
            &repo,
            AiOpIntent::ResetToCommit {
                commit: "HEAD~1\u{2066}\n".to_string(),
                keep_changes: true,
            },
            None,
        )
        .expect("Ok"),
    );
    assert!(reason.contains("'HEAD~1 '"), "sanitized echo, got: {reason:?}");
}
