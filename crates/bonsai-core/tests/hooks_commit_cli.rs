//! T2 Area 4 — hook-execution integration tests (F-A4-1 / F-A4-2 + tester pins).
//!
//! Regression anchor (F-A4-1, HIGH): with `core.hooksPath` set (Husky-style)
//! and the hook FILE ABSENT, `git hook run` exits 1 ("cannot find a hook") —
//! before the fix Bonsai mapped that to `HookRejected`, blocking EVERY
//! commit / amend / merge-commit / push. The fix adds `--ignore-missing` to
//! the `git hook run` argv (git ≥ 2.36, same floor as the subcommand itself)
//! plus a `core.hooksPath`-aware existence pre-check.
//!
//! All scratch repos live under `D:\Temp\bonsai-scratch` (`common::scratch_dir`).
//! Every test skips (passes with a note) when `git` is missing or < 2.36.

mod common;

use std::path::{Path, PathBuf};

use bonsai_core::error::AppError;
use bonsai_core::git::commit::{amend_commit, create_commit};
use bonsai_core::git::exec::SpawnGitExec;
use bonsai_core::git::hooks::{run_hook_nonblocking, HookName};
use bonsai_core::git::merge::{commit_merge, merge_branch, MergeOutcome};
use bonsai_core::git::remote::{push_current, PushResult};
use bonsai_core::git::stage::stage_paths;
use common::{commit_fixed, git, init_repo};

/// Gate: `git hook run` (and `--ignore-missing`) need Git ≥ 2.36.
macro_rules! require_hook_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
        if !common::git_version_at_least(2, 36) {
            eprintln!("skipping: git < 2.36 (no `git hook run`)");
            return;
        }
    };
}

/// Write an executable `#!/bin/sh` hook with LF endings into `dir`.
fn write_hook(dir: &Path, name: &str, body: &str) {
    std::fs::create_dir_all(dir).expect("mkdir hooks dir");
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}").replace("\r\n", "\n")).expect("write hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod");
    }
}

/// Repo with one seed commit on `main`.
fn seeded_repo() -> tempfile::TempDir {
    let dir = init_repo();
    std::fs::write(dir.path().join("seed.txt"), "seed\n").expect("write seed");
    git(dir.path(), &["add", "-A"]);
    commit_fixed(dir.path(), "seed");
    dir
}

/// Stage a fresh file so a commit has content.
fn stage_new_file(repo: &Path, name: &str) {
    std::fs::write(repo.join(name), "content\n").expect("write file");
    stage_paths(repo, &[name.to_string()]).expect("stage");
}

/// Husky-style setup: `core.hooksPath = .husky` (relative), dir EXISTS but
/// contains no hook files. Returns the hooks dir.
fn set_empty_hookspath(repo: &Path) -> PathBuf {
    let husky = repo.join(".husky");
    std::fs::create_dir_all(&husky).expect("mkdir .husky");
    git(repo, &["config", "core.hooksPath", ".husky"]);
    husky
}

fn head_message(repo: &Path) -> String {
    let r = git2::Repository::open(repo).expect("open");
    let msg = r
        .head()
        .expect("head")
        .peel_to_commit()
        .expect("commit")
        .message()
        .unwrap_or("")
        .to_string();
    msg
}

fn head_oid(repo: &Path) -> String {
    git(repo, &["rev-parse", "HEAD"])
}

/// Pause a clean merge via the CLI (`--no-commit` leaves MERGE_HEAD) so
/// `commit_merge` has a Merge state to finalize. Builds main/topic fork.
fn paused_merge_repo() -> tempfile::TempDir {
    let dir = seeded_repo();
    let p = dir.path();
    git(p, &["checkout", "-b", "topic"]);
    std::fs::write(p.join("topic.txt"), "t\n").expect("write");
    git(p, &["add", "-A"]);
    commit_fixed(p, "topic work");
    git(p, &["checkout", "main"]);
    std::fs::write(p.join("main.txt"), "m\n").expect("write");
    git(p, &["add", "-A"]);
    commit_fixed(p, "main work");
    git(p, &["merge", "--no-commit", "--no-ff", "topic"]);
    assert!(p.join(".git/MERGE_HEAD").exists(), "fixture: merge paused");
    dir
}

