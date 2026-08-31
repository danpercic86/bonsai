//! P11f (contract §1.1 algorithm, §1.5/§7 acceptance): `create_branch_here`
//! must carry uncommitted work across a checkout via auto-stash and NEVER be
//! lossy. Every test asserts the observable git state (HEAD ref/target,
//! worktree file contents, stash stack length) — not just the return value.
//!
//! Fixtures are built with git2 in a scratch `TempDir` (deterministic, no
//! network, no CLI), mirroring the style in `stash.rs`.

use super::*;
use crate::git::stash::{list_stashes, ApplyStashOutcome};

/// Init a scratch repo with a deterministic identity + autocrlf off
/// (== stash.rs `s9_init`).
fn cbh_init(dir: &Path) -> git2::Repository {
    let repo = git2::Repository::init(dir).expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    drop(cfg);
    repo
}

/// Stage + commit `files` on the CURRENT branch (moves HEAD + worktree).
fn cbh_commit(dir: &Path, msg: &str, files: &[(&str, &str)]) {
    use crate::git::stage::stage_paths;
    for (name, content) in files {
        std::fs::write(dir.join(name), content).expect("write file");
    }
    stage_paths(
        dir,
        &files.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>(),
    )
    .expect("stage");
    crate::git::commit::create_commit(dir, msg, None, false).expect("commit");
}

/// Build a commit on `refname` from `parent`'s tree WITHOUT moving HEAD or the
/// worktree (== stash.rs `s9_commit_on_ref`). Used to build a divergent tip.
fn cbh_commit_on_ref(
    repo: &git2::Repository,
    refname: &str,
    parent: &git2::Commit,
    files: &[(&str, &str)],
    msg: &str,
) -> git2::Oid {
    let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
    let mut tb = repo
        .treebuilder(Some(&parent.tree().expect("parent tree")))
        .expect("treebuilder");
    for (name, content) in files {
        let blob = repo.blob(content.as_bytes()).expect("blob");
        tb.insert(name, blob, 0o100644).expect("insert");
    }
    let tree = repo.find_tree(tb.write().expect("tree oid")).expect("tree");
    repo.commit(Some(refname), &sig, &sig, &format!("{msg}\n"), &tree, &[parent])
        .expect("commit on ref")
}

/// Full 40-hex oid of the current HEAD commit.
fn cbh_head_oid(dir: &Path) -> String {
    let repo = git2::Repository::open(dir).expect("open");
    let oid = repo
        .head()
        .expect("HEAD")
        .peel_to_commit()
        .expect("peel")
        .id();
    oid.to_string()
}

/// The short branch name HEAD points at, or None when detached/unborn.
fn cbh_head_branch(dir: &Path) -> Option<String> {
    let repo = git2::Repository::open(dir).expect("open");
    let head = repo.head().ok()?;
    if !head.is_branch() {
        return None;
    }
    head.shorthand().ok().map(str::to_string)
}

fn cbh_read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).expect("read file")
}

/// True when local branch `name` does not exist in the repo at `dir`.
fn cbh_branch_absent(dir: &Path, name: &str) -> bool {
    let repo = git2::Repository::open(dir).expect("open");
    let absent = repo.find_branch(name, git2::BranchType::Local).is_err();
    absent
}

// ------------------------------------------------------- Scenario 1: clean

/// §1.5/§7: clean worktree → `{ stashed:false, apply:None }`; HEAD is the new
/// branch pointing at the requested (older) commit.
#[test]
fn cbh_1_clean_worktree_creates_and_checks_out() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    cbh_init(d);
    cbh_commit(d, "C0", &[("a.txt", "base\n")]);
    let c0 = cbh_head_oid(d);
    cbh_commit(d, "C1", &[("b.txt", "b1\n")]);
    cbh_commit(d, "C2", &[("c.txt", "c1\n")]);

    // Clean worktree; create branch at the OLDER commit C0.
    let res = create_branch_here(d, "feat", &c0).expect("create_branch_here");
    assert_eq!(
        res,
        CreateBranchHereResult {
            stashed: false,
            apply: None
        },
        "clean worktree must not stash"
    );

    // HEAD now on the new branch, at C0.
    assert_eq!(cbh_head_branch(d).as_deref(), Some("feat"), "HEAD is 'feat'");
    assert_eq!(cbh_head_oid(d), c0, "'feat' points at C0");

    // Checkout to C0 removed the files introduced by C1/C2.
    assert_eq!(cbh_read(d, "a.txt"), "base\n");
    assert!(!d.join("b.txt").exists(), "b.txt (C1) gone at C0");
    assert!(!d.join("c.txt").exists(), "c.txt (C2) gone at C0");

    assert_eq!(list_stashes(d).expect("list").len(), 0, "no stash created");
}

