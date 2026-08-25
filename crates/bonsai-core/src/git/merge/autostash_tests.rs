// ============================================================ P8 §7 matrix
// Autostash-aware merge behavioral matrix. One test per §7 row. Each asserts
// BOTH the returned MergeOutcome AND the on-disk state. Fixtures are scratch
// repos built with git2 (deterministic, no network, no CLI).

use super::p8_helpers::*;
use super::*;
use crate::error::AppError;

// ---- Row 1: Not-dirty FF unchanged (identical to pre-P8) ---------------

/// FF-able upstream, CLEAN tree -> `FastForwarded { stashed: false }`,
/// exactly as P3c. No stash created; HEAD moves to the target.
#[test]
fn p8_1_not_dirty_ff_unchanged() {
    let dir = crate::testutil::scratch_dir();
    let repo = p8_init(dir.path());

    p8_commit(dir.path(), "base", &[("a.txt", "base\n")]);
    let base = repo.find_commit(p8_head_oid(&repo)).expect("base");
    // topic descends from base (adds a file) -> FF-able. HEAD stays on main.
    let topic = p8_commit_on_ref(
        &repo,
        "refs/heads/topic",
        &base,
        &[("feature.txt", "feature\n")],
        "topic advance",
    );
    // Read the real default branch name (libgit2 honors init.defaultBranch,
    // so it may be "master" or "main" depending on machine config).
    let branch = repo
        .head()
        .expect("HEAD")
        .shorthand()
        .expect("shorthand")
        .to_string();

    let outcome = merge_branch(dir.path(), "topic", false).expect("merge");
    assert_eq!(
        outcome,
        MergeOutcome::FastForwarded {
            branch: branch.clone(),
            to: topic.to_string(),
            stashed: false,
        },
        "clean FF must report stashed:false and target = topic tip"
    );

    let repo = git2::Repository::open(dir.path()).expect("reopen");
    assert_eq!(repo.state(), git2::RepositoryState::Clean);
    assert_eq!(p8_head_oid(&repo), topic, "HEAD must move to topic tip");
    assert_eq!(p8_read(dir.path(), "feature.txt"), "feature\n");
    assert_eq!(p8_stash_count(dir.path()), 0, "no stash created on clean FF");
}

// ---- Row 2 (matrix #2): Dirty (unstaged) FF round-trip -----------------

/// Unrelated tracked file edited but UNSTAGED; FF-able upstream ->
/// `FastForwarded { stashed: true }`. HEAD moves to target AND the local
/// edit is present in the worktree afterward (autostash restored).
#[test]
fn p8_2_dirty_unstaged_ff_round_trip() {
    let dir = crate::testutil::scratch_dir();
    let repo = p8_init(dir.path());

    p8_commit(
        dir.path(),
        "base",
        &[("a.txt", "base\n"), ("unrelated.txt", "orig\n")],
    );
    let base = repo.find_commit(p8_head_oid(&repo)).expect("base");
    // topic only ADDS feature.txt -> FF checkout won't touch unrelated.txt.
    let topic = p8_commit_on_ref(
        &repo,
        "refs/heads/topic",
        &base,
        &[("feature.txt", "feature\n")],
        "topic advance",
    );

    let branch = repo
        .head()
        .expect("HEAD")
        .shorthand()
        .expect("shorthand")
        .to_string();

    // Dirty: edit an unrelated tracked file, leave it UNSTAGED.
    std::fs::write(dir.path().join("unrelated.txt"), "locally edited\n")
        .expect("edit unrelated");

    let outcome = merge_branch(dir.path(), "topic", false).expect("merge");
    assert_eq!(
        outcome,
        MergeOutcome::FastForwarded {
            branch: branch.clone(),
            to: topic.to_string(),
            stashed: true,
        },
        "dirty FF must report stashed:true"
    );

    let repo = git2::Repository::open(dir.path()).expect("reopen");
    assert_eq!(repo.state(), git2::RepositoryState::Clean);
    assert_eq!(p8_head_oid(&repo), topic, "HEAD must move to topic tip");
    assert_eq!(
        p8_read(dir.path(), "feature.txt"),
        "feature\n",
        "FF must have brought in topic's new file"
    );
    assert_eq!(
        p8_read(dir.path(), "unrelated.txt"),
        "locally edited\n",
        "the stashed unstaged edit must be restored in the worktree"
    );
    assert_eq!(p8_stash_count(dir.path()), 0, "stash applied + dropped");

    // Oracle: replay the SAME history through the real `git` CLI with
    // --autostash and compare BEHAVIOR (not commit oids — those differ
    // across independently-built repos because a commit hash includes the
    // committer timestamp). Parity we assert: the FF landed feature.txt and
    // the unstaged edit was restored byte-identically. Skipped (not a hard
    // failure) if git is unavailable on PATH.
    if let Some((cli_feature, cli_unrelated)) = p8_git_cli_autostash_ff_oracle() {
        assert_eq!(
            "feature\n", cli_feature,
            "`git merge --autostash` FF must also bring in feature.txt"
        );
        assert_eq!(
            "locally edited\n", cli_unrelated,
            "`git merge --autostash` also restores the unstaged edit"
        );
        // Our worktree must match the CLI's for both files.
        assert_eq!(p8_read(dir.path(), "feature.txt"), cli_feature);
        assert_eq!(p8_read(dir.path(), "unrelated.txt"), cli_unrelated);
    }
}

