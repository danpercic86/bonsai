use super::*;
use crate::error::AppError;
use crate::git::commit::create_commit;
use crate::git::stage::stage_paths;
use crate::git::status::FileStatus;

/// git2-init a scratch repo with identity + autocrlf off (mirrors the
/// other tests in this module).
fn init_scratch() -> tempfile::TempDir {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    dir
}

/// P5 §6.2: `compare_head_diff(HEAD, earlier)` on a LINEAR history. HEAD = B,
/// `to` = A. Going B -> A: file1 Modified, file2 Deleted (matches
/// `git diff --name-status HEAD A`). Endpoints carry oids + summaries.
#[test]
fn compare_head_diff_linear_history() {
    let dir = init_scratch();
    let p = dir.path();

    std::fs::write(p.join("file1.txt"), "one\n").expect("write");
    stage_paths(p, &["file1.txt".into()]).expect("stage");
    let a = create_commit(p, "A", None, false).expect("commit A").oid;

    std::fs::write(p.join("file1.txt"), "one changed\n").expect("write");
    std::fs::write(p.join("file2.txt"), "two\n").expect("write");
    stage_paths(p, &["file1.txt".into(), "file2.txt".into()]).expect("stage");
    let b = create_commit(p, "B", None, false).expect("commit B").oid;

    let cmp = compare_head_diff(p, &a).expect("compare");
    assert_eq!(cmp.to.oid, a);
    assert_eq!(cmp.to.summary, "A");
    assert_eq!(cmp.from.oid, b);
    assert_eq!(cmp.from.summary, "B");

    let got: Vec<(String, FileStatus)> = cmp
        .files
        .iter()
        .map(|f| (f.path.clone(), f.status))
        .collect();
    assert_eq!(
        got,
        vec![
            ("file1.txt".to_string(), FileStatus::Modified),
            ("file2.txt".to_string(), FileStatus::Deleted),
        ]
    );
}

/// P5 §6.2: `compare_head_diff(HEAD, branch_tip)` across diverged branches.
/// main tip B has file_main; feat tip C has file_feat. HEAD = B; `to` = C.
/// Going B -> C: file_feat Added, file_main Deleted (byte-sorted).
#[test]
fn compare_head_diff_branch_tip() {
    let dir = init_scratch();
    let p = dir.path();

    std::fs::write(p.join("file1.txt"), "base\n").expect("write");
    stage_paths(p, &["file1.txt".into()]).expect("stage");
    let base = create_commit(p, "A", None, false).expect("commit A");
    // Default branch name is git2's choice (master/main) — resolve it.
    let main_name = base.branch.expect("base commit is on a branch");

    // feat diverges from A.
    crate::git::branches::create_branch(p, "feat").expect("create feat");
    crate::git::branches::checkout_branch(p, "feat").expect("checkout feat");
    std::fs::write(p.join("file_feat.txt"), "feat\n").expect("write");
    stage_paths(p, &["file_feat.txt".into()]).expect("stage");
    let c = create_commit(p, "C", None, false).expect("commit C").oid;

    // Back to the default branch, add a divergent commit B (now HEAD = B).
    crate::git::branches::checkout_branch(p, &main_name).expect("checkout base branch");
    std::fs::write(p.join("file_main.txt"), "main\n").expect("write");
    stage_paths(p, &["file_main.txt".into()]).expect("stage");
    let b = create_commit(p, "B", None, false).expect("commit B").oid;

    let cmp = compare_head_diff(p, &c).expect("compare");
    assert_eq!(cmp.from.oid, b);
    assert_eq!(cmp.to.oid, c);

    let got: Vec<(String, FileStatus)> = cmp
        .files
        .iter()
        .map(|f| (f.path.clone(), f.status))
        .collect();
    assert_eq!(
        got,
        vec![
            ("file_feat.txt".to_string(), FileStatus::Added),
            ("file_main.txt".to_string(), FileStatus::Deleted),
        ]
    );
}

/// P5 §1.3 / §6.2: comparing HEAD to itself -> `from.oid == to.oid`, empty
/// `files`, and NOT an error.
#[test]
fn compare_head_to_itself_is_empty() {
    let dir = init_scratch();
    let p = dir.path();
    std::fs::write(p.join("a.txt"), "one\n").expect("write");
    stage_paths(p, &["a.txt".into()]).expect("stage");
    let a = create_commit(p, "A", None, false).expect("commit").oid;

    let cmp = compare_head_diff(p, &a).expect("compare HEAD to itself");
    assert_eq!(cmp.from.oid, cmp.to.oid);
    assert_eq!(cmp.from.oid, a);
    assert!(cmp.files.is_empty());
}

