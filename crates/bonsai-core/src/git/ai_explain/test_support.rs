//! Shared git2 fixtures for the `ai_explain` test modules (analyze + digest).
//! Extracted verbatim from the former inline `mod tests`; `pub(super)` so both
//! sibling test modules reuse them.

#[allow(unused_imports)]
use super::*;

/// git2-init a scratch repo with identity + autocrlf off (mirrors `diff.rs`).
pub(super) fn init_scratch() -> tempfile::TempDir {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    dir
}

pub(super) fn commit_of<'r>(repo: &'r git2::Repository, oid: &str) -> git2::Commit<'r> {
    repo.find_commit(git2::Oid::from_str(oid).expect("oid"))
        .expect("commit")
}
/// git2-only fixture helpers: commits with controlled committer times and
/// per-commit unique trees (no workdir writes needed).
pub(super) fn tree_with<'r>(repo: &'r git2::Repository, key: &str) -> git2::Tree<'r> {
    let blob = repo.blob(key.as_bytes()).expect("blob");
    let mut tb = repo.treebuilder(None).expect("treebuilder");
    tb.insert("f.txt", blob, 0o100644).expect("insert");
    let oid = tb.write().expect("tree write");
    repo.find_tree(oid).expect("find tree")
}

pub(super) fn commit_at(
    repo: &git2::Repository,
    update_ref: Option<&str>,
    msg: &str,
    secs: i64,
    parents: &[&git2::Commit<'_>],
) -> git2::Oid {
    let sig = git2::Signature::new("Test User", "test@example.com", &git2::Time::new(secs, 0))
        .expect("signature");
    let tree = tree_with(repo, msg);
    repo.commit(update_ref, &sig, &sig, msg, &tree, parents)
        .expect("commit")
}

/// Builds the §10.1(2) fixture: `main` = A→B, `feature` = B→C→D, HEAD on
/// feature. Returns (dir, [a, b, c, d]).
pub(super) fn digest_fixture() -> (tempfile::TempDir, [git2::Oid; 4]) {
    let dir = init_scratch();
    let repo = git2::Repository::open(dir.path()).expect("open");
    let t = 1_700_000_000i64;
    let a = commit_at(&repo, None, "A", t, &[]);
    let a_c = repo.find_commit(a).expect("A");
    let b = commit_at(&repo, None, "B", t + 10, &[&a_c]);
    let b_c = repo.find_commit(b).expect("B");
    let c = commit_at(&repo, None, "C", t + 20, &[&b_c]);
    let c_c = repo.find_commit(c).expect("C");
    let d = commit_at(&repo, None, "D", t + 30, &[&c_c]);
    let d_c = repo.find_commit(d).expect("D");
    repo.branch("main", &b_c, true).expect("main");
    repo.branch("feature", &d_c, true).expect("feature");
    repo.set_head("refs/heads/feature").expect("head");
    drop((a_c, b_c, c_c, d_c));
    (dir, [a, b, c, d])
}
