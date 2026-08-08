//! P37 + P59b CLI-oracle force-push-with-lease tests (contract §8 / P59b §B3).
//!
//! Every "remote" is a LOCAL BARE repo (`git init --bare`) referenced by a
//! plain path — the local transport needs NO network and NO credentials, so
//! git's atomic `git push --force-with-lease` (P59b, via `SpawnGitExec`) runs
//! hermetically (no Git Credential Manager prompts; `credential.helper` is reset
//! empty per repo). All scratch repos live under `D:\Temp\bonsai-scratch`.
//!
//! Cross-check against the real `git` CLI: after each operation the origin's
//! `main` tip is read with `git rev-parse` and compared to the expected oid.
//!
//! P59b moved the push mechanism from the old client-side connect_auth+ls-remote
//! compare-and-swap to git's OWN atomic `--force-with-lease` (closing P37's
//! TOCTOU window). Tests that must NOT reach the push (up-to-date C, no-upstream
//! D, no-baseline E, detached H, unborn I) inject a `PanicExec` to prove no git
//! is spawned.
//!
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::{Path, PathBuf};

use bonsai_core::error::AppError;
use bonsai_core::git::exec::{GitExec, GitOutput, SpawnGitExec};
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

/// A `GitExec` that must NEVER be invoked. Used by the up-to-date / pre-check
/// tests to prove `force_push_with_lease` short-circuits BEFORE spawning git.
struct PanicExec;
impl GitExec for PanicExec {
    fn exec(
        &self,
        _args: &[&str],
        _cwd: &Path,
        _stdin: Option<&[u8]>,
        _env: &[(&str, &str)],
    ) -> Result<GitOutput, AppError> {
        panic!("force_push_with_lease must NOT spawn git on this path");
    }
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

