//! M6 CLI-oracle remote tests (contract §6.1–§6.3).
//!
//! Every "remote" is a LOCAL BARE repo (`git init --bare`) referenced by a
//! plain path — the local transport needs NO network and NO credentials.
//! Fixture pattern (contract §6): bare + a `seed` clone that publishes
//! commits + a `work` clone that Bonsai operates on (clone configures
//! `origin` and upstream tracking). All scratch repos live under
//! `D:\Temp\bonsai-scratch`.
//!
//! Honest coverage note (contract §6 preamble): the local transport never
//! invokes the credentials callback and never produces Net/Http/Ssh errors —
//! the retry guard and error mapping are covered structurally by the unit
//! tests in `src/git/remote.rs`; the real credential-helper/agent path is
//! covered only by the USER CHECKPOINT network round-trip.
//!
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::{Path, PathBuf};

use bonsai_core::error::AppError;
use bonsai_core::git::remote::{fetch_all, pull_ff, push_current, PullResult, PushResult};
use common::{commit_fixed, git, git_ok};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

/// bare "origin" + seed clone (publisher) + work clone (repo under test).
struct Fixture {
    _dir: tempfile::TempDir,
    bare: PathBuf,
    seed: PathBuf,
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
}

/// Clones `bare` into `<root>/<name>` with repo-local identity configured.
fn clone_from_bare(root: &Path, bare: &Path, name: &str) -> PathBuf {
    git(root, &["clone", &path_str(bare), name]);
    let dir = root.join(name);
    configure_identity(&dir);
    dir
}

/// Base fixture: bare origin with `main` = one commit (hello.txt +
/// shared.txt), seed + work clones both at that tip with upstream tracking.
fn setup() -> Fixture {
    let dir = common::scratch_dir();
    let root = dir.path().to_path_buf();

    git(&root, &["init", "--bare", "-b", "main", "origin.git"]);
    let bare = root.join("origin.git");

    let seed = clone_from_bare(&root, &bare, "seed");
    git(&seed, &["checkout", "-B", "main"]);
    std::fs::write(seed.join("hello.txt"), "hello\n").expect("write hello.txt");
    std::fs::write(seed.join("shared.txt"), "base\n").expect("write shared.txt");
    git(&seed, &["add", "-A"]);
    commit_fixed(&seed, "initial");
    git(&seed, &["push", "-u", "origin", "main"]);

    let work = clone_from_bare(&root, &bare, "work");

    Fixture {
        _dir: dir,
        bare,
        seed,
        work,
        root,
    }
}

/// Publishes a commit through the seed clone: write `file`, commit, push.
fn seed_publish(f: &Fixture, file: &str, content: &str, msg: &str) {
    std::fs::write(f.seed.join(file), content).expect("write seed file");
    git(&f.seed, &["add", "-A"]);
    commit_fixed(&f.seed, msg);
    git(&f.seed, &["push", "origin", "main"]);
}

/// Commits `file` with `content` in `repo` (local commit, no push).
fn local_commit(repo: &Path, file: &str, content: &str, msg: &str) {
    std::fs::write(repo.join(file), content).expect("write file");
    git(repo, &["add", "-A"]);
    commit_fixed(repo, msg);
}

fn rev_parse(repo: &Path, rev: &str) -> String {
    git(repo, &["rev-parse", rev])
}

// ---------------------------------------------------------- §6.1 fetch_all

/// §6.1.1: fetch after the seed publishes → remote-tracking ref equals the
/// bare tip; counters reflect the transfer. Twin oracle: `git fetch` in a
/// twin clone yields the same remote-tracking oid.
#[test]
fn fetch_updates_remote_tracking_ref() {
    require_git!();
    let f = setup();
    let twin = clone_from_bare(&f.root, &f.bare, "twin");

    seed_publish(&f, "second.txt", "two\n", "second");
    let bare_tip = rev_parse(&f.bare, "main");

    let res = fetch_all(&f.work).expect("fetch_all");
    assert_eq!(res.remotes.len(), 1);
    assert_eq!(res.remotes[0].remote, "origin");
    assert!(res.remotes[0].updated_refs >= 1, "expected updated refs");
    assert!(res.remotes[0].received_objects > 0, "expected received objects");
    assert_eq!(rev_parse(&f.work, "refs/remotes/origin/main"), bare_tip);

    git(&twin, &["fetch"]);
    assert_eq!(
        rev_parse(&twin, "refs/remotes/origin/main"),
        rev_parse(&f.work, "refs/remotes/origin/main"),
        "git2 fetch and CLI fetch must land the same remote-tracking oid"
    );
}

