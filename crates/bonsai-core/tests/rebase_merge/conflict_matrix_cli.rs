//! P3c CLI-oracle conflict tests — the §3.2 resolution matrix (contract §9.3).
//!
//! Each cell applies one (kind, resolution) pair with Bonsai's
//! `resolve_conflict` and the CLI-equivalent action on the twin, then compares
//! the full stage-0 index snapshot and the worktree bytes. Fixtures and helpers
//! live in `conflict_support.rs`.

use std::path::Path;

use crate::common;
use crate::common::git;
use crate::conflict_support::{
    cli_index_snapshot, conflicted_pair, require_git, worktree, write, Fixture,
};
use bonsai_core::git::conflict::{list_conflicts, resolve_conflict, ConflictResolution};

// ============================================================ §9.3 resolution matrix

/// The CLI-equivalent action for one (kind, resolution) cell of the §3.2
/// matrix, applied to the twin repo.
#[derive(Debug, Clone, Copy)]
enum CliAction {
    CheckoutOursAdd,
    CheckoutTheirsAdd,
    Rm,
    /// Hand-edit the file to `HAND_EDIT` then `git add` (MarkResolved twin).
    EditAdd,
    /// Plain `git add` of the current worktree content (MarkResolved where
    /// the worktree already holds the surviving side).
    Add,
}

const HAND_EDIT: &str = "resolved by hand\n";

fn apply_cli(twin: &Path, path: &str, action: CliAction) {
    match action {
        CliAction::CheckoutOursAdd => {
            git(twin, &["checkout", "--ours", "--", path]);
            git(twin, &["add", path]);
        }
        CliAction::CheckoutTheirsAdd => {
            git(twin, &["checkout", "--theirs", "--", path]);
            git(twin, &["add", path]);
        }
        CliAction::Rm => {
            git(twin, &["rm", "-f", "--", path]);
        }
        CliAction::EditAdd => {
            write(twin, path, HAND_EDIT);
            git(twin, &["add", path]);
        }
        CliAction::Add => {
            git(twin, &["add", path]);
        }
    }
}

/// Runs one matrix cell on twin repos and asserts index + worktree parity.
/// `pre` mutates BOTH worktrees identically before resolving (for the
/// MarkResolved hand-edit / hand-delete variants).
fn run_cell(
    f: Fixture,
    resolution: ConflictResolution,
    cli: CliAction,
    pre: Option<fn(&Path, &str)>,
) {
    let (bonsai, twin, _paths) = conflicted_pair(f);
    let path = f.path();

    if let Some(pre) = pre {
        pre(bonsai.path(), path);
        pre(twin.path(), path);
    }

    resolve_conflict(bonsai.path(), path, resolution).expect("resolve");
    apply_cli(twin.path(), path, cli);

    // (a) no longer conflicted.
    assert!(
        !list_conflicts(bonsai.path())
            .expect("list")
            .iter()
            .any(|e| e.path == path),
        "{f:?} x {resolution:?}: path still conflicted after resolve"
    );
    // (b) full stage-0 index snapshot (mode + blob oid + stage) matches twin.
    assert_eq!(
        cli_index_snapshot(bonsai.path()),
        cli_index_snapshot(twin.path()),
        "{f:?} x {resolution:?}: index differs from the CLI twin"
    );
    // (c) worktree bytes match (or both absent).
    assert_eq!(
        worktree(bonsai.path(), path),
        worktree(twin.path(), path),
        "{f:?} x {resolution:?}: worktree differs from the CLI twin"
    );
}

#[test]
fn matrix_both_modified() {
    require_git!();
    use ConflictResolution as R;
    run_cell(
        Fixture::BothModified,
        R::Ours,
        CliAction::CheckoutOursAdd,
        None,
    );
    run_cell(
        Fixture::BothModified,
        R::Theirs,
        CliAction::CheckoutTheirsAdd,
        None,
    );
    run_cell(
        Fixture::BothModified,
        R::MarkResolved,
        CliAction::EditAdd,
        Some(|d, p| write(d, p, HAND_EDIT)),
    );
    // MarkResolved with a hand-DELETED worktree file -> resolved as removed.
    run_cell(
        Fixture::BothModified,
        R::MarkResolved,
        CliAction::Rm,
        Some(|d, p| std::fs::remove_file(d.join(p)).expect("hand delete")),
    );
}

#[test]
fn matrix_both_added() {
    require_git!();
    use ConflictResolution as R;
    run_cell(
        Fixture::BothAdded,
        R::Ours,
        CliAction::CheckoutOursAdd,
        None,
    );
    run_cell(
        Fixture::BothAdded,
        R::Theirs,
        CliAction::CheckoutTheirsAdd,
        None,
    );
    run_cell(
        Fixture::BothAdded,
        R::MarkResolved,
        CliAction::EditAdd,
        Some(|d, p| write(d, p, HAND_EDIT)),
    );
}

#[test]
fn matrix_deleted_by_us() {
    require_git!();
    use ConflictResolution as R;
    // Ours = keep our deletion.
    run_cell(Fixture::DeletedByUs, R::Ours, CliAction::Rm, None);
    run_cell(
        Fixture::DeletedByUs,
        R::Theirs,
        CliAction::CheckoutTheirsAdd,
        None,
    );
    // Worktree holds theirs' version after the merge -> MarkResolved = add it.
    run_cell(Fixture::DeletedByUs, R::MarkResolved, CliAction::Add, None);
}

#[test]
fn matrix_deleted_by_them() {
    require_git!();
    use ConflictResolution as R;
    run_cell(
        Fixture::DeletedByThem,
        R::Ours,
        CliAction::CheckoutOursAdd,
        None,
    );
    // Theirs = accept their deletion.
    run_cell(Fixture::DeletedByThem, R::Theirs, CliAction::Rm, None);
    // Worktree keeps ours' version -> MarkResolved = add it.
    run_cell(
        Fixture::DeletedByThem,
        R::MarkResolved,
        CliAction::Add,
        None,
    );
}
