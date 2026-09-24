//! P119 T-R5: every variant of every §2.6 input type maps to its outcome.
//! The classifiers themselves match exhaustively, so a new variant fails to
//! compile there; this table pins the chosen mapping.

use super::*;

fn paths() -> Vec<String> {
    vec!["a.txt".to_string()]
}

#[test]
fn merge_table() {
    let cases = [
        (MergeOutcome::UpToDate, Some(GitRunOutcome::UpToDate)),
        (
            MergeOutcome::FastForwarded {
                branch: "main".into(),
                to: "abc".into(),
                stashed: false,
            },
            Some(GitRunOutcome::FastForwarded),
        ),
        (
            MergeOutcome::Merged {
                oid: "abc".into(),
                stashed: true,
            },
            Some(GitRunOutcome::Merged),
        ),
        (
            MergeOutcome::Conflicts {
                paths: paths(),
                stashed: false,
            },
            Some(GitRunOutcome::Conflicts),
        ),
        (
            MergeOutcome::StashPopConflicts {
                head: "abc".into(),
                paths: paths(),
            },
            Some(GitRunOutcome::Conflicts),
        ),
    ];
    for (input, want) in cases {
        assert_eq!(merge_outcome(&input), want, "{input:?}");
    }
}

#[test]
fn rebase_table() {
    let cases = [
        (RebaseOutcome::UpToDate, Some(GitRunOutcome::UpToDate)),
        (
            RebaseOutcome::FastForwarded {
                branch: "main".into(),
                to: "abc".into(),
            },
            Some(GitRunOutcome::FastForwarded),
        ),
        (
            RebaseOutcome::Rebased {
                branch: "main".into(),
                head: "abc".into(),
                steps: 2,
                warnings: vec![],
            },
            None,
        ),
        (
            RebaseOutcome::Conflicts {
                paths: paths(),
                current_step: 1,
                total_steps: 2,
            },
            Some(GitRunOutcome::Conflicts),
        ),
    ];
    for (input, want) in cases {
        assert_eq!(rebase_outcome(&input), want, "{input:?}");
    }
}

#[test]
fn cherrypick_and_revert_tables() {
    let picks = [
        (
            CherrypickOutcome::Committed {
                oid: "abc".into(),
                stashed: false,
            },
            None,
        ),
        (
            CherrypickOutcome::Conflicts {
                paths: paths(),
                stashed: true,
            },
            Some(GitRunOutcome::Conflicts),
        ),
        (
            CherrypickOutcome::StashPopConflicts {
                head: "abc".into(),
                paths: paths(),
            },
            Some(GitRunOutcome::Conflicts),
        ),
    ];
    for (input, want) in picks {
        assert_eq!(cherrypick_outcome(&input), want, "{input:?}");
    }
    let reverts = [
        (
            RevertOutcome::Committed {
                oid: "abc".into(),
                stashed: false,
            },
            None,
        ),
        (
            RevertOutcome::Conflicts {
                paths: paths(),
                stashed: false,
            },
            Some(GitRunOutcome::Conflicts),
        ),
        (
            RevertOutcome::StashPopConflicts {
                head: "abc".into(),
                paths: paths(),
            },
            Some(GitRunOutcome::Conflicts),
        ),
    ];
    for (input, want) in reverts {
        assert_eq!(revert_outcome(&input), want, "{input:?}");
    }
}

fn stash_variants() -> Vec<(ApplyStashOutcome, Option<GitRunOutcome>)> {
    vec![
        (ApplyStashOutcome::Applied, None),
        (
            ApplyStashOutcome::Conflicts { paths: paths() },
            Some(GitRunOutcome::Conflicts),
        ),
        (ApplyStashOutcome::ReservedPaths { paths: paths() }, None),
        (
            ApplyStashOutcome::AppliedSkippingReserved { skipped: paths() },
            None,
        ),
        (
            ApplyStashOutcome::AppliedPartially {
                unrestored: paths(),
            },
            None,
        ),
        (
            ApplyStashOutcome::NotApplied {
                message: "boom".into(),
            },
            None,
        ),
    ]
}

#[test]
fn stash_apply_table() {
    for (input, want) in stash_variants() {
        assert_eq!(stash_apply_outcome(&input), want, "{input:?}");
    }
}

#[test]
fn checkout_table() {
    // No stash: the auto-FF alone decides.
    let plain = |ff| CheckoutResult {
        stashed: false,
        fast_forwarded: ff,
        apply: None,
    };
    assert_eq!(checkout_outcome(&plain(false)), None);
    assert_eq!(
        checkout_outcome(&plain(true)),
        Some(GitRunOutcome::FastForwarded)
    );
    // With a stash re-apply: conflicts wins over FF; every other apply result
    // falls back to the FF flag.
    for ff in [false, true] {
        for (apply, stash_want) in stash_variants() {
            let r = CheckoutResult {
                stashed: true,
                fast_forwarded: ff,
                apply: Some(apply),
            };
            let want = if stash_want.is_some() {
                Some(GitRunOutcome::Conflicts)
            } else {
                ff.then_some(GitRunOutcome::FastForwarded)
            };
            assert_eq!(checkout_outcome(&r), want, "{r:?}");
        }
    }
}

#[test]
fn create_here_table() {
    assert_eq!(
        create_here_outcome(&CreateBranchHereResult {
            stashed: false,
            apply: None
        }),
        None
    );
    for (apply, want) in stash_variants() {
        let r = CreateBranchHereResult {
            stashed: true,
            apply: Some(apply),
        };
        assert_eq!(create_here_outcome(&r), want, "{r:?}");
    }
}

#[test]
fn no_outcome_is_always_none() {
    assert_eq!(no_outcome(&()), None);
    assert_eq!(no_outcome(&"anything"), None);
}