// ---- Row 3: Dirty (STAGED) FF round-trip -------------------------------

/// Stage an unrelated change, then FF -> `FastForwarded { stashed: true }`.
/// The change CONTENT survives. Per OPEN Q#1 (no REINSTATE_INDEX) it comes
/// back as an UNSTAGED worktree change, NOT re-staged — asserted explicitly.
#[test]
fn p8_3_dirty_staged_ff_round_trip() {
    use crate::git::stage::stage_paths;
    let dir = crate::testutil::scratch_dir();
    let repo = p8_init(dir.path());

    p8_commit(
        dir.path(),
        "base",
        &[("a.txt", "base\n"), ("unrelated.txt", "orig\n")],
    );
    let base = repo.find_commit(p8_head_oid(&repo)).expect("base");
    let topic = p8_commit_on_ref(
        &repo,
        "refs/heads/topic",
        &base,
        &[("feature.txt", "feature\n")],
        "topic advance",
    );

    let branch = repo
        .head()
        .expect("HEAD")
        .shorthand()
        .expect("shorthand")
        .to_string();

    // Dirty: edit + STAGE an unrelated tracked file.
    std::fs::write(dir.path().join("unrelated.txt"), "staged edit\n").expect("edit");
    stage_paths(dir.path(), &["unrelated.txt".to_string()]).expect("stage");

    let outcome = merge_branch(dir.path(), "topic", false).expect("merge");
    assert_eq!(
        outcome,
        MergeOutcome::FastForwarded {
            branch: branch.clone(),
            to: topic.to_string(),
            stashed: true,
        },
        "dirty (staged) FF must report stashed:true"
    );

    let repo = git2::Repository::open(dir.path()).expect("reopen");
    assert_eq!(repo.state(), git2::RepositoryState::Clean);
    assert_eq!(p8_head_oid(&repo), topic);
    assert_eq!(
        p8_read(dir.path(), "unrelated.txt"),
        "staged edit\n",
        "the staged change CONTENT must survive the autostash round-trip"
    );

    // OPEN Q#1: no REINSTATE_INDEX -> the change returns as UNSTAGED, i.e.
    // worktree-modified, NOT index-modified. Assert the split explicitly.
    let mut so = git2::StatusOptions::new();
    so.include_untracked(false);
    let statuses = repo.statuses(Some(&mut so)).expect("statuses");
    let entry = statuses
        .iter()
        .find(|e| e.path().ok() == Some("unrelated.txt"))
        .expect("unrelated.txt must show a pending change");
    assert!(
        entry.status().contains(git2::Status::WT_MODIFIED),
        "restored change must be an UNSTAGED (worktree) modification"
    );
    assert!(
        !entry.status().contains(git2::Status::INDEX_MODIFIED),
        "OPEN Q#1: without REINSTATE_INDEX the change must NOT be re-staged"
    );
    assert_eq!(p8_stash_count(dir.path()), 0, "stash applied + dropped");
}

// ---- Row 4 (matrix #3): Dirty clean normal merge -----------------------

