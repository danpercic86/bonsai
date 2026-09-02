//! P12 §6 CLI-oracle tests for `resolve_conflict_text` — the editor's single
//! save primitive (contract §1.2 / §6), split out of `conflict_cli.rs`.
//!
//! Fixtures and helpers live in `conflict_support.rs`.

use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::git::conflict::{list_conflicts, resolve_conflict_text};
use crate::common;
use crate::common::{assert_same_status, git};
use crate::conflict_support::{
    cli_index_snapshot, cli_stage_presence, conflicted_pair, require_git, worktree, write, Fixture,
};

// ============================================================ P12 §6 resolve_conflict_text oracle
//
// Oracle for the P12 editor's single save primitive `resolve_conflict_text`
// (contract §1.2 / §6 checklist last "Rust" item). BOTH view modes (unified &
// side-by-side) produce ONE resolved-text string; the backend just writes it to
// the worktree and `index.add_path`s it — semantically identical to the twin
// hand-editing the file then `git add`. We assert byte/oid parity: same staged
// stage-0 blob oid, same worktree bytes, same porcelain status, conflict gone
// on both sides.

/// Stage-0 blob oid of `path` via `git rev-parse :<path>` (fails if the path is
/// still conflicted — a conflicted path has no `:0:` entry).
fn cli_staged_oid(dir: &Path, path: &str) -> String {
    git(dir, &["rev-parse", &format!(":{path}")])
}

/// One `resolve_conflict_text` vs `git add` parity check on a `bothModified`
/// fixture, resolving `a.txt` with the given hand-merged content.
fn assert_text_resolution_matches_cli(hand_merged: &str) {
    let (bonsai, twin, _paths) = conflicted_pair(Fixture::BothModified);
    let path = "a.txt";

    // Sanity: both sides start conflicted at this path.
    assert!(
        list_conflicts(bonsai.path())
            .expect("list")
            .iter()
            .any(|e| e.path == path),
        "precondition: {path} must be conflicted before resolving"
    );

    // Bonsai: the library primitive under test.
    resolve_conflict_text(bonsai.path(), path, hand_merged).expect("resolve_conflict_text");
    // Twin: the real git equivalent — write the SAME bytes then `git add`.
    write(twin.path(), path, hand_merged);
    git(twin.path(), &["add", path]);

    // (a) conflict gone on BOTH sides.
    assert!(
        !list_conflicts(bonsai.path())
            .expect("list")
            .iter()
            .any(|e| e.path == path),
        "bonsai: {path} still conflicted after resolve_conflict_text"
    );
    assert!(
        !cli_stage_presence(bonsai.path()).contains_key(path),
        "bonsai: {path} still has conflict stages (ls-files -u)"
    );
    assert!(
        !cli_stage_presence(twin.path()).contains_key(path),
        "twin: {path} still has conflict stages (ls-files -u)"
    );

    // (b) staged stage-0 blob oid identical (both staged the identical bytes).
    let bonsai_oid = cli_staged_oid(bonsai.path(), path);
    let twin_oid = cli_staged_oid(twin.path(), path);
    assert_eq!(
        bonsai_oid, twin_oid,
        "staged blob oid differs from the CLI twin for content {hand_merged:?}"
    );

    // (c) full stage-0 index snapshot parity (mode + oid + stage for every path).
    assert_eq!(
        cli_index_snapshot(bonsai.path()),
        cli_index_snapshot(twin.path()),
        "index snapshot differs from the CLI twin for content {hand_merged:?}"
    );

    // (d) worktree bytes identical, and equal to the bytes we asked to write.
    assert_eq!(
        worktree(bonsai.path(), path),
        worktree(twin.path(), path),
        "worktree bytes differ from the CLI twin"
    );
    assert_eq!(
        worktree(bonsai.path(), path).as_deref(),
        Some(hand_merged.as_bytes()),
        "worktree bytes are not the verbatim hand-merged content"
    );

    // (e) porcelain status agreement (the standard cross-repo oracle).
    assert_same_status(bonsai.path(), twin.path());
}

#[test]
fn resolve_conflict_text_matches_cli_add() {
    require_git!();
    // A plausible hand merge that keeps both sides' lines (ours-then-theirs,
    // §0.4) — the common editor output. Every conflict marker is gone.
    assert_text_resolution_matches_cli("line1\nmain\ntopic\nline3\n");
    // A resolution that picks a single side.
    assert_text_resolution_matches_cli("line1\nmain\nline3\n");
    // Fully bespoke resolved text unrelated to either side.
    assert_text_resolution_matches_cli("completely rewritten by hand\n");
    // No trailing newline — verbatim bytes, no normalization (§1.2 step 7).
    assert_text_resolution_matches_cli("no-trailing-newline");
}

#[test]
fn resolve_conflict_text_with_leftover_markers_stages_like_git_add() {
    require_git!();
    // Trust model (contract §1.2 leftover-marker decision): content that STILL
    // contains `<<<<<<<` is accepted verbatim and stages at stage 0, exactly as
    // `git add` would stage a file the user left markers in. The frontend gates
    // Save on `hasUnresolvedMarkers`, so this never happens through the UI — but
    // the primitive must not second-guess the caller. Verified against the twin.
    let leftover = "line1\n<<<<<<< HEAD\nmain\n=======\ntopic\n>>>>>>> topic\nline3\n";
    assert_text_resolution_matches_cli(leftover);
}

#[test]
fn resolve_conflict_text_non_conflicted_path_errors() {
    require_git!();
    let (bonsai, _twin, _p) = conflicted_pair(Fixture::BothModified);
    // A path with no conflict entry (find_conflict guard fires first).
    let err = resolve_conflict_text(bonsai.path(), "not-conflicted.txt", "x\n")
        .expect_err("no conflict");
    match err {
        AppError::Git(m) => assert!(m.contains("has no conflict"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
}

#[test]
fn resolve_conflict_text_escape_path_errors() {
    require_git!();
    let (bonsai, _twin, _p) = conflicted_pair(Fixture::BothModified);
    for bad in ["../escape", "..\\escape", "C:\\Windows\\evil"] {
        let err = resolve_conflict_text(bonsai.path(), bad, "x\n").expect_err("escape");
        assert!(
            matches!(err, AppError::InvalidName(_)),
            "path {bad:?}: expected InvalidName, got {err:?}"
        );
    }
}
