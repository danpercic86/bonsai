//! M5 adversarial probes (tester gap-probing, beyond contract §6.1–§6.4).
//!
//! Same CLI-oracle / twin-repo machinery as `branches_cli.rs`. These tests
//! PIN observed behavior for risky uncovered cases; where our behavior
//! diverges from the plain `git` CLI, the divergence is asserted explicitly
//! and flagged in comments so it is a conscious decision, not an accident.
//!
//! All scratch repos live under `D:\Temp\bonsai-scratch` (common::scratch_dir).

mod common;

use std::path::Path;

use bonsai_lib::error::AppError;
use bonsai_lib::git::branches::{
    checkout_branch, create_branch, delete_branch, list_refs,
};
use common::{commit_fixed, git, git_ok, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

/// Base fixture: one committed file on `main`.
fn base_repo() -> tempfile::TempDir {
    let dir = init_repo();
    let path = dir.path();
    std::fs::write(path.join("file.txt"), "main v1\n").expect("write file.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "base");
    dir
}

fn read(path: &Path, name: &str) -> String {
    std::fs::read_to_string(path.join(name)).expect("read file")
}

// ---------------------------------------------------------------------------
// Probe 1: exotic-but-VALID branch names (unicode, inner dots, deep slashes)
// must round-trip create → list → checkout → delete exactly like the CLI,
// including as loose-ref FILENAMES on a Windows filesystem.
#[test]
fn exotic_valid_branch_names_round_trip() {
    require_git!();
    let names = ["feature/ünïcode", "a.b", "änderung.v2/тема"];

    for name in names {
        // Oracle: the git CLI considers each of these VALID.
        let a = base_repo(); // ours
        let b = base_repo(); // CLI twin
        assert!(
            git_ok(a.path(), &["check-ref-format", "--branch", name]),
            "oracle: git must accept {name:?} — fixture assumption broken"
        );

        // Create: ref exists at HEAD, byte-identical resolution via the CLI.
        create_branch(a.path(), name).unwrap_or_else(|e| panic!("create {name:?}: {e:?}"));
        git(b.path(), &["branch", name]);
        assert_eq!(
            git(a.path(), &["rev-parse", &format!("refs/heads/{name}")]),
            git(a.path(), &["rev-parse", "HEAD"]),
            "{name:?} must point at HEAD"
        );

        // List: shows up (as UTF-8, not mangled) in the snapshot.
        let snap = list_refs(a.path()).expect("list_refs");
        assert!(
            snap.local.iter().any(|br| br.name == name),
            "{name:?} missing from list_refs local: {:?}",
            snap.local.iter().map(|br| &br.name).collect::<Vec<_>>()
        );

        // Checkout: HEAD symref matches the CLI twin's.
        checkout_branch(a.path(), name).unwrap_or_else(|e| panic!("checkout {name:?}: {e:?}"));
        git(b.path(), &["checkout", name]);
        assert_eq!(
            git(a.path(), &["symbolic-ref", "HEAD"]),
            git(b.path(), &["symbolic-ref", "HEAD"]),
            "HEAD symref mismatch for {name:?}"
        );
        let snap = list_refs(a.path()).expect("list_refs after checkout");
        let entry = snap.local.iter().find(|br| br.name == name).expect("entry");
        assert!(entry.is_head, "{name:?} must be is_head after checkout");

        // Delete (must switch away first): both sides succeed, ref fully gone.
        checkout_branch(a.path(), "main").expect("back to main");
        git(b.path(), &["checkout", "main"]);
        delete_branch(a.path(), name).unwrap_or_else(|e| panic!("delete {name:?}: {e:?}"));
        assert!(git_ok(b.path(), &["branch", "-d", name]), "CLI twin -d {name:?}");
        assert!(
            !git_ok(a.path(), &["rev-parse", "--verify", &format!("refs/heads/{name}")]),
            "{name:?} must be gone after delete"
        );
        // Slash-named branches leave empty ref dirs; the repo must stay sane.
        assert!(git_ok(a.path(), &["fsck", "--strict"]), "fsck after deleting {name:?}");
    }
}

// ---------------------------------------------------------------------------
// Probe 2: merged-check vs TRUE MERGE commits. `graph_descendant_of` must see
// a branch merged via a real merge commit (not fast-forward) as deletable —
// and must re-block it the moment the branch advances past the merge.
#[test]
fn delete_after_true_merge_then_after_advance() {
    require_git!();

    // Fixture: main and topic diverge, then `git merge --no-ff topic` on main
    // creates a genuine 2-parent merge commit (topic tip is NOT an ancestor
    // via first-parent-only history — only via the second parent).
    let build = || {
        let dir = base_repo();
        let path = dir.path();
        git(path, &["checkout", "-b", "topic"]);
        std::fs::write(path.join("topic.txt"), "topic\n").expect("write topic.txt");
        git(path, &["add", "-A"]);
        commit_fixed(path, "topic work");
        git(path, &["checkout", "main"]);
        std::fs::write(path.join("main.txt"), "main\n").expect("write main.txt");
        git(path, &["add", "-A"]);
        commit_fixed(path, "main diverges");
        common::git_env(
            path,
            &["merge", "--no-ff", "--no-edit", "topic"],
            &[
                ("GIT_AUTHOR_DATE", common::FIXED_DATE),
                ("GIT_COMMITTER_DATE", common::FIXED_DATE),
            ],
        );
        dir
    };

    // Part A: merged via merge commit → deletable; twin `git branch -d` agrees.
    let a = build();
    let b = build();
    let merge_parents = git(a.path(), &["rev-list", "--parents", "-n", "1", "HEAD"]);
    assert_eq!(
        merge_parents.split_whitespace().count(),
        3,
        "fixture must end in a 2-parent merge commit, got: {merge_parents}"
    );
    delete_branch(a.path(), "topic").expect("branch merged via true merge must be deletable");
    assert!(!git_ok(a.path(), &["rev-parse", "--verify", "refs/heads/topic"]));
    assert!(git_ok(b.path(), &["branch", "-d", "topic"]), "CLI twin agrees");

    // Part B: same history but topic then ADVANCES one commit past the merge
    // → unmerged again; twin `git branch -d` also refuses.
    let a = build();
    let b = build();
    for p in [a.path(), b.path()] {
        git(p, &["checkout", "topic"]);
        std::fs::write(p.join("topic.txt"), "topic v2\n").expect("write topic.txt");
        git(p, &["add", "-A"]);
        commit_fixed(p, "topic advances past the merge");
        git(p, &["checkout", "main"]);
    }
    let err = delete_branch(a.path(), "topic").expect_err("advanced topic must be blocked");
    assert!(matches!(err, AppError::UnmergedBranch(_)), "got {err:?}");
    assert!(git_ok(a.path(), &["rev-parse", "--verify", "refs/heads/topic"]));
    assert!(!git_ok(b.path(), &["branch", "-d", "topic"]), "CLI twin also refuses");
}

// ---------------------------------------------------------------------------
// Probe 3: checkout with a STAGED (index-only) conflicting change. The
// worktree file was staged, so worktree == index != HEAD; contract §6.3.3
// only covered the unstaged case. Safe checkout must refuse and leave HEAD,
// index, and worktree untouched — exactly like the CLI twin.
#[test]
fn checkout_with_staged_conflicting_change_blocked() {
    require_git!();
    let build = || {
        let dir = base_repo();
        let path = dir.path();
        git(path, &["checkout", "-b", "side"]);
        std::fs::write(path.join("file.txt"), "side v1\n").expect("write file.txt");
        git(path, &["add", "-A"]);
        commit_fixed(path, "side change");
        git(path, &["checkout", "main"]);
        // Stage a local edit to the file that DIFFERS between branches.
        std::fs::write(path.join("file.txt"), "staged local edit\n").expect("write file.txt");
        git(path, &["add", "file.txt"]);
        dir
    };
    let a = build();
    let b = build();
    let head_before = git(a.path(), &["symbolic-ref", "HEAD"]);
    let porcelain_before = common::porcelain_records(a.path());
    let index_oid_before = git(a.path(), &["write-tree"]);

    let err = checkout_branch(a.path(), "side").expect_err("staged conflict must fail");
    assert!(matches!(err, AppError::CheckoutConflict(_)), "got {err:?}");

    // Twin oracle: git checkout also refuses.
    assert!(!git_ok(b.path(), &["checkout", "side"]), "CLI twin must refuse too");

    // Nothing moved: HEAD, worktree, porcelain, and the INDEX are untouched.
    assert_eq!(git(a.path(), &["symbolic-ref", "HEAD"]), head_before);
    assert_eq!(read(a.path(), "file.txt"), "staged local edit\n");
    assert_eq!(common::porcelain_records(a.path()), porcelain_before);
    assert_eq!(git(a.path(), &["write-tree"]), index_oid_before, "index must be untouched");
}

// ---------------------------------------------------------------------------
// Probe 4: case-colliding branch names (`feature` vs `Feature`) — the classic
// Windows loose-ref edge (NTFS is case-insensitive, so both names hit the
// same `.git/refs/heads` file). Create must fail cleanly like the CLI and
// must not corrupt the existing ref.
//
// NOTE (documented CLI quirk, out of our scope): on the same filesystem the
// CLI's `git checkout Feature` "succeeds", pointing HEAD at the nonexistent
// refs/heads/Feature — long-standing git-on-Windows weirdness. Bonsai never
// takes free-text checkout input (the UI only offers names from list_refs),
// so only the create path is reachable and is what we pin here.
#[test]
fn case_colliding_branch_create_matches_cli() {
    require_git!();
    let a = base_repo();
    let b = base_repo();
    for p in [a.path(), b.path()] {
        git(p, &["branch", "feature"]);
    }
    let refs_before = git(a.path(), &["for-each-ref"]);

    // Oracle: the CLI refuses ("a branch named 'Feature' already exists").
    assert!(
        !git_ok(b.path(), &["branch", "Feature"]),
        "oracle: CLI must refuse the case-colliding name on Windows"
    );

    // Ours: same refusal, surfaced as BranchExists.
    let err = create_branch(a.path(), "Feature").expect_err("case collision must fail");
    assert!(
        matches!(err, AppError::BranchExists(_)),
        "expected BranchExists for case-colliding name, got {err:?}"
    );

    // No ref was created, none was clobbered.
    assert_eq!(git(a.path(), &["for-each-ref"]), refs_before);
    let snap = list_refs(a.path()).expect("list_refs");
    let names: Vec<&str> = snap.local.iter().map(|br| br.name.as_str()).collect();
    assert_eq!(names, ["feature", "main"]);
}

// ---------------------------------------------------------------------------
// Probe 5: ahead/behind after the upstream is FORCE-MOVED onto a diverged
// line (simulated rewind + rewrite: `git update-ref` on the remote-tracking
// ref, exactly the state a `git fetch` after an upstream force-push leaves).
// Counts must still match `git rev-list --left-right --count`.
#[test]
fn ahead_behind_after_upstream_force_move() {
    require_git!();
    let dir = base_repo();
    let path = dir.path();

    // Diverged line `alt` from the base commit.
    git(path, &["branch", "alt"]);
    git(path, &["checkout", "alt"]);
    std::fs::write(path.join("alt.txt"), "alt\n").expect("write alt.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "alt line");
    git(path, &["checkout", "main"]);

    // main gains two commits of its own.
    for v in ["v2", "v3"] {
        std::fs::write(path.join("file.txt"), format!("main {v}\n")).expect("write");
        git(path, &["add", "-A"]);
        commit_fixed(path, &format!("main {v}"));
    }

    // Real upstream via a local bare remote, then force-move the
    // remote-tracking ref onto the diverged `alt` tip.
    let bare = common::scratch_dir();
    git(bare.path(), &["init", "--bare"]);
    let bare_url = bare.path().to_string_lossy().replace('\\', "/");
    git(path, &["remote", "add", "origin", &bare_url]);
    git(path, &["push", "origin", "main"]);
    git(path, &["fetch", "origin"]);
    git(path, &["branch", "--set-upstream-to=origin/main", "main"]);
    git(path, &["update-ref", "refs/remotes/origin/main", "refs/heads/alt"]);

    let snap = list_refs(path).expect("list_refs");
    let main = snap.local.iter().find(|br| br.name == "main").expect("main");
    assert_eq!(main.upstream.as_deref(), Some("origin/main"));

    let counts = git(
        path,
        &["rev-list", "--left-right", "--count", "origin/main...main"],
    );
    let mut parts = counts.split_whitespace();
    let behind: u32 = parts.next().expect("behind").parse().expect("behind u32");
    let ahead: u32 = parts.next().expect("ahead").parse().expect("ahead u32");
    assert_eq!(main.ahead, Some(ahead), "ahead vs rev-list oracle");
    assert_eq!(main.behind, Some(behind), "behind vs rev-list oracle");
    // The fixture is non-trivial by construction: 2 ahead, 1 behind.
    assert_eq!((ahead, behind), (2, 1), "fixture sanity");
}
