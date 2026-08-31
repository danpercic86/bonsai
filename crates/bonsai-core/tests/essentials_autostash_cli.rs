//! P47 CLI-oracle suite (contract §7.2): the genuinely-NEW cherry-pick / revert
//! logic that P47 added on top of the P20 oracle in `essentials_cli.rs` —
//! autostash of a dirty tracked worktree, the editable cherry-pick message
//! (incl. its survival across a conflict pause via MERGE_MSG), stash retention
//! on conflict, and the stash-pop-conflict outcome.
//!
//! Twin-repo pattern (identical to `essentials_cli.rs` / `merge_cli.rs`): two
//! scratch repos are built by the IDENTICAL scripted CLI setup (fixed dates →
//! identical base oids). Bonsai's core fns run on one; the real `git` CLI on the
//! other. We compare tree oids / messages / authors — never commit oids
//! (committer time = now() differs). Each test skips (passes with a note) if
//! `git` is absent.
//!
//! All scratch repos live under `D:\Data\Temp\bonsai-scratch` (C: is full).

mod common;

use std::path::Path;

use bonsai_core::git::cherrypick::{cherrypick_commit, cherrypick_continue, CherrypickOutcome};
use bonsai_core::git::conflict::resolve_conflict_text;
use bonsai_core::git::revert::{revert_commit, RevertOutcome};
use common::{git, git_env, git_ok, init_repo, FIXED_DATE};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

// ------------------------------------------------------------ small helpers

fn head_oid(dir: &Path) -> String {
    git(dir, &["rev-parse", "HEAD"])
}

fn tree_oid(dir: &Path) -> String {
    git(dir, &["rev-parse", "HEAD^{tree}"])
}

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

fn read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).expect("read fixture file")
}

/// CLI commit of the CURRENT worktree change to `name` with fixed dates.
fn add_commit(dir: &Path, name: &str, content: &str, msg: &str) {
    write(dir, name, content);
    git(dir, &["add", name]);
    git_env(
        dir,
        &["commit", "-m", msg],
        &[
            ("GIT_AUTHOR_DATE", FIXED_DATE),
            ("GIT_COMMITTER_DATE", FIXED_DATE),
        ],
    );
}

fn repo_state(dir: &Path) -> git2::RepositoryState {
    git2::Repository::open(dir).expect("open repo").state()
}

/// Number of entries on the stash stack (mirrors merge_cli.rs). git2's
/// stash_save2 writes the standard refs/stash + reflog, so `git stash list`
/// sees it.
fn stash_count(dir: &Path) -> usize {
    git(dir, &["stash", "list"])
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
}

/// Author epoch (%at) of a commit, to prove author preservation.
fn author_epoch(dir: &Path, rev: &str) -> i64 {
    git(dir, &["show", "-s", "--format=%at", rev])
        .parse()
        .expect("author epoch")
}

/// base → feature adds feature.txt (the pick) → main advances with main.txt so
/// the pick is a real (non-FF) pick that does NOT touch feature.txt/base.txt.
/// Returns (pick_oid, main_tip_oid).
fn build_disjoint_pick(d: &Path) -> (String, String) {
    add_commit(d, "base.txt", "base\n", "base");
    git(d, &["checkout", "-b", "feature"]);
    add_commit(d, "feature.txt", "feature\n", "add feature");
    let pick = head_oid(d);
    git(d, &["checkout", "main"]);
    add_commit(d, "main.txt", "main\n", "main work");
    (pick, head_oid(d))
}

/// base x.txt → feature edits x.txt one way → main edits the SAME line another
/// way; plus a disjoint `other.txt` that stays clean. Cherry-picking feature
/// onto main conflicts on x.txt. Returns the pick oid.
fn build_conflicting_pick(d: &Path) -> String {
    write(d, "x.txt", "line1\nbase\nline3\n");
    write(d, "other.txt", "other base\n");
    git(d, &["add", "-A"]);
    git_env(
        d,
        &["commit", "-m", "base"],
        &[
            ("GIT_AUTHOR_DATE", FIXED_DATE),
            ("GIT_COMMITTER_DATE", FIXED_DATE),
        ],
    );
    git(d, &["checkout", "-b", "feature"]);
    add_commit(d, "x.txt", "line1\nfeature\nline3\n", "feature edit");
    let pick = head_oid(d);
    git(d, &["checkout", "main"]);
    add_commit(d, "x.txt", "line1\nmain\nline3\n", "main edit");
    pick
}

