//! Stash management (P9 contract §2). Wraps every git2 stash primitive:
//! list / create / apply / pop / drop.
//!
//! Pure git2 logic, no Tauri types (runtime-free cores → unit-testable without
//! the Tauri "test" feature, same rule as merge/rebase). All stash APIs
//! (`stash_foreach` / `stash_save2` / `stash_apply` / `stash_pop` /
//! `stash_drop`) require `&mut Repository`, so callers bind `let mut repo`.
//!
//! SAFE by construction: apply/pop use the SAFE checkout default (no
//! REINSTATE_INDEX, OPEN Q#4) and a conflicting apply/pop RETAINS the stash
//! (never lossy). Pop uses the P8 `apply + inspect index + conditional drop`
//! pattern (mirrors `merge::pop_after_success`) because this libgit2 may report
//! a *content* conflict as `Ok(())`, and a naive `stash_pop` would then silently
//! drop the entry — data loss.

use std::path::Path;

use crate::error::AppError;
use crate::git::commit::resolve_signature;
use crate::git::conflict::list_conflicts;
use crate::git::stage::open_workdir_repo;

/// One stash stack entry. Wire: camelCase.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StashEntry {
    /// Stack index; 0 == most recent (== stash@{0}). SHIFTS after any drop/pop.
    pub index: usize,
    /// Full stash message, e.g. "WIP on main: 1a2b3c4 summary" or a custom message.
    pub message: String,
    /// Full 40-hex oid of the stash commit itself.
    pub oid: String,
    /// Full 40-hex oid of the stash's FIRST parent = the base commit it was
    /// created from (what the graph pill attaches to).
    pub base_oid: String,
    /// Stash commit author time, seconds since epoch (UTC) — drives relative age.
    pub ts: i64,
}

/// Result of apply/pop. Wire: tagged "kind", camelCase (same recipe as MergeOutcome).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ApplyStashOutcome {
    /// Clean apply/pop. (Pop additionally dropped the entry.)
    Applied,
    /// Worktree has <<<<<<< markers, index has conflict entries, and the stash
    /// entry is RETAINED (libgit2 does not drop on GIT_ECONFLICT). `paths` =
    /// sorted conflicted paths (the set `list_conflicts` returns).
    Conflicts { paths: Vec<String> },
}

/// Result of create_stash.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStashResult {
    /// false == nothing to stash (clean worktree) → NOT an error.
    pub created: bool,
}

/// Shared precondition: create/apply/pop require a Clean repo state (no
/// in-progress merge/rebase). Drop is exempt (touches only the stash reflog).
fn require_clean(repo: &git2::Repository) -> Result<(), AppError> {
    if repo.state() != git2::RepositoryState::Clean {
        return Err(AppError::OperationInProgress(
            "an operation is already in progress — finish or abort it first".to_string(),
        ));
    }
    Ok(())
}

/// Blocking. Enumerate the stash stack, index 0 (most recent) first.
/// `stash_foreach` is the ONLY enumeration API; its callback receives
/// (index, message, &oid). Empty stack → Ok(vec![]).
///
/// The closure cannot re-borrow `repo` (it is mutably borrowed for the
/// duration), so we collect (index, message) inside it and resolve
/// oid/base_oid/ts AFTER via the stash reflog (`refs/stash`), where entry `i`'s
/// `id_new()` is the stash commit oid.
pub fn list_stashes(workdir: &Path) -> Result<Vec<StashEntry>, AppError> {
    let mut repo = open_workdir_repo(workdir)?;

    let mut raw: Vec<(usize, String)> = Vec::new();
    repo.stash_foreach(|index, message, _oid| {
        // NOTE: we intentionally do NOT capture oid here — resolving base/ts
        // needs an immutable repo borrow, impossible inside this &mut closure.
        raw.push((index, message.to_string()));
        true
    })?;

    if raw.is_empty() {
        return Ok(Vec::new());
    }

    // Resolve each entry via the stash reflog (index order; entry 0 == stash@{0}).
    let reflog = repo.reflog("refs/stash")?;
    let mut out = Vec::with_capacity(raw.len());
    for (index, message) in raw {
        let entry = reflog.get(index).ok_or_else(|| {
            AppError::Git(format!("stash reflog entry {index} missing"))
        })?;
        let stash_oid = entry.id_new();
        let commit = repo.find_commit(stash_oid)?;
        let base_oid = commit.parent_id(0)?;
        out.push(StashEntry {
            index,
            message,
            oid: commit.id().to_string(),
            base_oid: base_oid.to_string(),
            ts: commit.author().when().seconds(),
        });
    }
    Ok(out)
}

