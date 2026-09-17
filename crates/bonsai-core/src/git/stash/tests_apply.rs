//! Apply/pop safety tests: the `is_windows_reserved` truth table, the P33b
//! reserved-name recovery tiers, and the checkout-blocked error path.
//! Extracted verbatim from the former inline `mod tests`; shared fixtures live
//! in `test_support`.

use super::test_support::*;
use super::*;

/// `is_windows_reserved` truth table: the reserved device names (any case,
/// with a trailing dot/space or an extension) match; near-misses do not.
#[test]
fn is_windows_reserved_truth_table() {
    for yes in [
        "NUL", "nul", "Nul", "NUL.txt", "NUL.", "NUL ", "CON", "PRN", "AUX", "COM1", "com9",
        "LPT1", "LPT9",
    ] {
        assert!(is_windows_reserved(yes), "{yes:?} must be reserved");
    }
    for no in [
        "NULl", "NULL2", "README", "COM", "COM0", "COM10", "LPT0", "LPT10", "NULfile", "myNUL", "",
    ] {
        assert!(!is_windows_reserved(no), "{no:?} must NOT be reserved");
    }
}

// =============================================== P33b reserved-name recovery
// Coverage for the Windows-reserved-path stash-apply fix (is_windows_reserved
// already truth-tabled above; wire shapes already covered). Three tiers:
//   A  stash_path_sets partitioning on SYNTHESIZED trees (cross-platform, no
//      real files — a `NUL` blob lives purely in the object DB);
//   B  the real git_stash_apply skip path, exercised end-to-end with an actual
//      on-disk `NUL` file (legal only on non-Windows → #[cfg(not(windows))]);
//   C  Windows detection + reflog-resolution via a fully synthesized stash
//      commit (a real `NUL` file cannot exist on NTFS → #[cfg(windows)]).

/// Build a tree from LEAF paths (forward-slash separators) via an in-memory
/// index — each becomes a blob entry, nested paths produce nested subtrees.
/// Synthesizes a stash's `^3` untracked tree (or a tracked stash tree) with a
/// reserved-name blob (e.g. `NUL`) that never touches the working directory,
/// so it lives purely in the object DB even on Windows.
fn rs_leaf_tree(repo: &git2::Repository, leaves: &[&str]) -> git2::Oid {
    let mut idx = git2::Index::new().expect("in-memory index");
    for p in leaves {
        let blob = repo
            .blob(format!("content:{p}\n").as_bytes())
            .expect("blob");
        let entry = make_index_entry(Path::new(p), blob, 0o100644).expect("entry");
        idx.add(&entry).expect("add");
    }
    idx.write_tree_to(repo).expect("write tree")
}

/// Register `oid` as stash@{0}: force-update `refs/stash` and guarantee EXACTLY
/// one reflog entry (mirrors create_staged_stash's log-once wiring — libgit2
/// auto-logs only when the reflog file already exists). This is what
/// stash_commit_oid / stash_path_sets / list_stashes resolve via reflog.get(0).
fn rs_register_stash(repo: &git2::Repository, oid: git2::Oid, msg: &str) {
    let before = repo.reflog("refs/stash").map(|r| r.len()).unwrap_or(0);
    repo.reference("refs/stash", oid, true, msg)
        .expect("force refs/stash");
    let after = repo.reflog("refs/stash").map(|r| r.len()).unwrap_or(0);
    if after == before {
        let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
        let mut reflog = repo.reflog("refs/stash").expect("reflog");
        reflog.append(oid, &sig, Some(msg)).expect("append");
        reflog.write().expect("write reflog");
    }
}

/// Synthesize + register a git-shaped 3-parent stash whose stash tree == base's
/// tree (no tracked delta) and whose `^3` untracked tree holds `untracked`
/// leaves. Parents: [base, index-commit, untracked-commit], mirroring git's
/// `stash_save --include-untracked` object shape.
fn rs_synth_untracked_stash(
    repo: &git2::Repository,
    base: &git2::Commit,
    untracked: &[&str],
    msg: &str,
) -> git2::Oid {
    let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
    let base_tree = base.tree().expect("base tree");
    let untracked_tree = repo
        .find_tree(rs_leaf_tree(repo, untracked))
        .expect("untracked tree");
    let untracked_commit = repo
        .find_commit(
            repo.commit(
                None,
                &sig,
                &sig,
                "untracked files on synthetic",
                &untracked_tree,
                &[base],
            )
            .expect("untracked commit"),
        )
        .expect("find untracked commit");
    let index_commit = repo
        .find_commit(
            repo.commit(None, &sig, &sig, "index on synthetic", &base_tree, &[base])
                .expect("index commit"),
        )
        .expect("find index commit");
    let stash_oid = repo
        .commit(
            None,
            &sig,
            &sig,
            msg,
            &base_tree,
            &[base, &index_commit, &untracked_commit],
        )
        .expect("stash commit");
    rs_register_stash(repo, stash_oid, msg);
    stash_oid
}