// ================================================= §7.2 autostash-clean parity

/// A dirty TRACKED worktree + `cherrypick_commit(None)` autostashes the edit,
/// runs a clean pick, then restores the edit. The committed HEAD tree + message
/// + author match `git stash` → `git cherry-pick` → `git stash pop`; the
///   previously-dirty worktree change is restored; the stash stack ends empty.
#[test]
fn p47_cherrypick_autostash_clean_matches_cli() {
    require_git!();
    let a_dir = init_repo();
    let b_dir = init_repo();
    let a = a_dir.path();
    let b = b_dir.path();

    let (pick_a, main_a) = build_disjoint_pick(a);
    let (pick_b, main_b) = build_disjoint_pick(b);
    assert_eq!(pick_a, pick_b, "twin pick oids must match");
    assert_eq!(main_a, main_b, "twin main tips must match");
    let pick_author_at = author_epoch(a, &pick_a);

    // Dirty tracked edit to base.txt on BOTH twins (untouched by the pick, so
    // the pop applies cleanly).
    let dirty = "base dirty local\n";
    write(a, "base.txt", dirty);
    write(b, "base.txt", dirty);

    // Bonsai: autostash + pick + restore in one call.
    let outcome = cherrypick_commit(a, &pick_a, None).expect("bonsai cherry-pick");
    match outcome {
        CherrypickOutcome::Committed { stashed, .. } => {
            assert!(stashed, "a dirty tracked worktree must autostash → stashed:true");
        }
        other => panic!("expected Committed{{stashed:true}}, got {other:?}"),
    }

    // Twin: the literal git autostash recipe from the contract.
    git(b, &["stash", "push", "-m", "twin autostash"]);
    assert!(git_ok(b, &["cherry-pick", &pick_b]), "twin pick must be clean");
    git(b, &["stash", "pop"]);

    // Committed HEAD tree matches (the dirty edit is NOT in the commit — it was
    // stashed away — on either side).
    assert_eq!(tree_oid(a), tree_oid(b), "committed pick tree must match the CLI");

    let repo = git2::Repository::open(a).expect("open A");
    let head = repo.head().expect("head").peel_to_commit().expect("peel");
    assert_eq!(head.parent_count(), 1, "pick has a single parent");
    assert_eq!(
        head.parent_id(0).expect("parent").to_string(),
        main_a,
        "HEAD advanced onto the former main tip"
    );
    assert_eq!(head.message().ok(), Some("add feature\n"), "picked message reused");
    assert_eq!(head.author().name().ok(), Some("Test User"));
    assert_eq!(
        head.author().when().seconds(),
        pick_author_at,
        "the ORIGINAL author time is preserved"
    );

    // The previously-dirty change is restored to the worktree, on both.
    assert_eq!(read(a, "base.txt"), dirty, "the autostashed edit must be restored");
    assert_eq!(read(a, "base.txt"), read(b, "base.txt"));
    assert_eq!(read(a, "feature.txt"), "feature\n");
    // Clean state, empty stash stack (a clean pop dropped the autostash).
    assert_eq!(repo_state(a), git2::RepositoryState::Clean);
    assert_eq!(stash_count(a), 0, "a clean pop drops the autostash");
}

// ====================================================== §7.2 custom-message parity