/// §6.1.2: a second fetch with nothing new reports zero updated refs.
#[test]
fn fetch_with_nothing_new_updates_no_refs() {
    require_git!();
    let f = setup();
    seed_publish(&f, "second.txt", "two\n", "second");
    fetch_all(&f.work).expect("first fetch");

    let res = fetch_all(&f.work).expect("second fetch");
    assert_eq!(res.remotes[0].updated_refs, 0);
}

/// §6.1.3: a repo with no remotes → NoRemote.
#[test]
fn fetch_without_remotes_is_no_remote() {
    require_git!();
    let dir = common::init_repo();
    local_commit(dir.path(), "a.txt", "a\n", "base");

    let err = fetch_all(dir.path()).expect_err("fetch with no remotes");
    assert!(matches!(err, AppError::NoRemote(_)), "got {err:?}");
}

/// §6.1.4: two remotes → both fetched, in `repo.remotes()` order.
#[test]
fn fetch_covers_all_remotes_in_order() {
    require_git!();
    let f = setup();
    git(&f.root, &["init", "--bare", "-b", "main", "backup.git"]);
    let backup = f.root.join("backup.git");
    git(&f.work, &["remote", "add", "backup", &path_str(&backup)]);

    let res = fetch_all(&f.work).expect("fetch_all");
    let names: Vec<&str> = res.remotes.iter().map(|r| r.remote.as_str()).collect();

    // Oracle: libgit2's own remote list order.
    let repo = git2::Repository::open(&f.work).expect("open work");
    let expected: Vec<String> = repo
        .remotes()
        .expect("remotes")
        .iter()
        .filter_map(Result::ok)
        .flatten()
        .map(str::to_string)
        .collect();
    assert_eq!(names, expected.iter().map(String::as_str).collect::<Vec<_>>());
    assert_eq!(expected.len(), 2);
}

/// §6.1.5: fetch works with detached HEAD (no branch involved).
#[test]
fn fetch_works_with_detached_head() {
    require_git!();
    let f = setup();
    git(&f.work, &["checkout", "--detach"]);
    seed_publish(&f, "second.txt", "two\n", "second");

    let res = fetch_all(&f.work).expect("fetch with detached HEAD");
    assert!(res.remotes[0].updated_refs >= 1);
}

// ------------------------------------------------------------ §6.2 pull_ff

/// §6.2.1: fast-forward pull moves branch ref AND worktree to the bare tip;
/// twin oracle `git pull --ff-only` agrees on ref + worktree.
#[test]
fn pull_fast_forwards_ref_and_worktree() {
    require_git!();
    let f = setup();
    let twin = clone_from_bare(&f.root, &f.bare, "twin");
    let old_tip = rev_parse(&f.work, "main");

    seed_publish(&f, "new-file.txt", "new\n", "add new-file");
    let bare_tip = rev_parse(&f.bare, "main");

    let res = pull_ff(&f.work).expect("pull_ff");
    match res {
        PullResult::FastForwarded { branch, from, to } => {
            assert_eq!(branch, "main");
            assert_eq!(from, old_tip);
            assert_eq!(to, bare_tip);
        }
        other => panic!("expected FastForwarded, got {other:?}"),
    }
    assert_eq!(rev_parse(&f.work, "main"), bare_tip);
    assert!(f.work.join("new-file.txt").exists(), "worktree not updated");
    assert_eq!(git(&f.work, &["status", "--porcelain"]), "");

    git(&twin, &["pull", "--ff-only"]);
    assert_eq!(rev_parse(&twin, "main"), rev_parse(&f.work, "main"));
    assert_eq!(
        std::fs::read_to_string(twin.join("new-file.txt")).expect("twin file"),
        std::fs::read_to_string(f.work.join("new-file.txt")).expect("work file"),
    );
}

/// §6.2.2: an immediate second pull is UpToDate.
#[test]
fn pull_twice_is_up_to_date() {
    require_git!();
    let f = setup();
    seed_publish(&f, "new-file.txt", "new\n", "add new-file");
    pull_ff(&f.work).expect("first pull");

    let res = pull_ff(&f.work).expect("second pull");
    assert!(matches!(res, PullResult::UpToDate), "got {res:?}");
}

/// §6.2.3: ahead-only (local commit, bare unchanged) → UpToDate, ref
/// unchanged.
#[test]
fn pull_when_ahead_only_is_up_to_date() {
    require_git!();
    let f = setup();
    local_commit(&f.work, "local.txt", "local\n", "local work");
    let tip = rev_parse(&f.work, "main");

    let res = pull_ff(&f.work).expect("pull");
    assert!(matches!(res, PullResult::UpToDate), "got {res:?}");
    assert_eq!(rev_parse(&f.work, "main"), tip, "branch ref must not move");
}