/// Unrelated dirty edit + a non-FF but cleanly-mergeable branch ->
/// `Merged { stashed: true }`. Assert a 2-parent merge commit AND the dirty
/// edit preserved.
#[test]
fn p8_4_dirty_clean_normal_merge() {
    let dir = crate::testutil::scratch_dir();
    let repo = p8_init(dir.path());

    p8_commit(
        dir.path(),
        "base",
        &[("a.txt", "base\n"), ("unrelated.txt", "orig\n")],
    );
    let base = repo.find_commit(p8_head_oid(&repo)).expect("base");
    // topic diverges from base by ADDING topic-only.txt (from base tree).
    p8_commit_on_ref(
        &repo,
        "refs/heads/topic",
        &base,
        &[("topic-only.txt", "topic\n")],
        "topic side",
    );
    // main advances by ADDING main-only.txt -> divergent (non-FF), clean.
    p8_commit(dir.path(), "main side", &[("main-only.txt", "main\n")]);

    // Dirty: unrelated tracked edit the merge never touches, UNSTAGED.
    std::fs::write(dir.path().join("unrelated.txt"), "locally edited\n").expect("edit");

    let outcome = merge_branch(dir.path(), "topic", false).expect("merge");
    let oid = match &outcome {
        MergeOutcome::Merged { oid, stashed } => {
            assert!(*stashed, "clean normal merge over dirty tree must be stashed:true");
            oid.clone()
        }
        other => panic!("expected Merged, got {other:?}"),
    };

    let repo = git2::Repository::open(dir.path()).expect("reopen");
    assert_eq!(repo.state(), git2::RepositoryState::Clean);
    let merge_commit = repo
        .find_commit(git2::Oid::from_str(&oid).expect("oid"))
        .expect("merge commit");
    assert_eq!(
        merge_commit.parent_count(),
        2,
        "a normal merge must produce a 2-parent commit"
    );
    assert_eq!(
        p8_head_oid(&repo),
        merge_commit.id(),
        "HEAD must point at the new merge commit"
    );
    // Both sides' files present + the dirty edit restored.
    assert_eq!(p8_read(dir.path(), "main-only.txt"), "main\n");
    assert_eq!(p8_read(dir.path(), "topic-only.txt"), "topic\n");
    assert_eq!(
        p8_read(dir.path(), "unrelated.txt"),
        "locally edited\n",
        "the stashed dirty edit must be restored after the merge commit"
    );
    assert_eq!(p8_stash_count(dir.path()), 0, "stash applied + dropped");
}

// ---- Row 5 (matrix #4): Stash-pop conflict -----------------------------

/// Locally edit file X (unstaged); the FF target ALSO modifies X so the
/// autostash re-apply conflicts -> `StashPopConflicts { paths: ["x.txt"] }`.
/// repo.state() == Clean, X has conflict markers, stash RETAINED (count==1).
#[test]
fn p8_5_stash_pop_conflict() {
    let dir = crate::testutil::scratch_dir();
    let repo = p8_init(dir.path());

    p8_commit(dir.path(), "base", &[("x.txt", "line1\nline2\nline3\n")]);
    let base = repo.find_commit(p8_head_oid(&repo)).expect("base");
    // topic (FF-able) modifies line2 -> "TOPIC".
    let topic = p8_commit_on_ref(
        &repo,
        "refs/heads/topic",
        &base,
        &[("x.txt", "line1\nTOPIC\nline3\n")],
        "topic edits x",
    );

    // Local UNSTAGED edit of the SAME line -> conflicts on stash re-apply.
    std::fs::write(dir.path().join("x.txt"), "line1\nLOCAL\nline3\n").expect("edit x");

    let outcome = merge_branch(dir.path(), "topic", false).expect("merge");
    match &outcome {
        MergeOutcome::StashPopConflicts { head, paths } => {
            assert_eq!(head, &topic.to_string(), "head = FF target");
            assert_eq!(paths, &vec!["x.txt".to_string()], "x.txt conflicted on pop");
        }
        other => panic!("expected StashPopConflicts, got {other:?}"),
    }

    let repo = git2::Repository::open(dir.path()).expect("reopen");
    // A conflicted stash-apply is NOT a merge op: state stays Clean.
    assert_eq!(
        repo.state(),
        git2::RepositoryState::Clean,
        "stash-pop conflict must leave state Clean (not Merge)"
    );
    assert_eq!(
        p8_head_oid(&repo),
        topic,
        "FF already landed: HEAD is at the target"
    );
    let x = p8_read(dir.path(), "x.txt");
    assert!(
        x.contains("<<<<<<<") && x.contains(">>>>>>>"),
        "x.txt must contain conflict markers, got:\n{x}"
    );
    assert_eq!(
        p8_stash_count(dir.path()),
        1,
        "libgit2 does NOT drop the stash on a conflicting pop: it is RETAINED"
    );
}

// ---- Row 6 (matrix #5): Normal-merge paused + dirty --------------------

