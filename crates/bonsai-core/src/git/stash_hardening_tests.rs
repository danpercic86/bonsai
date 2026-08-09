//! T2.6 hardening tests for `stash.rs` — kept in a sibling file (soft 500-line
//! limit; `stash.rs` already carries the P9/P34 matrices inline). Child module
//! of `stash` (declared with `#[path]`), so `super::*` reaches the private
//! helpers (`stash_path_sets`, `escape_pathspec`, `make_index_entry`, …).
//!
//! Covers: F-A6-A (staged-delete + rewritten worktree fold), F-A6-B
//! (expected_oid wrong-target guard), F-A6-E (non-UTF-8 path → hard error),
//! F-A6-F (glob-metachar allowlist self-match).

use super::*;

/// Scratch repo with deterministic identity + autocrlf off.
fn init(d: &Path) -> git2::Repository {
    let repo = git2::Repository::init(d).expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    drop(cfg);
    repo
}

/// Stage + commit `files` on the current branch.
fn commit(d: &Path, msg: &str, files: &[(&str, &str)]) {
    for (name, content) in files {
        std::fs::write(d.join(name), content).expect("write file");
    }
    crate::git::stage::stage_paths(
        d,
        &files.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>(),
    )
    .expect("stage");
    crate::git::commit::create_commit(d, msg, None, false).expect("commit");
}

fn read(d: &Path, name: &str) -> String {
    std::fs::read_to_string(d.join(name)).expect("read file")
}

/// `git rm --cached <name>`: stage a deletion while the file stays on disk.
fn rm_cached(d: &Path, name: &str) {
    let repo = git2::Repository::open(d).expect("open");
    let mut index = repo.index().expect("index");
    index.remove_path(Path::new(name)).expect("remove_path");
    index.write().expect("write index");
}

/// Paths whose INDEX differs from HEAD. Empty ⇒ nothing staged.
fn staged_paths(d: &Path) -> Vec<String> {
    let repo = git2::Repository::open(d).expect("open");
    let head_tree = repo.head().expect("head").peel_to_tree().expect("tree");
    let diff = repo
        .diff_tree_to_index(Some(&head_tree), None, None)
        .expect("diff");
    let mut v: Vec<String> = diff
        .deltas()
        .filter_map(|dl| {
            dl.new_file()
                .path()
                .or_else(|| dl.old_file().path())
                .map(|p| p.to_string_lossy().replace('\\', "/"))
        })
        .collect();
    v.sort();
    v
}

/// Blob content of `name` in stash@{0}'s tree, or None when absent.
fn stash_tree_blob(d: &Path, name: &str) -> Option<Vec<u8>> {
    let repo = git2::Repository::open(d).expect("open");
    let oid = stash_commit_oid(&repo, 0).expect("stash oid");
    let tree = repo.find_commit(oid).expect("commit").tree().expect("tree");
    let entry = tree.get_name(name)?;
    let content = repo.find_blob(entry.id()).expect("blob").content().to_vec();
    Some(content)
}

// ---- F-A6-A: staged deletion + rewritten worktree content -------------------

/// `git rm --cached` + rewrite the file on disk, then stash Staged: the
/// rewritten bytes must survive (FOLDED into the stash tree), never be
/// force-checked-out over. Pop restores them as an unstaged edit.
#[test]
fn staged_delete_plus_rewrite_folds_worktree_content() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    init(d);
    commit(d, "base", &[("a.txt", "base\n")]);

    rm_cached(d, "a.txt"); // staged deletion, file stays on disk
    std::fs::write(d.join("a.txt"), "rewritten\n").expect("rewrite");
    assert_eq!(staged_paths(d), vec!["a.txt".to_string()], "precondition");

    let res = create_stash(d, None, StashScope::Staged).expect("staged stash");
    assert!(res.created);

    // Post-stash: path back to HEAD, nothing staged — and the rewritten
    // content lives in the stash tree (the F-A6-A regression: it used to
    // exist NOWHERE).
    assert_eq!(read(d, "a.txt"), "base\n", "worktree reverted to HEAD");
    assert_eq!(staged_paths(d), Vec::<String>::new(), "index clean");
    assert_eq!(
        stash_tree_blob(d, "a.txt").as_deref(),
        Some(b"rewritten\n".as_slice()),
        "the rewritten worktree bytes must be FOLDED into the stash tree"
    );

    // Pop restores the rewritten content as an unstaged edit.
    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(outcome, ApplyStashOutcome::Applied);
    assert_eq!(read(d, "a.txt"), "rewritten\n", "content restored");
    assert_eq!(staged_paths(d), Vec::<String>::new(), "restored as UNSTAGED");
    assert_eq!(list_stashes(d).expect("list").len(), 0, "clean pop drops");
}