    let err = force_push_with_lease(&f.work, &SpawnGitExec, false).expect_err("lease must refuse");
    match err {
        AppError::PushRejected(m) => {
            let low = m.to_lowercase();
            // Contextual hint (lease_moved_msg) prepended by the caller.
            assert!(low.contains("moved"), "message must mention 'moved': {m}");
            assert!(low.contains("fetch"), "message must mention 'fetch': {m}");
            // The refusal is git's OWN atomic --force-with-lease check (its
            // stderr flows through) — NOT our old client-side compare. This is
            // the P37 TOCTOU-closing guarantee.
            assert!(
                low.contains("stale info") || low.contains("rejected"),
                "message must carry git's lease-refusal stderr: {m}"
            );
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

    let res = force_push_with_lease(&f.work, &SpawnGitExec, false).expect("lease should hold");
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

    // PanicExec: up-to-date must short-circuit in git2 with NO git spawned.
    let res = force_push_with_lease(&f.work, &PanicExec, false).expect("up-to-date is not an error");
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
    // PanicExec: the NoUpstream pre-check fires before any git push.
    let err = force_push_with_lease(&f.work, &PanicExec, false).expect_err("no upstream must error");
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
    // PanicExec: the no-baseline pre-check fires before any git push.
    let err = force_push_with_lease(&f.work, &PanicExec, false).expect_err("no baseline must refuse");
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

// ---------------------------------------- §8.F nested branch name lease (extra)

/// F. A nested/slashed branch name (`feature/x`) with a configured upstream:
/// the lease must resolve `refs/remotes/origin/feature/x` correctly and the
/// force-push must succeed, moving `refs/heads/feature/x` on origin. Guards the
/// `strip_prefix("refs/heads/")` + tracking-ref interpolation for slashed refs.
#[test]
fn lease_succeeds_on_nested_branch_name() {
    require_git!();
    let f = init_origin_and_clone();

    // Create feature/x tracking origin/feature/x (initial content = X).
    git(&f.work, &["checkout", "-b", "feature/x"]);
    std::fs::write(f.work.join("feat.txt"), "x\n").expect("write feat.txt");
    git(&f.work, &["add", "-A"]);
    commit_fixed(&f.work, "feature base");
    git(&f.work, &["push", "-u", "origin", "feature/x"]);

    let nested_tip = |f: &Fixture| git(&f.bare, &["rev-parse", "refs/heads/feature/x"]);
    let x = nested_tip(&f);

    // Rewrite feature/x -> Z (amend), no third-party push.
    let z = rewrite_head(&f.work, "z-nested");
    assert_ne!(z, x, "the rewrite must differ from the original tip");

    let res =
        force_push_with_lease(&f.work, &SpawnGitExec, false).expect("lease should hold on nested branch");
    match res {
        PushResult::Pushed {
            remote,
            branch,
            set_upstream,
        } => {
            assert_eq!(remote, "origin");
            assert_eq!(branch, "feature/x");
            assert!(!set_upstream);
        }
        other => panic!("expected Pushed, got {other:?}"),
    }
    assert_eq!(
        nested_tip(&f),
        z,
        "origin feature/x must move to the rewritten tip"
    );
}

// ---------------------- §8.G force-push drops a commit the remote had (extra)

/// G. A real non-fast-forward history rewrite that DROPS a commit the remote
/// already had. origin/main = Y (two commits X<-Y). Locally reset --hard to X
/// and build a fresh commit Z on top of X (Y is no longer an ancestor). Lease
/// holds (remote-tracking == live == Y), so the force-push must land Z on
/// origin and Y must no longer be reachable from origin/main.
#[test]
fn force_push_drops_a_remote_commit() {
    require_git!();
    let f = init_origin_and_clone();
    let x = origin_main(&f); // first commit "initial"

    // Add a second commit Y and push it so origin/main == Y (tracked).
    std::fs::write(f.work.join("second.txt"), "second\n").expect("write second.txt");
    git(&f.work, &["add", "-A"]);
    commit_fixed(&f.work, "second");
    git(&f.work, &["push", "origin", "main"]);
    let y = origin_main(&f);
    assert_ne!(x, y);

    // Drop Y locally: reset --hard to X, then build a divergent commit Z on X.
    git(&f.work, &["reset", "--hard", &x]);
    std::fs::write(f.work.join("third.txt"), "third\n").expect("write third.txt");
    git(&f.work, &["add", "-A"]);
    commit_fixed(&f.work, "replacement");
    let z = head_oid(&f.work);
    assert_ne!(z, y);

    let res = force_push_with_lease(&f.work, &SpawnGitExec, false).expect("lease should hold");
    assert!(matches!(res, PushResult::Pushed { .. }), "got {res:?}");

    // Oracle: origin/main == Z, its parent == X, and Y is no longer reachable.
    assert_eq!(origin_main(&f), z, "origin main must be the replacement tip Z");
    let z_parent = git(&f.bare, &["rev-parse", "refs/heads/main^"]);
    assert_eq!(z_parent, x, "Z's parent must be X (Y was dropped)");
    let reachable = git_ancestor(&f.bare, &y, "refs/heads/main");
    assert!(!reachable, "dropped commit Y must not be an ancestor of origin/main");
}

// ------------------------------------------ §8.H detached HEAD error (extra)

/// H. Detached HEAD (checkout a raw oid) has no branch to lease → `Git` error.
#[test]
fn detached_head_is_rejected() {
    require_git!();
    let f = init_origin_and_clone();
    let tip = head_oid(&f.work);
    git(&f.work, &["checkout", &tip]); // detach

    // PanicExec: the detached-HEAD guard fires before any git push.
    let err = force_push_with_lease(&f.work, &PanicExec, false).expect_err("detached HEAD must error");
    match err {
        AppError::Git(m) => assert!(
            m.to_lowercase().contains("detached"),
            "message must mention 'detached': {m}"
        ),
        other => panic!("expected Git, got {other:?}"),
    }
    // Nothing pushed: origin unchanged.
    assert_eq!(origin_main(&f), tip, "origin main must be unchanged");
}

// ------------------------------------------------ §8.I unborn HEAD (extra)

/// I. A fresh repo with no commits (unborn HEAD) → `Git` error, before any
/// network work. No remote required — the unborn guard fires first.
#[test]
fn unborn_head_is_rejected() {
    require_git!();
    let dir = common::scratch_dir();
    let repo = dir.path().join("empty");
    std::fs::create_dir_all(&repo).expect("mkdir empty");
    git(&repo, &["init", "-b", "main"]);
    configure_identity(&repo);

    // PanicExec: the unborn-HEAD guard fires before any git push.
    let err = force_push_with_lease(&repo, &PanicExec, false).expect_err("unborn HEAD must error");
    match err {
        AppError::Git(m) => assert!(
            m.to_lowercase().contains("no commits"),
            "message must mention 'no commits': {m}"
        ),
        other => panic!("expected Git, got {other:?}"),
    }
}

// ---------------------------------- §A6 pre-push hook oracle (P59a-2)

/// A failing `pre-push` hook ABORTS the force-push with `HookRejected`, and the
/// remote ref stays UNCHANGED. The hook echoes the stdin ref line — which for a
/// force-push carries the LEASE baseline as the remote-oid — proving the stdin
/// synthesis. Requires Git ≥ 2.36 (`git hook run`).
#[test]
fn pre_push_hook_blocks_force_push() {
    require_git!();
    if !common::git_version_at_least(2, 36) {
        eprintln!("skipping: git < 2.36 (no `git hook run`)");
        return;
    }
    let f = init_origin_and_clone();
    let x = origin_main(&f);

    common::write_pre_push_hook(&f.work, "read line\necho \"pre-push saw: $line\" >&2\nexit 1\n");
    let z = rewrite_head(&f.work, "z");
    assert_ne!(z, x);

    let err =
        force_push_with_lease(&f.work, &SpawnGitExec, false).expect_err("pre-push must block");
    match err {
        AppError::HookRejected(m) => {
            assert!(m.contains("pre-push hook failed:"), "prefix: {m}");
            assert!(m.contains("refs/heads/main"), "stdin ref surfaced: {m}");
            // The remote-oid field is the lease baseline X (not 40 zeros).
            assert!(m.contains(&x), "stdin remote-oid must be the lease baseline: {m}");
        }
        other => panic!("expected HookRejected, got {other:?}"),
    }
    // Oracle: the force-push never happened — origin unchanged.
    assert_eq!(origin_main(&f), x, "origin main must be unchanged after a blocked pre-push");
}

/// P59a-2: `skip_hooks = true` (≡ --no-verify) bypasses a failing pre-push — the
/// force-push proceeds and origin moves to the rewritten tip.
#[test]
fn pre_push_hook_skipped_allows_force_push() {
    require_git!();
    if !common::git_version_at_least(2, 36) {
        eprintln!("skipping: git < 2.36");
        return;
    }
    let f = init_origin_and_clone();
    common::write_pre_push_hook(&f.work, "exit 1\n");
    let z = rewrite_head(&f.work, "z");

    let res = force_push_with_lease(&f.work, &SpawnGitExec, true)
        .expect("skip_hooks bypasses the failing pre-push");
    assert!(matches!(res, PushResult::Pushed { .. }), "got {res:?}");
    assert_eq!(origin_main(&f), z, "origin main must move when the hook is skipped");
}

/// A PASSING pre-push (exit 0) allows the force-push through git's atomic lease.
#[test]
fn pre_push_hook_pass_allows_force_push() {
    require_git!();
    if !common::git_version_at_least(2, 36) {
        eprintln!("skipping: git < 2.36");
        return;
    }
    let f = init_origin_and_clone();
    common::write_pre_push_hook(&f.work, "exit 0\n");
    let z = rewrite_head(&f.work, "z");

    let res =
        force_push_with_lease(&f.work, &SpawnGitExec, false).expect("passing pre-push allows push");
    assert!(matches!(res, PushResult::Pushed { .. }), "got {res:?}");
    assert_eq!(origin_main(&f), z);
}

/// True if `ancestor` is reachable from `ref_name` in `repo` (git merge-base
/// --is-ancestor exit 0). Used to prove a dropped commit is gone.
fn git_ancestor(repo: &Path, ancestor: &str, ref_name: &str) -> bool {
    common::git_ok(repo, &["merge-base", "--is-ancestor", ancestor, ref_name])
}