/// P5 §2.2 / §6.2: unborn HEAD -> `from == {"",""}`, old tree is empty, so
/// every file of `to` shows as Added (compare-vs-empty-tree).
#[test]
fn compare_unborn_head_shows_all_added() {
    let dir = init_scratch();
    let p = dir.path();
    std::fs::write(p.join("a.txt"), "one\n").expect("write");
    std::fs::write(p.join("b.txt"), "two\n").expect("write");
    stage_paths(p, &["a.txt".into(), "b.txt".into()]).expect("stage");
    let a = create_commit(p, "A", None, false).expect("commit").oid;

    // Force HEAD unborn: point it at a branch with no commit. Commit A
    // still lives in the object DB (reachable via refs/heads/main).
    {
        let repo = git2::Repository::open(p).expect("open");
        repo.set_head("refs/heads/does-not-exist")
            .expect("set_head unborn");
    }

    let cmp = compare_head_diff(p, &a).expect("compare from unborn HEAD");
    assert_eq!(cmp.from.oid, "");
    assert_eq!(cmp.from.summary, "");
    assert_eq!(cmp.to.oid, a);
    assert!(cmp.files.iter().all(|f| f.status == FileStatus::Added));
    let paths: Vec<String> = cmp.files.iter().map(|f| f.path.clone()).collect();
    assert_eq!(paths, vec!["a.txt".to_string(), "b.txt".to_string()]);
}

/// P5 §2.2 / §6.2: malformed, unknown, and non-commit oids all map to
/// `AppError::Git`.
#[test]
fn compare_bad_or_non_commit_oid_errors() {
    let dir = init_scratch();
    let p = dir.path();
    std::fs::write(p.join("a.txt"), "one\n").expect("write");
    stage_paths(p, &["a.txt".into()]).expect("stage");
    create_commit(p, "A", None, false).expect("commit");

    // Malformed hex.
    let err = compare_head_diff(p, "notahexoid").expect_err("malformed oid");
    assert!(matches!(err, AppError::Git(_)));

    // Well-formed but unknown.
    let unknown = "0123456789abcdef0123456789abcdef01234567";
    let err = compare_head_diff(p, unknown).expect_err("unknown oid");
    assert!(matches!(err, AppError::Git(_)));

    // Non-commit oid (a tree): find_commit must reject it.
    let tree_oid = {
        let repo = git2::Repository::open(p).expect("open");
        let head = repo.head().expect("head").peel_to_commit().expect("commit");
        head.tree_id().to_string()
    };
    let err = compare_head_diff(p, &tree_oid).expect_err("tree oid is not a commit");
    assert!(matches!(err, AppError::Git(_)));
}

/// Tree of the commit `oid` points at, for direct `collect_file_diffs` tests.
fn tree_of<'r>(repo: &'r git2::Repository, oid: &str) -> git2::Tree<'r> {
    repo.find_commit(git2::Oid::from_str(oid).expect("oid"))
        .expect("commit")
        .tree()
        .expect("tree")
}

/// P25 §2.1 / §9.1(2): a two-file tree-to-tree diff yields two `FileDiff`s
/// with correct paths/status/hunks, in delta order.
#[test]
fn collect_file_diffs_multi_file() {
    let dir = init_scratch();
    let p = dir.path();

    std::fs::write(p.join("a.txt"), "a1\n").expect("write");
    std::fs::write(p.join("b.txt"), "b1\n").expect("write");
    stage_paths(p, &["a.txt".into(), "b.txt".into()]).expect("stage");
    let base = create_commit(p, "base", None, false).expect("commit").oid;

    std::fs::write(p.join("a.txt"), "a1 changed\n").expect("write");
    std::fs::write(p.join("b.txt"), "b1 changed\n").expect("write");
    stage_paths(p, &["a.txt".into(), "b.txt".into()]).expect("stage");
    let head = create_commit(p, "head", None, false).expect("commit").oid;

    let repo = git2::Repository::open(p).expect("open");
    let old = tree_of(&repo, &base);
    let new = tree_of(&repo, &head);
    let mut opts = build_diff_options(&[], false);
    let mut diff = repo
        .diff_tree_to_tree(Some(&old), Some(&new), Some(&mut opts))
        .expect("diff");
    apply_find_similar(&mut diff).expect("find_similar");

    let files = collect_file_diffs(&diff).expect("collect");
    assert_eq!(files.len(), 2, "two changed files");
    // Delta order is byte-sorted path order for tree-to-tree.
    assert_eq!(files[0].path, "a.txt");
    assert_eq!(files[1].path, "b.txt");
    for fd in &files {
        assert_eq!(fd.status, FileStatus::Modified);
        assert!(!fd.binary && !fd.too_large);
        assert_eq!(fd.hunks.len(), 1, "one hunk per file: {}", fd.path);
    }
    // Empty diff (tree vs itself) => empty Vec.
    let mut same = repo
        .diff_tree_to_tree(Some(&new), Some(&new), None)
        .expect("same diff");
    apply_find_similar(&mut same).expect("find_similar");
    assert!(collect_file_diffs(&same).expect("collect").is_empty());
}

