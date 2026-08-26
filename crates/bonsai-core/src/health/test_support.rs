//! Shared test fixtures for the `health` test modules. Extracted verbatim from
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

/// Stage + commit `files` on the current branch; returns the new HEAD oid.
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

pub(super) fn branch_at(repo: &git2::Repository, name: &str, oid: git2::Oid) {
    let c = repo.find_commit(oid).expect("find commit");
    repo.branch(name, &c, false).expect("create branch");
}

pub(super) fn have_git() -> bool {
    let ok = Command::new("git").arg("--version").output().is_ok();
    if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
        panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
    }
    ok
}

pub(super) fn caps(revwalk: usize, workdir: usize) -> StatsCaps {
    StatsCaps {
        revwalk,
        odb: ODB_SCAN_CAP,
        workdir,
        gitdir: GITDIR_WALK_CAP,
    }
}
