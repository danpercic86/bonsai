//! One-click undo — READ-ONLY reflog classifier (P60c contract §P60c).
//!
//! Mirrors P38's invariant (`reflog.rs`): this module reads HEAD reflog entry 0,
//! classifies the last HEAD-moving operation, and returns an [`UndoPlan`] naming
//! HOW to reverse it (target oid + reset mode + safety flags). It performs
//! **ZERO** mutation — no reset, no commit, no ref write. Execution is the
//! already-shipped `reset_branch` command, run behind an explicit confirm
//! dialog, so there is no new mutation primitive here.

use std::path::Path;

use crate::error::AppError;
use crate::git::autostash::is_dirty;
use crate::git::reset::ResetMode;
use crate::git::stage::open_workdir_repo;

/// The 40-zero oid marking a ref root — a freshly-born ref's `old_oid`, i.e. the
/// pre-image of the initial commit. An undo target of this cannot be expressed
/// by `reset_branch` (it would unborn HEAD), so it is reported as not undoable.
const ZERO_OID: &str = "0000000000000000000000000000000000000000";

/// Width of the short oid rendered in the confirm copy.
const SHORT_LEN: usize = 7;

/// Classified last-operation kind (drives the undo verb + reset mode). Wire:
/// camelCase, mirrored by the TS `UndoKind` union.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UndoKind {
    Commit,
    Amend,
    Merge,
    Rebase,
    FastForward,
    CherryPick,
    Revert,
    Reset,
    /// Not undoable in v1 (see reason); classified so the UI can explain why.
    BranchSwitch,
    /// Unrecognized reflog message.
    Unknown,
}

impl UndoKind {
    /// The reset mode that reverses this op, or `None` for the classes that are
    /// not undoable via `reset_branch` (BranchSwitch / Unknown). Mixed =
    /// ref-restore (worktree kept); Hard = full revert (worktree restored → the
    /// caller then requires a clean worktree).
    fn reset_mode(self) -> Option<ResetMode> {
        match self {
            UndoKind::Commit | UndoKind::Amend | UndoKind::Reset => Some(ResetMode::Mixed),
            UndoKind::Merge
            | UndoKind::Rebase
            | UndoKind::FastForward
            | UndoKind::CherryPick
            | UndoKind::Revert => Some(ResetMode::Hard),
            UndoKind::BranchSwitch | UndoKind::Unknown => None,
        }
    }
}

/// Plan for reversing the last HEAD-moving operation. Wire: camelCase, mirrored
/// by the TS `UndoPlan` interface.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoPlan {
    pub kind: UndoKind,
    /// Human summary from the reflog message, e.g. "commit: add feature".
    pub summary: String,
    /// Where undo would move the current branch (reflog `oldOid`). Full 40-hex.
    /// Empty string when there is nothing to undo / the target is the 40-zero root.
    pub target_oid: String,
    /// `short(target_oid)` for the confirm copy; "" when `target_oid` is empty.
    pub target_short: String,
    /// Reset mode to reverse this op: mixed (ref-restore, worktree kept) for
    /// Commit/Amend/Reset; hard (full revert) for Merge/Rebase/FastForward/
    /// CherryPick/Revert. `None` when `!undoable`.
    pub reset_mode: Option<ResetMode>,
    /// true only for the Hard classes — the frontend must refuse/warn while
    /// `worktree_dirty` (a hard reset would clobber new uncommitted work).
    pub requires_clean_worktree: bool,
    /// Current worktree dirtiness for the UI gate. TRACKED changes only
    /// (staged + unstaged), via the shipped `autostash::is_dirty`: `git reset
    /// --hard` preserves untracked files, so they cannot be clobbered and are
    /// excluded (mirrors git's autostash default).
    pub worktree_dirty: bool,
    /// Whether v1 can undo this op via `reset_branch`. false for BranchSwitch,
    /// Unknown, an empty reflog, and the initial commit (root/zero target).
    pub undoable: bool,
    /// Why not, when `!undoable` (shown as a disabled-button tooltip). None when undoable.
    pub reason: Option<String>,
}

