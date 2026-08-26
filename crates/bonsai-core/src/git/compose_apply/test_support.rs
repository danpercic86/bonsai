//! Shared git2 fixtures for the `compose_apply` test modules. Extracted verbatim
//! from the former inline `mod tests`; `pub(super)` so both sibling test modules
//! reuse them.

#[allow(unused_imports)]
use super::*;
use std::process::Command;

/// git2-init a scratch repo with identity + autocrlf off (mirrors `ai_compose`).
pub(super) fn init_scratch() -> tempfile::TempDir {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    dir
}

/// Same, but WITHOUT a usable identity (for the ConfigMissing case). The
/// machine running the suite may carry a GLOBAL `user.name`/`user.email`, so
/// pin EMPTY local values (highest precedence) to mask it — `resolve_signature`
/// treats empty as unset. `core.autocrlf` is pinned for deterministic hashing.
pub(super) fn init_scratch_no_identity() -> tempfile::TempDir {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "").expect("mask name");
    cfg.set_str("user.email", "").expect("mask email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    dir
}

pub(super) fn write(p: &Path, rel: &str, body: &str) {
    std::fs::write(p.join(rel), body).unwrap_or_else(|e| panic!("write {rel}: {e}"));
}

pub(super) fn stage(p: &Path, paths: &[&str]) {
    stage_paths(p, &paths.iter().map(|s| s.to_string()).collect::<Vec<_>>()).expect("stage");
}

pub(super) fn group(files: &[&str], message: &str) -> ComposeGroup {
    ComposeGroup {
        files: files.iter().map(|s| s.to_string()).collect(),
        message: message.to_string(),
    }
}

pub(super) fn have_git() -> bool {
    let ok = Command::new("git").arg("--version").output().is_ok();
    if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
        panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
    }
    ok
}

/// HEAD peeled to a commit oid (None on unborn HEAD).
pub(super) fn head_oid(p: &Path) -> Option<git2::Oid> {
    let repo = open_workdir_repo(p).expect("open");
    let oid = repo
        .head()
        .ok()
        .and_then(|h| h.peel_to_commit().ok())
        .map(|c| c.id());
    oid
}

/// Total commits reachable from HEAD (0 when unborn).
pub(super) fn commit_count(p: &Path) -> usize {
    let repo = open_workdir_repo(p).expect("open");
    let head = repo
        .head()
        .ok()
        .and_then(|h| h.peel_to_commit().ok())
        .map(|c| c.id());
    match head {
        None => 0,
        Some(oid) => {
            let mut walk = repo.revwalk().expect("revwalk");
            walk.push(oid).expect("push");
            walk.count()
        }
    }
}

/// Sorted new-file paths of `oid`'s diff vs its first parent (root => vs the
/// empty tree) — i.e. the commit's delta-to-parent.
pub(super) fn delta_paths(p: &Path, oid: &str) -> Vec<String> {
    let repo = open_workdir_repo(p).expect("open");
    let commit = repo
        .find_commit(git2::Oid::from_str(oid).expect("oid"))
        .expect("commit");
    let tree = commit.tree().expect("tree");
    let parent_tree = commit.parent(0).ok().map(|pc| pc.tree().expect("ptree"));
    let diff = repo
        .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
        .expect("diff");
    let mut out: Vec<String> = Vec::new();
    diff.foreach(
        &mut |d, _| {
            if let Some(path) = d.new_file().path() {
                out.push(path.to_string_lossy().into_owned());
            }
            true
        },
        None,
        None,
        None,
    )
    .expect("foreach");
    out.sort();
    out
}

/// The tree oid the current on-disk index would write (fresh repo handle so no
/// cached index leaks across the assertion).
pub(super) fn index_tree(p: &Path) -> git2::Oid {
    let repo = open_workdir_repo(p).expect("open");
    let mut index = repo.index().expect("index");
    index.write_tree().expect("write_tree")
}

/// The HEAD commit's tree oid (panics on unborn — callers know HEAD exists).
pub(super) fn head_tree(p: &Path) -> git2::Oid {
    let repo = open_workdir_repo(p).expect("open");
    let id = repo
        .head()
        .expect("head")
        .peel_to_commit()
        .expect("commit")
        .tree()
        .expect("tree")
        .id();
    id
}

/// Names of the files `oid` changed vs its parent, via the `git` CLI
/// (`diff-tree`) — the oracle cross-check for the per-commit delta.
pub(super) fn git_delta_names(p: &Path, oid: &str) -> Vec<String> {
    let out = Command::new("git")
        .current_dir(p)
        .args(["diff-tree", "--no-commit-id", "--name-only", "-r", oid])
        .output()
        .expect("git diff-tree");
    let mut v: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    v.sort();
    v
}
