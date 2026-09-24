//! P119 T-R1: every category and outcome serializes to its exact wire string
//! (the TS union in `src/ipc/types/activity.ts` mirrors this list verbatim).

use super::*;

/// Wire strings in `ALL` (declaration) order.
const WIRE: [&str; 63] = [
    "commit",
    "amend",
    "mergeCommit",
    "push",
    "forcePush",
    "fetch",
    "pull",
    "checkoutBranch",
    "checkoutCommit",
    "checkoutRemote",
    "createBranch",
    "deleteBranch",
    "deleteBranches",
    "renameBranch",
    "deleteRemoteTracking",
    "merge",
    "abortMerge",
    "rebase",
    "interactiveRebase",
    "rebaseContinue",
    "rebaseSkip",
    "rebaseAbort",
    "cherryPick",
    "cherryPickContinue",
    "cherryPickAbort",
    "revert",
    "revertContinue",
    "revertAbort",
    "resetSoft",
    "resetMixed",
    "resetHard",
    "stashCreate",
    "stashApply",
    "stashPop",
    "stashDrop",
    "createTag",
    "deleteTag",
    "pushTag",
    "deleteRemoteTag",
    "forceRefreshTag",
    "submoduleAdd",
    "submoduleInit",
    "submoduleUpdate",
    "submoduleSync",
    "submoduleDeinit",
    "submoduleRemove",
    "worktreeAdd",
    "worktreeRemove",
    "worktreeLock",
    "worktreeUnlock",
    "discard",
    "bisectStart",
    "bisectGood",
    "bisectBad",
    "bisectSkip",
    "bisectReset",
    "composeCommits",
    "cloneRepo",
    "initRepo",
    "addRemote",
    "removeRemote",
    "renameRemote",
    "setRemoteUrl",
];

#[test]
fn every_category_has_its_exact_wire_string() {
    for (cat, wire) in GitActivityCategory::ALL.iter().zip(WIRE) {
        let json = serde_json::to_value(cat).expect("json");
        assert_eq!(json, serde_json::Value::String(wire.to_string()), "{cat:?}");
    }
}

#[test]
fn all_is_duplicate_free() {
    for (i, a) in GitActivityCategory::ALL.iter().enumerate() {
        for b in &GitActivityCategory::ALL[i + 1..] {
            assert_ne!(a, b, "ALL lists {a:?} twice");
        }
    }
}

#[test]
fn outcome_wire_strings() {
    let cases = [
        (GitRunOutcome::Conflicts, "conflicts"),
        (GitRunOutcome::FastForwarded, "fastForwarded"),
        (GitRunOutcome::Merged, "merged"),
        (GitRunOutcome::UpToDate, "upToDate"),
    ];
    for (outcome, wire) in cases {
        let json = serde_json::to_value(outcome).expect("json");
        assert_eq!(json, serde_json::Value::String(wire.to_string()));
    }
}