/// Fork fixture for `merge_branch` (NOT paused — Bonsai drives the merge).
fn fork_repo() -> tempfile::TempDir {
    let dir = seeded_repo();
    let p = dir.path();
    git(p, &["checkout", "-b", "topic"]);
    std::fs::write(p.join("topic.txt"), "t\n").expect("write");
    git(p, &["add", "-A"]);
    commit_fixed(p, "topic work");
    git(p, &["checkout", "main"]);
    std::fs::write(p.join("main.txt"), "m\n").expect("write");
    git(p, &["add", "-A"]);
    commit_fixed(p, "main work");
    dir
}

// ======================================================================
// F-A4-1 — absent hook under core.hooksPath must NOT block (Husky blocker)
// ======================================================================

/// commit + amend succeed with `core.hooksPath` set and NO hook files; the
/// non-blocking post-commit reports `ran: false` (nothing to run).
#[test]
fn absent_hook_under_hookspath_allows_commit_and_amend() {
    require_hook_git!();
    let dir = seeded_repo();
    set_empty_hookspath(dir.path());

    stage_new_file(dir.path(), "a.txt");
    create_commit(dir.path(), "commit under empty hooksPath", None, false)
        .expect("absent hook must be a no-op, not a rejection");

    amend_commit(dir.path(), "amended under empty hooksPath", None, false)
        .expect("amend: absent hook must be a no-op");
    assert_eq!(head_message(dir.path()), "amended under empty hooksPath\n");

    let info = run_hook_nonblocking(&SpawnGitExec, dir.path(), HookName::PostCommit, &[]);
    assert!(!info.ran, "absent hook must report ran: false");
    assert!(info.success, "absent hook is a successful no-op");
}

/// `commit_merge` succeeds with `core.hooksPath` set and NO hook files.
#[test]
fn absent_hook_under_hookspath_allows_commit_merge() {
    require_hook_git!();
    let dir = paused_merge_repo();
    set_empty_hookspath(dir.path());

    let res = commit_merge(dir.path(), "merge topic", None, false)
        .expect("commit_merge: absent hook must be a no-op");
    assert_eq!(res.summary, "merge topic");
    assert!(!dir.path().join(".git/MERGE_HEAD").exists(), "merge concluded");
}

/// `push_current` succeeds with `core.hooksPath` set and NO pre-push hook.
#[test]
fn absent_hook_under_hookspath_allows_push() {
    require_hook_git!();
    let root_dir = common::scratch_dir();
    let root = root_dir.path();
    git(root, &["init", "--bare", "-b", "main", "origin.git"]);
    let work = root.join("work");
    git(root, &["-c", "core.autocrlf=false", "clone", &root.join("origin.git").to_string_lossy(), "work"]);
    git(&work, &["config", "user.name", "Test User"]);
    git(&work, &["config", "user.email", "test@example.com"]);
    git(&work, &["config", "core.autocrlf", "false"]);
    git(&work, &["checkout", "-B", "main"]);
    std::fs::write(work.join("f.txt"), "x\n").expect("write");
    git(&work, &["add", "-A"]);
    commit_fixed(&work, "first");
    git(&work, &["push", "-u", "origin", "main"]);
    std::fs::write(work.join("g.txt"), "y\n").expect("write");
    git(&work, &["add", "-A"]);
    commit_fixed(&work, "second");
    set_empty_hookspath(&work);

    let res = push_current(&work, &SpawnGitExec, false)
        .expect("push: absent pre-push under hooksPath must be a no-op");
    assert!(matches!(res, PushResult::Pushed { .. }), "got {res:?}");
}

