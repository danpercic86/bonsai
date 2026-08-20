//! P33 (contract §5 acceptance): `checkout_branch_autostash` must carry
//! uncommitted work across a branch switch via auto-stash, auto-fast-forward
//! the switched-to branch to its upstream (no fetch), and NEVER be lossy.
//! Every test asserts the observable git state (returned `CheckoutResult`,
//! HEAD ref/target, branch tip oid, worktree contents, stash stack) — not
//! just the return value.
//!
//! Fixtures are built with git2 in a scratch `TempDir` (deterministic, no
//! network, no CLI), mirroring `create_branch_here_tests` above. The
//! "upstream" for the FF cases is a plain remote-tracking ref
//! (`refs/remotes/origin/<name>`) plus `branch.<name>.remote/merge` config
//! and a dummy `origin` remote — NO network fetch (parity with health.rs).

use super::*;
use crate::git::stash::{list_stashes, ApplyStashOutcome};

/// Init a scratch repo with a deterministic identity + autocrlf off.
/// Pins the initial branch to "main" via `initial_head` rather than
/// relying on `init.defaultBranch` — libgit2 falls back to "master" when
/// that config is unset, which this module's "main" assertions assume.
fn ca_init(dir: &Path) -> git2::Repository {
    let repo = git2::Repository::init_opts(dir, git2::RepositoryInitOptions::new().initial_head("main"))
        .expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    drop(cfg);
    repo
}

/// Stage + commit `files` on the CURRENT branch (moves HEAD + worktree).
fn ca_commit(dir: &Path, msg: &str, files: &[(&str, &str)]) {
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

/// Build a commit on `refname` from `parent`'s tree WITHOUT moving HEAD or
/// the worktree. Used to build divergent / ahead tips and upstream refs.
fn ca_commit_on_ref(
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

fn ca_find_commit<'a>(repo: &'a git2::Repository, oid: git2::Oid) -> git2::Commit<'a> {
    repo.find_commit(oid).expect("find commit")
}

/// Configure `origin/<name>` as `local_name`'s upstream, pointing the
/// remote-tracking ref at `upstream_oid`. Creates the dummy `origin` remote
/// once (its default fetch refspec is what lets `Branch::upstream()` resolve
/// the tracking ref — with NO network). Idempotent on the remote.
fn ca_set_upstream(repo: &git2::Repository, local_name: &str, upstream_oid: git2::Oid) {
    if repo.find_remote("origin").is_err() {
        repo.remote("origin", "https://example.invalid/x.git")
            .expect("remote");
    }
    repo.reference(
        &format!("refs/remotes/origin/{local_name}"),
        upstream_oid,
        true,
        "seed upstream",
    )
    .expect("remote-tracking ref");
    let mut cfg = repo.config().expect("config");
    cfg.set_str(&format!("branch.{local_name}.remote"), "origin")
        .expect("remote cfg");
    cfg.set_str(
        &format!("branch.{local_name}.merge"),
        &format!("refs/heads/{local_name}"),
    )
    .expect("merge cfg");
}

/// Full 40-hex oid of the current HEAD commit.
fn ca_head_oid(dir: &Path) -> String {
    let repo = git2::Repository::open(dir).expect("open");
    let oid = repo
        .head()
        .expect("HEAD")
        .peel_to_commit()
        .expect("peel")
        .id()
        .to_string();
    oid
}

/// The short branch name HEAD points at, or None when detached/unborn.
fn ca_head_branch(dir: &Path) -> Option<String> {
    let repo = git2::Repository::open(dir).expect("open");
    let head = repo.head().ok()?;
    if !head.is_branch() {
        return None;
    }
    head.shorthand().ok().map(str::to_string)
}

/// Full 40-hex oid of LOCAL branch `name`'s tip.
fn ca_branch_tip(dir: &Path, name: &str) -> String {
    let repo = git2::Repository::open(dir).expect("open");
    let tip = repo
        .find_branch(name, git2::BranchType::Local)
        .expect("branch")
        .get()
        .target()
        .expect("target")
        .to_string();
    tip
}

fn ca_read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).expect("read file")
}

// ---------------------------------- Case 1: clean switch, up-to-date upstream