// ---- F-A6-B: expected_oid wrong-target guard --------------------------------

/// A stack shift between render and confirm must block apply/pop/drop with
/// "stash list changed; refresh and retry" — and a MATCHING oid still works.
#[test]
fn expected_oid_mismatch_blocks_apply_pop_drop() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    init(d);
    commit(d, "base", &[("a.txt", "base\n")]);

    // Stash A rendered by the "UI" at index 0…
    std::fs::write(d.join("a.txt"), "edit-A\n").expect("edit A");
    create_stash(d, Some("stash-A"), StashScope::All).expect("stash A");
    let oid_a = list_stashes(d).expect("list")[0].oid.clone();

    // …then a stack shift: stash B pushed on top (A moves to index 1).
    std::fs::write(d.join("a.txt"), "edit-B\n").expect("edit B");
    create_stash(d, Some("stash-B"), StashScope::All).expect("stash B");
    let oid_b = list_stashes(d).expect("list")[0].oid.clone();

    let expect_changed = |label: &str, r: Result<ApplyStashOutcome, AppError>| {
        let err = r.expect_err(label);
        assert!(
            matches!(&err, AppError::Git(m) if m == "stash list changed; refresh and retry"),
            "{label}: got {err:?}"
        );
    };
    expect_changed("apply", apply_stash(d, 0, false, Some(&oid_a)));
    expect_changed("pop", pop_stash(d, 0, false, Some(&oid_a)));
    let err = drop_stash(d, 0, Some(&oid_a)).expect_err("drop");
    assert!(
        matches!(&err, AppError::Git(m) if m == "stash list changed; refresh and retry"),
        "drop: got {err:?}"
    );
    assert_eq!(
        list_stashes(d).expect("list").len(),
        2,
        "NOTHING applied or dropped on a mismatch"
    );

    // Matching oid → the guard passes and the RIGHT entry is targeted.
    drop_stash(d, 0, Some(&oid_b)).expect("drop with matching oid");
    let after = list_stashes(d).expect("list");
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].oid, oid_a, "survivor is stash-A");
    let outcome = apply_stash(d, 0, false, Some(&oid_a)).expect("apply with matching oid");
    assert_eq!(outcome, ApplyStashOutcome::Applied);
    assert_eq!(read(d, "a.txt"), "edit-A\n");
}

/// Out-of-range index with an expected oid → clean error, nothing touched.
#[test]
fn expected_oid_on_missing_index_errors_cleanly() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    init(d);
    commit(d, "base", &[("a.txt", "base\n")]);

    let err = drop_stash(d, 0, Some(&"a".repeat(40))).expect_err("empty stack");
    assert!(matches!(err, AppError::Git(_)), "got {err:?}");
}

// ---- F-A6-E: non-UTF-8 stash path → hard error ------------------------------

/// Register `oid` as stash@{0} with exactly one reflog entry (mirrors
/// `create_staged_stash`'s log-once wiring).
fn register_stash(repo: &git2::Repository, oid: git2::Oid, msg: &str) {
    let before = repo.reflog("refs/stash").map(|r| r.len()).unwrap_or(0);
    repo.reference("refs/stash", oid, true, msg).expect("ref");
    let after = repo.reflog("refs/stash").map(|r| r.len()).unwrap_or(0);
    if after == before {
        let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
        let mut reflog = repo.reflog("refs/stash").expect("reflog");
        reflog.append(oid, &sig, Some(msg)).expect("append");
        reflog.write().expect("write");
    }
}