/// A hook that DOES exist under a RELATIVE `core.hooksPath` (.husky style)
/// still runs and can block — the absent-hook fix must not skip real hooks.
#[test]
fn relative_hookspath_failing_hook_still_blocks() {
    require_hook_git!();
    let dir = seeded_repo();
    let husky = set_empty_hookspath(dir.path());
    write_hook(&husky, "pre-commit", "echo \"husky says no\" >&2\nexit 1\n");
    stage_new_file(dir.path(), "a.txt");

    let before = head_oid(dir.path());
    let err = create_commit(dir.path(), "subject", None, false).expect_err("must block");
    match err {
        AppError::HookRejected(m) => assert!(m.contains("husky says no"), "output: {m}"),
        other => panic!("expected HookRejected, got {other:?}"),
    }
    assert_eq!(head_oid(dir.path()), before, "no commit landed");
}

// ======================================================================
// F-A4-2 — clean auto-merge runs the commit-msg hook (only)
// ======================================================================

fn git_hooks_dir(repo: &Path) -> PathBuf {
    repo.join(".git").join("hooks")
}

const TRAILER_HOOK: &str =
    "printf '\\nSigned-off-by: Hook <hook@example.com>\\n' >> \"$1\"\nexit 0\n";

/// A commit-msg hook rewrites the CLEAN auto-merge message (round-trip visible
/// in the merge commit), while a FAILING pre-commit hook is NOT run —
/// pre-merge-commit stays unsupported and pre-commit is not a merge hook.
#[test]
fn clean_merge_runs_commit_msg_hook_not_pre_commit() {
    require_hook_git!();
    let dir = fork_repo();
    write_hook(&git_hooks_dir(dir.path()), "commit-msg", TRAILER_HOOK);
    write_hook(&git_hooks_dir(dir.path()), "pre-commit", "exit 1\n");

    let outcome = merge_branch(dir.path(), "topic", false).expect("clean merge");
    assert!(matches!(outcome, MergeOutcome::Merged { .. }), "got {outcome:?}");
    let msg = head_message(dir.path());
    assert!(msg.starts_with("Merge branch 'topic'"), "subject kept: {msg}");
    assert!(
        msg.contains("Signed-off-by: Hook <hook@example.com>"),
        "commit-msg rewrite must land in the merge commit: {msg}"
    );
    let parents = git(dir.path(), &["rev-list", "--parents", "-1", "HEAD"]);
    assert_eq!(parents.split_whitespace().count(), 3, "2-parent merge: {parents}");
}

/// A FAILING commit-msg hook blocks the clean auto-merge with the merge left
/// PAUSED — MERGE_HEAD retained, HEAD unchanged, merged content staged —
/// exactly git's "Not committing merge; use 'git commit' to complete the
/// merge." state. Recovery: `commit_merge` with skip_hooks completes it.
#[test]
fn clean_merge_commit_msg_fail_leaves_merge_paused() {
    require_hook_git!();
    let dir = fork_repo();
    write_hook(
        &git_hooks_dir(dir.path()),
        "commit-msg",
        "echo \"policy says no\" >&2\nexit 1\n",
    );
    let before = head_oid(dir.path());

    let err = merge_branch(dir.path(), "topic", false).expect_err("must block");
    match err {
        AppError::HookRejected(m) => assert!(m.contains("policy says no"), "output: {m}"),
        other => panic!("expected HookRejected, got {other:?}"),
    }
    assert_eq!(head_oid(dir.path()), before, "no merge commit landed");
    assert!(
        dir.path().join(".git/MERGE_HEAD").exists(),
        "merge must be left PAUSED (MERGE_HEAD retained), not half-done"
    );
    {
        let repo = git2::Repository::open(dir.path()).expect("open");
        assert_eq!(repo.state(), git2::RepositoryState::Merge, "state Merge");
    }

    // Recovery path: conclude the paused merge, bypassing the failing hook.
    let res = commit_merge(dir.path(), "merge topic anyway", None, true).expect("recover");
    assert_eq!(res.summary, "merge topic anyway");
    assert!(!dir.path().join(".git/MERGE_HEAD").exists(), "merge concluded");
}