/// AC1/AC6: clean worktree, target has an upstream that is up-to-date
/// (behind==0) → `{ stashed:false, fast_forwarded:false, apply:None }`; HEAD
/// moves to the target; the target ref is unchanged; no stash created.
#[test]
fn ca_1_clean_switch_no_divergence() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = ca_init(d);
    ca_commit(d, "C0", &[("a.txt", "base\n")]);
    let c0 = repo.head().expect("HEAD").target().expect("oid");

    // feat branches off C0 and adds its own commit; upstream == feat tip.
    let feat_tip = ca_commit_on_ref(
        &repo,
        "refs/heads/feat",
        &ca_find_commit(&repo, c0),
        &[("feat.txt", "f1\n")],
        "F1",
    );
    ca_set_upstream(&repo, "feat", feat_tip); // behind 0, ahead 0

    // main moves on so the switch is a real HEAD/worktree change.
    ca_commit(d, "C1", &[("main.txt", "m1\n")]);
    assert_eq!(ca_head_branch(d).as_deref(), Some("main"));

    let res = checkout_branch_autostash(d, "feat").expect("switch");
    assert_eq!(
        res,
        CheckoutResult {
            stashed: false,
            fast_forwarded: false,
            apply: None
        },
        "clean, up-to-date upstream → no stash, no FF"
    );

    assert_eq!(ca_head_branch(d).as_deref(), Some("feat"), "HEAD is feat");
    assert_eq!(ca_head_oid(d), feat_tip.to_string(), "feat tip unchanged");
    assert_eq!(ca_read(d, "feat.txt"), "f1\n", "feat content present");
    assert!(!d.join("main.txt").exists(), "main.txt gone on feat");
    assert_eq!(list_stashes(d).expect("list").len(), 0, "no stash created");
}

// ------------------------------------- Case 2: dirty tree, clean re-apply

/// AC2: uncommitted edit that does NOT conflict with the target →
/// `{ stashed:true, fast_forwarded:false, apply:Some(Applied) }`; the edit is
/// present on the new branch; the stash was DROPPED (stack empty).
#[test]
fn ca_2_dirty_clean_reapply() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = ca_init(d);
    // a.txt is set at C0 and never changes on either branch, so the stashed
    // edit re-applies cleanly.
    ca_commit(d, "C0", &[("a.txt", "base\n")]);
    let c0 = repo.head().expect("HEAD").target().expect("oid");
    ca_commit_on_ref(
        &repo,
        "refs/heads/feat",
        &ca_find_commit(&repo, c0),
        &[("feat.txt", "f1\n")],
        "F1",
    );
    ca_commit(d, "C1", &[("main.txt", "m1\n")]);

    // Dirty: unstaged edit to a.txt (unchanged on feat → clean carry-over).
    std::fs::write(d.join("a.txt"), "edited\n").expect("edit a.txt");

    let res = checkout_branch_autostash(d, "feat").expect("switch");
    assert_eq!(
        res,
        CheckoutResult {
            stashed: true,
            fast_forwarded: false,
            apply: Some(ApplyStashOutcome::Applied)
        },
        "dirty tree carries cleanly across → Applied, no FF (no upstream)"
    );

    assert_eq!(ca_head_branch(d).as_deref(), Some("feat"));
    assert_eq!(ca_read(d, "a.txt"), "edited\n", "carried edit present");
    assert_eq!(ca_read(d, "feat.txt"), "f1\n", "on the feat tree");
    assert_eq!(
        list_stashes(d).expect("list").len(),
        0,
        "clean pop dropped the stash"
    );
}

// ------------------------------------ Case 3: dirty tree, conflicting re-apply