// ---- Tier A.1: partition an untracked ^3 tree (reserved vs allowed) --------

#[test]
fn rs_a_untracked_reserved_partition() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);
    let base = repo.head().expect("HEAD").peel_to_commit().expect("base");

    // ^3 untracked tree: three reserved leaves + three benign look-alikes.
    rs_synth_untracked_stash(
        &repo,
        &base,
        &[
            "src/x/NUL",
            "a/b/PRN",
            "COM1",
            "src/x/keep.txt",
            "NULl",
            "readme.md",
        ],
        "WIP on main: synthetic reserved stash",
    );

    let (reserved, allowed) = stash_path_sets(&repo, 0).expect("path sets");
    assert_eq!(
        reserved,
        vec![
            "COM1".to_string(),
            "a/b/PRN".to_string(),
            "src/x/NUL".to_string(),
        ],
        "reserved must be the sorted device-name leaves only"
    );
    for benign in ["NULl", "readme.md", "src/x/keep.txt"] {
        assert!(
            allowed.contains(&benign.to_string()),
            "{benign} must be in the allowed set, got {allowed:?}"
        );
    }
    // Leaf paths only — no directory prefixes ever leak into either set.
    for p in reserved.iter().chain(allowed.iter()) {
        assert!(
            !matches!(p.as_str(), "src" | "src/x" | "a" | "a/b"),
            "directory prefix leaked into a path set: {p}"
        );
    }
}

// ---- Tier A.2: a 2-parent stash has no ^3 → untracked walk skipped --------

#[test]
fn rs_a_two_parent_stash_no_reserved() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);
    let base = repo.head().expect("HEAD").peel_to_commit().expect("base");
    let base_tree = base.tree().expect("base tree");
    let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");

    // stash tree = base + one benign tracked add; parents = [base, index] (NO ^3).
    let stash_tree = {
        let mut tb = repo.treebuilder(Some(&base_tree)).expect("treebuilder");
        let blob = repo.blob(b"tracked\n").expect("blob");
        tb.insert("tracked.txt", blob, 0o100644).expect("insert");
        repo.find_tree(tb.write().expect("tree oid")).expect("tree")
    };
    let index_commit = repo
        .find_commit(
            repo.commit(None, &sig, &sig, "index on synthetic", &base_tree, &[&base])
                .expect("index commit"),
        )
        .expect("find index commit");
    let stash_oid = repo
        .commit(
            None,
            &sig,
            &sig,
            "WIP two-parent (no untracked)",
            &stash_tree,
            &[&base, &index_commit],
        )
        .expect("stash commit");
    rs_register_stash(&repo, stash_oid, "WIP two-parent (no untracked)");

    let (reserved, allowed) = stash_path_sets(&repo, 0).expect("path sets");
    assert!(
        reserved.is_empty(),
        "parent_count 2 → no ^3 walk → no reserved paths, got {reserved:?}"
    );
    assert_eq!(
        allowed,
        vec!["tracked.txt".to_string()],
        "only the tracked leaf is collected"
    );
}

// ---- Tier B: real-NUL end-to-end (the definitive skip test) ---------------
// `NUL` is a legal filename off Windows, so these exercise the actual
// git_stash_apply checkout path. Compiled + run on Linux CI; cfg'd out here.

#[cfg(not(windows))]
fn rs_b_tempdir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("bonsai-nul-")
        .tempdir()
        .expect("tempdir")
}

/// Build a scratch repo with an untracked `dir/NUL` + `dir/keep.txt` AND a
/// tracked modification, then `stash push -u`. Returns the live TempDir.
#[cfg(not(windows))]
fn rs_b_nul_stash_fixture() -> tempfile::TempDir {
    let dir = rs_b_tempdir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("tracked.txt", "base\n")]);

    std::fs::create_dir_all(d.join("dir")).expect("mkdir");
    std::fs::write(d.join("dir/NUL"), "nul-content\n").expect("write NUL");
    std::fs::write(d.join("dir/keep.txt"), "keep\n").expect("write keep");
    std::fs::write(d.join("tracked.txt"), "modified\n").expect("modify tracked");

    let res = create_stash(d, None, StashScope::AllWithUntracked).expect("create_stash -u");
    assert!(res.created, "dirty tree + untracked NUL must stash");
    dir
}