/// A synthesized stash whose ^3 untracked tree holds a NON-UTF-8 filename:
/// `stash_path_sets` (via the apply preflight) must ERROR — a silent drop from
/// the allowlist would make a skip-reserved apply silently not restore it.
#[test]
fn non_utf8_stash_path_errors_instead_of_silent_drop() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = init(d);
    commit(d, "base", &[("a.txt", "base\n")]);
    let base = repo.head().expect("HEAD").peel_to_commit().expect("base");
    let base_tree = base.tree().expect("base tree");
    let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");

    // ^3 untracked tree with a \xFF-bearing name — lives purely in the odb.
    let untracked_tree = {
        let mut tb = repo.treebuilder(None).expect("treebuilder");
        let blob = repo.blob(b"payload\n").expect("blob");
        tb.insert(&b"\xffbad.txt"[..], blob, 0o100644).expect("insert");
        repo.find_tree(tb.write().expect("tree oid")).expect("tree")
    };
    let untracked_commit = repo
        .find_commit(
            repo.commit(None, &sig, &sig, "untracked", &untracked_tree, &[&base])
                .expect("u commit"),
        )
        .expect("find u");
    let index_commit = repo
        .find_commit(
            repo.commit(None, &sig, &sig, "index", &base_tree, &[&base])
                .expect("i commit"),
        )
        .expect("find i");
    let stash_oid = repo
        .commit(
            None,
            &sig,
            &sig,
            "WIP non-utf8",
            &base_tree,
            &[&base, &index_commit, &untracked_commit],
        )
        .expect("stash commit");
    register_stash(&repo, stash_oid, "WIP non-utf8");

    for (label, r) in [
        ("path_sets", stash_path_sets(&repo, 0).map(|_| ())),
        ("apply", apply_stash(d, 0, false, None).map(|_| ())),
        ("apply-skip", apply_stash(d, 0, true, None).map(|_| ())),
    ] {
        let err = r.expect_err(label);
        assert!(
            matches!(&err, AppError::Git(m) if m.contains("non-unicode")),
            "{label}: got {err:?}"
        );
    }
    assert_eq!(list_stashes(d).expect("list").len(), 1, "stash retained");
}

// ---- F-A6-F: glob-metachar allowlist self-match ------------------------------

#[test]
fn escape_pathspec_truth_table() {
    assert_eq!(escape_pathspec("plain/path.txt"), None);
    assert_eq!(
        escape_pathspec("foo[1].txt").as_deref(),
        Some(r"foo\[1\].txt")
    );
    assert_eq!(escape_pathspec("a*b?c").as_deref(), Some(r"a\*b\?c"));
    assert_eq!(escape_pathspec(r"back\slash").as_deref(), Some(r"back\\slash"));
    assert_eq!(escape_pathspec("dir/[x]/f").as_deref(), Some(r"dir/\[x\]/f"));
}

/// End-to-end (non-Windows: a real `NUL` file is legal there): a skip-reserved
/// apply must restore an untracked path carrying fnmatch metacharacters —
/// pre-fix, the untracked-checkout phase treated `foo[1].txt` as a PATTERN
/// that does not self-match and silently skipped it.
#[cfg(not(windows))]
#[test]
fn skip_reserved_restores_metachar_untracked_path() {
    let dir = tempfile::Builder::new()
        .prefix("bonsai-meta-")
        .tempdir()
        .expect("tempdir");
    let d = dir.path();
    init(d);
    commit(d, "base", &[("tracked.txt", "base\n")]);

    std::fs::create_dir_all(d.join("dir")).expect("mkdir");
    std::fs::write(d.join("dir/NUL"), "nul-content\n").expect("write NUL");
    std::fs::write(d.join("foo[1].txt"), "meta\n").expect("write meta");

    let res = create_stash(d, None, StashScope::AllWithUntracked).expect("stash -u");
    assert!(res.created);
    assert!(!d.join("foo[1].txt").exists(), "stashed away");

    match apply_stash(d, 0, true, None).expect("apply(skip)") {
        ApplyStashOutcome::AppliedSkippingReserved { skipped } => {
            assert!(skipped.iter().any(|p| p == "dir/NUL"), "got {skipped:?}");
        }
        other => panic!("expected AppliedSkippingReserved, got {other:?}"),
    }
    assert_eq!(
        read(d, "foo[1].txt"),
        "meta\n",
        "metachar-bearing untracked path must be restored by the skip-apply"
    );
    assert!(!d.join("dir/NUL").exists(), "reserved path still skipped");
}