/// A conflicting merge on file X PLUS an unrelated dirty file Y ->
/// `Conflicts { stashed: true }`. repo.state() == Merge, MERGE_HEAD present,
/// stash RETAINED (count==1), Y's worktree content at the HEAD version
/// (Y was stashed, not restored — deferred re-apply, OPEN Q#2).
#[test]
fn p8_6_normal_merge_paused_plus_dirty() {
    let dir = crate::testutil::scratch_dir();
    let repo = p8_init(dir.path());

    p8_commit(
        dir.path(),
        "base",
        &[("x.txt", "base\n"), ("y.txt", "y-base\n")],
    );
    let base = repo.find_commit(p8_head_oid(&repo)).expect("base");
    // topic diverges: x.txt -> "topic".
    p8_commit_on_ref(
        &repo,
        "refs/heads/topic",
        &base,
        &[("x.txt", "topic\n")],
        "topic edits x",
    );
    // main diverges: x.txt -> "main" (conflict) ; y.txt untouched by both.
    p8_commit(dir.path(), "main edits x", &[("x.txt", "main\n")]);

    // Unrelated dirty file Y (UNSTAGED). The merge never touches Y, so Y
    // lands on the autostash and the paused merge does not restore it.
    std::fs::write(dir.path().join("y.txt"), "y-locally-edited\n").expect("edit y");

    let outcome = merge_branch(dir.path(), "topic", false).expect("merge");
    assert_eq!(
        outcome,
        MergeOutcome::Conflicts {
            paths: vec!["x.txt".to_string()],
            stashed: true,
        },
        "paused conflicting merge over a dirty tree must be stashed:true"
    );

    let repo = git2::Repository::open(dir.path()).expect("reopen");
    assert_eq!(
        repo.state(),
        git2::RepositoryState::Merge,
        "a conflicting merge must PAUSE in state Merge"
    );
    assert!(
        repo.path().join("MERGE_HEAD").exists(),
        "MERGE_HEAD must be written for the paused merge"
    );
    assert_eq!(
        p8_stash_count(dir.path()),
        1,
        "deferred re-apply (OPEN Q#2): the autostash is RETAINED on the stack"
    );
    // Y was stashed and NOT re-applied -> worktree Y is at HEAD (main) value.
    assert_eq!(
        p8_read(dir.path(), "y.txt"),
        "y-base\n",
        "Y's dirty edit is on the stash; worktree Y must be at the HEAD version"
    );
}

// ---- Row 7 (matrix #6): Rollback on blocked FF -------------------------

/// A dirty tracked edit + an UNTRACKED file that the FF would create ->
/// `Err(CheckoutConflict)`. repo.state() == Clean, the dirty tracked edit is
/// restored in the worktree, and stash_foreach count == 0 (rolled back —
/// nothing left on the stack).
#[test]
fn p8_7_rollback_on_blocked_ff() {
    let dir = crate::testutil::scratch_dir();
    let repo = p8_init(dir.path());

    p8_commit(
        dir.path(),
        "base",
        &[("a.txt", "base\n"), ("unrelated.txt", "orig\n")],
    );
    let base_oid = p8_head_oid(&repo);
    let base = repo.find_commit(base_oid).expect("base");
    // topic (FF-able) ADDS new.txt with committed content "from-topic".
    p8_commit_on_ref(
        &repo,
        "refs/heads/topic",
        &base,
        &[("new.txt", "from-topic\n")],
        "topic adds new.txt",
    );

    // Dirty tracked edit (UNSTAGED) -> triggers the autostash.
    std::fs::write(dir.path().join("unrelated.txt"), "locally edited\n").expect("edit");
    // UNTRACKED file physically in the way of the FF checkout of new.txt.
    // INCLUDE_UNTRACKED is off, so this is NOT stashed and blocks the SAFE
    // checkout with a Conflict.
    std::fs::write(dir.path().join("new.txt"), "untracked in the way\n").expect("untracked");

    let err = merge_branch(dir.path(), "topic", false).expect_err("blocked FF must error");
    assert!(
        matches!(err, AppError::CheckoutConflict(_)),
        "an untracked file blocking the FF checkout must map to CheckoutConflict, got {err:?}"
    );

    let repo = git2::Repository::open(dir.path()).expect("reopen");
    assert_eq!(
        repo.state(),
        git2::RepositoryState::Clean,
        "a failed merge_branch must leave state Clean"
    );
    assert_eq!(
        p8_head_oid(&repo),
        base_oid,
        "the FF set_target never ran: HEAD is unchanged at base"
    );
    assert_eq!(
        p8_read(dir.path(), "unrelated.txt"),
        "locally edited\n",
        "rollback_stash must restore the dirty tracked edit"
    );
    assert_eq!(
        p8_read(dir.path(), "new.txt"),
        "untracked in the way\n",
        "the untracked file must be left untouched"
    );
    assert_eq!(
        p8_stash_count(dir.path()),
        0,
        "rollback popped the stash: nothing left on the stack"
    );
}
