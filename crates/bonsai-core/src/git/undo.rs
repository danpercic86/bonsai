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
mod tests {
    use super::*;
    use std::path::Path;
    use std::process::Command;

    // ------------------------------------------------------------------ wire

    /// `UndoPlan` serializes with EXACTLY the camelCase keys the TS wire type
    /// declares (guards the TS `UndoPlan`/`UndoKind`). Covers a mixed undoable
    /// plan, a hard plan, and a not-undoable plan (resetMode → null).
    #[test]
    fn undo_plan_wire_shape_is_camel_case() {
        let mixed = serde_json::to_value(UndoPlan {
            kind: UndoKind::Commit,
            summary: "commit: add feature".to_string(),
            target_oid: "abc".to_string(),
            target_short: "abc".to_string(),
            reset_mode: Some(ResetMode::Mixed),
            requires_clean_worktree: false,
            worktree_dirty: true,
            undoable: true,
            reason: None,
        })
        .expect("json");
        assert_eq!(
            mixed,
            serde_json::json!({
                "kind": "commit",
                "summary": "commit: add feature",
                "targetOid": "abc",
                "targetShort": "abc",
                "resetMode": "mixed",
                "requiresCleanWorktree": false,
                "worktreeDirty": true,
                "undoable": true,
                "reason": null,
            })
        );

        let hard = serde_json::to_value(UndoPlan {
            kind: UndoKind::Merge,
            summary: "merge feature".to_string(),
            target_oid: "def".to_string(),
            target_short: "def".to_string(),
            reset_mode: Some(ResetMode::Hard),
            requires_clean_worktree: true,
            worktree_dirty: false,
            undoable: true,
            reason: None,
        })
        .expect("json");
        assert_eq!(hard["kind"], "merge");
        assert_eq!(hard["resetMode"], "hard");
        assert_eq!(hard["requiresCleanWorktree"], true);

        let blocked = serde_json::to_value(nothing_to_undo(false)).expect("json");
        assert_eq!(blocked["kind"], "unknown");
        assert_eq!(blocked["undoable"], false);
        assert_eq!(blocked["resetMode"], serde_json::Value::Null);
        assert_eq!(blocked["reason"], "nothing to undo");
    }

    /// Every `UndoKind` variant serializes to the exact camelCase wire string.
    #[test]
    fn undo_kind_wire_strings() {
        let cases = [
            (UndoKind::Commit, "commit"),
            (UndoKind::Amend, "amend"),
            (UndoKind::Merge, "merge"),
            (UndoKind::Rebase, "rebase"),
            (UndoKind::FastForward, "fastForward"),
            (UndoKind::CherryPick, "cherryPick"),
            (UndoKind::Revert, "revert"),
            (UndoKind::Reset, "reset"),
            (UndoKind::BranchSwitch, "branchSwitch"),
            (UndoKind::Unknown, "unknown"),
        ];
        for (kind, wire) in cases {
            assert_eq!(serde_json::to_value(kind).expect("json"), serde_json::json!(wire));
        }
    }

    // ----------------------------------------------------- classifier table

    /// The normative prefix → kind truth-table over synthetic reflog messages
    /// (the exact tags git writes). First-match-wins.
    #[test]
    fn classify_truth_table() {
        let cases: &[(&str, UndoKind)] = &[
            ("commit: add feature", UndoKind::Commit),
            ("commit (initial): base", UndoKind::Commit),
            ("commit (merge): resolve", UndoKind::Commit),
            ("commit (amend): tidy message", UndoKind::Amend),
            ("merge feature: Merge made by the 'ort' strategy.", UndoKind::Merge),
            ("pull: Merge made by the 'ort' strategy.", UndoKind::Merge),
            ("rebase (finish): returning to refs/heads/main", UndoKind::Rebase),
            ("rebase -i (finish): returning to refs/heads/main", UndoKind::Rebase),
            ("rebase (start): checkout main", UndoKind::Rebase),
            ("pull: Fast-forward", UndoKind::FastForward),
            ("merge: Fast-forward", UndoKind::FastForward),
            ("pull origin main: Fast-forward", UndoKind::FastForward),
            ("cherry-pick: add feature", UndoKind::CherryPick),
            ("revert: Revert \"add feature\"", UndoKind::Revert),
            ("reset: moving to HEAD~1", UndoKind::Reset),
            ("checkout: moving from feature to main", UndoKind::BranchSwitch),
            ("something totally unexpected", UndoKind::Unknown),
            ("", UndoKind::Unknown),
        ];
        for (msg, want) in cases {
            assert_eq!(classify(msg), *want, "classify({msg:?})");
        }
    }

