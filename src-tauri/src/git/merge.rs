//! Merge a local or remote-tracking branch into the current branch.
//! Clean merges auto-commit; conflicts pause into RepoOpState::Merge.
//! Pure git2, no Tauri types, no network (merging origin/x uses the local
//! remote-tracking ref — Fetch first is the user's job, same as GitKraken).
//! (P3c contract §4.)

use std::path::Path;

use crate::error::AppError;
use crate::git::commit::{resolve_signature, CommitResult};
use crate::git::conflict::list_conflicts;
use crate::git::repo::read_head_info;
use crate::git::stage::open_workdir_repo;

/// Wire: tagged "kind", camelCase (same recipe as PullResult).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum MergeOutcome {
    /// Incoming is already reachable from HEAD. Nothing changed.
    UpToDate,
    /// HEAD branch fast-forwarded to `to` (full oid). No merge commit.
    FastForwarded { branch: String, to: String },
    /// Clean merge, auto-committed. `oid` = the new 2-parent merge commit.
    Merged { oid: String },
    /// Conflicts recorded in index + worktree; MERGE_HEAD/MERGE_MSG written;
    /// repo paused in state Merge. Sorted conflicted paths (same set
    /// list_conflicts returns).
    Conflicts { paths: Vec<String> },
}

/// Prepared MERGE_MSG first line (contract §4.3, byte-exact for the oracle):
/// `Merge branch '<name>'` / `Merge remote-tracking branch '<name>'` —
/// no `into <branch>` suffix (locked decision §11.4).
fn prepared_merge_message(name: &str, incoming_is_remote: bool) -> String {
    if incoming_is_remote {
        format!("Merge remote-tracking branch '{name}'")
    } else {
        format!("Merge branch '{name}'")
    }
}