/// `cherrypick_commit(Some(m))` on a clean pick commits with the normalized
/// custom message, preserves the ORIGINAL author, uses the configured committer,
/// and produces the SAME tree as a plain `git cherry-pick`.
#[test]
fn p47_cherrypick_custom_message_matches_cli() {
    require_git!();
    let a_dir = init_repo();
    let b_dir = init_repo();
    let a = a_dir.path();
    let b = b_dir.path();

    let (pick_a, _main_a) = build_disjoint_pick(a);
    let (pick_b, _main_b) = build_disjoint_pick(b);
    assert_eq!(pick_a, pick_b, "twin pick oids must match");
    let pick_author_at = author_epoch(a, &pick_a);

    let custom = "custom subject\n\nbody line one\nbody line two";
    let outcome = cherrypick_commit(a, &pick_a, Some(custom)).expect("bonsai cherry-pick");
    match outcome {
        CherrypickOutcome::Committed { stashed, .. } => {
            assert!(!stashed, "clean tree → no autostash");
        }
        other => panic!("expected Committed, got {other:?}"),
    }

    // Twin: plain cherry-pick — the message differs but the TREE is identical
    // (message never affects the tree).
    git(b, &["cherry-pick", &pick_b]);
    assert_eq!(tree_oid(a), tree_oid(b), "custom-message pick tree must match a plain pick");

    let repo = git2::Repository::open(a).expect("open A");
    let head = repo.head().expect("head").peel_to_commit().expect("peel");
    // Message == normalize(custom): trimmed + single trailing newline.
    assert_eq!(
        head.message().ok(),
        Some("custom subject\n\nbody line one\nbody line two\n"),
        "committed message must equal the normalized custom text"
    );
    // Author == the ORIGINAL commit's author (name + time preserved).
    assert_eq!(head.author().name().ok(), Some("Test User"));
    assert_eq!(head.author().email().ok(), Some("test@example.com"));
    assert_eq!(
        head.author().when().seconds(),
        pick_author_at,
        "custom-message pick preserves the original author time"
    );
    // Committer == the configured signature.
    assert_eq!(head.committer().name().ok(), Some("Test User"));
    assert_eq!(head.committer().email().ok(), Some("test@example.com"));
    assert_eq!(repo_state(a), git2::RepositoryState::Clean);
}

// ============================================ §7.2 custom-message survives conflict

/// A conflicting pick with `Some(m)` persists the normalized message to
/// `.git/MERGE_MSG`; after resolving + `cherrypick_continue`, the final commit
/// message equals `normalize(m)` (proving the override survives the pause).
#[test]
fn p47_cherrypick_custom_message_survives_conflict() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    let pick = build_conflicting_pick(d);

    let custom = "reworded pick subject\n\nrationale in the body";
    match cherrypick_commit(d, &pick, Some(custom)).expect("bonsai cherry-pick") {
        CherrypickOutcome::Conflicts { paths, stashed } => {
            assert_eq!(paths, vec!["x.txt".to_string()], "x.txt must be conflicted");
            assert!(!stashed, "clean tree → no autostash");
        }
        other => panic!("expected Conflicts, got {other:?}"),
    }
    assert_eq!(repo_state(d), git2::RepositoryState::CherryPick);

    // The override is persisted (normalized) to MERGE_MSG so continue honors it.
    let merge_msg = std::fs::read_to_string(d.join(".git").join("MERGE_MSG")).expect("MERGE_MSG");
    assert_eq!(
        merge_msg, "reworded pick subject\n\nrationale in the body\n",
        "the custom message must be persisted to MERGE_MSG across the pause"
    );

    // Resolve + continue.
    resolve_conflict_text(d, "x.txt", "line1\nresolved\nline3\n").expect("resolve index");
    match cherrypick_continue(d).expect("bonsai continue") {
        CherrypickOutcome::Committed { .. } => {}
        other => panic!("expected Committed after resolve, got {other:?}"),
    }

    let repo = git2::Repository::open(d).expect("open");
    let head = repo.head().expect("head").peel_to_commit().expect("peel");
    assert_eq!(
        head.message().ok(),
        Some("reworded pick subject\n\nrationale in the body\n"),
        "the final commit message must be the custom override, not the picked message"
    );
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
}

// ===================================================== §7.2 autostash retains stash