/// §6.2.4: diverged → WouldNotFastForward{1,1}; branch ref, worktree and
/// porcelain unchanged — but the remote-tracking ref DID move (fetch landed).
#[test]
fn pull_diverged_reports_and_changes_nothing() {
    require_git!();
    let f = setup();
    local_commit(&f.work, "local.txt", "local\n", "local work");
    seed_publish(&f, "upstream.txt", "up\n", "upstream work");

    let tip_before = rev_parse(&f.work, "main");
    let porcelain_before = git(&f.work, &["status", "--porcelain"]);
    let bare_tip = rev_parse(&f.bare, "main");

    let res = pull_ff(&f.work).expect("pull");
    match res {
        PullResult::WouldNotFastForward { branch, ahead, behind } => {
            assert_eq!(branch, "main");
            assert_eq!((ahead, behind), (1, 1));
        }
        other => panic!("expected WouldNotFastForward, got {other:?}"),
    }
    assert_eq!(rev_parse(&f.work, "main"), tip_before, "ref must not move");
    assert!(!f.work.join("upstream.txt").exists(), "worktree must not change");
    assert_eq!(git(&f.work, &["status", "--porcelain"]), porcelain_before);
    assert_eq!(
        rev_parse(&f.work, "refs/remotes/origin/main"),
        bare_tip,
        "remote-tracking ref must have moved (the fetch happened)"
    );
}

/// §6.2.5: dirty worktree conflicting with the update → CheckoutConflict;
/// ref + file content untouched; remote-tracking ref moved.
#[test]
fn pull_dirty_conflict_changes_nothing() {
    require_git!();
    let f = setup();
    seed_publish(&f, "shared.txt", "upstream change\n", "touch shared");
    std::fs::write(f.work.join("shared.txt"), "local uncommitted\n").expect("dirty write");
    let tip_before = rev_parse(&f.work, "main");
    let bare_tip = rev_parse(&f.bare, "main");

    let err = pull_ff(&f.work).expect_err("dirty pull must conflict");
    assert!(matches!(err, AppError::CheckoutConflict(_)), "got {err:?}");
    assert_eq!(rev_parse(&f.work, "main"), tip_before, "ref must not move");
    assert_eq!(
        std::fs::read_to_string(f.work.join("shared.txt")).expect("read shared"),
        "local uncommitted\n",
        "local modification must survive"
    );
    assert_eq!(
        rev_parse(&f.work, "refs/remotes/origin/main"),
        bare_tip,
        "remote-tracking ref must have moved (the fetch happened)"
    );
}

/// §6.2.6: no upstream configured → NoUpstream.
#[test]
fn pull_without_upstream_is_no_upstream() {
    require_git!();
    let f = setup();
    git(&f.work, &["branch", "--unset-upstream"]);

    let err = pull_ff(&f.work).expect_err("pull without upstream");
    assert!(matches!(err, AppError::NoUpstream(_)), "got {err:?}");
}

/// §6.2.7: detached HEAD and unborn HEAD are guarded with kind `git`.
#[test]
fn pull_detached_and_unborn_are_guarded() {
    require_git!();
    let f = setup();
    git(&f.work, &["checkout", "--detach"]);
    let err = pull_ff(&f.work).expect_err("detached pull");
    match err {
        AppError::Git(m) => assert_eq!(m, "cannot pull: HEAD is detached"),
        other => panic!("expected Git, got {other:?}"),
    }

    let unborn = common::init_repo();
    git(unborn.path(), &["remote", "add", "origin", &path_str(&f.bare)]);
    let err = pull_ff(unborn.path()).expect_err("unborn pull");
    match err {
        AppError::Git(m) => assert_eq!(m, "cannot pull: the repository has no commits yet"),
        other => panic!("expected Git, got {other:?}"),
    }
}

// ------------------------------------------------------- §6.3 push_current

/// §6.3.1: push a local commit → bare ref equals work tip; remote-tracking
/// ref updated. Twin oracle: an identical CLI push from a twin fixture
/// produces the identical bare tip (fixed dates → deterministic oids).
#[test]
fn push_updates_bare_and_tracking_ref() {
    require_git!();
    let f = setup();
    local_commit(&f.work, "feature.txt", "feat\n", "feature work");
    let tip = rev_parse(&f.work, "main");

    let res = push_current(&f.work).expect("push");
    match res {
        PushResult::Pushed { remote, branch, set_upstream } => {
            assert_eq!(remote, "origin");
            assert_eq!(branch, "main");
            assert!(!set_upstream, "upstream was already configured");
        }
        other => panic!("expected Pushed, got {other:?}"),
    }
    assert_eq!(rev_parse(&f.bare, "main"), tip, "bare must have the commit");
    assert_eq!(
        rev_parse(&f.work, "refs/remotes/origin/main"),
        tip,
        "remote-tracking ref must be updated by the push"
    );

    // Twin oracle: same script, CLI push — identical bare tip.
    let t = setup();
    local_commit(&t.work, "feature.txt", "feat\n", "feature work");
    git(&t.work, &["push"]);
    assert_eq!(rev_parse(&t.bare, "main"), tip, "CLI twin must agree");
}