/// Classify a HEAD reflog message by its PREFIX (first match wins — these are
/// the operation tags git itself writes). Normative table (contract §P60c):
///
/// | prefix                                   | kind         |
/// |------------------------------------------|--------------|
/// | `commit (amend)`                         | Amend        |
/// | `commit` (`: `, ` (initial): `, …)       | Commit       |
/// | `reset:`                                 | Reset        |
/// | `cherry-pick`                            | CherryPick   |
/// | `revert:`                                | Revert       |
/// | `rebase` (` `, ` (finish): `, ` -i …`)   | Rebase       |
/// | `pull: Fast-forward` / `merge: Fast-forward` / `pull ` | FastForward |
/// | `merge ` / `pull:`                       | Merge        |
/// | `checkout: moving from `                 | BranchSwitch |
/// | anything else                            | Unknown      |
pub(crate) fn classify(message: &str) -> UndoKind {
    // `commit (amend)` before the general `commit` catch: amend is a distinct
    // reversal (OQ4 — the amended message is discarded) and the plain-commit
    // branch would otherwise swallow it.
    if message.starts_with("commit (amend)") {
        UndoKind::Amend
    } else if message.starts_with("commit") {
        // `commit: `, `commit (initial): ` and (defensively) `commit (merge): `
        // all reverse identically — a mixed ref-restore.
        UndoKind::Commit
    } else if message.starts_with("reset:") {
        UndoKind::Reset
    } else if message.starts_with("cherry-pick") {
        UndoKind::CherryPick
    } else if message.starts_with("revert:") {
        UndoKind::Revert
    } else if message.starts_with("rebase") {
        // `rebase `, `rebase (finish): `, `rebase -i (…)`, `rebase (start): `, …
        UndoKind::Rebase
    } else if message.starts_with("pull: Fast-forward")
        || message.starts_with("merge: Fast-forward")
        || message.starts_with("pull ")
    {
        UndoKind::FastForward
    } else if message.starts_with("merge ") || message.starts_with("pull:") {
        // A merge that created a merge commit, or a pull that resolved to a merge.
        UndoKind::Merge
    } else if message.starts_with("checkout: moving from ") {
        UndoKind::BranchSwitch
    } else {
        UndoKind::Unknown
    }
}

/// The plan for "nothing to undo" (empty reflog / unborn HEAD). `dirty` is
/// carried through for the UI even though it is irrelevant when `!undoable`.
fn nothing_to_undo(dirty: bool) -> UndoPlan {
    UndoPlan {
        kind: UndoKind::Unknown,
        summary: String::new(),
        target_oid: String::new(),
        target_short: String::new(),
        reset_mode: None,
        requires_clean_worktree: false,
        worktree_dirty: dirty,
        undoable: false,
        reason: Some("nothing to undo".to_string()),
    }
}

/// `short(oid)` — the first [`SHORT_LEN`] hex chars, or "" for an empty oid.
fn short(oid: &str) -> String {
    oid.chars().take(SHORT_LEN).collect()
}

/// Blocking. READ-ONLY. Inspects HEAD reflog entry 0 and returns how to reverse
/// it. Never mutates. Empty reflog / unborn HEAD → `UndoPlan{ undoable:false,
/// kind:Unknown, reason:"nothing to undo" }`. Errors: `Git` | `NoRepo`.
pub fn describe_last_undo(workdir: &Path) -> Result<UndoPlan, AppError> {
    let repo = open_workdir_repo(workdir)?;

    // Worktree dirtiness (tracked, staged + unstaged) — the gate for a hard-reset
    // undo. Untracked files are excluded because `git reset --hard` preserves
    // them (mirrors autostash's default), so they cannot be clobbered.
    let dirty = is_dirty(&repo)?;

    let reflog = match repo.reflog("HEAD") {
        Ok(r) => r,
        // A HEAD that was never updated (unborn) has no reflog on disk → nothing
        // to undo (mirror `read_reflog`'s NotFound handling).
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(nothing_to_undo(dirty)),
        Err(e) => return Err(e.into()),
    };

    let Some(entry0) = reflog.iter().next() else {
        return Ok(nothing_to_undo(dirty));
    };

    let message = entry0
        .message_bytes()
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_default();
    let old_oid = entry0.id_old().to_string();
    let kind = classify(&message);

    // Target = the ref position BEFORE the last op (reflog oldOid). The 40-zero
    // root (the initial commit's pre-image) is not expressible by reset_branch.
    let is_root = old_oid == ZERO_OID;
    let target_oid = if is_root { String::new() } else { old_oid };
    let target_short = short(&target_oid);

    let reset_mode = kind.reset_mode();

    // Undoable = a mode-bearing class WITH a real (non-root) target. BranchSwitch
    // / Unknown (reset_mode None) and the initial commit (root target) are not.
    let (undoable, reason) = match (reset_mode, kind, is_root) {
        (_, UndoKind::BranchSwitch, _) => (
            false,
            Some(
                "switching branches isn't undone here — check out the previous branch instead"
                    .to_string(),
            ),
        ),
        (None, _, _) => (
            false,
            Some("the last operation isn't one Bonsai can undo automatically".to_string()),
        ),
        (Some(_), _, true) => (false, Some("cannot undo the initial commit".to_string())),
        (Some(_), _, false) => (true, None),
    };

    let requires_clean_worktree = undoable && matches!(reset_mode, Some(ResetMode::Hard));

    Ok(UndoPlan {
        kind,
        summary: message,
        target_oid,
        target_short,
        reset_mode: if undoable { reset_mode } else { None },
        requires_clean_worktree,
        worktree_dirty: dirty,
        undoable,
        reason,
    })
}

#[cfg(test)]
mod tests;