/// A dirty tracked worktree + a CONFLICTING pick → `Conflicts{stashed:true}`
/// with the autostash RETAINED. `cherrypick_continue` finalizes WITHOUT
/// auto-popping the stash (F5) — the stash entry is still present afterward.
#[test]
fn p47_cherrypick_autostash_conflict_retains_stash_then_continue_keeps_it() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    let pick = build_conflicting_pick(d);

    // Dirty tracked edit to the DISJOINT other.txt → autostash happens; the pick
    // still conflicts on x.txt.
    write(d, "other.txt", "other dirty\n");

    match cherrypick_commit(d, &pick, None).expect("bonsai cherry-pick") {
        CherrypickOutcome::Conflicts { paths, stashed } => {
            assert_eq!(paths, vec!["x.txt".to_string()]);
            assert!(stashed, "the dirty other.txt edit must be autostashed → stashed:true");
        }
        other => panic!("expected Conflicts{{stashed:true}}, got {other:?}"),
    }
    assert_eq!(repo_state(d), git2::RepositoryState::CherryPick);
    assert_eq!(stash_count(d), 1, "the autostash is retained during the paused pick");
    // Mid-pause, other.txt sits at HEAD (the edit is on the stash, not the tree).
    assert_eq!(read(d, "other.txt"), "other base\n", "the edit is on the stash");

    // Resolve + continue: finalizes but does NOT auto-pop the retained stash.
    resolve_conflict_text(d, "x.txt", "line1\nresolved\nline3\n").expect("resolve index");
    match cherrypick_continue(d).expect("continue") {
        CherrypickOutcome::Committed { stashed, .. } => {
            assert!(!stashed, "continue never re-pops (F5) → stashed:false");
        }
        other => panic!("expected Committed after resolve, got {other:?}"),
    }
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert_eq!(stash_count(d), 1, "continue must NOT auto-pop the retained autostash (F5)");

    // Data-safety proof: the retained stash still restores the edit.
    git(d, &["stash", "pop"]);
    assert_eq!(read(d, "other.txt"), "other dirty\n", "the edit is recoverable from stash@{{0}}");
    assert_eq!(stash_count(d), 0);
}

/// The abort twin of the previous test: `cherrypick_abort` on a dirty-tree
/// paused pick also leaves the retained autostash untouched (F5).
#[test]
fn p47_cherrypick_autostash_conflict_abort_keeps_stash() {
    require_git!();
    use bonsai_core::git::cherrypick::cherrypick_abort;
    let dir = init_repo();
    let d = dir.path();
    let pick = build_conflicting_pick(d);
    let head_before = head_oid(d);

    write(d, "other.txt", "other dirty\n");
    match cherrypick_commit(d, &pick, None).expect("cherry-pick") {
        CherrypickOutcome::Conflicts { stashed, .. } => assert!(stashed),
        other => panic!("expected Conflicts{{stashed:true}}, got {other:?}"),
    }
    assert_eq!(stash_count(d), 1);

    cherrypick_abort(d).expect("abort");
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
    assert_eq!(head_oid(d), head_before, "abort restores HEAD");
    assert_eq!(stash_count(d), 1, "abort must NOT drop the retained autostash (F5)");

    // The edit is still recoverable.
    git(d, &["stash", "pop"]);
    assert_eq!(read(d, "other.txt"), "other dirty\n");
}

// ======================================================= §7.2 stash-pop-conflict

/// A pick that commits cleanly but whose autostash re-apply COLLIDES with the
/// picked change → `StashPopConflicts{head,paths}`; the stash is RETAINED and
/// the committed tree matches a plain `git cherry-pick`.
#[test]
fn p47_cherrypick_stash_pop_conflict() {
    require_git!();
    let a_dir = init_repo();
    let b_dir = init_repo();
    let a = a_dir.path();
    let b = b_dir.path();

    // base f.txt → feature edits f.txt (the pick) → main advances with a
    // disjoint file (f.txt untouched on main, so the pick applies cleanly).
    let build = |d: &Path| -> String {
        add_commit(d, "f.txt", "base\n", "base");
        git(d, &["checkout", "-b", "feature"]);
        add_commit(d, "f.txt", "feature\n", "feature edit");
        let pick = head_oid(d);
        git(d, &["checkout", "main"]);
        add_commit(d, "main.txt", "main\n", "main work");
        pick
    };
    let pick_a = build(a);
    let pick_b = build(b);
    assert_eq!(pick_a, pick_b, "twin pick oids must match");

    // Dirty edit to f.txt — the SAME file the pick modifies. Autostash resets it
    // to HEAD (base), the pick sets it to "feature", then the pop of the stashed
    // base→local diff collides on f.txt.
    write(a, "f.txt", "local unstaged\n");

    let (head, paths) = match cherrypick_commit(a, &pick_a, None).expect("cherry-pick") {
        CherrypickOutcome::StashPopConflicts { head, paths } => (head, paths),
        other => panic!("expected StashPopConflicts, got {other:?}"),
    };
    assert_eq!(paths, vec!["f.txt".to_string()], "f.txt conflicted on the pop");
    assert_eq!(head, head_oid(a), "head = the new pick-commit oid");

    // A conflicted stash-apply is NOT a cherry-pick op: state stays Clean.
    assert_eq!(repo_state(a), git2::RepositoryState::Clean, "state must be Clean");
    assert!(
        !a.join(".git").join("CHERRY_PICK_HEAD").exists(),
        "the pick committed → no CHERRY_PICK_HEAD"
    );
    let f = read(a, "f.txt");
    assert!(
        f.contains("<<<<<<<") && f.contains(">>>>>>>"),
        "f.txt must carry conflict markers, got:\n{f}"
    );
    assert_eq!(stash_count(a), 1, "a conflicting pop RETAINS the autostash");

    // The committed pick tree matches a plain twin cherry-pick (the dirty edit
    // never entered the commit).
    git(b, &["cherry-pick", &pick_b]);
    assert_eq!(tree_oid(a), tree_oid(b), "committed pick tree must match a plain pick");
}