/// `skip_hooks = true` (≡ --no-verify) bypasses a failing commit-msg on the
/// clean auto-merge.
#[test]
fn clean_merge_skip_hooks_bypasses_commit_msg() {
    require_hook_git!();
    let dir = fork_repo();
    write_hook(&git_hooks_dir(dir.path()), "commit-msg", "exit 1\n");

    let outcome = merge_branch(dir.path(), "topic", true).expect("skip bypasses");
    assert!(matches!(outcome, MergeOutcome::Merged { .. }), "got {outcome:?}");
}

// ======================================================================
// Amend hooks (tester pin: block + trailer + skip)
// ======================================================================

/// A failing pre-commit blocks `amend_commit`; HEAD unchanged.
#[test]
fn amend_failing_pre_commit_blocks() {
    require_hook_git!();
    let dir = seeded_repo();
    write_hook(&git_hooks_dir(dir.path()), "pre-commit", "echo nope >&2\nexit 1\n");
    let before = head_oid(dir.path());

    let err = amend_commit(dir.path(), "amended", None, false).expect_err("must block");
    assert!(matches!(err, AppError::HookRejected(ref m) if m.contains("nope")), "got {err:?}");
    assert_eq!(head_oid(dir.path()), before, "HEAD unchanged");
}

/// commit-msg rewrites the amended message; skip_hooks bypasses a failing one.
#[test]
fn amend_commit_msg_trailer_and_skip() {
    require_hook_git!();
    let dir = seeded_repo();
    write_hook(&git_hooks_dir(dir.path()), "commit-msg", TRAILER_HOOK);
    amend_commit(dir.path(), "amended subject", None, false).expect("amend");
    let msg = head_message(dir.path());
    assert!(msg.starts_with("amended subject"), "{msg}");
    assert!(msg.contains("Signed-off-by: Hook"), "trailer in amend: {msg}");

    write_hook(&git_hooks_dir(dir.path()), "commit-msg", "exit 1\n");
    amend_commit(dir.path(), "amended again", None, true).expect("skip bypasses");
    assert_eq!(head_message(dir.path()), "amended again\n");
}

// ======================================================================
// commit_merge hooks (tester pin: fail -> MERGE_HEAD retained; rewrite;
// post-commit sees MERGE_HEAD)
// ======================================================================

/// A failing pre-commit blocks `commit_merge` with the merge state RETAINED
/// (MERGE_HEAD still present, retry/abort possible).
#[test]
fn commit_merge_hook_fail_retains_merge_head() {
    require_hook_git!();
    let dir = paused_merge_repo();
    write_hook(&git_hooks_dir(dir.path()), "pre-commit", "echo blocked >&2\nexit 1\n");

    let err = commit_merge(dir.path(), "merge topic", None, false).expect_err("must block");
    assert!(matches!(err, AppError::HookRejected(_)), "got {err:?}");
    assert!(
        dir.path().join(".git/MERGE_HEAD").exists(),
        "MERGE_HEAD retained for retry/abort"
    );
}

/// commit-msg rewrites the merge-commit message, and post-commit (run BEFORE
/// cleanup_state) still sees MERGE_HEAD.
#[test]
fn commit_merge_rewrite_and_post_commit_sees_merge_head() {
    require_hook_git!();
    let dir = paused_merge_repo();
    write_hook(&git_hooks_dir(dir.path()), "commit-msg", TRAILER_HOOK);
    write_hook(
        &git_hooks_dir(dir.path()),
        "post-commit",
        "if test -f \"$(git rev-parse --git-dir)/MERGE_HEAD\"; then echo present > post-commit-saw.txt; fi\nexit 0\n",
    );

    commit_merge(dir.path(), "merge topic", None, false).expect("commit merge");
    let msg = head_message(dir.path());
    assert!(msg.contains("Signed-off-by: Hook"), "rewritten: {msg}");
    let saw = std::fs::read_to_string(dir.path().join("post-commit-saw.txt"))
        .expect("post-commit must have run and seen MERGE_HEAD");
    assert_eq!(saw.trim(), "present");
    assert!(!dir.path().join(".git/MERGE_HEAD").exists(), "cleanup after post-commit");
}