    /// Mode mapping: Commit/Amend/Reset → Mixed; Merge/Rebase/FastForward/
    /// CherryPick/Revert → Hard; BranchSwitch/Unknown → None.
    #[test]
    fn reset_mode_per_kind() {
        assert_eq!(UndoKind::Commit.reset_mode(), Some(ResetMode::Mixed));
        assert_eq!(UndoKind::Amend.reset_mode(), Some(ResetMode::Mixed));
        assert_eq!(UndoKind::Reset.reset_mode(), Some(ResetMode::Mixed));
        assert_eq!(UndoKind::Merge.reset_mode(), Some(ResetMode::Hard));
        assert_eq!(UndoKind::Rebase.reset_mode(), Some(ResetMode::Hard));
        assert_eq!(UndoKind::FastForward.reset_mode(), Some(ResetMode::Hard));
        assert_eq!(UndoKind::CherryPick.reset_mode(), Some(ResetMode::Hard));
        assert_eq!(UndoKind::Revert.reset_mode(), Some(ResetMode::Hard));
        assert_eq!(UndoKind::BranchSwitch.reset_mode(), None);
        assert_eq!(UndoKind::Unknown.reset_mode(), None);
    }

    // ------------------------------------------------ describe_last_undo (git2)

    fn init_repo(dir: &Path) -> git2::Repository {
        let mut opts = git2::RepositoryInitOptions::new();
        opts.initial_head("main");
        let repo = git2::Repository::init_opts(dir, &opts).expect("init repo");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "Test User").expect("name");
            cfg.set_str("user.email", "test@example.com").expect("email");
        }
        repo
    }

    fn commit_file(dir: &Path, name: &str, content: &str, msg: &str) -> String {
        std::fs::write(dir.join(name), content).expect("write");
        crate::git::stage::stage_paths(dir, &[name.to_string()]).expect("stage");
        crate::git::commit::create_commit(dir, msg, None, false).expect("commit").oid
    }

    /// Empty reflog / unborn HEAD → not undoable, kind Unknown, "nothing to undo".
    #[test]
    fn describe_unborn_is_nothing_to_undo() {
        let dir = crate::testutil::scratch_dir();
        init_repo(dir.path());
        let plan = describe_last_undo(dir.path()).expect("describe");
        assert_eq!(plan.kind, UndoKind::Unknown);
        assert!(!plan.undoable);
        assert_eq!(plan.reason.as_deref(), Some("nothing to undo"));
        assert_eq!(plan.target_oid, "");
    }

    /// The FIRST commit's reflog[0] is `commit (initial): …` with a 40-zero
    /// oldOid → classified Commit but NOT undoable (initial-commit rule).
    #[test]
    fn describe_initial_commit_is_not_undoable() {
        let dir = crate::testutil::scratch_dir();
        init_repo(dir.path());
        commit_file(dir.path(), "a.txt", "one\n", "base");
        let plan = describe_last_undo(dir.path()).expect("describe");
        assert_eq!(plan.kind, UndoKind::Commit);
        assert!(!plan.undoable, "initial commit is not undoable");
        assert_eq!(plan.reason.as_deref(), Some("cannot undo the initial commit"));
        assert_eq!(plan.target_oid, "", "root target is reported as empty");
        assert_eq!(plan.reset_mode, None);
    }

    /// A SECOND commit → Commit / Mixed / undoable, target = the first commit
    /// (reflog oldOid). Mixed classes are undoable even when the tree is dirty.
    #[test]
    fn describe_second_commit_is_mixed_undoable() {
        let dir = crate::testutil::scratch_dir();
        let path = dir.path();
        init_repo(path);
        let c1 = commit_file(path, "a.txt", "one\n", "base");
        commit_file(path, "a.txt", "two\n", "edit");
        let plan = describe_last_undo(path).expect("describe");
        assert_eq!(plan.kind, UndoKind::Commit);
        assert!(plan.undoable);
        assert_eq!(plan.reset_mode, Some(ResetMode::Mixed));
        assert!(!plan.requires_clean_worktree);
        assert_eq!(plan.target_oid, c1, "target = the previous HEAD (reflog oldOid)");
        assert_eq!(plan.target_short, &c1[..SHORT_LEN]);
    }

    // ---------------------------------------------------- CLI oracle (real git)

    fn have_git() -> bool {
        let ok = Command::new("git").arg("--version").output().is_ok();
        if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
            panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
        }
        ok
    }

    /// Run `git <args>` in `dir`, asserting success; returns trimmed stdout.
    fn run_git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .current_dir(dir)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args(args)
            .output()
            .expect("spawn git");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// CLI oracle: build a scratch repo doing commit → merge → reset → branch
    /// switch and assert `describe_last_undo` classifies each LATEST op with the
    /// right kind and target (target == `git rev-parse HEAD@{1}`, i.e. the
    /// reflog oldOid), matching the real git binary.
    #[test]
    fn describe_matches_git_reflog_oracle() {
        if !have_git() {
            eprintln!("skipping describe_matches_git_reflog_oracle: git not on PATH");
            return;
        }
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();

        run_git(d, &["init", "-q"]);
        run_git(d, &["config", "user.name", "Test User"]);
        run_git(d, &["config", "user.email", "test@example.com"]);
        run_git(d, &["config", "commit.gpgsign", "false"]);
        // `symbolic-ref` (not `rev-parse --abbrev-ref`) resolves the branch name
        // while HEAD is still unborn (before the first commit). main | master.
        let main = run_git(d, &["symbolic-ref", "--short", "HEAD"]);

        // --- commit (initial) → not undoable (root oldOid)
        std::fs::write(d.join("a.txt"), "one\n").expect("write");
        run_git(d, &["add", "a.txt"]);
        run_git(d, &["commit", "-q", "-m", "c1"]);
        let p = describe_last_undo(d).expect("describe c1");
        assert_eq!(p.kind, UndoKind::Commit);
        assert!(!p.undoable, "initial commit is not undoable");

        // --- second commit → Commit / Mixed / undoable, target == HEAD@{1}
        std::fs::write(d.join("a.txt"), "two\n").expect("write");
        run_git(d, &["add", "a.txt"]);
        run_git(d, &["commit", "-q", "-m", "c2"]);
        let p = describe_last_undo(d).expect("describe c2");
        assert_eq!(p.kind, UndoKind::Commit);
        assert!(p.undoable);
        assert_eq!(p.reset_mode, Some(ResetMode::Mixed));
        assert_eq!(p.target_oid, run_git(d, &["rev-parse", "HEAD@{1}"]));

        // --- diverge & merge → Merge / Hard, target == HEAD@{1} (pre-merge tip)
        run_git(d, &["checkout", "-q", "-b", "feature"]);
        std::fs::write(d.join("b.txt"), "feat\n").expect("write");
        run_git(d, &["add", "b.txt"]);
        run_git(d, &["commit", "-q", "-m", "c3 on feature"]);
        run_git(d, &["checkout", "-q", &main]);
        std::fs::write(d.join("a.txt"), "three\n").expect("write");
        run_git(d, &["add", "a.txt"]);
        run_git(d, &["commit", "-q", "-m", "c4 on main"]);
        let pre_merge = run_git(d, &["rev-parse", "HEAD"]);
        run_git(d, &["merge", "--no-ff", "--no-edit", "-q", "feature"]);
        let p = describe_last_undo(d).expect("describe merge");
        assert_eq!(p.kind, UndoKind::Merge, "reflog msg: {:?}", p.summary);
        assert_eq!(p.reset_mode, Some(ResetMode::Hard));
        assert!(p.requires_clean_worktree);
        assert!(p.undoable);
        assert_eq!(p.target_oid, run_git(d, &["rev-parse", "HEAD@{1}"]));
        assert_eq!(p.target_oid, pre_merge, "undo target == pre-merge tip");

        // --- reset → Reset / Mixed, target == HEAD@{1} (pre-reset tip)
        let pre_reset = run_git(d, &["rev-parse", "HEAD"]);
        run_git(d, &["reset", "-q", "--soft", "HEAD~1"]);
        let p = describe_last_undo(d).expect("describe reset");
        assert_eq!(p.kind, UndoKind::Reset, "reflog msg: {:?}", p.summary);
        assert_eq!(p.reset_mode, Some(ResetMode::Mixed));
        assert!(p.undoable);
        assert_eq!(p.target_oid, run_git(d, &["rev-parse", "HEAD@{1}"]));
        assert_eq!(p.target_oid, pre_reset, "undo target == pre-reset tip");

        // --- branch switch → BranchSwitch, not undoable
        run_git(d, &["checkout", "-q", "feature"]);
        let p = describe_last_undo(d).expect("describe checkout");
        assert_eq!(p.kind, UndoKind::BranchSwitch, "reflog msg: {:?}", p.summary);
        assert!(!p.undoable);
        assert!(p.reason.is_some());
    }
}
