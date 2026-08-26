//! Shared git2 fixtures for the `ai_operation` test modules. Extracted verbatim
//! from the former inline `mod tests`; `pub(super)` so both sibling test modules
//! reuse them.

#[allow(unused_imports)]
use super::*;
use crate::git::commit::create_commit;
use crate::git::stage::stage_paths;

// ----------------------------------------------------------- fixtures

/// git2-init a scratch repo with identity + autocrlf off (mirrors ai_explain).
pub(super) fn init_scratch() -> tempfile::TempDir {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    dir
}

/// Commit `file`=`content` with `msg` on the current branch; returns full oid.
pub(super) fn commit(dir: &Path, file: &str, content: &str, msg: &str) -> String {
    std::fs::write(dir.join(file), content).expect("write");
    stage_paths(dir, &[file.to_string()]).expect("stage");
    create_commit(dir, msg, None, false).expect("commit").oid
}

pub(super) fn oid(s: &str) -> git2::Oid {
    git2::Oid::from_str(s).expect("oid")
}

/// Linear A→B repo (HEAD=B). Returns (dir, a_oid, b_oid).
pub(super) fn linear_repo() -> (tempfile::TempDir, String, String) {
    let dir = init_scratch();
    let p = dir.path();
    let a = commit(p, "a.txt", "a\n", "A");
    let b = commit(p, "b.txt", "b\n", "B");
    (dir, a, b)
}

/// Repo whose HEAD is a MERGE commit M with parents [A(main), B(feature)].
/// Uses A's tree for every commit so the worktree stays clean. Returns
/// (dir, a_oid, m_oid, head_branch_name).
pub(super) fn merge_repo() -> (tempfile::TempDir, String, String, String) {
    let dir = init_scratch();
    let p = dir.path();
    let a = commit(p, "a.txt", "a\n", "A");
    let repo = git2::Repository::open(p).expect("open");
    let head_branch = repo
        .head()
        .expect("head")
        .shorthand()
        .expect("shorthand")
        .to_string();
    let a_c = repo.find_commit(oid(&a)).expect("A");
    let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
    let tree = a_c.tree().expect("tree");
    let b = repo
        .commit(Some("refs/heads/feature"), &sig, &sig, "B", &tree, &[&a_c])
        .expect("feature commit");
    let b_c = repo.find_commit(b).expect("B");
    let m = repo
        .commit(
            Some(&format!("refs/heads/{head_branch}")),
            &sig,
            &sig,
            "Merge branch 'feature'",
            &tree,
            &[&a_c, &b_c],
        )
        .expect("merge commit");
    (dir, a, m.to_string(), head_branch)
}

/// Byte-snapshot of the repo state that a plan MUST NOT touch: HEAD oid, the
/// raw index file, and a worktree file.
pub(super) fn snapshot(p: &Path) -> (Option<String>, Vec<u8>, Vec<u8>) {
    let repo = git2::Repository::open(p).expect("open");
    let head = repo.head().ok().and_then(|r| r.target()).map(|o| o.to_string());
    let index = std::fs::read(repo.path().join("index")).unwrap_or_default();
    let file = std::fs::read(p.join("a.txt")).unwrap_or_default();
    (head, index, file)
}

pub(super) fn expect_unsupported(o: PlanOutcome) -> String {
    match o {
        PlanOutcome::Unsupported { reason, .. } => reason,
        other => panic!("expected Unsupported, got {other:?}"),
    }
}