// ---------------------------------------------- Scenario 2: dirty, applies

/// §1.5/§7: dirty worktree, branch created at an OLDER commit, changes apply
/// cleanly → `{ stashed:true, apply:Some(Applied) }`; carried change present
/// on the new branch; the stash stack is EMPTY (clean pop dropped).
#[test]
fn cbh_2_dirty_clean_carry_over() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    cbh_init(d);
    // a.txt is added at C0 and NEVER changes through C2, so the stashed edit
    // (base a.txt == C2 a.txt) re-applies cleanly onto C0.
    cbh_commit(d, "C0", &[("a.txt", "base\n")]);
    let c0 = cbh_head_oid(d);
    cbh_commit(d, "C1", &[("b.txt", "b1\n")]);
    cbh_commit(d, "C2", &[("c.txt", "c1\n")]);

    // Dirty: unstaged edit to a.txt.
    std::fs::write(d.join("a.txt"), "edited\n").expect("edit a.txt");

    let res = create_branch_here(d, "feat", &c0).expect("create_branch_here");
    assert_eq!(
        res,
        CreateBranchHereResult {
            stashed: true,
            apply: Some(ApplyStashOutcome::Applied)
        },
        "dirty tree carries cleanly across → Applied"
    );

    // HEAD on 'feat' at C0, carrying the edit.
    assert_eq!(cbh_head_branch(d).as_deref(), Some("feat"));
    assert_eq!(cbh_head_oid(d), c0, "'feat' points at C0");
    assert_eq!(
        cbh_read(d, "a.txt"),
        "edited\n",
        "carried edit present on the new branch"
    );

    assert_eq!(
        list_stashes(d).expect("list").len(),
        0,
        "clean pop dropped the stash; stack empty"
    );
}

// ------------------------------------------- Scenario 3: dirty, conflicts

/// §1.5/§7: dirty edit to a file whose content differs at the target commit
/// → `{ stashed:true, apply:Some(Conflicts{paths}) }`; index has conflicts;
/// the stash is RETAINED (never lossy).
#[test]
fn cbh_3_dirty_conflict_carry_over_retains_stash() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    cbh_init(d);
    // a.txt changes on every commit so the 3-way apply of the stash onto C0
    // conflicts (ancestor=C2, ours=C0, theirs=dirty all differ).
    cbh_commit(d, "C0", &[("a.txt", "base\n")]);
    let c0 = cbh_head_oid(d);
    cbh_commit(d, "C1", &[("a.txt", "c1\n")]);
    cbh_commit(d, "C2", &[("a.txt", "c2\n")]);

    // Dirty edit to a.txt (base of stash == C2's "c2\n").
    std::fs::write(d.join("a.txt"), "dirty\n").expect("edit a.txt");

    let res = create_branch_here(d, "feat", &c0).expect("create_branch_here");
    assert_eq!(
        res,
        CreateBranchHereResult {
            stashed: true,
            apply: Some(ApplyStashOutcome::Conflicts {
                paths: vec!["a.txt".to_string()]
            })
        },
        "carry-over onto a divergent file must report Conflicts on a.txt"
    );

    // Branch was created & checked out (Conflicts is a SUCCESS return).
    assert_eq!(cbh_head_branch(d).as_deref(), Some("feat"));
    assert_eq!(cbh_head_oid(d), c0, "'feat' points at C0");

    // Index carries conflict entries; markers present in the worktree.
    let repo = git2::Repository::open(d).expect("reopen");
    assert!(
        repo.index().expect("index").has_conflicts(),
        "index must carry conflict entries"
    );
    assert!(
        cbh_read(d, "a.txt").contains("<<<<<<<"),
        "worktree a.txt must carry conflict markers"
    );

    // DATA SAFETY: conflicting pop retains the stash.
    assert_eq!(
        list_stashes(d).expect("list").len(),
        1,
        "conflicting carry-over must RETAIN the stash (never lossy)"
    );
}

// ----------------------------------------- Scenario 4: name already exists