/// AC3 (KEY DATA-SAFETY CASE): edit to a file that differs on the target such
/// that the 3-way re-apply conflicts → `apply:Some(Conflicts{paths})` as an
/// `Ok` return (NOT `Err`); worktree/index carry the conflict; the stash is
/// RETAINED at stash@{0}; repo state stays Clean (not Merge).
#[test]
fn ca_3_dirty_conflicting_reapply_retains_stash() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = ca_init(d);
    // a.txt differs on the two tips; the dirty edit vs both differs → conflict.
    ca_commit(d, "C0", &[("a.txt", "base\n")]);
    let c0 = repo.head().expect("HEAD").target().expect("oid");
    ca_commit_on_ref(
        &repo,
        "refs/heads/feat",
        &ca_find_commit(&repo, c0),
        &[("a.txt", "feat-side\n")],
        "F1",
    );
    ca_commit(d, "C1", &[("a.txt", "main-side\n")]);

    // Dirty edit to a.txt (stash base == main C1 "main-side").
    std::fs::write(d.join("a.txt"), "dirty\n").expect("edit a.txt");

    let res = checkout_branch_autostash(d, "feat").expect("switch is Ok");
    assert_eq!(
        res,
        CheckoutResult {
            stashed: true,
            fast_forwarded: false,
            apply: Some(ApplyStashOutcome::Conflicts {
                paths: vec!["a.txt".to_string()]
            })
        },
        "conflicting carry-over reports Conflicts on a.txt as a SUCCESS"
    );

    assert_eq!(ca_head_branch(d).as_deref(), Some("feat"), "switch happened");

    let repo = git2::Repository::open(d).expect("reopen");
    assert!(
        repo.index().expect("index").has_conflicts(),
        "index must carry conflict entries"
    );
    assert!(
        ca_read(d, "a.txt").contains("<<<<<<<"),
        "worktree a.txt must carry conflict markers"
    );
    assert_eq!(
        repo.state(),
        git2::RepositoryState::Clean,
        "a conflicted stash pop must NOT leave the repo in Merge state"
    );
    // DATA SAFETY: the user's work must be recoverable.
    assert_eq!(
        list_stashes(d).expect("list").len(),
        1,
        "conflicting carry-over must RETAIN the stash (never lossy)"
    );
}

// ------------------------------------------- Case 4: auto fast-forward (no fetch)

/// AC4: target local branch is strictly behind its upstream (behind>0,
/// ahead==0), clean worktree → `fast_forwarded:true`; the local ref now
/// points at the upstream oid; the upstream tree is checked out. No network:
/// the upstream oid comes solely from the remote-tracking ref.
#[test]
fn ca_4_auto_fast_forward() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = ca_init(d);
    ca_commit(d, "C0", &[("a.txt", "base\n")]);
    let c0 = repo.head().expect("HEAD").target().expect("oid");

    // feat sits at C0; its upstream advances one commit past C0.
    repo.branch("feat", &ca_find_commit(&repo, c0), false)
        .expect("feat at C0");
    let feat_before = ca_branch_tip(d, "feat");
    let upstream_tip = ca_commit_on_ref(
        &repo,
        "refs/remotes/origin/feat",
        &ca_find_commit(&repo, c0),
        &[("upstream.txt", "u1\n")],
        "U1",
    );
    ca_set_upstream(&repo, "feat", upstream_tip); // behind 1, ahead 0
    assert_ne!(feat_before, upstream_tip.to_string());

    // main moves so the switch is a real change.
    ca_commit(d, "C1", &[("main.txt", "m1\n")]);

    let res = checkout_branch_autostash(d, "feat").expect("switch");
    assert_eq!(
        res,
        CheckoutResult {
            stashed: false,
            fast_forwarded: true,
            apply: None
        },
        "behind & not diverged → fast-forwarded, clean"
    );

    assert_eq!(ca_head_branch(d).as_deref(), Some("feat"));
    assert_eq!(
        ca_branch_tip(d, "feat"),
        upstream_tip.to_string(),
        "feat ref fast-forwarded to the upstream oid"
    );
    assert_eq!(ca_head_oid(d), upstream_tip.to_string(), "HEAD at FF tip");
    assert_eq!(
        ca_read(d, "upstream.txt"),
        "u1\n",
        "fast-forwarded tree checked out"
    );
}

// ------------------------------------------------- Case 5: diverged → no FF

