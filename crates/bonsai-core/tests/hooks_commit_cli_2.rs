//! T2 Area 4 (pinned) — commit-msg non-UTF-8 round-trip + pre-push stdin format.
//!
//! Splits off `hooks_commit_cli.rs` (file-size discipline). Two guarantees:
//!  1. a `commit-msg` hook that rewrites the message file to INVALID UTF-8 bytes
//!     surfaces as a clean `AppError::Io` (the re-read is `read_to_string`) — no
//!     panic, no commit written (documented in `commit.rs::run_commit_msg_hook`);
//!  2. the pre-push hook receives stdin in the githooks(5) documented shape
//!     `<local-ref> <local-oid> <remote-ref> <remote-oid>` — a baseline pin so a
//!     future change to Bonsai's `--to-stdin` payload is caught.
//!
//! All scratch repos live under `D:\Data\Temp\bonsai-scratch` (`common::scratch_dir`).
//! Every test skips (passes with a note) when `git` is missing or < 2.36.

mod common;

use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::git::commit::create_commit;
use bonsai_core::git::exec::SpawnGitExec;
use bonsai_core::git::remote::{push_current, PushResult};
use bonsai_core::git::stage::stage_paths;
use common::{commit_fixed, git, init_repo};

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

/// Write an executable `#!/bin/sh` hook (LF) into `.git/hooks/<name>`.
fn write_git_hook(repo: &Path, name: &str, body: &str) {
    let hooks = repo.join(".git").join("hooks");
    std::fs::create_dir_all(&hooks).expect("mkdir hooks");
    let path = hooks.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}").replace("\r\n", "\n")).expect("write hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod");
    }
}

fn head_oid(repo: &Path) -> Option<String> {
    let r = git2::Repository::open(repo).ok()?;
    let oid = r.head().ok()?.target().map(|o| o.to_string());
    oid
}

fn head_message(repo: &Path) -> String {
    let r = git2::Repository::open(repo).expect("open");
    let commit = r.head().expect("head").peel_to_commit().expect("commit");
    let msg = commit.message().unwrap_or("").to_string();
    msg
}

/// A `commit-msg` hook that overwrites the message file (`$1`) with two 0xFF
/// bytes — invalid UTF-8 — must yield a clean `AppError::Io`, and MUST NOT write
/// a commit (the hook runs before `write_tree`/commit).
#[test]
fn commit_msg_hook_writing_non_utf8_is_clean_io_error_no_commit() {
    require_hook_git!();
    let dir = init_repo();
    let repo = dir.path();
    std::fs::write(repo.join("seed.txt"), "seed\n").expect("write seed");
    git(repo, &["add", "-A"]);
    commit_fixed(repo, "seed");
    let before = head_oid(repo);

    // printf interprets the octal escapes → raw 0xFF 0xFF into the msg file.
    write_git_hook(repo, "commit-msg", "printf '\\377\\377' > \"$1\"\n");

    std::fs::write(repo.join("a.txt"), "content\n").expect("write");
    stage_paths(repo, &["a.txt".to_string()]).expect("stage");

    let err = create_commit(repo, "subject", None, false)
        .expect_err("non-UTF-8 rewritten message must error, not commit");
    assert!(
        matches!(err, AppError::Io(_)),
        "expected a clean AppError::Io from the re-read, got {err:?}"
    );
    // No commit was created — HEAD is still the seed commit.
    assert_eq!(head_oid(repo), before, "no commit must be written");
    assert!(head_message(repo).starts_with("seed"), "HEAD unchanged");
}

/// Baseline pin: the pre-push hook receives stdin lines shaped
/// `<local-ref> <local-oid> <remote-ref> <remote-oid>` (githooks(5)). We dump
/// stdin from the hook to a sentinel file and assert the token shape + the local
/// ref/oid Bonsai's `--to-stdin` payload carries.
#[test]
fn pre_push_stdin_has_documented_ref_oid_shape() {
    require_hook_git!();
    let root_dir = common::scratch_dir();
    let root = root_dir.path();
    git(root, &["init", "--bare", "-b", "main", "origin.git"]);
    let work = root.join("work");
    git(
        root,
        &["-c", "core.autocrlf=false", "clone", &root.join("origin.git").to_string_lossy(), "work"],
    );
    git(&work, &["config", "user.name", "Test User"]);
    git(&work, &["config", "user.email", "test@example.com"]);
    git(&work, &["config", "core.autocrlf", "false"]);
    git(&work, &["checkout", "-B", "main"]);
    std::fs::write(work.join("f.txt"), "x\n").expect("write");
    git(&work, &["add", "-A"]);
    commit_fixed(&work, "first");
    git(&work, &["push", "-u", "origin", "main"]);

    // Second commit → the thing push_current will push.
    std::fs::write(work.join("g.txt"), "y\n").expect("write");
    git(&work, &["add", "-A"]);
    commit_fixed(&work, "second");
    let local_oid = head_oid(&work).expect("local head");

    // pre-push hook dumps its stdin VERBATIM to an absolute (forward-slash) path.
    let sentinel = work.join("prepush_stdin.txt");
    let sentinel_sh = sentinel.to_string_lossy().replace('\\', "/");
    write_git_hook(&work, "pre-push", &format!("cat > \"{sentinel_sh}\"\n"));

    let res = push_current(&work, &SpawnGitExec, false).expect("push should succeed");
    assert!(matches!(res, PushResult::Pushed { .. }), "got {res:?}");

    let captured = std::fs::read_to_string(&sentinel).expect("pre-push stdin captured");
    let line = captured.lines().next().expect("at least one ref line");
    let tokens: Vec<&str> = line.split_whitespace().collect();
    assert_eq!(tokens.len(), 4, "githooks(5) shape: 4 tokens, got {line:?}");
    assert_eq!(tokens[0], "refs/heads/main", "local ref");
    assert_eq!(tokens[1], local_oid, "local oid = the pushed HEAD");
    assert_eq!(tokens[2], "refs/heads/main", "remote ref");
    // tokens[3] = remote oid (the FIRST commit) — a 40-hex baseline check.
    assert_eq!(tokens[3].len(), 40, "remote oid is a full sha: {:?}", tokens[3]);
}