// ======================================================================
// Tester pins: CRLF trailer normalization, hook-emptied message,
// skip_hooks sentinel matrix
// ======================================================================

/// A commit-msg hook that appends a CRLF trailer: the committed message is
/// re-normalized (no `\r` survives into the object).
#[test]
fn crlf_trailer_from_hook_is_normalized() {
    require_hook_git!();
    let dir = seeded_repo();
    write_hook(
        &git_hooks_dir(dir.path()),
        "commit-msg",
        "printf '\\r\\nCRLF-Trailer: yes\\r\\n' >> \"$1\"\nexit 0\n",
    );
    stage_new_file(dir.path(), "a.txt");
    create_commit(dir.path(), "subject", None, false).expect("commit");
    let msg = head_message(dir.path());
    assert!(msg.contains("CRLF-Trailer: yes"), "trailer present: {msg:?}");
    assert!(!msg.contains('\r'), "no CR may survive normalization: {msg:?}");
}

/// A commit-msg hook that TRUNCATES the message file to empty ⇒ clean
/// `EmptyMessage` error, no commit.
#[test]
fn hook_emptied_message_is_empty_message_error() {
    require_hook_git!();
    let dir = seeded_repo();
    write_hook(&git_hooks_dir(dir.path()), "commit-msg", ": > \"$1\"\nexit 0\n");
    stage_new_file(dir.path(), "a.txt");
    let before = head_oid(dir.path());

    let err = create_commit(dir.path(), "subject", None, false).expect_err("must fail");
    assert!(matches!(err, AppError::EmptyMessage), "got {err:?}");
    assert_eq!(head_oid(dir.path()), before, "no commit landed");
}

/// skip_hooks sentinel matrix over the commit-side sites: with FAILING
/// pre-commit + commit-msg hooks that also drop a sentinel file,
/// `skip_hooks = true` means the hooks NEVER EXECUTE (no sentinel) and every
/// operation succeeds — create_commit, amend_commit, merge_branch (clean
/// auto-merge), commit_merge.
#[test]
fn skip_hooks_sentinel_matrix_commit_sites() {
    require_hook_git!();
    const SENTINEL_HOOK: &str = "echo ran >> hook-sentinel.txt\nexit 1\n";
    let no_sentinel = |repo: &Path, site: &str| {
        assert!(
            !repo.join("hook-sentinel.txt").exists(),
            "{site}: skipped hook must not have executed"
        );
    };

    // create_commit + amend_commit.
    let dir = seeded_repo();
    write_hook(&git_hooks_dir(dir.path()), "pre-commit", SENTINEL_HOOK);
    write_hook(&git_hooks_dir(dir.path()), "commit-msg", SENTINEL_HOOK);
    stage_new_file(dir.path(), "a.txt");
    create_commit(dir.path(), "created", None, true).expect("create with skip");
    no_sentinel(dir.path(), "create_commit");
    amend_commit(dir.path(), "amended", None, true).expect("amend with skip");
    no_sentinel(dir.path(), "amend_commit");

    // merge_branch clean auto-merge.
    let m = fork_repo();
    write_hook(&git_hooks_dir(m.path()), "pre-commit", SENTINEL_HOOK);
    write_hook(&git_hooks_dir(m.path()), "commit-msg", SENTINEL_HOOK);
    let outcome = merge_branch(m.path(), "topic", true).expect("merge with skip");
    assert!(matches!(outcome, MergeOutcome::Merged { .. }), "got {outcome:?}");
    no_sentinel(m.path(), "merge_branch");

    // commit_merge (paused merge).
    let c = paused_merge_repo();
    write_hook(&git_hooks_dir(c.path()), "pre-commit", SENTINEL_HOOK);
    write_hook(&git_hooks_dir(c.path()), "commit-msg", SENTINEL_HOOK);
    commit_merge(c.path(), "merge topic", None, true).expect("commit_merge with skip");
    no_sentinel(c.path(), "commit_merge");
}