/// AC5: target branch is BOTH ahead>0 and behind>0 vs upstream →
/// `fast_forwarded:false`; the local ref is UNCHANGED (no commits lost).
#[test]
fn ca_5_diverged_no_ff() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = ca_init(d);
    ca_commit(d, "C0", &[("a.txt", "base\n")]);
    let c0 = repo.head().expect("HEAD").target().expect("oid");

    // feat = own commit off C0 (ahead); upstream = different commit off C0 (behind).
    let feat_tip = ca_commit_on_ref(
        &repo,
        "refs/heads/feat",
        &ca_find_commit(&repo, c0),
        &[("feat.txt", "fa\n")],
        "FA",
    );
    let upstream_tip = ca_commit_on_ref(
        &repo,
        "refs/remotes/origin/feat",
        &ca_find_commit(&repo, c0),
        &[("up.txt", "fb\n")],
        "FB",
    );
    ca_set_upstream(&repo, "feat", upstream_tip); // ahead 1, behind 1

    ca_commit(d, "C1", &[("main.txt", "m1\n")]);

    let res = checkout_branch_autostash(d, "feat").expect("switch");
    assert_eq!(
        res,
        CheckoutResult {
            stashed: false,
            fast_forwarded: false,
            apply: None
        },
        "diverged (ahead>0 && behind>0) → no FF"
    );
    assert_eq!(ca_head_branch(d).as_deref(), Some("feat"));
    assert_eq!(
        ca_branch_tip(d, "feat"),
        feat_tip.to_string(),
        "diverged local ref must be UNCHANGED (no commits lost)"
    );
}

// ----------------------------------------- Case 5b: ahead-only → no FF

/// AC6: target branch is ahead-only (behind==0) → `fast_forwarded:false`;
/// the local ref is unchanged.
#[test]
fn ca_5b_ahead_only_no_ff() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = ca_init(d);
    ca_commit(d, "C0", &[("a.txt", "base\n")]);
    let c0 = repo.head().expect("HEAD").target().expect("oid");

    // feat is one commit ahead of an upstream pinned at C0.
    let feat_tip = ca_commit_on_ref(
        &repo,
        "refs/heads/feat",
        &ca_find_commit(&repo, c0),
        &[("feat.txt", "fa\n")],
        "FA",
    );
    ca_set_upstream(&repo, "feat", c0); // ahead 1, behind 0

    ca_commit(d, "C1", &[("main.txt", "m1\n")]);

    let res = checkout_branch_autostash(d, "feat").expect("switch");
    assert!(!res.fast_forwarded, "ahead-only → no FF");
    assert_eq!(
        ca_branch_tip(d, "feat"),
        feat_tip.to_string(),
        "ahead-only local ref unchanged"
    );
}

// ------------------------------------------------- Case 6: no upstream → no FF

/// AC7: target branch has no upstream configured → `fast_forwarded:false`,
/// switch still succeeds cleanly.
#[test]
fn ca_6_no_upstream_no_ff() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = ca_init(d);
    ca_commit(d, "C0", &[("a.txt", "base\n")]);
    let c0 = repo.head().expect("HEAD").target().expect("oid");
    let feat_tip = ca_commit_on_ref(
        &repo,
        "refs/heads/feat",
        &ca_find_commit(&repo, c0),
        &[("feat.txt", "f1\n")],
        "F1",
    );
    ca_commit(d, "C1", &[("main.txt", "m1\n")]);

    let res = checkout_branch_autostash(d, "feat").expect("switch");
    assert_eq!(
        res,
        CheckoutResult {
            stashed: false,
            fast_forwarded: false,
            apply: None
        },
        "no upstream → no FF, switch still succeeds"
    );
    assert_eq!(ca_head_branch(d).as_deref(), Some("feat"));
    assert_eq!(ca_branch_tip(d, "feat"), feat_tip.to_string(), "ref unchanged");
}

// ------------------------------- Case 7: FF + carried stash ordering (AC11)