/// §1.5/§7: name already exists → `Err(BranchExists)` with NOTHING stashed
/// (stack unchanged) and HEAD unchanged (pre-check runs before any side
/// effect, even with a dirty tree).
#[test]
fn cbh_4_existing_name_no_side_effects() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    cbh_init(d);
    cbh_commit(d, "C0", &[("a.txt", "base\n")]);
    let c0 = cbh_head_oid(d);

    // A local branch that already exists.
    create_branch(d, "existing").expect("seed branch");

    let head_before = cbh_head_branch(d);
    let oid_before = cbh_head_oid(d);

    // Dirty tree, to prove the pre-check runs BEFORE the auto-stash.
    std::fs::write(d.join("a.txt"), "edited\n").expect("edit a.txt");

    match create_branch_here(d, "existing", &c0) {
        Err(AppError::BranchExists(_)) => {}
        other => panic!("expected BranchExists, got {other:?}"),
    }

    // No stash was created; HEAD + worktree untouched.
    assert_eq!(
        list_stashes(d).expect("list").len(),
        0,
        "BranchExists must NOT strand a stash"
    );
    assert_eq!(cbh_head_branch(d), head_before, "HEAD branch unchanged");
    assert_eq!(cbh_head_oid(d), oid_before, "HEAD oid unchanged");
    assert_eq!(cbh_read(d, "a.txt"), "edited\n", "dirty edit still present");
}

// ------------------------------------------------ Scenario 5: bad/unknown oid

/// §1.5/§7: bad oid → `Err(Git)` before ANY side effect. Covers both a
/// malformed string and a well-formed but non-existent 40-hex oid; a dirty
/// tree proves nothing gets stashed.
#[test]
fn cbh_5_bad_oid_errors_before_side_effects() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    cbh_init(d);
    cbh_commit(d, "C0", &[("a.txt", "base\n")]);

    let head_before = cbh_head_branch(d);
    let oid_before = cbh_head_oid(d);

    // Dirty, so a stray auto-stash would be observable.
    std::fs::write(d.join("a.txt"), "edited\n").expect("edit a.txt");

    // (a) malformed oid string.
    match create_branch_here(d, "feat", "not-a-valid-oid") {
        Err(AppError::Git(_)) => {}
        other => panic!("malformed oid: expected Git error, got {other:?}"),
    }

    // (b) well-formed hex but no such commit.
    let missing = "0".repeat(40);
    match create_branch_here(d, "feat", &missing) {
        Err(AppError::Git(_)) => {}
        other => panic!("unknown oid: expected Git error, got {other:?}"),
    }

    assert_eq!(
        list_stashes(d).expect("list").len(),
        0,
        "a bad oid must error before the auto-stash"
    );
    assert!(
        cbh_branch_absent(d, "feat"),
        "no branch should have been created"
    );
    assert_eq!(cbh_head_branch(d), head_before, "HEAD branch unchanged");
    assert_eq!(cbh_head_oid(d), oid_before, "HEAD oid unchanged");
    assert_eq!(cbh_read(d, "a.txt"), "edited\n", "dirty edit still present");
}

// ---------------------------------------------- Scenario 6: mid-operation

/// §1.5/§7: mid-merge (an operation in progress) → `Err(OperationInProgress)`
/// (via `create_stash`'s `require_clean` gate) and no branch created.
///
/// The mid-op state is produced deterministically with a conflicting
/// auto-stashing merge (== stash.rs `s9_7`), which pauses the repo in Merge
/// state — so this scenario is exercised, not skipped.
#[test]
fn cbh_6_mid_merge_operation_in_progress() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = cbh_init(d);

    cbh_commit(d, "base", &[("x.txt", "base\n"), ("y.txt", "y-base\n")]);
    let base = repo
        .find_commit(repo.head().expect("HEAD").target().expect("oid"))
        .expect("base");
    // topic diverges on x.txt.
    cbh_commit_on_ref(
        &repo,
        "refs/heads/topic",
        &base,
        &[("x.txt", "topic\n")],
        "topic edits x",
    );
    // main diverges on x.txt (guaranteed conflict on merge).
    cbh_commit(d, "main edits x", &[("x.txt", "main\n")]);

    // Dirty an unrelated file so the merge auto-stashes then pauses in Merge.
    std::fs::write(d.join("y.txt"), "y-edited\n").expect("edit y");
    crate::git::merge::merge_branch(d, "topic", false).expect("merge");

    let repo = git2::Repository::open(d).expect("reopen");
    assert_eq!(
        repo.state(),
        git2::RepositoryState::Merge,
        "conflicting merge over a dirty tree must pause in Merge state"
    );

    let target = cbh_head_oid(d);
    match create_branch_here(d, "feat", &target) {
        Err(AppError::OperationInProgress(_)) => {}
        other => panic!("expected OperationInProgress mid-merge, got {other:?}"),
    }

    assert!(
        cbh_branch_absent(d, "feat"),
        "no branch created while an operation is in progress"
    );
}
