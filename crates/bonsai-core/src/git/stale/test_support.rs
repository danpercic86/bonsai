//! Shared test fixtures for the `stale` test modules. Extracted verbatim from
//! the former inline `mod tests`; `pub(super)` so both sibling test modules
//! reuse them.

#[allow(unused_imports)]
use super::*;
use std::process::Command;

pub(super) fn init(dir: &Path) -> git2::Repository {
    let repo = git2::Repository::init_opts(dir, git2::RepositoryInitOptions::new().initial_head("main"))
        .expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    drop(cfg);
    repo
}

/// Stage + commit `files` on the CURRENT branch (moves HEAD + worktree).
/// Returns the new HEAD oid.
pub(super) fn commit(dir: &Path, msg: &str, files: &[(&str, &str)]) -> git2::Oid {
    use crate::git::stage::stage_paths;
    for (name, content) in files {
        std::fs::write(dir.join(name), content).expect("write file");
    }
    stage_paths(
        dir,
        &files.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>(),
    )
    .expect("stage");
    crate::git::commit::create_commit(dir, msg, None, false).expect("commit");
    let repo = git2::Repository::open(dir).expect("open");
    let oid = repo.head().expect("HEAD").peel_to_commit().expect("peel").id();
    oid
}

/// Build a commit on `refname` from `parent`'s tree WITHOUT moving HEAD or
/// the worktree (== branches.rs `cbh_commit_on_ref`). Builds a divergent tip.
pub(super) fn commit_on_ref(
    repo: &git2::Repository,
    refname: &str,
    parent_oid: git2::Oid,
    files: &[(&str, &str)],
    msg: &str,
) -> git2::Oid {
    let parent = repo.find_commit(parent_oid).expect("parent commit");
    let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
    let mut tb = repo
        .treebuilder(Some(&parent.tree().expect("parent tree")))
        .expect("treebuilder");
    for (name, content) in files {
        let blob = repo.blob(content.as_bytes()).expect("blob");
        tb.insert(name, blob, 0o100644).expect("insert");
    }
    let tree = repo.find_tree(tb.write().expect("tree oid")).expect("tree");
    repo.commit(Some(refname), &sig, &sig, &format!("{msg}\n"), &tree, &[&parent])
        .expect("commit on ref")
}

/// Create a local branch `name` at `oid` (no checkout).
pub(super) fn branch_at(repo: &git2::Repository, name: &str, oid: git2::Oid) {
    let commit = repo.find_commit(oid).expect("find commit");
    repo.branch(name, &commit, false).expect("create branch");
}

/// True when local branch `name` still exists.
pub(super) fn branch_exists(dir: &Path, name: &str) -> bool {
    let repo = git2::Repository::open(dir).expect("open");
    let exists = repo.find_branch(name, git2::BranchType::Local).is_ok();
    exists
}

pub(super) fn have_git() -> bool {
    let ok = Command::new("git").arg("--version").output().is_ok();
    if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
        panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
    }
    ok
}