/// AC11: dirty tree AND target behind upstream → `fast_forwarded:true` AND
/// `stashed:true`; the re-applied edit sits ON TOP of the fast-forwarded tip
/// (both the FF file and the carried edit are present, and feat == upstream).
#[test]
fn ca_7_ff_plus_carried_stash_ordering() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = ca_init(d);
    ca_commit(d, "C0", &[("a.txt", "base\n")]);
    let c0 = repo.head().expect("HEAD").target().expect("oid");

    // feat at C0; upstream one ahead (adds upstream.txt, leaves a.txt alone).
    repo.branch("feat", &ca_find_commit(&repo, c0), false)
        .expect("feat at C0");
    let upstream_tip = ca_commit_on_ref(
        &repo,
        "refs/remotes/origin/feat",
        &ca_find_commit(&repo, c0),
        &[("upstream.txt", "u1\n")],
        "U1",
    );
    ca_set_upstream(&repo, "feat", upstream_tip); // behind 1, ahead 0

    ca_commit(d, "C1", &[("main.txt", "m1\n")]);

    // Dirty edit to a.txt (unchanged through the FF → clean carry-over).
    std::fs::write(d.join("a.txt"), "edited\n").expect("edit a.txt");

    let res = checkout_branch_autostash(d, "feat").expect("switch");
    assert_eq!(
        res,
        CheckoutResult {
            stashed: true,
            fast_forwarded: true,
            apply: Some(ApplyStashOutcome::Applied)
        },
        "dirty + behind → stashed AND fast-forwarded, clean carry-over"
    );

    assert_eq!(ca_head_branch(d).as_deref(), Some("feat"));
    assert_eq!(
        ca_branch_tip(d, "feat"),
        upstream_tip.to_string(),
        "feat fast-forwarded to upstream tip"
    );
    // Ordering: the carried edit sits on the fast-forwarded tip.
    assert_eq!(
        ca_read(d, "upstream.txt"),
        "u1\n",
        "FF tip's file present under the restored work"
    );
    assert_eq!(ca_read(d, "a.txt"), "edited\n", "carried edit on top of FF tip");
    assert_eq!(
        list_stashes(d).expect("list").len(),
        0,
        "clean carry-over dropped the stash"
    );
}

// ---------------------------------------- Case 8: already checked out (AC8)

/// AC8: target is already HEAD → `{ false, false, None }`, no side effects
/// (even with a dirty tree: no stash created).
#[test]
fn ca_8_already_checked_out_noop() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    ca_init(d);
    ca_commit(d, "C0", &[("a.txt", "base\n")]);
    assert_eq!(ca_head_branch(d).as_deref(), Some("main"));

    // Dirty tree, so a stray auto-stash would be observable.
    std::fs::write(d.join("a.txt"), "edited\n").expect("edit a.txt");

    let res = checkout_branch_autostash(d, "main").expect("no-op switch");
    assert_eq!(
        res,
        CheckoutResult {
            stashed: false,
            fast_forwarded: false,
            apply: None
        },
        "switching to the current branch is a no-op"
    );
    assert_eq!(ca_read(d, "a.txt"), "edited\n", "dirty edit untouched");
    assert_eq!(list_stashes(d).expect("list").len(), 0, "no stash created");
}

// ------------------------------------------- Case 9: branch not found (AC9)

/// AC9: unknown branch name → `Err(BranchNotFound)`, no side effects.
#[test]
fn ca_9_branch_not_found() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    ca_init(d);
    ca_commit(d, "C0", &[("a.txt", "base\n")]);

    std::fs::write(d.join("a.txt"), "edited\n").expect("edit a.txt");
    match checkout_branch_autostash(d, "does-not-exist") {
        Err(AppError::BranchNotFound(_)) => {}
        other => panic!("expected BranchNotFound, got {other:?}"),
    }
    assert_eq!(
        list_stashes(d).expect("list").len(),
        0,
        "a missing branch must error before any auto-stash"
    );
    assert_eq!(ca_read(d, "a.txt"), "edited\n", "dirty edit untouched");
}

// -------------------------------------- Case 10: op in progress (mid-merge, AC10)

/// AC10: dirty tree mid-merge → `create_stash`'s `require_clean` gate rejects
/// with `OperationInProgress`; nothing is switched.
#[test]
fn ca_10_mid_merge_operation_in_progress() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = ca_init(d);

    ca_commit(d, "base", &[("x.txt", "base\n"), ("y.txt", "y-base\n")]);
    let base = ca_find_commit(&repo, repo.head().expect("HEAD").target().expect("oid"));
    // topic diverges on x.txt.
    ca_commit_on_ref(
        &repo,
        "refs/heads/topic",
        &base,
        &[("x.txt", "topic\n")],
        "topic edits x",
    );
    // main diverges on x.txt (guaranteed conflict on merge).
    ca_commit(d, "main edits x", &[("x.txt", "main\n")]);

    // Dirty an unrelated file so the merge auto-stashes then pauses in Merge.
    std::fs::write(d.join("y.txt"), "y-edited\n").expect("edit y");
    crate::git::merge::merge_branch(d, "topic", false).expect("merge");

    let repo = git2::Repository::open(d).expect("reopen");
    assert_eq!(
        repo.state(),
        git2::RepositoryState::Merge,
        "conflicting merge over a dirty tree must pause in Merge state"
    );

    let head_before = ca_head_branch(d);
    match checkout_branch_autostash(d, "topic") {
        Err(AppError::OperationInProgress(_)) => {}
        other => panic!("expected OperationInProgress mid-merge, got {other:?}"),
    }
    assert_eq!(
        ca_head_branch(d),
        head_before,
        "nothing switched while an operation is in progress"
    );
}

