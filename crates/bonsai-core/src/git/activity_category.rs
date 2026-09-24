//! P119 §2.1: which operation a git-activity run is, and how a successful run
//! ended. Split out of `activity.rs` (size); re-exported from there, so every
//! existing `git::activity::GitActivityCategory` import path is unchanged.
//!
//! The wire strings are camelCase (`checkoutBranch`, `resetHard`, ...) and are
//! mirrored exactly by the TS union in `src/ipc/types/activity.ts`.

/// Which operation an activity is (set once, on `Started`).
///
/// The first seven are P87 and keep their order; the rest are P119 §1 rows
/// 1–56, in table order. Row 51 is `CloneRepo`, never `Clone`: a variant named
/// `Clone` would shadow the derived trait under a glob import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitActivityCategory {
    // P87
    Commit,
    Amend,
    MergeCommit,
    Push,
    ForcePush,
    Fetch,
    Pull,
    // P119 — branches
    CheckoutBranch,
    CheckoutCommit,
    CheckoutRemote,
    CreateBranch,
    DeleteBranch,
    DeleteBranches,
    RenameBranch,
    DeleteRemoteTracking,
    // merge / rebase / pick / revert
    Merge,
    AbortMerge,
    Rebase,
    InteractiveRebase,
    RebaseContinue,
    RebaseSkip,
    RebaseAbort,
    CherryPick,
    CherryPickContinue,
    CherryPickAbort,
    Revert,
    RevertContinue,
    RevertAbort,
    // reset (three categories, one per mode — P119-ui §9-3)
    ResetSoft,
    ResetMixed,
    ResetHard,
    // stash
    StashCreate,
    StashApply,
    StashPop,
    StashDrop,
    // tags
    CreateTag,
    DeleteTag,
    PushTag,
    DeleteRemoteTag,
    ForceRefreshTag,
    // submodules
    SubmoduleAdd,
    SubmoduleInit,
    SubmoduleUpdate,
    SubmoduleSync,
    SubmoduleDeinit,
    SubmoduleRemove,
    // worktrees
    WorktreeAdd,
    WorktreeRemove,
    WorktreeLock,
    WorktreeUnlock,
    // working tree
    Discard,
    // bisect
    BisectStart,
    BisectGood,
    BisectBad,
    BisectSkip,
    BisectReset,
    // compose / repo / remotes
    ComposeCommits,
    CloneRepo,
    InitRepo,
    AddRemote,
    RemoveRemote,
    RenameRemote,
    SetRemoteUrl,
}

impl GitActivityCategory {
    /// Every category, in declaration order (tests iterate this so a new
    /// variant is covered the moment it is added here).
    pub const ALL: [GitActivityCategory; 63] = {
        use GitActivityCategory::*;
        [
            Commit,
            Amend,
            MergeCommit,
            Push,
            ForcePush,
            Fetch,
            Pull,
            CheckoutBranch,
            CheckoutCommit,
            CheckoutRemote,
            CreateBranch,
            DeleteBranch,
            DeleteBranches,
            RenameBranch,
            DeleteRemoteTracking,
            Merge,
            AbortMerge,
            Rebase,
            InteractiveRebase,
            RebaseContinue,
            RebaseSkip,
            RebaseAbort,
            CherryPick,
            CherryPickContinue,
            CherryPickAbort,
            Revert,
            RevertContinue,
            RevertAbort,
            ResetSoft,
            ResetMixed,
            ResetHard,
            StashCreate,
            StashApply,
            StashPop,
            StashDrop,
            CreateTag,
            DeleteTag,
            PushTag,
            DeleteRemoteTag,
            ForceRefreshTag,
            SubmoduleAdd,
            SubmoduleInit,
            SubmoduleUpdate,
            SubmoduleSync,
            SubmoduleDeinit,
            SubmoduleRemove,
            WorktreeAdd,
            WorktreeRemove,
            WorktreeLock,
            WorktreeUnlock,
            Discard,
            BisectStart,
            BisectGood,
            BisectBad,
            BisectSkip,
            BisectReset,
            ComposeCommits,
            CloneRepo,
            InitRepo,
            AddRemote,
            RemoveRemote,
            RenameRemote,
            SetRemoteUrl,
        ]
    };
}

/// How a SUCCESSFUL run ended, when that is more than "done". Carried on
/// `finished` only; absent on failed runs and on runs with nothing to add.
///
/// `Conflicts` = the command returned `Ok` but left the repo paused on
/// conflicts (the frontend derives its third status from it, P119 §2.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitRunOutcome {
    Conflicts,
    FastForwarded,
    Merged,
    UpToDate,
}

#[cfg(test)]
#[path = "activity_category_tests.rs"]
mod tests;
