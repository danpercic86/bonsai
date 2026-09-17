//! Shared helpers and fixtures for the P23a CLI-oracle interactive-rebase tests
//! (contract §13.1), extracted verbatim from `rebase_interactive_cli.rs`.
//!
//! The scripted fixtures build DISJOINT-file topic histories (plus one
//! guaranteed-conflict history) with the `git` CLI at fixed dates, so base oids
//! are deterministic across the whole family of interactive-rebase test modules.

use std::path::Path;

use crate::common::{commit_fixed, git};

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

pub(crate) fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

pub(crate) fn read_str(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).expect("read fixture file")
}

pub(crate) fn rev(dir: &Path, r: &str) -> String {
    git(dir, &["rev-parse", r])
}

pub(crate) fn tree_of(dir: &Path, r: &str) -> String {
    git(dir, &["rev-parse", &format!("{r}^{{tree}}")])
}

pub(crate) fn msg_of(dir: &Path, r: &str) -> String {
    git(dir, &["log", "-1", "--format=%B", r])
        .trim()
        .to_string()
}

pub(crate) fn author_of(dir: &Path, r: &str) -> String {
    git(dir, &["log", "-1", "--format=%an <%ae> %at", r])
}

pub(crate) fn count_ahead(dir: &Path, base: &str, r: &str) -> usize {
    git(dir, &["rev-list", "--count", &format!("{base}..{r}")])
        .parse()
        .expect("count parse")
}

pub(crate) fn repo_state(dir: &Path) -> git2::RepositoryState {
    git2::Repository::open(dir).expect("open repo").state()
}

pub(crate) fn has_bonsai_dir(dir: &Path) -> bool {
    dir.join(".git")
        .join("bonsai-rebase")
        .join("state.json")
        .exists()
}

pub(crate) fn symbolic_head(dir: &Path) -> String {
    git(dir, &["symbolic-ref", "HEAD"])
}

pub(crate) fn tree_files(dir: &Path, r: &str) -> Vec<String> {
    let mut v: Vec<String> = git(dir, &["ls-tree", "-r", "--name-only", r])
        .lines()
        .map(String::from)
        .collect();
    v.sort();
    v
}

// ------------------------------------------------------------ fixtures

/// 3 linear topic commits touching disjoint files a/b/c on top of `base`.
pub(crate) fn script_three_disjoint(d: &Path) {
    write(d, "base.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "a.txt", "a\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "c1");
    write(d, "b.txt", "b\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "c2");
    write(d, "c.txt", "c\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "c3");
}

/// 2 linear topic commits touching disjoint files a/b on top of `base`.
pub(crate) fn script_two_disjoint(d: &Path) {
    write(d, "base.txt", "base\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");
    git(d, &["checkout", "-b", "topic"]);
    write(d, "a.txt", "a\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "c1");
    write(d, "b.txt", "b\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "c2");
}

/// topic edits a.txt one way; main edits the same line differently -> a pick of
/// topic onto main conflicts on a.txt. Ends checked out on `topic`.
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
    git(d, &["checkout", "topic"]);
}
