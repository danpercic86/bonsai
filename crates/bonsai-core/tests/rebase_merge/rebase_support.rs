//! Shared helpers and fixtures for the P3d CLI-oracle rebase tests
//! (contract §9), extracted verbatim from `rebase_cli.rs`.
//!
//! Twin-repo scaffolding: `twin_pair` builds two scratch repos from the
//! IDENTICAL scripted CLI setup (fixed dates -> identical base oids), and
//! `CInfo`/`top_infos` capture only the fields §9 mandates comparing (tree,
//! author identity + author time, message) — never commit oids or committer.

use std::path::Path;
use std::process::Command;

use crate::common;
use crate::common::{commit_fixed, git, git_raw, init_repo};

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

/// Runs `git <args>` expecting FAILURE (e.g. a conflicted `git rebase`).
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

pub(crate) fn rev_parse(dir: &Path, rev: &str) -> String {
    git(dir, &["rev-parse", rev])
}

/// Number of commits on `rev` that are not reachable from `base` (i.e. the
/// replayed range `base..rev`).
pub(crate) fn count_ahead(dir: &Path, base: &str, rev: &str) -> usize {
    git(dir, &["rev-list", "--count", &format!("{base}..{rev}")])
        .parse()
        .expect("count parse")
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

pub(crate) fn read(dir: &Path, name: &str) -> Vec<u8> {
    std::fs::read(dir.join(name)).expect("read fixture file")
}

pub(crate) fn repo_state(dir: &Path) -> git2::RepositoryState {
    git2::Repository::open(dir).expect("open repo").state()
}

pub(crate) fn has_rebase_dir(dir: &Path) -> bool {
    dir.join(".git").join("rebase-merge").exists() || dir.join(".git").join("rebase-apply").exists()
}

pub(crate) fn checkout(dir: &Path, name: &str) {
    git(dir, &["checkout", name]);
}

/// Per-commit descriptor that survives the "committer time differs" rule: it
/// carries ONLY the fields the contract mandates comparing (tree, author
/// identity + author time, message) — never the commit oid or committer.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CInfo {
    tree: String,
    author_name: String,
    author_email: String,
    /// author unix timestamp + ISO (tz) — preserved across a rebase.
    author_time: String,
    message: Vec<u8>,
}

pub(crate) fn commit_info(dir: &Path, rev: &str) -> CInfo {
    CInfo {
        tree: git(dir, &["rev-parse", &format!("{rev}^{{tree}}")]),
        author_name: git(dir, &["log", "-1", "--format=%an", rev]),
        author_email: git(dir, &["log", "-1", "--format=%ae", rev]),
        author_time: git(dir, &["log", "-1", "--format=%at %aI", rev]),
        message: git_raw(dir, &["log", "-1", "--format=%B", rev], &[]),
    }
}

/// The top `count` commits reachable from HEAD, newest first.
pub(crate) fn top_infos(dir: &Path, count: usize) -> Vec<CInfo> {
    let arg = format!("--max-count={count}");
    git(dir, &["rev-list", &arg, "HEAD"])
        .lines()
        .map(|r| commit_info(dir, r))
        .collect()
}

/// Twin `git rebase <onto>` with the editor disabled (never blocks on a message
/// prompt). Asserts success.
pub(crate) fn cli_rebase(dir: &Path, onto: &str) {
    common::git_env(dir, &["rebase", onto], &[("GIT_EDITOR", "true")]);
}

/// Twin `git rebase --continue` (editor disabled).
pub(crate) fn cli_rebase_continue(dir: &Path) {
    common::git_env(dir, &["rebase", "--continue"], &[("GIT_EDITOR", "true")]);
}

/// Twin `git rebase --skip` (editor disabled).
pub(crate) fn cli_rebase_skip(dir: &Path) {
    common::git_env(dir, &["rebase", "--skip"], &[("GIT_EDITOR", "true")]);
}

/// Builds twin repos by applying the same script to two fresh scratch repos.
/// Returns (bonsai, twin). Asserts identical base histories.
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

// ------------------------------------------------------------ fixtures

/// topic = 2 commits touching DISJOINT files; main advances a disjoint file.
/// Ends on `main`. -> clean linear rebase, steps == 2.
pub(crate) fn script_clean_linear(d: &Path) {
    write(d, "a.txt", "a base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "t1.txt", "t1\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic one");
    write(d, "t2.txt", "t2\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic two");
    git(d, &["checkout", "main"]);
    write(d, "m.txt", "m\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main advance");
}

/// topic edits the same line of a.txt as main -> a single pick conflicts.
/// Ends on `main`.
pub(crate) fn script_conflict_one(d: &Path) {
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

/// One topic commit touching THREE files, all conflicting with main -> single
/// pick conflicts on a/b/c (exercises Ours/Theirs/hand-edit in one step).
pub(crate) fn script_conflict_three(d: &Path) {
    write(d, "a.txt", "a\nX\n");
    write(d, "b.txt", "b\nX\n");
    write(d, "c.txt", "c\nX\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "a.txt", "a\ntopic\n");
    write(d, "b.txt", "b\ntopic\n");
    write(d, "c.txt", "c\ntopic\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic edit");
    git(d, &["checkout", "main"]);
    write(d, "a.txt", "a\nmain\n");
    write(d, "b.txt", "b\nmain\n");
    write(d, "c.txt", "c\nmain\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main edit");
}

/// topic = [t1 edits a.txt (conflicts with main), t2 edits other.txt (clean)].
/// Ends on `main` (which also edits a.txt). Rebasing conflicts on t1.
pub(crate) fn script_skip_first(d: &Path) {
    write(d, "a.txt", "line1\nbase\nline3\n");
    write(d, "other.txt", "other base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "a.txt", "line1\ntopic\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic a change");
    write(d, "other.txt", "other topic\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic other change");
    git(d, &["checkout", "main"]);
    write(d, "a.txt", "line1\nmain\nline3\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "main a change");
}