// ------------------------------------------------ Case 11: wire shape (AC12)

/// AC12: `CheckoutResult` serializes camelCase with `apply` null when None
/// and a tagged `{ "kind": ... }` object otherwise — matches the TS type.
#[test]
fn ca_11_wire_shape_camel_case() {
    use serde_json::json;

    let clean = serde_json::to_value(CheckoutResult {
        stashed: false,
        fast_forwarded: true,
        apply: None,
    })
    .expect("serialize clean");
    assert_eq!(
        clean,
        json!({ "stashed": false, "fastForwarded": true, "apply": null })
    );

    let conflicted = serde_json::to_value(CheckoutResult {
        stashed: true,
        fast_forwarded: false,
        apply: Some(ApplyStashOutcome::Conflicts {
            paths: vec!["src/app.ts".to_string()],
        }),
    })
    .expect("serialize conflicted");
    assert_eq!(
        conflicted,
        json!({
            "stashed": true,
            "fastForwarded": false,
            "apply": { "kind": "conflicts", "paths": ["src/app.ts"] }
        })
    );

    let applied = serde_json::to_value(CheckoutResult {
        stashed: true,
        fast_forwarded: false,
        apply: Some(ApplyStashOutcome::Applied),
    })
    .expect("serialize applied");
    assert_eq!(
        applied["apply"],
        json!({ "kind": "applied" }),
        "Applied serializes to a kind:applied object"
    );
}

// ------------------------------------- P36 §1.3: worktree-collision guard

/// §1.3 data-loss guard: checking out a branch that is checked out in ANOTHER
/// worktree is refused with `BranchCheckedOutElsewhere` BEFORE any side effect
/// — no stash created, HEAD unchanged, and a dirty working state (a modified
/// tracked file + an untracked file) is left exactly as-is on disk.
#[test]
fn cbh_autostash_refuses_branch_in_other_worktree() {
    use crate::git::worktree::add_worktree;

    let dir = crate::testutil::scratch_dir();
    // Init in a subdir so the derived `.worktrees/` container has a unique
    // parent (mirrors worktree.rs's derive tests).
    let repo_dir = dir.path().join("repo");
    let d = repo_dir.as_path();
    ca_init(d);
    ca_commit(d, "base", &[("a.txt", "base\n")]);
    create_branch(d, "feature").expect("create feature");
    let created = add_worktree(d, "feature", "feature").expect("add worktree on feature");

    // Dirty the MAIN worktree: a modified TRACKED file + an UNTRACKED file.
    std::fs::write(d.join("a.txt"), "dirty\n").expect("edit tracked");
    std::fs::write(d.join("new.txt"), "brand new\n").expect("add untracked");

    let head_before = ca_head_oid(d);
    let branch_before = ca_head_branch(d);
    assert!(
        list_stashes(d).expect("list stashes").is_empty(),
        "no stash before the call"
    );

    let err = checkout_branch_autostash(d, "feature").expect_err("must refuse");
    match &err {
        AppError::BranchCheckedOutElsewhere(m) => {
            assert!(m.contains("already checked out at"), "git-like message: {m}");
            assert!(
                m.contains(&created.abs_path),
                "message names the linked worktree path ({}): {m}",
                created.abs_path
            );
            assert!(m.contains("feature"), "message names the branch: {m}");
        }
        other => panic!("expected BranchCheckedOutElsewhere, got {other:?}"),
    }

    // The refusal mutated NOTHING.
    assert!(
        list_stashes(d).expect("list stashes").is_empty(),
        "no stash created on refusal"
    );
    assert_eq!(ca_head_oid(d), head_before, "HEAD oid unchanged");
    assert_eq!(ca_head_branch(d), branch_before, "HEAD branch unchanged");
    assert_eq!(
        ca_read(d, "a.txt"),
        "dirty\n",
        "modified tracked file preserved on disk"
    );
    assert_eq!(
        ca_read(d, "new.txt"),
        "brand new\n",
        "untracked file preserved on disk"
    );
}