/// §6.3.2: an immediate second push is UpToDate (local short-circuit).
#[test]
fn push_twice_is_up_to_date() {
    require_git!();
    let f = setup();
    local_commit(&f.work, "feature.txt", "feat\n", "feature work");
    push_current(&f.work).expect("first push");

    let res = push_current(&f.work).expect("second push");
    match res {
        PushResult::UpToDate { remote, branch } => {
            assert_eq!(remote, "origin");
            assert_eq!(branch, "main");
        }
        other => panic!("expected UpToDate, got {other:?}"),
    }
}

/// §6.3.3: new branch without upstream → pushed to origin/<branch> AND
/// upstream configured (git config oracle).
#[test]
fn push_without_upstream_sets_upstream() {
    require_git!();
    let f = setup();
    git(&f.work, &["checkout", "-b", "topic"]);
    local_commit(&f.work, "topic.txt", "t\n", "topic work");
    let tip = rev_parse(&f.work, "topic");

    let res = push_current(&f.work).expect("push");
    match res {
        PushResult::Pushed { remote, branch, set_upstream } => {
            assert_eq!(remote, "origin");
            assert_eq!(branch, "topic");
            assert!(set_upstream, "must report the upstream was set");
        }
        other => panic!("expected Pushed, got {other:?}"),
    }
    assert_eq!(rev_parse(&f.bare, "refs/heads/topic"), tip);
    assert_eq!(git(&f.work, &["config", "branch.topic.remote"]), "origin");
    assert_eq!(git(&f.work, &["config", "branch.topic.merge"]), "refs/heads/topic");
    assert_eq!(rev_parse(&f.work, "refs/remotes/origin/topic"), tip);
}

/// §6.3.4: no upstream and no origin remote → NoRemote.
#[test]
fn push_without_upstream_or_origin_is_no_remote() {
    require_git!();
    let f = setup();
    // `git remote remove` also deletes branch.<name>.remote/.merge config.
    git(&f.work, &["remote", "remove", "origin"]);
    git(&f.work, &["checkout", "-b", "topic"]);
    local_commit(&f.work, "topic.txt", "t\n", "topic work");

    let err = push_current(&f.work).expect_err("push with no origin");
    assert!(matches!(err, AppError::NoRemote(_)), "got {err:?}");
}

/// §6.3.5: non-fast-forward push → PushRejected, bare ref UNCHANGED; the
/// CLI twin push also fails non-zero.
#[test]
fn push_non_fast_forward_is_rejected_and_bare_unchanged() {
    require_git!();
    let f = setup();
    seed_publish(&f, "upstream.txt", "up\n", "upstream work");
    local_commit(&f.work, "local.txt", "local\n", "local work");
    let bare_before = rev_parse(&f.bare, "main");

    let err = push_current(&f.work).expect_err("non-ff push must fail");
    match err {
        AppError::PushRejected(m) => assert!(!m.is_empty(), "message must not be empty"),
        other => panic!("expected PushRejected, got {other:?}"),
    }
    assert_eq!(rev_parse(&f.bare, "main"), bare_before, "bare must be unchanged");

    assert!(
        !git_ok(&f.work, &["push"]),
        "CLI twin push must also fail non-zero"
    );
}

/// §6.3.6: detached HEAD and unborn HEAD are guarded with kind `git`.
#[test]
fn push_detached_and_unborn_are_guarded() {
    require_git!();
    let f = setup();
    git(&f.work, &["checkout", "--detach"]);
    let err = push_current(&f.work).expect_err("detached push");
    match err {
        AppError::Git(m) => assert_eq!(m, "cannot push: HEAD is detached"),
        other => panic!("expected Git, got {other:?}"),
    }

    let unborn = common::init_repo();
    git(unborn.path(), &["remote", "add", "origin", &path_str(&f.bare)]);
    let err = push_current(unborn.path()).expect_err("unborn push");
    match err {
        AppError::Git(m) => assert_eq!(m, "cannot push: the repository has no commits yet"),
        other => panic!("expected Git, got {other:?}"),
    }
}
