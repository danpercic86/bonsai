//! M3 adversarial probes (tester gap-probing, beyond contract §6.1/§6.2).
//!
//! Same CLI-oracle / twin-repo machinery as `stage_cli.rs` / `commit_cli.rs`.
//! These tests PIN observed behavior for risky uncovered cases; where our
//! behavior diverges from the plain `git` CLI, the divergence is asserted
//! explicitly and flagged in comments so it is a conscious decision, not an
//! accident.

mod common;

use std::path::Path;

use bonsai_lib::git::commit::create_commit;
use bonsai_lib::git::stage::{stage_paths, unstage_paths};
use common::{assert_same_status, commit_fixed, git, git_ok, git_raw, init_repo, porcelain_records};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

fn strings(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|s| s.to_string()).collect()
}

/// Base fixture: committed `.gitignore` (ignoring `ignored.txt`) plus a
/// rename-detectable tracked file, fixed-date commit.
fn base_repo() -> tempfile::TempDir {
    let dir = init_repo();
    let path = dir.path();
    std::fs::write(path.join(".gitignore"), "ignored.txt\n").expect("write .gitignore");
    std::fs::write(
        path.join("tracked.txt"),
        "line one\nline two\nline three\nline four\nline five\n",
    )
    .expect("write tracked.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "base");
    dir
}

// Probe A: staging a gitignored file.
//
// DOCUMENTED DIVERGENCE (contract §2.3 note): `index.add_path` has
// `git add -f` semantics — it stages ignored files that plain `git add`
// refuses (exit 1, "Use -f if you really want to add them"). The contract
// accepts this because the UI only offers paths already present in
// StatusSnapshot (and read_status excludes ignored files), so the backend can
// only receive an ignored path via a race or a bug upstream. This test pins
// both facts: plain CLI add refuses; our stage_paths behaves like `add -f`.
#[test]
fn stage_ignored_file_diverges_from_plain_git_add() {
    require_git!();
    let a = base_repo();
    let b = base_repo();
    for p in [a.path(), b.path()] {
        std::fs::write(p.join("ignored.txt"), "secret\n").expect("write ignored.txt");
    }

    // Oracle fact 1: the plain CLI refuses ignored paths...
    assert!(
        !git_ok(b.path(), &["add", "--", "ignored.txt"]),
        "plain `git add` must refuse an ignored path"
    );
    // ...and refusing means the index is untouched.
    assert!(porcelain_records(b.path()).is_empty(), "CLI twin unchanged");

    // Our behavior: stage_paths succeeds (add -f semantics, per contract).
    stage_paths(a.path(), &strings(&["ignored.txt"])).expect("stage_paths stages ignored file");
    let records = porcelain_records(a.path());
    assert!(
        records.iter().any(|(r, _)| r == "A  ignored.txt"),
        "expected staged Added for the ignored file, got: {records:?}"
    );

    // Parity holds against the FORCED CLI form.
    git(b.path(), &["add", "-f", "--", "ignored.txt"]);
    assert_same_status(a.path(), b.path());
}

// Probe B: unstage a rename (both sides), then re-stage only ONE side.
//
// The backend is rename-agnostic (present -> add, missing -> remove), so
// re-staging just the new side must match `git add -- <new>` exactly:
// porcelain shows a staged Added new-side plus an unstaged Deleted old-side
// (no rename pairing without the delete in the index).
#[test]
fn restage_only_new_side_after_unstaging_rename() {
    require_git!();
    let a = base_repo();
    let b = base_repo();

    // Identical setup on both twins: worktree rename, stage both sides,
    // unstage both sides (ours on A, CLI on B).
    for p in [a.path(), b.path()] {
        std::fs::rename(p.join("tracked.txt"), p.join("renamed.txt")).expect("fs rename");
    }
    stage_paths(a.path(), &strings(&["tracked.txt", "renamed.txt"])).expect("stage rename");
    unstage_paths(a.path(), &strings(&["tracked.txt", "renamed.txt"])).expect("unstage rename");
    git(b.path(), &["add", "-A", "--", "tracked.txt", "renamed.txt"]);
    git(b.path(), &["restore", "--staged", "--", "tracked.txt", "renamed.txt"]);
    assert_same_status(a.path(), b.path());

    // Re-stage ONLY the new side.
    stage_paths(a.path(), &strings(&["renamed.txt"])).expect("re-stage new side only");
    git(b.path(), &["add", "--", "renamed.txt"]);

    assert_same_status(a.path(), b.path());
    let records = porcelain_records(a.path());
    assert!(
        records.iter().any(|(r, _)| r == "A  renamed.txt"),
        "new side staged as Added (no rename pair), got: {records:?}"
    );
    assert!(
        records.iter().any(|(r, _)| r == " D tracked.txt"),
        "old side stays an unstaged deletion, got: {records:?}"
    );
}

// Probe C: commit message with CRLF line endings (Windows textarea!).
//
// `git commit -m` applies cleanup=whitespace, which strips the trailing `\r`
// of every line — verified empirically: the CLI stores
// "subject line\nsecond line\n" for a CRLF input. create_commit normalizes
// CRLF / lone CR to `\n` before its trim + trailing-newline cleanup
// (orchestrator decision after this probe originally pinned a divergence),
// so the stored message must be byte-identical to the CLI twin's.
#[test]
fn crlf_message_matches_cli_cleanup() {
    require_git!();
    let msg = "subject line\r\nsecond line\r"; // CRLF interior + trailing lone \r
    let a = base_repo();
    let b = base_repo();
    for p in [a.path(), b.path()] {
        std::fs::write(p.join("new.txt"), "content\n").expect("write new.txt");
        git(p, &["add", "--", "new.txt"]);
    }

    let res = create_commit(a.path(), msg).expect("create_commit");
    git(b.path(), &["commit", "-m", msg]);

    let message_of = |dir: &Path| {
        let raw = git_raw(dir, &["cat-file", "commit", "HEAD"], &[]);
        let text = String::from_utf8(raw).expect("utf-8 commit object");
        text.split_once("\n\n").expect("header/message separator").1.to_string()
    };

    // Byte parity with the CLI's cleanup=whitespace result — no stray \r.
    assert_eq!(message_of(b.path()), "subject line\nsecond line\n");
    assert_eq!(message_of(a.path()), message_of(b.path()));
    assert_eq!(res.summary, "subject line");
}
