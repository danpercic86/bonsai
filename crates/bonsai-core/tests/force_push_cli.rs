//! P37 CLI-oracle force-push-with-lease tests (contract §8).
//!
//! Every "remote" is a LOCAL BARE repo (`git init --bare`) referenced by a
//! plain path — the local transport needs NO network and NO credentials, so
//! the lease pre-check (connect_auth(Push) + ls-remote) and the force-push run
//! hermetically (no Git Credential Manager prompts). All scratch repos live
//! under `D:\Temp\bonsai-scratch`.
//!
//! Cross-check against the real `git` CLI: after each operation the origin's
//! `main` tip is read with `git rev-parse` and compared to the expected oid.
//!
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::{Path, PathBuf};

use bonsai_core::error::AppError;
use bonsai_core::git::remote::{force_push_with_lease, PushResult};
use common::{commit_fixed, git, git_env};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

/// bare "origin" + a `work` clone (the repo under test) with an initial commit
/// on `main` pushed to origin (so `refs/remotes/origin/main` exists).
struct Fixture {
    _dir: tempfile::TempDir,
    bare: PathBuf,
    work: PathBuf,
    root: PathBuf,
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn configure_identity(repo: &Path) {
    git(repo, &["config", "user.name", "Test User"]);
    git(repo, &["config", "user.email", "test@example.com"]);
    git(repo, &["config", "core.autocrlf", "false"]);
    // Belt-and-braces: never consult a real credential helper (contract §8).
    git(repo, &["config", "--add", "credential.helper", ""]);
}

/// Clones `bare` into `<root>/<name>` with repo-local identity configured.
fn clone_from_bare(root: &Path, bare: &Path, name: &str) -> PathBuf {
    git(root, &["clone", &path_str(bare), name]);
    let dir = root.join(name);
    configure_identity(&dir);
    dir
}

/// bare origin + `work` clone with `main` = one commit pushed & tracked.
fn init_origin_and_clone() -> Fixture {
    let dir = common::scratch_dir();
    let root = dir.path().to_path_buf();

    git(&root, &["init", "--bare", "-b", "main", "origin.git"]);
    let bare = root.join("origin.git");

    let work = clone_from_bare(&root, &bare, "work");
    git(&work, &["checkout", "-B", "main"]);
    std::fs::write(work.join("hello.txt"), "hello\n").expect("write hello.txt");
    git(&work, &["add", "-A"]);
    commit_fixed(&work, "initial");
    git(&work, &["push", "-u", "origin", "main"]);

    Fixture {
        _dir: dir,
        bare,
        work,
        root,
    }
}

/// The origin's `main` tip, per the real `git` CLI (the oracle).
fn origin_main(f: &Fixture) -> String {
    git(&f.bare, &["rev-parse", "refs/heads/main"])
}

/// The local `HEAD` tip, per the real `git` CLI.
fn head_oid(repo: &Path) -> String {
    git(repo, &["rev-parse", "HEAD"])
}

/// Rewrites `main`'s tip in `repo` (amend with a changed tree → new oid),
/// simulating a rebase/amend. Returns the new HEAD oid.
fn rewrite_head(repo: &Path, marker: &str) -> String {
    std::fs::write(repo.join("rewrite.txt"), format!("{marker}\n")).expect("write rewrite.txt");
    git(repo, &["add", "-A"]);
    git_env(
        repo,
        &["commit", "--amend", "-m", "rewritten"],
        &[
            ("GIT_AUTHOR_DATE", "2026-02-03T04:05:06+0000"),
            ("GIT_COMMITTER_DATE", "2026-02-03T04:05:06+0000"),
        ],
    );
    head_oid(repo)
}

// ------------------------------------------------ §8.A lease refuses (moved)

/// A. A THIRD party advanced origin/main (tip = Y). `work`'s remote-tracking
/// ref is still X (no fetch). We rewrite `main` to Z locally →
/// force_push_with_lease must REFUSE (PushRejected, "moved"/"fetch") and leave
/// origin's `main` UNCHANGED at Y.
#[test]
fn lease_refuses_when_remote_moved() {
    require_git!();
    let f = init_origin_and_clone();

    // A second clone publishes commit Y to origin/main.
    let other = clone_from_bare(&f.root, &f.bare, "other");
    std::fs::write(other.join("other.txt"), "y\n").expect("write other.txt");
    git(&other, &["add", "-A"]);
    commit_fixed(&other, "third-party");
    git(&other, &["push", "origin", "main"]);
    let y = origin_main(&f);

    // `work` never fetched, so its remote-tracking ref is still X. Rewrite Z.
    let z = rewrite_head(&f.work, "z");
    assert_ne!(z, y, "local rewrite must differ from the third-party tip");

    let err = force_push_with_lease(&f.work).expect_err("lease must refuse");
    match err {
        AppError::PushRejected(m) => {
            let low = m.to_lowercase();
            assert!(low.contains("moved"), "message must mention 'moved': {m}");
            assert!(low.contains("fetch"), "message must mention 'fetch': {m}");
        }
        other => panic!("expected PushRejected, got {other:?}"),
    }

    // Oracle: origin's main is untouched (still Y).
    assert_eq!(origin_main(&f), y, "origin main must be unchanged after refusal");
}

// --------------------------------------------- §8.B lease succeeds (moves)

/// B. No third-party push (origin tip = X = remote-tracking). We rewrite `main`
/// to Z → force_push_with_lease SUCCEEDS (Pushed { set_upstream: false }) and
/// origin's `main` moves to Z.
#[test]
fn lease_succeeds_and_moves_the_ref() {
    require_git!();
    let f = init_origin_and_clone();
    let x = origin_main(&f);

    let z = rewrite_head(&f.work, "z");
    assert_ne!(z, x, "the rewrite must differ from the original tip");

    let res = force_push_with_lease(&f.work).expect("lease should hold");
    match res {
        PushResult::Pushed {
            remote,
            branch,
            set_upstream,
        } => {
            assert_eq!(remote, "origin");
            assert_eq!(branch, "main");
            assert!(!set_upstream, "force-push never sets upstream");
        }
        other => panic!("expected Pushed, got {other:?}"),
    }

    // Oracle: origin's main now equals the rewritten tip Z.
    assert_eq!(origin_main(&f), z, "origin main must move to the rewritten tip");
}

// ------------------------------------------------------- §8.C up-to-date

/// C. Baseline (remote-tracking) already equals the local tip → UpToDate; no
/// network mutation, origin unchanged.
#[test]
fn up_to_date_when_baseline_equals_local_tip() {
    require_git!();
    let f = init_origin_and_clone();
    let x = origin_main(&f);

    let res = force_push_with_lease(&f.work).expect("up-to-date is not an error");
    match res {
        PushResult::UpToDate { remote, branch } => {
            assert_eq!(remote, "origin");
            assert_eq!(branch, "main");
        }
        other => panic!("expected UpToDate, got {other:?}"),
    }
    assert_eq!(origin_main(&f), x, "origin main must be unchanged");
}

// --------------------------------------------------------- §8.D no upstream

/// D. A local branch with no upstream → NoUpstream (use a normal push).
#[test]
fn no_upstream_is_rejected() {
    require_git!();
    let f = init_origin_and_clone();

    git(&f.work, &["checkout", "-b", "nolease"]);
    let err = force_push_with_lease(&f.work).expect_err("no upstream must error");
    assert!(
        matches!(err, AppError::NoUpstream(_)),
        "expected NoUpstream, got {err:?}"
    );
}

// --------------------------------------------------------- §8.E no baseline

/// E. No remote-tracking baseline (never fetched) → PushRejected ("Fetch
/// first"); origin unchanged.
#[test]
fn no_baseline_refuses_with_fetch_hint() {
    require_git!();
    let f = init_origin_and_clone();
    let x = origin_main(&f);

    // Simulate a never-fetched branch: drop the remote-tracking ref.
    git(&f.work, &["update-ref", "-d", "refs/remotes/origin/main"]);

    let _z = rewrite_head(&f.work, "z");
    let err = force_push_with_lease(&f.work).expect_err("no baseline must refuse");
    match err {
        AppError::PushRejected(m) => {
            assert!(
                m.to_lowercase().contains("fetch first"),
                "message must mention 'Fetch first': {m}"
            );
        }
        other => panic!("expected PushRejected, got {other:?}"),
    }
    assert_eq!(origin_main(&f), x, "origin main must be unchanged");
}