/// Blocking. Merges `branch_name` (local shorthand "feature/x" OR
/// remote-tracking shorthand "origin/main") into the current branch.
///
/// Preconditions (contract §4.1, checked in order BEFORE anything mutates):
/// state Clean; HEAD attached + born; branch resolvable (local then remote);
/// index matches HEAD (unstaged worktree changes ARE allowed — they only fail
/// as CheckoutConflict if the merge would overwrite them, in which case
/// nothing is left behind); git identity configured (a clean merge
/// auto-commits).
pub fn merge_branch(workdir: &Path, branch_name: &str) -> Result<MergeOutcome, AppError> {
    let mut repo = open_workdir_repo(workdir)?;

    if repo.state() != git2::RepositoryState::Clean {
        return Err(AppError::OperationInProgress(
            "an operation is already in progress — commit or abort it first".to_string(),
        ));
    }

    let head = read_head_info(&repo)?;
    if head.unborn {
        return Err(AppError::Git(
            "cannot merge: the repository has no commits yet".to_string(),
        ));
    }
    if head.detached {
        return Err(AppError::Git("cannot merge: HEAD is detached".to_string()));
    }
    let head_branch = head
        .branch_name
        .ok_or_else(|| AppError::Git("cannot merge: HEAD has no branch name".to_string()))?;

    // Resolve incoming: local first, then remote-tracking. Merging the
    // current branch by name falls out as UpToDate naturally.
    let (incoming, incoming_is_remote) =
        match repo.find_branch(branch_name, git2::BranchType::Local) {
            Ok(b) => (b, false),
            Err(_) => match repo.find_branch(branch_name, git2::BranchType::Remote) {
                Ok(b) => (b, true),
                Err(_) => {
                    return Err(AppError::BranchNotFound(format!(
                        "branch '{branch_name}' not found (local or remote-tracking)"
                    )));
                }
            },
        };

    // Dirty-index guard (git2 merge semantics, locked): staged changes or a
    // conflicted index refuse the merge, mirroring `git merge`.
    let mut index = repo.index()?;
    let head_commit = repo.head()?.peel_to_commit()?;
    if index.has_conflicts() || index.write_tree_to(&repo)? != head_commit.tree_id() {
        return Err(AppError::Git(
            "cannot merge: your index contains uncommitted changes — commit or unstage them first"
                .to_string(),
        ));
    }

    // Identity check EARLY: a clean merge auto-commits, so ConfigMissing must
    // surface before the worktree is touched.
    resolve_signature(&repo.config()?.snapshot()?)?;

    let annotated = repo.reference_to_annotated_commit(incoming.get())?;
    let (analysis, _pref) = repo.merge_analysis(&[&annotated])?;

    if analysis.is_up_to_date() {
        return Ok(MergeOutcome::UpToDate);
    }

    if analysis.is_fast_forward() {
        // merge.ff config NOT consulted in v1 — FF whenever possible.
        // Same safe-FF recipe as remote.rs pull_ff: checkout BEFORE set_target.
        let obj = repo.find_object(annotated.id(), None)?;
        let mut opts = git2::build::CheckoutBuilder::new();
        opts.safe(); // DEFAULT SAFE MODE — never .force()
        match repo.checkout_tree(&obj, Some(&mut opts)) {
            Ok(()) => {}
            Err(e) if e.code() == git2::ErrorCode::Conflict => {
                return Err(AppError::CheckoutConflict(
                    "cannot merge: local changes would be overwritten. \
                     Commit or discard them first."
                        .to_string(),
                ));
            }
            Err(e) => return Err(e.into()),
        }
        repo.find_reference(&format!("refs/heads/{head_branch}"))?
            .set_target(annotated.id(), &format!("merge {branch_name}: fast-forward"))?;
        return Ok(MergeOutcome::FastForwarded {
            branch: head_branch,
            to: annotated.id().to_string(),
        });
    }

    // analysis.is_normal(): true merge.
    let mut message = prepared_merge_message(branch_name, incoming_is_remote);
    let mut merge_opts = git2::MergeOptions::new(); // defaults: find_renames on
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout
        .safe()
        .allow_conflicts(true)
        .conflict_style_merge(true); // <<<<<<< ======= >>>>>>> markers

    if let Err(e) = repo.merge(&[&annotated], Some(&mut merge_opts), Some(&mut checkout)) {
        // libgit2 may have written MERGE_HEAD/MERGE_MSG/MERGE_MODE before the
        // checkout failed. Guarantee: a failed merge_branch leaves state Clean.
        let _ = repo.cleanup_state();
        if let Ok(tree) = head_commit.tree() {
            // Re-open the index to see whatever repo.merge() left on disk;
            // fall back to the pre-merge handle if even that fails (best
            // effort — this whole block is cleanup on an already-failed op).
            let mut index = repo.index().unwrap_or(index);
            let _ = index.read_tree(&tree);
            let _ = index.write();
        }
        if e.code() == git2::ErrorCode::Conflict {
            return Err(AppError::CheckoutConflict(
                "cannot merge: local changes would be overwritten. \
                 Commit or discard them first."
                    .to_string(),
            ));
        }
        return Err(e.into());
    }

    let index = repo.index()?;
    if index.has_conflicts() {
        let paths: Vec<String> = list_conflicts(workdir)?
            .into_iter()
            .map(|c| c.path)
            .collect();
        // Conflicts block, exactly like git (contract §4.3); overwrite
        // libgit2's MERGE_MSG so the on-disk message is deterministic.
        message.push_str("\n\nConflicts:\n");
        message.push_str(
            &paths
                .iter()
                .map(|p| format!("\t{p}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        std::fs::write(repo.path().join("MERGE_MSG"), format!("{message}\n"))?;
        return Ok(MergeOutcome::Conflicts { paths });
    }

    // Clean: auto-commit (git-like), then cleanup. Keep the on-disk message
    // equal to the committed message until cleanup_state removes it.
    std::fs::write(repo.path().join("MERGE_MSG"), format!("{message}\n"))?;
    // Release every repo-lifetime borrow before the &mut borrow below.
    drop(annotated);
    drop(index);
    drop(head_commit);
    drop(incoming);
    let result = finalize_merge_commit(&mut repo, &message)?;
    Ok(MergeOutcome::Merged { oid: result.oid })
}

/// Shared core of commit_merge and the clean-merge auto-commit path
/// (contract §4.4 steps 3–9): normalize the message, resolve the signature,
/// collect HEAD + every MERGE_HEAD as parents, commit, cleanup_state.
fn finalize_merge_commit(
    repo: &mut git2::Repository,
    message: &str,
) -> Result<CommitResult, AppError> {
    // Normalize exactly like create_commit (CRLF/CR -> \n, trim).
    let normalized = message.replace("\r\n", "\n").replace('\r', "\n");
    let msg = normalized.trim();
    if msg.is_empty() {
        return Err(AppError::EmptyMessage);
    }

    let sig = resolve_signature(&repo.config()?.snapshot()?)?;

    // Parents: HEAD commit first, then EVERY MERGE_HEAD oid in file order
    // (v1 UI only produces one, but octopus state written by the CLI must not
    // be silently truncated). MERGE_HEADs are collected before any immutable
    // repo borrow (mergehead_foreach takes &mut self).
    let mut merge_oids: Vec<git2::Oid> = Vec::new();
    repo.mergehead_foreach(|oid| {
        merge_oids.push(*oid);
        true
    })?;
    let head_commit = repo.head()?.peel_to_commit()?;
    if merge_oids.is_empty() {
        return Err(AppError::Git("MERGE_HEAD missing".to_string()));
    }
    let mut parents: Vec<git2::Commit> = vec![head_commit];
    for oid in merge_oids {
        parents.push(repo.find_commit(oid)?);
    }
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    // NO nothing-to-commit check: an empty-diff merge commit is legitimate —
    // it records ancestry.
    let mut index = repo.index()?;
    let tree = repo.find_tree(index.write_tree()?)?;
    let oid = repo.commit(Some("HEAD"), &sig, &sig, &format!("{msg}\n"), &tree, &parent_refs)?;

    repo.cleanup_state()?; // removes MERGE_HEAD/MERGE_MSG/MERGE_MODE -> Clean

    let branch = repo
        .head()
        .ok()
        .filter(|h| h.is_branch())
        .and_then(|h| h.shorthand().map(String::from));
    let summary = msg.lines().next().unwrap_or(msg).to_string();
    Ok(CommitResult {
        oid: oid.to_string(),
        summary,
        branch,
    })
}

/// Blocking. Finalizes a paused merge as a 2(+)-parent commit
/// (contract §4.4 — cheap checks first).
pub fn commit_merge(workdir: &Path, message: &str) -> Result<CommitResult, AppError> {
    let mut repo = open_workdir_repo(workdir)?;

    if repo.state() != git2::RepositoryState::Merge {
        return Err(AppError::NoOperationInProgress(
            "no merge in progress".to_string(),
        ));
    }
    let index = repo.index()?;
    if index.has_conflicts() {
        let n = index.conflicts()?.count();
        return Err(AppError::UnresolvedConflicts(format!(
            "cannot commit: {n} unresolved conflict(s) remain"
        )));
    }
    finalize_merge_commit(&mut repo, message)
}

/// Blocking. Aborts a paused merge; restores pre-merge index + the worktree
/// files the merge touched (approximate `git reset --merge`, contract §4.5 —
/// NOT reset --hard).
///
/// Guarantee: files with pre-merge unstaged edits that the merge did NOT
/// touch survive an abort byte-identically. Files the merge touched are
/// restored to HEAD (a pre-merge unstaged edit to a merge-touched file cannot
/// exist — it would have failed merge_branch with CheckoutConflict before any
/// state was written).
pub fn abort_merge(workdir: &Path) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;

    if repo.state() != git2::RepositoryState::Merge {
        return Err(AppError::NoOperationInProgress(
            "no merge in progress".to_string(),
        ));
    }

    let head_tree = repo.head()?.peel_to_commit()?.tree()?;
    let mut index = repo.index()?;

    // Every path the merge touched: index-vs-HEAD differences UNION all
    // conflicted paths (ours/theirs/base sides). Raw path bytes — a lossy
    // UTF-8 conversion would silently fail to restore non-UTF-8 paths.
    let mut touched: Vec<Vec<u8>> = Vec::new();
    let diff = repo.diff_tree_to_index(Some(&head_tree), Some(&index), None)?;
    for delta in diff.deltas() {
        for f in [delta.old_file(), delta.new_file()] {
            if let Some(p) = f.path_bytes() {
                touched.push(p.to_vec());
            }
        }
    }
    for c in index.conflicts()? {
        let c = c?;
        for e in [c.ancestor, c.our, c.their].into_iter().flatten() {
            touched.push(e.path.clone());
        }
    }
    touched.sort();
    touched.dedup();

    // Force-checkout ONLY the touched paths: restores/deletes exactly the
    // merge-touched files, leaving unrelated unstaged edits alone.
    //
    // CRITICAL: a CheckoutBuilder with ZERO .path() calls matches ALL paths,
    // so an empty `touched` set (e.g. every conflict already resolved as Ours
    // before Abort) must SKIP the checkout entirely — otherwise force()
    // clobbers the whole worktree, the exact data loss §4.5/§11.2 prevents.
    if !touched.is_empty() {
        let mut cb = git2::build::CheckoutBuilder::new();
        cb.force().remove_untracked(false);
        for p in &touched {
            cb.path(p.as_slice());
        }
        repo.checkout_tree(head_tree.as_object(), Some(&mut cb))?;
    }

    // Drop all conflict + merged entries from the index, then clear the op.
    index.read_tree(&head_tree)?;
    index.write()?;
    repo.cleanup_state()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------- wire shape (TS mirrors)

    /// The serde tag/casing must match the TS MergeOutcome union exactly.
    #[test]
    fn wire_shapes_are_camel_case_tagged() {
        let v = serde_json::to_value(MergeOutcome::UpToDate).expect("json");
        assert_eq!(v, serde_json::json!({ "kind": "upToDate" }));

        let v = serde_json::to_value(MergeOutcome::FastForwarded {
            branch: "main".to_string(),
            to: "a".repeat(40),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "fastForwarded", "branch": "main", "to": "a".repeat(40) })
        );

        let v = serde_json::to_value(MergeOutcome::Merged {
            oid: "b".repeat(40),
        })
        .expect("json");
        assert_eq!(v, serde_json::json!({ "kind": "merged", "oid": "b".repeat(40) }));

        let v = serde_json::to_value(MergeOutcome::Conflicts {
            paths: vec!["README.md".to_string(), "src/auth.ts".to_string()],
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "conflicts", "paths": ["README.md", "src/auth.ts"] })
        );
    }

    // -------------------------------------------- §4.3 prepared MERGE_MSG

    #[test]
    fn prepared_message_is_byte_exact() {
        assert_eq!(
            prepared_merge_message("feature/login", false),
            "Merge branch 'feature/login'"
        );
        assert_eq!(
            prepared_merge_message("origin/main", true),
            "Merge remote-tracking branch 'origin/main'"
        );
    }

    /// Regression (reviewer MUST-FIX): resolving every conflict as Ours
    /// before Abort leaves the index == HEAD tree with zero conflicts, so the
    /// `touched` set is EMPTY. A CheckoutBuilder with zero .path() calls
    /// matches ALL paths — the empty set must skip the force checkout
    /// entirely, or an unrelated pre-merge unstaged edit gets clobbered.
    #[test]
    fn abort_with_empty_touched_set_preserves_unrelated_unstaged_edit() {
        use crate::git::conflict::{resolve_conflict, ConflictResolution};
        use crate::git::stage::stage_paths;

        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "Test User").expect("name");
            cfg.set_str("user.email", "test@example.com").expect("email");
            cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        }
        let commit_all = |msg: &str, files: &[(&str, &str)]| {
            for (name, content) in files {
                std::fs::write(dir.path().join(name), content).expect("write");
            }
            stage_paths(
                dir.path(),
                &files.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>(),
            )
            .expect("stage");
            crate::git::commit::create_commit(dir.path(), msg).expect("commit")
        };

        // Base commit with the conflict file + an unrelated file.
        commit_all("base", &[("a.txt", "base\n"), ("unrelated.txt", "orig\n")]);
        let base_oid = repo.head().expect("HEAD").target().expect("oid");

        // topic edits a.txt one way; main edits it another -> guaranteed conflict.
        repo.branch("topic", &repo.find_commit(base_oid).expect("base"), false)
            .expect("branch");
        commit_all("main change", &[("a.txt", "main\n")]);
        {
            // Commit the divergent topic-side change directly on the branch.
            let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
            let base = repo.find_commit(base_oid).expect("base commit");
            let mut tb = repo.treebuilder(Some(&base.tree().expect("tree"))).expect("tb");
            let blob = repo.blob(b"topic\n").expect("blob");
            tb.insert("a.txt", blob, 0o100644).expect("insert");
            let tree = repo.find_tree(tb.write().expect("tree oid")).expect("tree");
            repo.commit(
                Some("refs/heads/topic"),
                &sig,
                &sig,
                "topic change\n",
                &tree,
                &[&base],
            )
            .expect("topic commit");
        }

        // Unrelated pre-merge UNSTAGED edit that must survive the abort.
        std::fs::write(dir.path().join("unrelated.txt"), "edited but not staged\n")
            .expect("edit unrelated");

        let outcome = merge_branch(dir.path(), "topic").expect("merge");
        assert_eq!(
            outcome,
            MergeOutcome::Conflicts {
                paths: vec!["a.txt".to_string()]
            }
        );

        // Resolve the ONLY conflict as Ours: index returns to == HEAD tree,
        // zero conflicts -> abort's touched set is empty.
        resolve_conflict(dir.path(), "a.txt", ConflictResolution::Ours).expect("resolve");

        abort_merge(dir.path()).expect("abort");

        assert_eq!(repo.state(), git2::RepositoryState::Clean);
        let unrelated =
            std::fs::read_to_string(dir.path().join("unrelated.txt")).expect("read unrelated");
        assert_eq!(unrelated, "edited but not staged\n", "unstaged edit clobbered");
        let a = std::fs::read_to_string(dir.path().join("a.txt")).expect("read a.txt");
        assert_eq!(a, "main\n", "a.txt must be back at HEAD's version");
    }

    // ------------------------------------------------------- preconditions

    #[test]
    fn merge_preconditions_on_fresh_repo() {
        let dir = crate::testutil::scratch_dir();
        git2::Repository::init(dir.path()).expect("init repo");

        // Unborn HEAD refuses before branch resolution.
        let err = merge_branch(dir.path(), "topic").expect_err("unborn");
        match err {
            AppError::Git(m) => assert!(m.contains("no commits yet"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }

        // commit_merge / abort_merge with no merge in progress.
        let err = commit_merge(dir.path(), "msg").expect_err("no merge");
        assert!(matches!(err, AppError::NoOperationInProgress(_)));
        let err = abort_merge(dir.path()).expect_err("no merge");
        assert!(matches!(err, AppError::NoOperationInProgress(_)));
    }
}