// ==================================================== §7.2 revert autostash parity

/// A dirty TRACKED worktree + a clean revert autostashes the edit, reverts, then
/// restores the edit. The committed tree + byte-exact `git revert --no-edit`
/// message match the CLI; the dirty change is restored; the stash ends empty.
#[test]
fn p47_revert_autostash_clean_matches_cli() {
    require_git!();
    let a_dir = init_repo();
    let b_dir = init_repo();
    let a = a_dir.path();
    let b = b_dir.path();

    // base adds x.txt + unrelated.txt; second edits x.txt. Reverting second
    // undoes only x.txt; unrelated.txt is where the dirty edit lives.
    let build = |d: &Path| -> String {
        write(d, "x.txt", "base\n");
        write(d, "unrelated.txt", "u base\n");
        git(d, &["add", "-A"]);
        git_env(
            d,
            &["commit", "-m", "base"],
            &[
                ("GIT_AUTHOR_DATE", FIXED_DATE),
                ("GIT_COMMITTER_DATE", FIXED_DATE),
            ],
        );
        add_commit(d, "x.txt", "v2\n", "second");
        head_oid(d)
    };
    let c2_a = build(a);
    let c2_b = build(b);
    assert_eq!(c2_a, c2_b, "twin target oids must match (fixed dates)");

    // Dirty tracked edit to unrelated.txt on BOTH twins (revert leaves it alone,
    // so the pop is clean).
    let dirty = "u dirty local\n";
    write(a, "unrelated.txt", dirty);
    write(b, "unrelated.txt", dirty);

    match revert_commit(a, &c2_a).expect("bonsai revert") {
        RevertOutcome::Committed { stashed, .. } => {
            assert!(stashed, "a dirty tracked worktree must autostash → stashed:true");
        }
        other => panic!("expected Committed{{stashed:true}}, got {other:?}"),
    }

    // Twin: the literal git autostash recipe.
    git(b, &["stash", "push", "-m", "twin autostash"]);
    assert!(git_ok(b, &["revert", "--no-edit", &c2_b]), "twin revert must be clean");
    git(b, &["stash", "pop"]);

    assert_eq!(tree_oid(a), tree_oid(b), "committed revert tree must match the CLI");
    assert_eq!(read(a, "x.txt"), "base\n", "revert undoes the second commit");

    let repo = git2::Repository::open(a).expect("open A");
    let head = repo.head().expect("head").peel_to_commit().expect("peel");
    let expected = format!("Revert \"second\"\n\nThis reverts commit {c2_a}.\n");
    assert_eq!(head.message().ok(), Some(expected.as_str()), "byte-exact revert message");

    // The previously-dirty change is restored, on both.
    assert_eq!(read(a, "unrelated.txt"), dirty, "the autostashed edit must be restored");
    assert_eq!(read(a, "unrelated.txt"), read(b, "unrelated.txt"));
    assert_eq!(repo_state(a), git2::RepositoryState::Clean);
    assert_eq!(stash_count(a), 0, "a clean pop drops the autostash");
}
