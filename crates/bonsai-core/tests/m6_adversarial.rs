//! M6 adversarial probes (tester gap-probing, beyond contract §6.1–§6.3).
//!
//! Same LOCAL-bare-remote machinery as `remote_cli.rs` — every "remote" is a
//! `git init --bare` under `D:\Temp\bonsai-scratch`, reached by plain path
//! (no network, no credentials, ever). These tests PIN observed behavior for
//! risky uncovered cases; where our behavior diverges from the plain `git`
//! CLI the divergence is asserted explicitly and flagged in comments so it is
//! a conscious decision, not an accident.

mod common;

use std::path::{Path, PathBuf};

use bonsai_core::error::AppError;
use bonsai_core::git::exec::SpawnGitExec;
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
/// Mirrors the `remote_cli.rs` fixture (integration binaries cannot share
/// non-`common` modules).
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

fn clone_from_bare(root: &Path, bare: &Path, name: &str) -> PathBuf {
    git(root, &["clone", &path_str(bare), name]);
    let dir = root.join(name);
    configure_identity(&dir);
    dir
}

fn setup() -> Fixture {
    let dir = common::scratch_dir();
    let root = dir.path().to_path_buf();

    git(&root, &["init", "--bare", "-b", "main", "origin.git"]);
    let bare = root.join("origin.git");

    let seed = clone_from_bare(&root, &bare, "seed");
    git(&seed, &["checkout", "-B", "main"]);
    std::fs::write(seed.join("hello.txt"), "hello\n").expect("write hello.txt");
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

fn seed_publish(f: &Fixture, file: &str, content: &str, msg: &str) {
    std::fs::write(f.seed.join(file), content).expect("write seed file");
    git(&f.seed, &["add", "-A"]);
    commit_fixed(&f.seed, msg);
    git(&f.seed, &["push", "origin", "main"]);
}

fn local_commit(repo: &Path, file: &str, content: &str, msg: &str) {
    std::fs::write(repo.join(file), content).expect("write file");
    git(repo, &["add", "-A"]);
    commit_fixed(repo, msg);
}

fn rev_parse(repo: &Path, rev: &str) -> String {
    git(repo, &["rev-parse", rev])
}

// ---------------------------------------------------------------------------
// Probe 1: pull after the upstream was FORCE-REWRITTEN to a diverged history.
// This is the data-loss scenario ff-only exists to prevent: a rival clone
// rewinds the bare's main and force-pushes a different line. Our pull must
// report WouldNotFastForward and change NOTHING locally — losing the local
// commit by "catching up" to the rewrite would be catastrophic. Twin oracle:
// `git pull --ff-only` also refuses and moves nothing.
#[test]
fn pull_after_upstream_force_rewrite_changes_nothing() {
    require_git!();
    let f = setup();

    // Work pulls the published commit X and is clean at X.
    seed_publish(&f, "x.txt", "x\n", "commit X");
    pull_ff(&f.work).expect("pull to X");
    let x_tip = rev_parse(&f.work, "main");

    // Twin in the identical pre-rewrite state.
    let twin = clone_from_bare(&f.root, &f.bare, "twin");

    // Rival force-rewrites the bare: rewinds to the initial commit and
    // publishes a DIVERGED commit Y (scratch bare only — never a real repo).
    let rival = clone_from_bare(&f.root, &f.bare, "rival");
    git(&rival, &["reset", "--hard", "HEAD~1"]);
    std::fs::write(rival.join("y.txt"), "y\n").expect("write y.txt");
    git(&rival, &["add", "-A"]);
    commit_fixed(&rival, "commit Y (rewrite)");
    git(&rival, &["push", "--force", "origin", "main"]);
    let y_tip = rev_parse(&f.bare, "main");
    assert_ne!(y_tip, x_tip, "fixture: bare must have been rewritten");

    let porcelain_before = git(&f.work, &["status", "--porcelain"]);
    let res = pull_ff(&f.work).expect("pull after rewrite must not error");
    match res {
        PullResult::WouldNotFastForward { branch, ahead, behind } => {
            assert_eq!(branch, "main");
            assert_eq!((ahead, behind), (1, 1), "X vs rewritten Y diverge 1/1");
        }
        other => panic!("expected WouldNotFastForward, got {other:?}"),
    }
    // NOTHING moved locally: ref still X, X's file intact, Y's file absent,
    // porcelain unchanged. Only the remote-tracking ref followed the rewrite.
    assert_eq!(rev_parse(&f.work, "main"), x_tip, "local ref must not move");
    assert!(f.work.join("x.txt").exists(), "X's worktree file must survive");
    assert!(!f.work.join("y.txt").exists(), "Y must not appear in worktree");
    assert_eq!(git(&f.work, &["status", "--porcelain"]), porcelain_before);
    assert_eq!(rev_parse(&f.work, "refs/remotes/origin/main"), y_tip);

    // Twin oracle: ff-only pull fails non-zero and moves nothing either.
    assert!(
        !git_ok(&twin, &["pull", "--ff-only"]),
        "CLI twin `git pull --ff-only` must refuse the rewritten upstream"
    );
    assert_eq!(rev_parse(&twin, "main"), x_tip, "twin ref must not move");
}

// ---------------------------------------------------------------------------
// Probe 2: fetch with a BROKEN remote (URL → nonexistent path) fails fast and
// leaves the valid remote untouched. libgit2 lists remotes alphabetically, so
// "aaa-broken" is attempted BEFORE "origin" — the error must abort the whole
// fetch (contract §9 fail-fast) and origin's remote-tracking ref must NOT
// have been updated even though the seed published a new commit.
//
// OBSERVED (pinned): a nonexistent `D:\...` path is parsed by libgit2 as an
// scp-style URL (host "D"), so the failure surfaces as class Net → kind
// `networkError` NAMING THE REMOTE per §2.3: "network error talking to
// 'aaa-broken': failed to resolve address for D: No such host is known."
// (If libgit2 ever resolves it as a local path instead, the error would be
// class Os → kind `git` with the raw path message — also tolerated below.)
#[test]
fn fetch_fail_fast_broken_remote_leaves_valid_remote_untouched() {
    require_git!();
    let f = setup();

    let missing = f.root.join("no-such-remote.git");
    git(&f.work, &["remote", "add", "aaa-broken", &path_str(&missing)]);

    // Ordering oracle: libgit2 must list the broken remote first.
    let repo = git2::Repository::open(&f.work).expect("open work");
    let order: Vec<String> = repo
        .remotes()
        .expect("remotes")
        .iter()
        .filter_map(Result::ok)
        .flatten()
        .map(str::to_string)
        .collect();
    assert_eq!(order, ["aaa-broken", "origin"], "ordering assumption broken");
    drop(repo);

    seed_publish(&f, "new.txt", "new\n", "published after clone");
    let tracking_before = rev_parse(&f.work, "refs/remotes/origin/main");
    let bare_tip = rev_parse(&f.bare, "main");
    assert_ne!(tracking_before, bare_tip, "fixture: origin must have news");

    let err = fetch_all(&f.work).expect_err("broken remote must fail the fetch");
    match err {
        // OBSERVED path: scp-style misparse → class Net → networkError,
        // message names the failing remote (§2.3 context interpolation).
        AppError::NetworkError(m) => {
            assert!(m.contains("aaa-broken"), "context missing: {m}");
        }
        // Tolerated alternative: local-path resolution → class Os → git.
        AppError::Git(m) => assert!(!m.is_empty(), "message must not be empty"),
        other => panic!("expected NetworkError or Git, got {other:?}"),
    }

    // Fail-fast: origin was never fetched — tracking ref unchanged.
    assert_eq!(
        rev_parse(&f.work, "refs/remotes/origin/main"),
        tracking_before,
        "origin must be untouched when an earlier remote fails"
    );

    // CLI oracle: `git fetch <broken>` also fails non-zero.
    assert!(!git_ok(&f.work, &["fetch", "aaa-broken"]));
}

// ---------------------------------------------------------------------------
// Probe 3: push with a STALE remote-tracking ref (remote moved after our last
// fetch, local == tracking). Bonsai's §2.6 step-3 short-circuit returns
// UpToDate WITHOUT contacting the remote — a DOCUMENTED DIVERGENCE from the
// CLI (decision §9 "stale-tracking edge"): `git push` in the same state
// contacts the remote, sees main at B, and REJECTS the A←B rewind as
// non-fast-forward. Pinned so the divergence stays conscious. Crucially the
// bare must keep the rival's commit — we never touched the network.
#[test]
fn push_with_stale_tracking_is_local_up_to_date_divergence() {
    require_git!();
    let f = setup();

    // Work is fully pushed at A (clone tip); tracking ref == local tip == A.
    let a_tip = rev_parse(&f.work, "main");
    assert_eq!(rev_parse(&f.work, "refs/remotes/origin/main"), a_tip);

    // Rival pushes B on top of A. Bare now at B; work's tracking ref stale.
    let rival = clone_from_bare(&f.root, &f.bare, "rival");
    local_commit(&rival, "b.txt", "b\n", "rival commit B");
    git(&rival, &["push", "origin", "main"]);
    let b_tip = rev_parse(&f.bare, "main");
    assert_ne!(b_tip, a_tip);

    // Ours: local short-circuit → UpToDate, bare untouched (still B).
    let res = push_current(&f.work, &SpawnGitExec, false).expect("push");
    match res {
        PushResult::UpToDate { remote, branch } => {
            assert_eq!(remote, "origin");
            assert_eq!(branch, "main");
        }
        other => panic!("expected UpToDate (stale-tracking short-circuit), got {other:?}"),
    }
    assert_eq!(rev_parse(&f.bare, "main"), b_tip, "bare must keep rival's B");

    // CLI divergence oracle: `git push` contacts the remote and refuses the
    // non-fast-forward rewind instead of reporting up-to-date.
    assert!(
        !git_ok(&f.work, &["push", "origin", "main"]),
        "CLI `git push` must reject the stale-state push (documented divergence)"
    );
    assert_eq!(rev_parse(&f.bare, "main"), b_tip, "CLI must not move bare either");

    // After a fetch the truth is visible again and pull fast-forwards to B.
    fetch_all(&f.work).expect("fetch");
    let res = pull_ff(&f.work).expect("pull");
    assert!(
        matches!(res, PullResult::FastForwarded { .. }),
        "post-fetch pull must fast-forward to B, got {res:?}"
    );
    assert_eq!(rev_parse(&f.work, "main"), b_tip);
}

// ---------------------------------------------------------------------------
// Probe 4: fast-forward pull across an upstream advance that carries an
// ANNOTATED TAG. AutotagOption::Auto must bring the tag object down with the
// fetch, the ff must land, and the twin `git pull --ff-only` must agree on
// ref, tag oid, and tag object type.
#[test]
fn pull_ff_across_annotated_tag_fetches_tag() {
    require_git!();
    let f = setup();
    let twin = clone_from_bare(&f.root, &f.bare, "twin");

    // Seed publishes commit X, annotated tag v1.0 on X, pushes both.
    std::fs::write(f.seed.join("x.txt"), "x\n").expect("write x.txt");
    git(&f.seed, &["add", "-A"]);
    commit_fixed(&f.seed, "commit X");
    common::git_env(
        &f.seed,
        &["tag", "-a", "v1.0", "-m", "release v1.0"],
        &[
            ("GIT_AUTHOR_DATE", common::FIXED_DATE),
            ("GIT_COMMITTER_DATE", common::FIXED_DATE),
        ],
    );
    git(&f.seed, &["push", "origin", "main", "v1.0"]);
    let bare_tip = rev_parse(&f.bare, "main");

    let res = pull_ff(&f.work).expect("pull_ff");
    match res {
        PullResult::FastForwarded { to, .. } => assert_eq!(to, bare_tip),
        other => panic!("expected FastForwarded, got {other:?}"),
    }
    assert_eq!(rev_parse(&f.work, "main"), bare_tip);

    // The annotated tag arrived (Auto), is a real tag OBJECT, and peels to X.
    assert_eq!(git(&f.work, &["cat-file", "-t", "v1.0"]), "tag");
    assert_eq!(rev_parse(&f.work, "v1.0^{commit}"), bare_tip);

    // Twin oracle: identical ref + tag state after `git pull --ff-only`.
    git(&twin, &["pull", "--ff-only"]);
    assert_eq!(rev_parse(&twin, "main"), rev_parse(&f.work, "main"));
    assert_eq!(rev_parse(&twin, "v1.0"), rev_parse(&f.work, "v1.0"));
    assert_eq!(git(&twin, &["cat-file", "-t", "v1.0"]), "tag");
}

// ---------------------------------------------------------------------------
// Probe 5: unicode branch name round-trip through push. First push of
// `feature/ünïcode` (no upstream) must publish the ref to the bare with the
// name intact, set the upstream config (git config oracle), leave ahead/
// behind at 0/0 (rev-list oracle), and an immediate second push must be
// UpToDate. Exercises UTF-8 refname handling through libgit2's push path AND
// loose-ref filenames on NTFS.
#[test]
fn unicode_branch_push_round_trips() {
    require_git!();
    let f = setup();
    let name = "feature/ünïcode";
    assert!(
        git_ok(&f.work, &["check-ref-format", "--branch", name]),
        "oracle: git must accept {name:?}"
    );

    git(&f.work, &["checkout", "-b", name]);
    local_commit(&f.work, "u.txt", "ü\n", "unicode branch work");
    let tip = rev_parse(&f.work, name);

    let res = push_current(&f.work, &SpawnGitExec, false).expect("push unicode branch");
    match res {
        PushResult::Pushed { remote, branch, set_upstream } => {
            assert_eq!(remote, "origin");
            assert_eq!(branch, name, "branch name must round-trip un-mangled");
            assert!(set_upstream, "first push must set the upstream");
        }
        other => panic!("expected Pushed, got {other:?}"),
    }

    // Bare has the ref, byte-identical name, correct tip.
    assert_eq!(rev_parse(&f.bare, &format!("refs/heads/{name}")), tip);
    // Upstream config exactly as `git push -u` would leave it.
    assert_eq!(git(&f.work, &["config", &format!("branch.{name}.remote")]), "origin");
    assert_eq!(
        git(&f.work, &["config", &format!("branch.{name}.merge")]),
        format!("refs/heads/{name}")
    );
    // Tracking ref exists and ahead/behind is clean (rev-list oracle).
    assert_eq!(rev_parse(&f.work, &format!("refs/remotes/origin/{name}")), tip);
    assert_eq!(
        git(
            &f.work,
            &[
                "rev-list",
                "--left-right",
                "--count",
                &format!("origin/{name}...{name}"),
            ],
        ),
        "0\t0",
        "ahead/behind must be 0/0 after the push"
    );

    // Idempotence: immediate second push is UpToDate.
    let res = push_current(&f.work, &SpawnGitExec, false).expect("second push");
    assert!(
        matches!(res, PushResult::UpToDate { .. }),
        "expected UpToDate, got {res:?}"
    );
}