/// Blocking. Stash the dirty worktree. `message: None` → git default
/// ("WIP on <branch>: <short> <summary>"). Precondition: state Clean else
/// OperationInProgress. Nothing to stash → Ok(CreateStashResult{created:false}).
pub fn create_stash(
    workdir: &Path,
    message: Option<&str>,
    include_untracked: bool,
) -> Result<CreateStashResult, AppError> {
    let mut repo = open_workdir_repo(workdir)?;
    require_clean(&repo)?;

    // Identity is required to author the stash commit; surface ConfigMissing
    // early, consistent with commit/merge.
    let sig = resolve_signature(&repo.config()?.snapshot()?)?;

    let mut flags = git2::StashFlags::DEFAULT;
    if include_untracked {
        flags |= git2::StashFlags::INCLUDE_UNTRACKED;
    }

    match repo.stash_save2(&sig, message, Some(flags)) {
        Ok(_oid) => Ok(CreateStashResult { created: true }),
        // libgit2 returns GIT_ENOTFOUND when there is nothing to stash.
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            Ok(CreateStashResult { created: false })
        }
        Err(e) => Err(e.into()),
    }
}

/// Blocking. Apply stash `index` WITHOUT dropping it. Precondition: state Clean
/// else OperationInProgress. Conflicts → Ok(Conflicts{paths}) (stash retained).
pub fn apply_stash(workdir: &Path, index: usize) -> Result<ApplyStashOutcome, AppError> {
    let mut repo = open_workdir_repo(workdir)?;
    require_clean(&repo)?;

    let mut opts = git2::StashApplyOptions::new(); // SAFE default; NO REINSTATE_INDEX (OPEN Q#4)
    match repo.stash_apply(index, Some(&mut opts)) {
        Ok(()) => {
            // This libgit2 may report a CONTENT conflict as Ok(()) with markers
            // in the worktree + conflict entries in the index. Inspect the index
            // rather than trusting the return code (P8 lesson).
            if repo.index()?.has_conflicts() {
                Ok(ApplyStashOutcome::Conflicts {
                    paths: conflict_paths(workdir)?,
                })
            } else {
                Ok(ApplyStashOutcome::Applied)
            }
        }
        Err(e) if e.code() == git2::ErrorCode::Conflict => Ok(ApplyStashOutcome::Conflicts {
            paths: conflict_paths(workdir)?,
        }),
        Err(e) => Err(e.into()),
    }
}

/// Blocking. Apply stash `index` and drop it on clean success only.
/// Precondition: state Clean else OperationInProgress. Conflicts →
/// Ok(Conflicts{paths}) and the entry is RETAINED (libgit2 does not drop).
///
/// Mirrors `merge::pop_after_success`: uses `stash_apply` + inspect index +
/// conditional `stash_drop` so WE control dropping. A naive `stash_pop` would
/// silently drop the entry on this libgit2's false-clean content conflict —
/// data loss.
pub fn pop_stash(workdir: &Path, index: usize) -> Result<ApplyStashOutcome, AppError> {
    let mut repo = open_workdir_repo(workdir)?;
    require_clean(&repo)?;

    let mut opts = git2::StashApplyOptions::new(); // SAFE default; NO REINSTATE_INDEX (OPEN Q#4)
    match repo.stash_apply(index, Some(&mut opts)) {
        Ok(()) => {
            if repo.index()?.has_conflicts() {
                // Conflicted re-apply: LEAVE the stash on the stack (do NOT
                // drop) → retained for the user to resolve.
                Ok(ApplyStashOutcome::Conflicts {
                    paths: conflict_paths(workdir)?,
                })
            } else {
                // Clean apply → now drop, equivalent to a clean pop.
                repo.stash_drop(index)?;
                Ok(ApplyStashOutcome::Applied)
            }
        }
        // A checkout-level conflict means nothing droppable was applied →
        // stash retained.
        Err(e) if e.code() == git2::ErrorCode::Conflict => Ok(ApplyStashOutcome::Conflicts {
            paths: conflict_paths(workdir)?,
        }),
        Err(e) => Err(e.into()),
    }
}

/// Blocking. Permanently discard stash `index`. Allowed in ANY repo state
/// (touches only the stash reflog). UI confirms first (destructive). An
/// out-of-range index surfaces as the underlying git2 error → AppError::Git.
pub fn drop_stash(workdir: &Path, index: usize) -> Result<(), AppError> {
    let mut repo = open_workdir_repo(workdir)?;
    repo.stash_drop(index)?;
    Ok(())
}