#[cfg(not(windows))]
#[test]
fn rs_b_apply_reserved_then_skip() {
    let dir = rs_b_nul_stash_fixture();
    let d = dir.path();

    // Attempt 1: skip_reserved=false → blocked, nothing applied, stash retained.
    match apply_stash(d, 0, false, None).expect("apply(false)") {
        ApplyStashOutcome::ReservedPaths { paths } => assert!(
            paths.iter().any(|p| p == "dir/NUL"),
            "ReservedPaths must name dir/NUL, got {paths:?}"
        ),
        other => panic!("expected ReservedPaths, got {other:?}"),
    }
    assert_eq!(
        s9_read(d, "tracked.txt"),
        "base\n",
        "tracked mod NOT applied"
    );
    assert!(
        !d.join("dir/keep.txt").exists(),
        "benign untracked NOT applied"
    );
    assert_eq!(list_stashes(d).expect("list").len(), 1, "stash retained");

    // Attempt 2: skip_reserved=true → applies everything but the NUL leaf.
    match apply_stash(d, 0, true, None).expect("apply(true)") {
        ApplyStashOutcome::AppliedSkippingReserved { skipped } => assert!(
            skipped.iter().any(|p| p == "dir/NUL"),
            "skipped must name dir/NUL, got {skipped:?}"
        ),
        other => panic!("expected AppliedSkippingReserved, got {other:?}"),
    }
    assert_eq!(
        s9_read(d, "dir/keep.txt"),
        "keep\n",
        "benign untracked restored"
    );
    assert_eq!(
        s9_read(d, "tracked.txt"),
        "modified\n",
        "tracked mod restored"
    );
    assert!(!d.join("dir/NUL").exists(), "reserved NUL NOT restored");
    assert_eq!(
        list_stashes(d).expect("list").len(),
        1,
        "apply must NOT drop the stash"
    );
}

#[cfg(not(windows))]
#[test]
fn rs_b_pop_skip_retains_stash() {
    let dir = rs_b_nul_stash_fixture();
    let d = dir.path();

    match pop_stash(d, 0, true, None).expect("pop(true)") {
        ApplyStashOutcome::AppliedSkippingReserved { skipped } => assert!(
            skipped.iter().any(|p| p == "dir/NUL"),
            "skipped must name dir/NUL, got {skipped:?}"
        ),
        other => panic!("expected AppliedSkippingReserved, got {other:?}"),
    }
    assert_eq!(
        s9_read(d, "dir/keep.txt"),
        "keep\n",
        "benign untracked restored"
    );
    assert_eq!(
        s9_read(d, "tracked.txt"),
        "modified\n",
        "tracked mod restored"
    );
    assert!(!d.join("dir/NUL").exists(), "reserved NUL NOT restored");
    assert_eq!(
        list_stashes(d).expect("list").len(),
        1,
        "DATA SAFETY: pop+skip must RETAIN the stash (reserved blobs live only there)"
    );
}

// ---- Tier C: Windows synthetic-stash detection (no real NUL possible) ------

#[cfg(windows)]
#[test]
fn rs_c_windows_synthetic_reserved_detection() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);
    let base = repo.head().expect("HEAD").peel_to_commit().expect("base");

    // ^3 untracked tree: an un-writable NUL blob + a benign keep.txt. The whole
    // stash is synthesized in the object DB — no file ever hits NTFS.
    rs_synth_untracked_stash(
        &repo,
        &base,
        &["dir/NUL", "dir/keep.txt"],
        "WIP on main: synthetic NUL stash",
    );

    // The `false` path validates Windows detection + reflog resolution without
    // needing the un-writable file: preflight blocks, mutating nothing.
    match apply_stash(d, 0, false, None).expect("apply(false)") {
        ApplyStashOutcome::ReservedPaths { paths } => assert!(
            paths.iter().any(|p| p == "dir/NUL"),
            "ReservedPaths must name dir/NUL, got {paths:?}"
        ),
        other => panic!("expected ReservedPaths, got {other:?}"),
    }
    assert!(
        !d.join("dir/keep.txt").exists(),
        "preflight must not write anything"
    );
    assert_eq!(list_stashes(d).expect("list").len(), 1, "stash retained");
}

// ===================================================== audit 2026-08-07

/// §3.2: a CHECKOUT-level GIT_ECONFLICT (a dirty file in the way; nothing
/// applied, no index conflict entries) must surface as `AppError::Git`
/// carrying libgit2's message — NOT as `Conflicts { paths: [] }`. The
/// stash is retained and the dirty file untouched.
#[test]
fn apply_blocked_at_checkout_errors_instead_of_empty_conflicts() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("f.txt", "base\n")]);

    // Stash a change (worktree reverts to base), then dirty the same file.
    std::fs::write(d.join("f.txt"), "stashed\n").expect("edit");
    assert!(
        create_stash(d, None, StashScope::All)
            .expect("stash")
            .created
    );
    std::fs::write(d.join("f.txt"), "dirty\n").expect("dirty");

    for (label, result) in [
        ("apply", apply_stash(d, 0, false, None)),
        ("pop", pop_stash(d, 0, false, None)),
    ] {
        let err = result.expect_err(&format!("{label} must error, not empty Conflicts"));
        assert!(
            matches!(&err, AppError::Git(m) if m.contains("blocked at checkout")),
            "{label}: got {err:?}"
        );
    }
    assert_eq!(list_stashes(d).expect("list").len(), 1, "stash retained");
    assert_eq!(
        std::fs::read_to_string(d.join("f.txt")).expect("read"),
        "dirty\n",
        "the blocking dirty file is untouched"
    );
}
