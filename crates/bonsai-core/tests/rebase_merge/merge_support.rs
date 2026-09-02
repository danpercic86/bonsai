//! Shared helpers and fixtures for the P3c CLI-oracle merge tests
//! (contract §9), extracted verbatim from `merge_cli.rs`.
//!
//! Twin-repo scaffolding: `twin_pair` builds two scratch repos from the
//! IDENTICAL scripted CLI setup (fixed dates -> identical base oids), so
//! Bonsai's merge fns and the real `git` CLI can be compared byte-exactly
//! (tree oids, parents, messages, conflicted sets).

use std::path::Path;
use std::process::Command;

use crate::common;
use crate::common::{commit_fixed, git, git_raw, init_repo, FIXED_DATE};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

pub(crate) use require_git;

// ------------------------------------------------------------ small helpers

/// Runs `git <args>` expecting FAILURE (e.g. a conflicted `git merge`).
pub(crate) fn git_fail(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("failed to run git {args:?}: {e}"));
    assert!(
        !out.status.success(),
        "expected git {args:?} to fail, but it succeeded: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

pub(crate) fn head_oid(dir: &Path) -> String {
    git(dir, &["rev-parse", "HEAD"])
}

pub(crate) fn tree_oid(dir: &Path) -> String {
    git(dir, &["rev-parse", "HEAD^{tree}"])
}

/// Parent oids of HEAD, in order.
pub(crate) fn parents(dir: &Path) -> Vec<String> {
    git(dir, &["log", "-1", "--format=%P"])
        .split_whitespace()
        .map(String::from)
        .collect()
}

/// Raw commit-message body of HEAD (byte-exact).
pub(crate) fn message(dir: &Path) -> Vec<u8> {
    git_raw(dir, &["log", "-1", "--format=%B"], &[])
}

/// Conflicted path set per the CLI (`git diff --name-only --diff-filter=U`).
pub(crate) fn cli_conflicted(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = git(dir, &["diff", "--name-only", "--diff-filter=U"])
        .lines()
        .map(String::from)
        .collect();
    v.sort();
    v
}

pub(crate) fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

pub(crate) fn repo_state(dir: &Path) -> git2::RepositoryState {
    git2::Repository::open(dir).expect("open repo").state()
}

/// Number of entries on the stash stack (P8 autostash assertions). Uses the
/// git CLI since these tests already `require_git!`; git2's stash_save2 writes
/// the standard refs/stash + reflog, so `git stash list` sees it.
pub(crate) fn stash_count(dir: &Path) -> usize {
    git(dir, &["stash", "list"])
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
}

/// Twin `git merge --no-edit <name>` with fixed committer/author dates.
pub(crate) fn cli_merge(dir: &Path, name: &str) {
    common::git_env(
        dir,
        &["merge", "--no-edit", name],
        &[
            ("GIT_AUTHOR_DATE", FIXED_DATE),
            ("GIT_COMMITTER_DATE", FIXED_DATE),
        ],
    );
}

// ------------------------------------------------------------ fixtures

/// Diverged branches touching DISJOINT files -> clean true merge.
pub(crate) fn script_clean_diverged(d: &Path) {
    write(d, "a.txt", "a base\n");
    write(d, "b.txt", "b base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "b.txt", "b topic\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic change");
    git(d, &["checkout", "main"]);
    write(d, "a.txt", "a main\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main change");
}

/// Same line edited on both sides -> guaranteed conflict on a.txt.
pub(crate) fn script_conflict(d: &Path) {
    write(d, "a.txt", "line1\nbase\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "a.txt", "line1\ntopic\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic change");
    git(d, &["checkout", "main"]);
    write(d, "a.txt", "line1\nmain\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main change");
}

/// TWO guaranteed-conflict files (a.txt, b.txt).
pub(crate) fn script_conflict_two_files(d: &Path) {
    write(d, "a.txt", "a base\n");
    write(d, "b.txt", "b base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "a.txt", "a topic\n");
    write(d, "b.txt", "b topic\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic change");
    git(d, &["checkout", "main"]);
    write(d, "a.txt", "a main\n");
    write(d, "b.txt", "b main\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main change");
}

/// Builds twin repos by applying the same script to two fresh scratch repos.
/// Returns (bonsai, twin).
pub(crate) fn twin_pair(script: fn(&Path)) -> (tempfile::TempDir, tempfile::TempDir) {
    let bonsai = init_repo();
    let twin = init_repo();
    script(bonsai.path());
    script(twin.path());
    assert_eq!(
        head_oid(bonsai.path()),
        head_oid(twin.path()),
        "fixture scripts must produce identical base histories"
    );
    (bonsai, twin)
}
