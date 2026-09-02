//! P3c CLI-oracle conflict tests (contract §9, `conflict_cli.rs`).
//!
//! Twin-repo pattern: identical scripted fixtures (fixed dates -> identical
//! base oids); Bonsai starts its merge via `merge_branch`, the twin via
//! `git merge` (expected to fail with conflicts). Bonsai's conflict listing,
//! marker view, and resolution matrix are then compared against the CLI:
//! stage presence via `git ls-files -u`, stage-0 index via `git ls-files -s`,
//! and worktree bytes directly.
//!
//! All scratch repos live under `D:\Data\Temp\bonsai-scratch` (C: is full).
//! Each test skips (passes with a note) if `git` is not on PATH.

use std::collections::BTreeMap;

use bonsai_core::error::AppError;
use bonsai_core::git::conflict::{
    get_conflict, list_conflicts, resolve_conflict, ConflictKind, ConflictResolution,
    MAX_CONFLICT_BYTES,
};
use crate::common;
use crate::conflict_support::{cli_stage_presence, conflicted_pair, require_git, Fixture};

// ============================================================ §9.1 kind derivation vs ls-files -u

#[test]
fn conflict_kinds_and_stage_flags_match_cli_stage_presence() {
    require_git!();
    for f in [
        Fixture::BothModified,
        Fixture::BothAdded,
        Fixture::DeletedByUs,
        Fixture::DeletedByThem,
        Fixture::RenameDelete,
    ] {
        let (bonsai, twin, _paths) = conflicted_pair(f);

        let ours = list_conflicts(bonsai.path()).expect("list");
        let cli = cli_stage_presence(twin.path());

        let ours_map: BTreeMap<String, (bool, bool, bool)> = ours
            .iter()
            .map(|e| (e.path.clone(), (e.has_base, e.has_ours, e.has_theirs)))
            .collect();
        assert_eq!(
            ours_map,
            f.expected_presence(&cli),
            "fixture {f:?}: Bonsai stage presence differs from the oracle \
             (CLI `git ls-files -u`, or the documented libgit2 shape for RenameDelete)"
        );

        let entry = ours
            .iter()
            .find(|e| e.path == f.path())
            .unwrap_or_else(|| panic!("fixture {f:?}: no entry for {}", f.path()));
        assert_eq!(entry.kind, f.expected_kind(), "fixture {f:?}");

        // Sorted ascending by path bytes.
        let mut sorted = ours_map.keys().cloned().collect::<Vec<_>>();
        sorted.sort();
        assert_eq!(
            ours.iter().map(|e| e.path.clone()).collect::<Vec<_>>(),
            sorted
        );
    }
}

// ============================================================ §9.2 get_conflict

#[test]
fn marker_text_is_byte_identical_to_cli_worktree_file() {
    require_git!();
    let (bonsai, twin, _paths) = conflicted_pair(Fixture::BothModified);

    let view = get_conflict(bonsai.path(), "a.txt").expect("get_conflict");
    assert_eq!(view.kind, ConflictKind::BothModified);
    assert!(!view.binary && !view.too_large && !view.missing);

    let cli_bytes = std::fs::read(twin.path().join("a.txt")).expect("twin a.txt");
    assert_eq!(
        view.text,
        String::from_utf8_lossy(&cli_bytes).into_owned(),
        "marker view must be byte-identical to the CLI's conflicted worktree file"
    );
    assert!(view.text.contains("<<<<<<<") && view.text.contains("=======") && view.text.contains(">>>>>>>"));
}

#[test]
fn binary_too_large_and_missing_flags() {
    require_git!();
    // binary: NUL bytes in the worktree file.
    let (bonsai, _twin, _p) = conflicted_pair(Fixture::BothModified);
    std::fs::write(bonsai.path().join("a.txt"), b"\x00\x01binary blob").expect("write binary");
    let v = get_conflict(bonsai.path(), "a.txt").expect("get");
    assert!(v.binary && v.text.is_empty() && !v.too_large && !v.missing);

    // too_large: > 1 MiB.
    let (bonsai, _twin, _p) = conflicted_pair(Fixture::BothModified);
    std::fs::write(
        bonsai.path().join("a.txt"),
        vec![b'a'; MAX_CONFLICT_BYTES as usize + 1],
    )
    .expect("write huge");
    let v = get_conflict(bonsai.path(), "a.txt").expect("get");
    assert!(v.too_large && v.text.is_empty() && !v.binary && !v.missing);

    // missing: worktree file removed by hand.
    let (bonsai, _twin, _p) = conflicted_pair(Fixture::BothModified);
    std::fs::remove_file(bonsai.path().join("a.txt")).expect("remove");
    let v = get_conflict(bonsai.path(), "a.txt").expect("get");
    assert!(v.missing && v.text.is_empty() && !v.binary && !v.too_large);
}

#[test]
fn get_conflict_on_non_conflicted_path_errors() {
    require_git!();
    let (bonsai, _twin, _p) = conflicted_pair(Fixture::BothModified);
    let err = get_conflict(bonsai.path(), "does-not-exist.txt").expect_err("no conflict");
    match err {
        AppError::Git(m) => assert!(m.contains("has no conflict"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
}

// ============================================================ §9.4 guards

#[test]
fn resolve_guards() {
    require_git!();
    let (bonsai, _twin, _p) = conflicted_pair(Fixture::BothModified);
    let d = bonsai.path();

    // Non-conflicted path -> AppError::Git("... has no conflict").
    let err = resolve_conflict(d, "keep-me.txt", ConflictResolution::Ours).expect_err("none");
    match err {
        AppError::Git(m) => assert!(m.contains("has no conflict"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }

    // Escape path -> invalidName.
    for bad in ["../escape", "..\\escape", "C:\\Windows\\evil"] {
        let err = resolve_conflict(d, bad, ConflictResolution::Ours).expect_err("escape");
        assert!(
            matches!(err, AppError::InvalidName(_)),
            "path {bad:?}: expected InvalidName, got {err:?}"
        );
    }
}