/// P25 §2.1 / §9.1(2): a too-large file is flagged `too_large` with empty
/// hunks while its siblings collect normally, all in one pass.
#[test]
fn collect_file_diffs_too_large_sibling() {
    let dir = init_scratch();
    let p = dir.path();

    // A huge new file (> MAX_FILE_DIFF_LINES added lines) + a small sibling.
    let big: String = (0..MAX_FILE_DIFF_LINES + 100)
        .map(|i| format!("line {i}\n"))
        .collect();
    std::fs::write(p.join("big.txt"), &big).expect("write big");
    std::fs::write(p.join("small.txt"), "small\n").expect("write small");
    stage_paths(p, &["big.txt".into(), "small.txt".into()]).expect("stage");
    let head = create_commit(p, "add files", None, false).expect("commit").oid;

    let repo = git2::Repository::open(p).expect("open");
    let new = tree_of(&repo, &head);
    let mut opts = build_diff_options(&[], false);
    // Root commit: diff vs empty (None old tree) => everything Added.
    let mut diff = repo
        .diff_tree_to_tree(None, Some(&new), Some(&mut opts))
        .expect("diff");
    apply_find_similar(&mut diff).expect("find_similar");

    let files = collect_file_diffs(&diff).expect("collect");
    assert_eq!(files.len(), 2);
    let big = files.iter().find(|f| f.path == "big.txt").expect("big");
    assert!(big.too_large, "big file over budget must be too_large");
    assert!(big.hunks.is_empty(), "too_large => empty hunks");
    let small = files.iter().find(|f| f.path == "small.txt").expect("small");
    assert!(!small.too_large, "small sibling collects normally");
    assert_eq!(small.hunks.len(), 1);
}

/// P5 §6.2: `compare_head_file_diff` hunks for one changed file. HEAD = B
/// (f.txt = "line1\nCHANGED"); `to` = A (f.txt = "line1\nline2"). The B -> A
/// diff deletes "CHANGED" and adds "line2".
#[test]
fn compare_head_file_diff_hunks_match() {
    let dir = init_scratch();
    let p = dir.path();

    std::fs::write(p.join("f.txt"), "line1\nline2\n").expect("write");
    stage_paths(p, &["f.txt".into()]).expect("stage");
    let a = create_commit(p, "A", None, false).expect("commit A").oid;

    std::fs::write(p.join("f.txt"), "line1\nCHANGED\n").expect("write");
    stage_paths(p, &["f.txt".into()]).expect("stage");
    create_commit(p, "B", None, false).expect("commit B");

    let fd = compare_head_file_diff(p, &a, "f.txt", None, false, false).expect("file diff");
    assert_eq!(fd.path, "f.txt");
    assert_eq!(fd.status, FileStatus::Modified);
    assert!(!fd.binary && !fd.too_large);
    assert_eq!(fd.hunks.len(), 1);

    let lines: Vec<(LineKind, &str)> = fd.hunks[0]
        .lines
        .iter()
        .map(|l| (l.kind, l.content.as_str()))
        .collect();
    assert!(lines.contains(&(LineKind::Del, "CHANGED")), "{lines:?}");
    assert!(lines.contains(&(LineKind::Add, "line2")), "{lines:?}");
    assert!(lines.contains(&(LineKind::Context, "line1")), "{lines:?}");
}

/// Audit 2026-08-07 §3.3: `collect_file_diff` must ERROR when the walked
/// diff contains two deltas with DIFFERENT paths (a pathspec that matched
/// twice), never merge both files' hunks into one corrupted FileDiff.
/// Exercised directly with an unrestricted two-file diff.
#[test]
fn collect_file_diff_refuses_second_delta() {
    let dir = init_scratch();
    let p = dir.path();

    std::fs::write(p.join("a.txt"), "a1\n").expect("write");
    std::fs::write(p.join("b.txt"), "b1\n").expect("write");
    stage_paths(p, &["a.txt".into(), "b.txt".into()]).expect("stage");
    create_commit(p, "base", None, false).expect("commit");
    std::fs::write(p.join("a.txt"), "a2\n").expect("edit a");
    std::fs::write(p.join("b.txt"), "b2\n").expect("edit b");

    let repo = git2::Repository::open(p).expect("open");
    let diff = repo
        .diff_index_to_workdir(None, None)
        .expect("two-delta diff");
    let err = collect_file_diff(&diff).expect_err("second delta must error");
    assert!(
        matches!(&err, AppError::Git(m) if m.contains("multiple")),
        "got {err:?}"
    );
}

/// Same-path re-entry must KEEP working: a single-file diff with two
/// far-apart edits (two hunks, one delta) still collects normally.
#[test]
fn collect_file_diff_same_path_two_hunks_still_collects() {
    let dir = init_scratch();
    let p = dir.path();

    let base: String = (1..=20).map(|i| format!("line{i}\n")).collect();
    std::fs::write(p.join("f.txt"), &base).expect("write");
    stage_paths(p, &["f.txt".into()]).expect("stage");
    create_commit(p, "base", None, false).expect("commit");
    let edited = base.replace("line2\n", "LINE2\n").replace("line19\n", "LINE19\n");
    std::fs::write(p.join("f.txt"), edited).expect("edit");

    let repo = git2::Repository::open(p).expect("open");
    let diff = repo
        .diff_index_to_workdir(None, None)
        .expect("one-delta diff");
    let fd = collect_file_diff(&diff)
        .expect("collect")
        .expect("one delta present");
    assert_eq!(fd.path, "f.txt");
    assert_eq!(fd.hunks.len(), 2, "two far-apart hunks, one file");
}