/// Sorted conflicted paths — the exact set `list_conflicts` returns (P8 reuse).
fn conflict_paths(workdir: &Path) -> Result<Vec<String>, AppError> {
    Ok(list_conflicts(workdir)?
        .into_iter()
        .map(|c| c.path)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wire shapes (P9 §8 test 8): serde tag/casing must match the TS mirrors
    /// (ApplyStashOutcome union + StashEntry camelCase).
    #[test]
    fn wire_shapes_are_camel_case_tagged() {
        let v = serde_json::to_value(ApplyStashOutcome::Applied).expect("json");
        assert_eq!(v, serde_json::json!({ "kind": "applied" }));

        let v = serde_json::to_value(ApplyStashOutcome::Conflicts {
            paths: vec!["src/app.ts".to_string(), "README.md".to_string()],
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "conflicts", "paths": ["src/app.ts", "README.md"] })
        );

        let v = serde_json::to_value(StashEntry {
            index: 0,
            message: "WIP on main: 1a2b3c4 summary".to_string(),
            oid: "a".repeat(40),
            base_oid: "b".repeat(40),
            ts: 1_700_000_000,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "index": 0,
                "message": "WIP on main: 1a2b3c4 summary",
                "oid": "a".repeat(40),
                "baseOid": "b".repeat(40),
                "ts": 1_700_000_000
            })
        );

        let v = serde_json::to_value(CreateStashResult { created: true }).expect("json");
        assert_eq!(v, serde_json::json!({ "created": true }));
    }

    // ============================================================ P9 §8 matrix
    // Behavioral stash matrix (one test per §8 row, 1..=7). Each asserts BOTH the
    // returned outcome AND the on-disk state via a scratch repo, echoing the
    // merge.rs (P8) fixtures. Fixtures are built with git2 — deterministic, no
    // network, no CLI.

    /// Init a scratch repo with a deterministic identity + autocrlf off (== p8_init).
    fn s9_init(dir: &Path) -> git2::Repository {
        let repo = git2::Repository::init(dir).expect("init repo");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        drop(cfg);
        repo
    }

    /// Stage + commit `files` on the CURRENT branch (moves HEAD + worktree).
    fn s9_commit(dir: &Path, msg: &str, files: &[(&str, &str)]) {
        use crate::git::stage::stage_paths;
        for (name, content) in files {
            std::fs::write(dir.join(name), content).expect("write file");
        }
        stage_paths(
            dir,
            &files.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>(),
        )
        .expect("stage");
        crate::git::commit::create_commit(dir, msg).expect("commit");
    }

    /// Build a commit on `refname` from `parent`'s tree WITHOUT moving HEAD or the
    /// worktree (== p8_commit_on_ref). Used to build a divergent topic tip.
    fn s9_commit_on_ref(
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

    fn s9_head_oid(dir: &Path) -> String {
        let repo = git2::Repository::open(dir).expect("open");
        let oid = repo
            .head()
            .expect("HEAD")
            .peel_to_commit()
            .expect("peel")
            .id();
        oid.to_string()
    }

    fn s9_read(dir: &Path, name: &str) -> String {
        std::fs::read_to_string(dir.join(name)).expect("read file")
    }

    // ---- Row 1: Round-trip (apply does NOT drop) ---------------------------

    #[test]
    fn s9_1_round_trip_apply_keeps_stash() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n")]);
        let head = s9_head_oid(d);

        // Dirty: edit a tracked file (unstaged).
        std::fs::write(d.join("a.txt"), "edited\n").expect("edit");

        let res = create_stash(d, None, false).expect("create_stash");
        assert!(res.created, "dirty tracked edit must stash");

        // Worktree returned to the committed state (stash reset both index+worktree).
        assert_eq!(s9_read(d, "a.txt"), "base\n", "worktree must be clean after stash");

        let list = list_stashes(d).expect("list");
        assert_eq!(list.len(), 1, "one entry on the stack");
        assert_eq!(list[0].index, 0);
        assert_eq!(list[0].base_oid, head, "base_oid must == HEAD at stash time");
        assert!(!list[0].message.is_empty(), "default message must be non-empty");

        let outcome = apply_stash(d, 0).expect("apply");
        assert_eq!(outcome, ApplyStashOutcome::Applied, "clean apply");
        assert_eq!(s9_read(d, "a.txt"), "edited\n", "edit restored to worktree");

        let list = list_stashes(d).expect("list after apply");
        assert_eq!(list.len(), 1, "apply must NOT drop the stash");
    }

    // ---- Row 2: Pop drops --------------------------------------------------

    #[test]
    fn s9_2_pop_drops_stash() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n")]);

        std::fs::write(d.join("a.txt"), "edited\n").expect("edit");
        let res = create_stash(d, None, false).expect("create_stash");
        assert!(res.created);
        assert_eq!(s9_read(d, "a.txt"), "base\n");

        let outcome = pop_stash(d, 0).expect("pop");
        assert_eq!(outcome, ApplyStashOutcome::Applied, "clean pop");
        assert_eq!(s9_read(d, "a.txt"), "edited\n", "edit restored to worktree");

        let list = list_stashes(d).expect("list after pop");
        assert_eq!(list.len(), 0, "pop must drop the stash on clean apply");
    }

    // ---- Row 3: Nothing to stash -------------------------------------------

    #[test]
    fn s9_3_nothing_to_stash() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n")]);

        // Clean tree.
        let res = create_stash(d, None, false).expect("create_stash");
        assert!(!res.created, "clean tree -> created:false, NOT an error");

        let list = list_stashes(d).expect("list");
        assert_eq!(list.len(), 0, "nothing pushed onto the stack");
    }

    // ---- Row 4: Include untracked ------------------------------------------

    #[test]
    fn s9_4_include_untracked_round_trip() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n")]);

        // An untracked file present.
        std::fs::write(d.join("untracked.txt"), "new\n").expect("write untracked");

        let res = create_stash(d, None, true).expect("create_stash include_untracked");
        assert!(res.created, "untracked file must stash under include_untracked");
        assert!(
            !d.join("untracked.txt").exists(),
            "include_untracked must remove the untracked file from the worktree"
        );

        let list = list_stashes(d).expect("list");
        assert_eq!(list.len(), 1);

        let outcome = pop_stash(d, 0).expect("pop");
        assert_eq!(outcome, ApplyStashOutcome::Applied);
        assert!(
            d.join("untracked.txt").exists(),
            "pop must restore the stashed untracked file"
        );
        assert_eq!(s9_read(d, "untracked.txt"), "new\n", "untracked content restored");
        assert_eq!(list_stashes(d).expect("list").len(), 0, "clean pop drops entry");
    }

    // ---- Row 5: Pop conflict retains (P8-lesson data safety) + apply variant

    /// Build a repo where popping/applying stash@{0} necessarily conflicts on X:
    /// base has X="base"; stash records X="stashed"; then HEAD advances X="head".
    /// Returns the scratch dir (kept alive by the caller).
    fn s9_conflict_fixture() -> tempfile::TempDir {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("x.txt", "base\n")]);

        // Stash an unstaged edit to X.
        std::fs::write(d.join("x.txt"), "stashed\n").expect("edit x");
        let res = create_stash(d, None, false).expect("create_stash");
        assert!(res.created);
        assert_eq!(s9_read(d, "x.txt"), "base\n", "worktree reset after stash");

        // Advance HEAD with a DIFFERENT change to the same file -> guaranteed
        // 3-way conflict on re-apply (base=base, ours=head, theirs=stashed).
        s9_commit(d, "head change", &[("x.txt", "head\n")]);
        dir
    }

    #[test]
    fn s9_5_pop_conflict_retains_stash() {
        let dir = s9_conflict_fixture();
        let d = dir.path();

        let outcome = pop_stash(d, 0).expect("pop");
        assert_eq!(
            outcome,
            ApplyStashOutcome::Conflicts {
                paths: vec!["x.txt".to_string()]
            },
            "conflicting pop must report Conflicts on x.txt"
        );

        // No merge started; only the index carries conflict entries.
        let repo = git2::Repository::open(d).expect("reopen");
        assert_eq!(
            repo.state(),
            git2::RepositoryState::Clean,
            "stash apply must not enter a Merge state"
        );

        assert!(
            s9_read(d, "x.txt").contains("<<<<<<<"),
            "worktree X must carry conflict markers"
        );

        let list = list_stashes(d).expect("list after conflicting pop");
        assert_eq!(
            list.len(),
            1,
            "DATA SAFETY: a conflicting pop must RETAIN the stash (never lossy)"
        );
    }

    #[test]
    fn s9_5b_apply_conflict_retains_stash() {
        let dir = s9_conflict_fixture();
        let d = dir.path();

        let outcome = apply_stash(d, 0).expect("apply");
        assert_eq!(
            outcome,
            ApplyStashOutcome::Conflicts {
                paths: vec!["x.txt".to_string()]
            },
            "conflicting apply must report Conflicts on x.txt"
        );

        let repo = git2::Repository::open(d).expect("reopen");
        assert_eq!(repo.state(), git2::RepositoryState::Clean);
        assert!(s9_read(d, "x.txt").contains("<<<<<<<"));

        let list = list_stashes(d).expect("list after conflicting apply");
        assert_eq!(list.len(), 1, "apply never drops; stash retained on conflict");
    }

    // ---- Row 6: Drop re-indexes the stack ----------------------------------

    #[test]
    fn s9_6_drop_reindexes_stack() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n")]);

        // Stash A (becomes stash@{1} after the second push).
        std::fs::write(d.join("a.txt"), "edit-A\n").expect("edit A");
        create_stash(d, Some("stash-A"), false).expect("stash A");
        // Stash B (most recent, stash@{0}).
        std::fs::write(d.join("a.txt"), "edit-B\n").expect("edit B");
        create_stash(d, Some("stash-B"), false).expect("stash B");

        let before = list_stashes(d).expect("list before drop");
        assert_eq!(before.len(), 2);
        assert!(before[0].message.contains("stash-B"), "stash@{{0}} is the newest");
        assert!(before[1].message.contains("stash-A"), "stash@{{1}} is the oldest");
        let survivor_oid = before[1].oid.clone();

        // Drop the most recent (index 0). Entries above shift down by one.
        drop_stash(d, 0).expect("drop");

        let after = list_stashes(d).expect("list after drop");
        assert_eq!(after.len(), 1, "one entry survives");
        assert_eq!(after[0].index, 0, "surviving entry re-indexed to 0 (§2.4 shift)");
        assert!(
            after[0].message.contains("stash-A"),
            "the survivor is the entry previously at index 1 (stash-A)"
        );
        assert_eq!(after[0].oid, survivor_oid, "survivor identity confirmed by oid");
    }

    // ---- Row 7: Op-state guard (Merge in progress) -------------------------

    #[test]
    fn s9_7_op_state_guard_blocks_all_but_drop() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = s9_init(d);

        s9_commit(d, "base", &[("x.txt", "base\n"), ("y.txt", "y-base\n")]);
        let base = repo
            .find_commit(
                repo.head().expect("HEAD").target().expect("oid"),
            )
            .expect("base");
        // topic diverges on x.txt.
        s9_commit_on_ref(&repo, "refs/heads/topic", &base, &[("x.txt", "topic\n")], "topic edits x");
        // main diverges on x.txt (guaranteed conflict).
        s9_commit(d, "main edits x", &[("x.txt", "main\n")]);

        // Dirty unrelated file -> merge autostashes it, then pauses in Merge state.
        // This leaves BOTH a Merge state AND a retained stash on the stack, so we
        // can exercise the guard AND prove drop still works.
        std::fs::write(d.join("y.txt"), "y-edited\n").expect("edit y");

        crate::git::merge::merge_branch(d, "topic").expect("merge");
        let repo = git2::Repository::open(d).expect("reopen");
        assert_eq!(
            repo.state(),
            git2::RepositoryState::Merge,
            "conflicting merge over a dirty tree must pause in Merge state"
        );
        assert_eq!(
            list_stashes(d).expect("list").len(),
            1,
            "the retained autostash gives us something to drop"
        );

        // create/apply/pop are all rejected while an operation is in progress.
        match create_stash(d, None, false) {
            Err(AppError::OperationInProgress(_)) => {}
            other => panic!("create_stash: expected OperationInProgress, got {other:?}"),
        }
        match apply_stash(d, 0) {
            Err(AppError::OperationInProgress(_)) => {}
            other => panic!("apply_stash: expected OperationInProgress, got {other:?}"),
        }
        match pop_stash(d, 0) {
            Err(AppError::OperationInProgress(_)) => {}
            other => panic!("pop_stash: expected OperationInProgress, got {other:?}"),
        }

        // Drop is allowed in ANY repo state (touches only the stash reflog).
        drop_stash(d, 0).expect("drop must succeed mid-merge");
        assert_eq!(
            list_stashes(d).expect("list after drop").len(),
            0,
            "drop removed the autostash even though a merge is in progress"
        );
    }
}