/// §1.3: a branch NOT checked out elsewhere still checks out normally even
/// while a linked worktree (on a DIFFERENT branch) exists.
#[test]
fn cbh_autostash_succeeds_for_branch_not_elsewhere() {
    use crate::git::worktree::add_worktree;

    let dir = crate::testutil::scratch_dir();
    let repo_dir = dir.path().join("repo");
    let d = repo_dir.as_path();
    ca_init(d);
    ca_commit(d, "base", &[("a.txt", "base\n")]);
    create_branch(d, "feature").expect("create feature");
    add_worktree(d, "feature", "feature").expect("add worktree on feature");
    // A FREE branch: exists, not checked out in any worktree.
    create_branch(d, "free").expect("create free");

    let res = checkout_branch_autostash(d, "free").expect("checkout free must succeed");
    assert!(!res.stashed, "clean worktree must not stash");
    assert_eq!(
        ca_head_branch(d).as_deref(),
        Some("free"),
        "switched to the free branch"
    );
}

/// Audit 2026-08-07 §3.5: the plain `checkout_branch` shares the
/// other-worktree guard — git itself refuses checking out a branch that is
/// already checked out in another worktree. Refusal precedes any side
/// effect (HEAD and worktree unchanged).
#[test]
fn checkout_branch_refuses_branch_in_other_worktree() {
    use crate::git::worktree::add_worktree;

    let dir = crate::testutil::scratch_dir();
    let repo_dir = dir.path().join("repo");
    let d = repo_dir.as_path();
    ca_init(d);
    ca_commit(d, "base", &[("a.txt", "base\n")]);
    create_branch(d, "feature").expect("create feature");
    let created = add_worktree(d, "feature", "feature").expect("add worktree on feature");

    let head_before = ca_head_oid(d);
    let branch_before = ca_head_branch(d);
    let err = checkout_branch(d, "feature").expect_err("must refuse");
    match &err {
        AppError::BranchCheckedOutElsewhere(m) => {
            assert!(m.contains("already checked out at"), "git-like message: {m}");
            assert!(m.contains(&created.abs_path), "names the worktree: {m}");
        }
        other => panic!("expected BranchCheckedOutElsewhere, got {other:?}"),
    }
    assert_eq!(ca_head_oid(d), head_before, "HEAD oid unchanged");
    assert_eq!(ca_head_branch(d), branch_before, "HEAD branch unchanged");
}

/// Audit 2026-08-07 §3.5: `checkout_remote` reusing an EXISTING local
/// branch that lives in another worktree must refuse the same way (a
/// freshly-created tracking branch cannot be elsewhere).
#[test]
fn checkout_remote_refuses_existing_local_in_other_worktree() {
    use crate::git::worktree::add_worktree;

    let dir = crate::testutil::scratch_dir();
    let repo_dir = dir.path().join("repo");
    let d = repo_dir.as_path();
    let repo = ca_init(d);
    ca_commit(d, "base", &[("a.txt", "base\n")]);
    create_branch(d, "feature").expect("create feature");
    add_worktree(d, "feature", "feature").expect("add worktree on feature");
    // Fabricate the remote-tracking ref locally (no network needed).
    let tip = repo.head().expect("head").peel_to_commit().expect("c").id();
    repo.reference("refs/remotes/origin/feature", tip, true, "test remote ref")
        .expect("remote-tracking ref");

    let branch_before = ca_head_branch(d);
    let err = checkout_remote(d, "origin/feature").expect_err("must refuse");
    assert!(
        matches!(&err, AppError::BranchCheckedOutElsewhere(m)
            if m.contains("already checked out at")),
        "got {err:?}"
    );
    assert_eq!(ca_head_branch(d), branch_before, "HEAD branch unchanged");
}
