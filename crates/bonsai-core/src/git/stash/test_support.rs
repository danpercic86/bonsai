//! Shared test fixtures for the `stash` test modules (scratch-repo init,
//! commit/read helpers, and the staged/unstaged index probes). Extracted
//! verbatim from the former inline `mod tests`; `pub(super)` so every sibling
//! test module reuses them.

#[allow(unused_imports)]
use super::*;

/// Init a scratch repo with a deterministic identity + autocrlf off (== p8_init).
pub(super) fn s9_init(dir: &Path) -> git2::Repository {
    let repo = git2::Repository::init(dir).expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    drop(cfg);
    repo
}

/// Stage + commit `files` on the CURRENT branch (moves HEAD + worktree).
pub(super) fn s9_commit(dir: &Path, msg: &str, files: &[(&str, &str)]) {
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
}

/// Build a commit on `refname` from `parent`'s tree WITHOUT moving HEAD or the
/// worktree (== p8_commit_on_ref). Used to build a divergent topic tip.
pub(super) fn s9_commit_on_ref(
    repo: &git2::Repository,
    refname: &str,
    parent: &git2::Commit,
    files: &[(&str, &str)],
    msg: &str,
) -> git2::Oid {
    let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
    let mut tb = repo
        .treebuilder(Some(&parent.tree().expect("parent tree")))
        .expect("treebuilder");
    for (name, content) in files {
        let blob = repo.blob(content.as_bytes()).expect("blob");
        tb.insert(name, blob, 0o100644).expect("insert");
    }
    let tree = repo.find_tree(tb.write().expect("tree oid")).expect("tree");
    repo.commit(Some(refname), &sig, &sig, &format!("{msg}\n"), &tree, &[parent])
        .expect("commit on ref")
}

pub(super) fn s9_head_oid(dir: &Path) -> String {
    let repo = git2::Repository::open(dir).expect("open");
    let oid = repo
        .head()
        .expect("HEAD")
        .peel_to_commit()
        .expect("peel")
        .id();
    oid.to_string()
}

pub(super) fn s9_read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).expect("read file")
}

/// Stage `names` (delegates to the real staging path used by the app).
pub(super) fn p34_stage(dir: &Path, names: &[&str]) {
    use crate::git::stage::stage_paths;
    stage_paths(
        dir,
        &names.iter().map(|n| n.to_string()).collect::<Vec<_>>(),
    )
    .expect("stage");
}

/// `git rm --cached <name>`: stage a deletion in the index while the file
/// STAYS on disk with its HEAD content (index.remove_path, no worktree touch).
pub(super) fn p34_rm_cached(dir: &Path, name: &str) {
    let repo = git2::Repository::open(dir).expect("open");
    let mut index = repo.index().expect("index");
    index.remove_path(Path::new(name)).expect("remove_path");
    index.write().expect("write index");
}

/// Paths whose INDEX differs from HEAD (== "staged" set). Empty ⇒ index==HEAD.
pub(super) fn p34_staged_paths(dir: &Path) -> Vec<String> {
    let repo = git2::Repository::open(dir).expect("open");
    let head_tree = repo
        .head()
        .expect("head")
        .peel_to_tree()
        .expect("head tree");
    let diff = repo
        .diff_tree_to_index(Some(&head_tree), None, None)
        .expect("diff tree->index");
    let mut v: Vec<String> = diff
        .deltas()
        .filter_map(|d| {
            d.new_file()
                .path()
                .or_else(|| d.old_file().path())
                .map(|p| p.to_string_lossy().replace('\\', "/"))
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Paths whose WORKTREE differs from the index (tracked "unstaged" set;
/// untracked files are excluded by libgit2's default).
pub(super) fn p34_unstaged_paths(dir: &Path) -> Vec<String> {
    let repo = git2::Repository::open(dir).expect("open");
    let diff = repo
        .diff_index_to_workdir(None, None)
        .expect("diff index->workdir");
    let mut v: Vec<String> = diff
        .deltas()
        .filter_map(|d| {
            d.new_file()
                .path()
                .or_else(|| d.old_file().path())
                .map(|p| p.to_string_lossy().replace('\\', "/"))
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

pub(super) fn p34_assert_index_clean(dir: &Path) {
    assert_eq!(
        p34_staged_paths(dir),
        Vec::<String>::new(),
        "index must equal HEAD (nothing staged)"
    );
}
