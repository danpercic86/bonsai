//! AI commit composer — APPLY side (P54b). Applies a user-finalized
//! [`ComposePlan`] as an ORDERED stage+commit sequence, turning a messy working
//! tree into N logical commits. NOT AI-gated (pure git2 — the house shape of
//! `commit`, not the `ai_*` triple); the plan is whatever the user reviewed/edited.
//!
//! # The three safety guarantees (contract §0 D4/D5, §4.1)
//! 1. **ATOMIC.** The WHOLE plan is validated BEFORE anything mutates (identity via
//!    [`resolve_signature`], every path present in the current change set + assigned
//!    to exactly one group, non-empty messages, non-empty file lists). Any failure
//!    during the commit loop rolls back so ZERO commits land.
//! 2. **HEAD + index ROLLBACK.** The original HEAD (peeled to a commit oid; `None`
//!    when unborn) is recorded before mutating. On failure [`rollback`] restores the
//!    branch/detached HEAD (or deletes the branch the loop created, when started
//!    unborn) and re-reads the index to match.
//! 3. **WORKING TREE IS NEVER TOUCHED.** No checkout, no hard reset, no workdir
//!    writes — ever. [`reset_index_to_head`] is a mixed-reset equivalent (read the
//!    HEAD tree into the index; `clear()` when unborn). On success, failure, or
//!    cancel the bytes on disk are exactly as the user left them; files in no group
//!    stay uncommitted (unstaged) in the working tree.
//!
//! Because it is a FILE-LEVEL partition (D2), after the index reset staging group K
//! only advances files untouched by earlier groups, so each commit's delta-to-parent
//! is exactly its group's files (no line renumbering — v1 stages whole files).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::error::AppError;
use crate::git::ai_compose::ComposeGroup;
use crate::git::ai_explain::gather_worktree;
use crate::git::bisect::require_no_bisect;
use crate::git::commit::{create_commit, resolve_signature};
use crate::git::stage::{open_workdir_repo, stage_paths, validate_rel_path};

/// User-finalized plan to apply: an ORDERED list of groups (first = oldest
/// commit). A changed file absent from every group is intentionally left
/// uncommitted in the working tree. COMMAND INPUT (Deserialize).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposePlan {
    pub groups: Vec<ComposeGroup>,
}

/// Result of applying a plan: created commits, oldest→newest. Serialize only.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeApplyResult {
    pub commits: Vec<ComposeCommit>,
}

/// One created commit (contract §2.1). `oid` is the full 40-hex id.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeCommit {
    /// Full 40-char hex oid of the new commit.
    pub oid: String,
    /// First line of the (cleaned) commit message.
    pub summary: String,
}

/// Blocking. Applies `plan` as an ORDERED stage+commit sequence (contract §4.1).
/// ATOMIC: validates the whole plan first, resets the index to HEAD (working tree
/// UNTOUCHED), then commits each group; ANY mid-sequence failure rolls HEAD+index
/// back so NOTHING is committed. Does NOT emit `repo-changed` — the caller refetches.
///
/// Errors: `NoRepo` (via `open_workdir_repo`) | `OperationInProgress` (mid-op) |
/// `Git` (unresolved conflicts) | `EmptyMessage` | `ConfigMissing` (identity unset) |
/// `NothingToCommit` (empty plan) | `Other` (unknown/duplicate path, empty group,
/// a group whose staged files net to no change / stale plan).
pub fn apply_composed_commits(
    workdir: &Path,
    plan: &ComposePlan,
) -> Result<ComposeApplyResult, AppError> {
    let repo = open_workdir_repo(workdir)?;

    // A Bonsai bisect runs on a clean detached HEAD, invisible to `state()` below.
    require_no_bisect(&repo)?;
    if repo.state() != git2::RepositoryState::Clean {
        return Err(AppError::OperationInProgress(
            "an operation is in progress — finish or abort it before composing commits"
                .to_string(),
        ));
    }
    if repo.index()?.has_conflicts() {
        return Err(AppError::Git(
            "cannot compose: unresolved conflicts".to_string(),
        ));
    }

    // ---- validate the WHOLE plan first; nothing mutates yet (safety guarantee 1) ----
    if plan.groups.is_empty() {
        return Err(AppError::NothingToCommit);
    }
    // Identity resolves to a signature or `ConfigMissing` — EARLY, before any commit
    // (so an unset identity never lands a partial sequence).
    resolve_signature(&repo.config()?.snapshot()?)?;

    // The authoritative HEAD→workdir change set (index-aware, incl. untracked). Every
    // planned path must belong to it; renames also carry their OLD path so staging a
    // rename's NEW path stages both the add and the delete side.
    let worktree = gather_worktree(workdir)?;
    let changed: HashSet<&str> = worktree.iter().map(|f| f.path.as_str()).collect();
    let rename_origs: HashMap<&str, &str> = worktree
        .iter()
        .filter_map(|f| f.orig_path.as_deref().map(|o| (f.path.as_str(), o)))
        .collect();

    let mut seen: HashSet<&str> = HashSet::new();
    for g in &plan.groups {
        if g.message.trim().is_empty() {
            return Err(AppError::EmptyMessage);
        }
        if g.files.is_empty() {
            return Err(AppError::Other("a group has no files".to_string()));
        }
        for f in &g.files {
            validate_rel_path(f)?;
            if !changed.contains(f.as_str()) {
                return Err(AppError::Other(format!(
                    "file '{f}' is not in the working changes; refresh the composer"
                )));
            }
            if !seen.insert(f.as_str()) {
                return Err(AppError::Other(format!(
                    "file '{f}' is assigned to more than one group"
                )));
            }
        }
    }

    // ---- rollback anchor: HEAD peeled to a commit oid (None on unborn HEAD) ----
    let orig_head: Option<git2::Oid> = repo
        .head()
        .ok()
        .and_then(|h| h.peel_to_commit().ok())
        .map(|c| c.id());

    // ---- take over the index so uncovered changes cannot leak into any commit ----
    reset_index_to_head(&repo, orig_head)?;

    // ---- create commits in order; atomic on any failure ----
    let mut commits: Vec<ComposeCommit> = Vec::with_capacity(plan.groups.len());
    for (idx, g) in plan.groups.iter().enumerate() {
        let paths = files_with_rename_origs(&rename_origs, &g.files);
        // Stage ONLY this group, then commit the whole (cumulative) index. Because
        // the partition is disjoint, the commit's delta-to-parent is exactly `g`.
        // skip_hooks = TRUE (P59; F-A4-4 DECISION, T2 Area 4 audit 2026-08-09): AI-composed
        // commits BYPASS ALL GIT HOOKS — deliberately. A re-staging pre-commit hook
        // (`git add -u` / lint-staged) would, via the commit's `index.read(true)`, pull OTHER
        // groups' working-tree changes into this commit and silently break that partition
        // invariant; a commit-msg hook would rewrite each generated group message. The composer
        // is a mechanical history-organizer, so hooks stay OFF for its split commits (a normal
        // commit still runs them). Consequence a hook-policy shop should know: commit-message
        // policy hooks do NOT vet composer-generated messages. Documented in the P59 user
        // checklist ("Known v1 hook divergences") and flagged FOR USER REVIEW in
        // docs/testing-campaign-2026-08/FINDINGS.md; revisit commit-msg-only execution later.
        let step =
            stage_paths(workdir, &paths).and_then(|()| create_commit(workdir, &g.message, None, true));
        match step {
            Ok(cr) => commits.push(ComposeCommit {
                oid: cr.oid,
                summary: cr.summary,
            }),
            Err(e) => {
                // Any failure (incl. a group that nets to no change / a stale plan =>
                // `create_commit` `NothingToCommit`) unwinds EVERYTHING landed so far.
                rollback(&repo, orig_head)?;
                return Err(annotate(e, idx));
            }
        }
    }

    Ok(ComposeApplyResult { commits })
}

/// Resets the index to `orig_head`'s tree (mixed-reset equivalent) — or empties it
/// when unborn — and writes it. The WORKING TREE IS NEVER TOUCHED (no checkout, no
/// `--hard`): only the on-disk index moves.
fn reset_index_to_head(
    repo: &git2::Repository,
    orig_head: Option<git2::Oid>,
) -> Result<(), AppError> {
    let mut index = repo.index()?;
    match orig_head {
        Some(oid) => {
            let tree = repo.find_commit(oid)?.tree()?;
            index.read_tree(&tree)?;
        }
        None => index.clear()?,
    }
    index.write()?;
    Ok(())
}

/// Expands a group's file list with the OLD path of any rename (from the
/// HEAD→workdir `rename_origs` map) so [`stage_paths`] stages both the delete (old)
/// and add (new) sides. Non-renames pass through unchanged; input order preserved.
fn files_with_rename_origs(rename_origs: &HashMap<&str, &str>, files: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(files.len());
    for f in files {
        out.push(f.clone());
        if let Some(orig) = rename_origs.get(f.as_str()) {
            out.push((*orig).to_string());
        }
    }
    out
}

/// Undoes every commit the loop landed (safety guarantee 2). Working tree untouched
/// ⇒ all original changes stay on disk; the index is re-read to match HEAD.
/// - `Some(oid)`: point HEAD back at `oid` — a branch HEAD moves its branch ref;
///   a detached HEAD is re-pointed directly — then reset the index to `oid`'s tree.
/// - `None` (started unborn): the loop created the branch tip HEAD symbolically
///   points at; delete that ref so HEAD is unborn again, then empty the index.
fn rollback(repo: &git2::Repository, orig_head: Option<git2::Oid>) -> Result<(), AppError> {
    match orig_head {
        Some(oid) => {
            let mut head_ref = repo.head()?;
            if head_ref.is_branch() {
                head_ref.set_target(oid, "bonsai: composer rollback")?;
            } else {
                repo.set_head_detached(oid)?;
            }
            reset_index_to_head(repo, Some(oid))?;
        }
        None => {
            // Started unborn: the loop's first commit created the branch tip HEAD
            // symbolically points at. Delete that ref so HEAD is unborn again.
            let mut branch_name: Option<String> = None;
            if let Ok(head_ref) = repo.find_reference("HEAD") {
                // git2 0.21: `symbolic_target` is `Result<Option<&str>>` (non-utf8
                // symbolic target => Err). A missing/invalid target just leaves HEAD
                // as-is — still unborn — which is the intended end state.
                if let Ok(Some(target)) = head_ref.symbolic_target() {
                    branch_name = Some(target.to_owned());
                }
            }
            if let Some(name) = branch_name {
                if let Ok(mut branch_ref) = repo.find_reference(&name) {
                    branch_ref.delete()?;
                }
            }
            let mut index = repo.index()?;
            index.clear()?;
            index.write()?;
        }
    }
    Ok(())
}

/// Enriches a group's failure with its 1-based index. A bare `NothingToCommit`
/// (a no-op / stale group) becomes an actionable `Other`; message-bearing variants
/// keep their kind with a `group N:` prefix. `EmptyMessage`/`ConfigMissing` are
/// validated up front, so they are preserved verbatim if ever seen here.
fn annotate(e: AppError, group_index: usize) -> AppError {
    let n = group_index + 1;
    match e {
        AppError::NothingToCommit => AppError::Other(format!(
            "group {n}: its staged files produce no change to commit \
             (a no-op group or a stale plan); refresh the composer"
        )),
        AppError::Git(m) => AppError::Git(format!("group {n}: {m}")),
        AppError::Io(m) => AppError::Io(format!("group {n}: {m}")),
        AppError::Other(m) => AppError::Other(format!("group {n}: {m}")),
        AppError::OperationInProgress(m) => {
            AppError::OperationInProgress(format!("group {n}: {m}"))
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    use crate::git::status::read_status;

    /// git2-init a scratch repo with identity + autocrlf off (mirrors `ai_compose`).
    fn init_scratch() -> tempfile::TempDir {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        dir
    }

    /// Same, but WITHOUT a usable identity (for the ConfigMissing case). The
    /// machine running the suite may carry a GLOBAL `user.name`/`user.email`, so
    /// pin EMPTY local values (highest precedence) to mask it — `resolve_signature`
    /// treats empty as unset. `core.autocrlf` is pinned for deterministic hashing.
    fn init_scratch_no_identity() -> tempfile::TempDir {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "").expect("mask name");
        cfg.set_str("user.email", "").expect("mask email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        dir
    }

    fn write(p: &Path, rel: &str, body: &str) {
        std::fs::write(p.join(rel), body).unwrap_or_else(|e| panic!("write {rel}: {e}"));
    }

    fn stage(p: &Path, paths: &[&str]) {
        stage_paths(p, &paths.iter().map(|s| s.to_string()).collect::<Vec<_>>()).expect("stage");
    }

    fn group(files: &[&str], message: &str) -> ComposeGroup {
        ComposeGroup {
            files: files.iter().map(|s| s.to_string()).collect(),
            message: message.to_string(),
        }
    }

    fn have_git() -> bool {
        let ok = Command::new("git").arg("--version").output().is_ok();
        if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
            panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
        }
        ok
    }

    /// HEAD peeled to a commit oid (None on unborn HEAD).
    fn head_oid(p: &Path) -> Option<git2::Oid> {
        let repo = open_workdir_repo(p).expect("open");
        let oid = repo
            .head()
            .ok()
            .and_then(|h| h.peel_to_commit().ok())
            .map(|c| c.id());
        oid
    }

    /// Total commits reachable from HEAD (0 when unborn).
    fn commit_count(p: &Path) -> usize {
        let repo = open_workdir_repo(p).expect("open");
        let head = repo
            .head()
            .ok()
            .and_then(|h| h.peel_to_commit().ok())
            .map(|c| c.id());
        match head {
            None => 0,
            Some(oid) => {
                let mut walk = repo.revwalk().expect("revwalk");
                walk.push(oid).expect("push");
                walk.count()
            }
        }
    }

    /// Sorted new-file paths of `oid`'s diff vs its first parent (root => vs the
    /// empty tree) — i.e. the commit's delta-to-parent.
    fn delta_paths(p: &Path, oid: &str) -> Vec<String> {
        let repo = open_workdir_repo(p).expect("open");
        let commit = repo
            .find_commit(git2::Oid::from_str(oid).expect("oid"))
            .expect("commit");
        let tree = commit.tree().expect("tree");
        let parent_tree = commit.parent(0).ok().map(|pc| pc.tree().expect("ptree"));
        let diff = repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
            .expect("diff");
        let mut out: Vec<String> = Vec::new();
        diff.foreach(
            &mut |d, _| {
                if let Some(path) = d.new_file().path() {
                    out.push(path.to_string_lossy().into_owned());
                }
                true
            },
            None,
            None,
            None,
        )
        .expect("foreach");
        out.sort();
        out
    }

    /// The tree oid the current on-disk index would write (fresh repo handle so no
    /// cached index leaks across the assertion).
    fn index_tree(p: &Path) -> git2::Oid {
        let repo = open_workdir_repo(p).expect("open");
        let mut index = repo.index().expect("index");
        index.write_tree().expect("write_tree")
    }

    /// The HEAD commit's tree oid (panics on unborn — callers know HEAD exists).
    fn head_tree(p: &Path) -> git2::Oid {
        let repo = open_workdir_repo(p).expect("open");
        let id = repo
            .head()
            .expect("head")
            .peel_to_commit()
            .expect("commit")
            .tree()
            .expect("tree")
            .id();
        id
    }

    /// Names of the files `oid` changed vs its parent, via the `git` CLI
    /// (`diff-tree`) — the oracle cross-check for the per-commit delta.
    fn git_delta_names(p: &Path, oid: &str) -> Vec<String> {
        let out = Command::new("git")
            .current_dir(p)
            .args(["diff-tree", "--no-commit-id", "--name-only", "-r", oid])
            .output()
            .expect("git diff-tree");
        let mut v: Vec<String> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        v.sort();
        v
    }

    // ------------------------------------------------------------ §8.9–§8.15

    /// §8.9: 3 changed files split 2+1 => two commits; commit-1's delta-to-parent
    /// == group-1's files, commit-2 == group-2's file; HEAD advanced by 2. Guarded
    /// by `have_git()`, the `git diff-tree` oracle confirms each per-commit delta.
    #[test]
    fn apply_two_groups_creates_two_commits_each_its_own_delta() {
        let dir = init_scratch();
        let p = dir.path();
        write(p, "base.txt", "base\n");
        stage(p, &["base.txt"]);
        create_commit(p, "base", None, false).expect("commit");
        let base = head_oid(p).expect("base head");

        write(p, "f1.txt", "1\n");
        write(p, "f2.txt", "2\n");
        write(p, "f3.txt", "3\n");
        let plan = ComposePlan {
            groups: vec![
                group(&["f1.txt", "f2.txt"], "feat: g1"),
                group(&["f3.txt"], "test: g2"),
            ],
        };

        let res = apply_composed_commits(p, &plan).expect("apply");
        assert_eq!(res.commits.len(), 2, "two commits created");
        for c in &res.commits {
            assert_eq!(c.oid.len(), 40, "full 40-hex oid: {}", c.oid);
            assert!(c.oid.chars().all(|ch| ch.is_ascii_hexdigit()), "hex oid");
        }
        assert_eq!(res.commits[0].summary, "feat: g1");
        assert_eq!(res.commits[1].summary, "test: g2");

        // HEAD advanced by exactly 2 (base + 2), newest = commits[1].
        assert_eq!(commit_count(p), 3, "base + 2 new commits");
        assert_eq!(head_oid(p).expect("head").to_string(), res.commits[1].oid);
        assert_ne!(head_oid(p).expect("head"), base);

        // Each commit's delta-to-parent is EXACTLY its group's files.
        assert_eq!(delta_paths(p, &res.commits[0].oid), vec!["f1.txt", "f2.txt"]);
        assert_eq!(delta_paths(p, &res.commits[1].oid), vec!["f3.txt"]);

        // CLI oracle (guarded): `git diff-tree` agrees per commit.
        if have_git() {
            assert_eq!(git_delta_names(p, &res.commits[0].oid), vec!["f1.txt", "f2.txt"]);
            assert_eq!(git_delta_names(p, &res.commits[1].oid), vec!["f3.txt"]);
        }
    }

    /// §8.10: a changed file in NO group is left uncommitted (still dirty in
    /// `read_status`) after apply; the covered file is committed (gone from status).
    #[test]
    fn apply_leaves_uncovered_files_uncommitted() {
        let dir = init_scratch();
        let p = dir.path();
        write(p, "base.txt", "base\n");
        stage(p, &["base.txt"]);
        create_commit(p, "base", None, false).expect("commit");

        write(p, "covered.txt", "c\n");
        write(p, "uncovered.txt", "u\n");
        let plan = ComposePlan {
            groups: vec![group(&["covered.txt"], "only covered")],
        };
        let res = apply_composed_commits(p, &plan).expect("apply");
        assert_eq!(res.commits.len(), 1);

        let st = read_status(p).expect("status");
        let dirty: Vec<&str> = st
            .staged
            .iter()
            .chain(st.unstaged.iter())
            .chain(st.untracked.iter())
            .map(|e| e.path.as_str())
            .collect();
        assert!(dirty.contains(&"uncovered.txt"), "uncovered file stays dirty: {dirty:?}");
        assert!(!dirty.contains(&"covered.txt"), "covered file committed: {dirty:?}");
    }

    /// §8.11: EVERY validation failure rejects BEFORE any mutation — HEAD unchanged
    /// and NOTHING committed. Covers empty message, empty file list, duplicate path,
    /// path not in the change set, empty plan, and unset identity.
    #[test]
    fn apply_rejects_before_any_commit() {
        // --- born repo with identity + two real changes (f1, f2) ---
        let dir = init_scratch();
        let p = dir.path();
        write(p, "base.txt", "base\n");
        stage(p, &["base.txt"]);
        create_commit(p, "base", None, false).expect("commit");
        write(p, "f1.txt", "1\n");
        write(p, "f2.txt", "2\n");
        let orig = head_oid(p).expect("head");

        // Each case: apply, expect the named error, assert NOTHING mutated.
        let expect_untouched = |res: Result<ComposeApplyResult, AppError>| {
            assert!(res.is_err(), "must reject");
            assert_eq!(head_oid(p).expect("head"), orig, "HEAD unchanged");
            assert_eq!(commit_count(p), 1, "no commit landed");
        };

        // empty message => EmptyMessage.
        let e = apply_composed_commits(p, &ComposePlan { groups: vec![group(&["f1.txt"], "   ")] })
            .expect_err("empty message");
        assert!(matches!(e, AppError::EmptyMessage), "got {e:?}");
        expect_untouched(Err(e));

        // empty file list => Other.
        let e = apply_composed_commits(p, &ComposePlan { groups: vec![group(&[], "msg")] })
            .expect_err("empty files");
        assert!(matches!(e, AppError::Other(_)), "got {e:?}");
        expect_untouched(Err(e));

        // duplicate path across groups => Other.
        let e = apply_composed_commits(
            p,
            &ComposePlan {
                groups: vec![group(&["f1.txt"], "a"), group(&["f1.txt"], "b")],
            },
        )
        .expect_err("duplicate path");
        match e {
            AppError::Other(m) => assert!(m.contains("more than one group"), "got {m}"),
            other => panic!("expected Other, got {other:?}"),
        }
        assert_eq!(head_oid(p).expect("head"), orig);
        assert_eq!(commit_count(p), 1);

        // path not in the change set => Other.
        let e = apply_composed_commits(p, &ComposePlan { groups: vec![group(&["ghost.txt"], "m")] })
            .expect_err("unknown path");
        match e {
            AppError::Other(m) => assert!(m.contains("not in the working changes"), "got {m}"),
            other => panic!("expected Other, got {other:?}"),
        }
        assert_eq!(head_oid(p).expect("head"), orig);
        assert_eq!(commit_count(p), 1);

        // empty plan => NothingToCommit.
        let e = apply_composed_commits(p, &ComposePlan { groups: vec![] })
            .expect_err("empty plan");
        assert!(matches!(e, AppError::NothingToCommit), "got {e:?}");
        expect_untouched(Err(e));

        // --- unset identity => ConfigMissing (unborn, no-identity repo) ---
        let dir2 = init_scratch_no_identity();
        let p2 = dir2.path();
        write(p2, "f1.txt", "1\n");
        let e = apply_composed_commits(p2, &ComposePlan { groups: vec![group(&["f1.txt"], "m")] })
            .expect_err("no identity");
        assert!(matches!(e, AppError::ConfigMissing(_)), "got {e:?}");
        assert!(head_oid(p2).is_none(), "still unborn — nothing committed");
    }

    /// §8.12: a mid-sequence failure rolls back EVERYTHING. Group 2 references a
    /// staged-then-reverted file (in the change set — git2's index-aware worktree
    /// diff surfaces it — but whose workdir matches HEAD, so after the index reset
    /// staging it nets to no change => `create_commit` `NothingToCommit`). Assert
    /// HEAD == original, index == HEAD, the working tree STILL holds ALL original
    /// changes, and zero commits landed.
    #[test]
    fn apply_rolls_back_on_mid_sequence_failure() {
        let dir = init_scratch();
        let p = dir.path();
        write(p, "a.txt", "a\n");
        write(p, "b.txt", "b\n");
        stage(p, &["a.txt", "b.txt"]);
        create_commit(p, "base", None, false).expect("commit");
        let orig = head_oid(p).expect("head");

        // a.txt: a genuine working-tree change. b.txt: staged then reverted — its
        // WORKDIR equals HEAD, so committing it after the reset is a no-op.
        write(p, "a.txt", "aa\n");
        write(p, "b.txt", "bb\n");
        stage(p, &["b.txt"]);
        write(p, "b.txt", "b\n");

        // Both are in the change set (pre-condition for this test's mechanism).
        let changed: Vec<String> = gather_worktree(p)
            .expect("gather")
            .iter()
            .map(|f| f.path.clone())
            .collect();
        assert!(
            changed.contains(&"a.txt".to_string()) && changed.contains(&"b.txt".to_string()),
            "both files must be in the change set: {changed:?}"
        );

        let plan = ComposePlan {
            groups: vec![
                group(&["a.txt"], "commit a"),
                group(&["b.txt"], "commit b (nets to no change)"),
            ],
        };
        let err = apply_composed_commits(p, &plan).expect_err("group 2 nets to no change");
        match err {
            AppError::Other(m) => assert!(m.contains("group 2"), "annotated with group index: {m}"),
            other => panic!("expected Other(group 2 ...), got {other:?}"),
        }

        // ROLLBACK proven: HEAD restored, index back at HEAD, zero commits landed.
        assert_eq!(head_oid(p).expect("head"), orig, "HEAD rolled back to original");
        assert_eq!(commit_count(p), 1, "zero commits landed (only base remains)");
        assert_eq!(index_tree(p), head_tree(p), "index reset to HEAD");

        // WORKING TREE UNTOUCHED: all original on-disk content preserved.
        assert_eq!(std::fs::read_to_string(p.join("a.txt")).expect("a"), "aa\n");
        assert_eq!(std::fs::read_to_string(p.join("b.txt")).expect("b"), "b\n");
    }

    /// §8.12 (detached-HEAD variant — reviewer should-fix): the same mid-sequence
    /// rollback but with HEAD DETACHED (as after `git checkout <sha>`). This is the
    /// one rollback HEAD-state the suite didn't cover — branch HEAD (§8.12) and
    /// unborn HEAD (§8.13) are already exercised. It locks the `else` arm of
    /// [`rollback`]/[`apply_composed_commits`] that re-points a detached HEAD via
    /// `set_head_detached` (a branch-only rollback would either error or wrongly
    /// move/create a branch). Group 2 references a staged-then-reverted file (in the
    /// change set, but workdir == HEAD, so after the index reset it nets to no change
    /// => `create_commit` `NothingToCommit`) to force the group-2 failure.
    #[test]
    fn apply_rolls_back_on_mid_sequence_failure_detached_head() {
        let dir = init_scratch();
        let p = dir.path();
        write(p, "a.txt", "a\n");
        write(p, "b.txt", "b\n");
        stage(p, &["a.txt", "b.txt"]);
        create_commit(p, "base", None, false).expect("commit");
        let orig = head_oid(p).expect("head");

        // Detach HEAD at the base commit (mirrors `git checkout <sha>`).
        {
            let repo = open_workdir_repo(p).expect("open");
            repo.set_head_detached(orig).expect("detach HEAD");
            assert!(
                !repo.head().expect("head").is_branch(),
                "precondition: HEAD is detached before apply"
            );
        }

        // a.txt: a genuine working-tree change (spans file #1). b.txt: staged then
        // reverted — its WORKDIR equals HEAD, so committing it after the reset is a
        // no-op (spans file #2, and forces the group-2 failure).
        write(p, "a.txt", "aa\n");
        write(p, "b.txt", "bb\n");
        stage(p, &["b.txt"]);
        write(p, "b.txt", "b\n");

        // Both are in the change set (pre-condition for this test's mechanism).
        let changed: Vec<String> = gather_worktree(p)
            .expect("gather")
            .iter()
            .map(|f| f.path.clone())
            .collect();
        assert!(
            changed.contains(&"a.txt".to_string()) && changed.contains(&"b.txt".to_string()),
            "both files must be in the change set: {changed:?}"
        );

        let plan = ComposePlan {
            groups: vec![
                group(&["a.txt"], "commit a"),
                group(&["b.txt"], "commit b (nets to no change)"),
            ],
        };
        let err = apply_composed_commits(p, &plan).expect_err("group 2 nets to no change");
        match err {
            AppError::Other(m) => assert!(m.contains("group 2"), "annotated with group index: {m}"),
            other => panic!("expected Other(group 2 ...), got {other:?}"),
        }

        // ROLLBACK proven on a DETACHED HEAD: HEAD restored to the original detached
        // oid, STILL detached (no branch created/moved), index back at HEAD, zero
        // commits landed.
        assert_eq!(
            head_oid(p).expect("head"),
            orig,
            "detached HEAD rolled back to the original oid"
        );
        {
            let repo = open_workdir_repo(p).expect("open");
            assert!(
                !repo.head().expect("head").is_branch(),
                "HEAD is STILL detached after rollback (not re-attached to a branch)"
            );
        }
        assert_eq!(commit_count(p), 1, "zero commits landed (only base remains)");
        assert_eq!(index_tree(p), head_tree(p), "index reset to HEAD");

        // WORKING TREE UNTOUCHED: all original on-disk content preserved.
        assert_eq!(std::fs::read_to_string(p.join("a.txt")).expect("a"), "aa\n");
        assert_eq!(std::fs::read_to_string(p.join("b.txt")).expect("b"), "b\n");
    }

    /// §8.13: unborn HEAD + 2 groups => 2 commits, the first is the root (0
    /// parents), each with its own delta; and a forced rollback from the unborn
    /// anchor returns HEAD to unborn + an empty index (working tree untouched).
    #[test]
    fn apply_first_commits_on_unborn_head() {
        // --- success path: 2 groups => 2 commits, first is root ---
        let dir = init_scratch();
        let p = dir.path();
        assert!(head_oid(p).is_none(), "starts unborn");
        write(p, "f1.txt", "1\n");
        write(p, "f2.txt", "2\n");
        let plan = ComposePlan {
            groups: vec![group(&["f1.txt"], "root: f1"), group(&["f2.txt"], "second: f2")],
        };
        let res = apply_composed_commits(p, &plan).expect("apply");
        assert_eq!(res.commits.len(), 2);
        assert_eq!(commit_count(p), 2);
        assert_eq!(head_oid(p).expect("head").to_string(), res.commits[1].oid);

        let repo = open_workdir_repo(p).expect("open");
        let root = repo
            .find_commit(git2::Oid::from_str(&res.commits[0].oid).expect("oid"))
            .expect("commit");
        assert_eq!(root.parent_count(), 0, "first commit is the root");
        assert_eq!(delta_paths(p, &res.commits[0].oid), vec!["f1.txt"]);
        assert_eq!(delta_paths(p, &res.commits[1].oid), vec!["f2.txt"]);

        // --- forced rollback from unborn: HEAD returns to unborn + empty index ---
        let dir2 = init_scratch();
        let p2 = dir2.path();
        write(p2, "g1.txt", "1\n");
        write(p2, "g2.txt", "2\n");
        let repo2 = open_workdir_repo(p2).expect("open");
        assert!(repo2.head().is_err(), "unborn anchor");
        // Mirror the apply loop up to a group-2 failure: reset (clear), land group
        // 1's root commit, then roll back from the `None` (unborn) anchor.
        reset_index_to_head(&repo2, None).expect("reset");
        stage(p2, &["g1.txt"]);
        create_commit(p2, "root: g1", None, false).expect("commit");
        assert!(head_oid(p2).is_some(), "root landed");
        rollback(&repo2, None).expect("rollback");

        assert!(head_oid(p2).is_none(), "HEAD back to unborn");
        assert_eq!(commit_count(p2), 0, "no commit reachable");
        let repo3 = open_workdir_repo(p2).expect("open");
        assert!(repo3.index().expect("index").is_empty(), "index emptied");
        assert_eq!(std::fs::read_to_string(p2.join("g1.txt")).expect("g1"), "1\n");
        assert_eq!(std::fs::read_to_string(p2.join("g2.txt")).expect("g2"), "2\n");
    }

    /// §8.14: a SUCCESSFUL apply never touches the working tree — every changed
    /// file's bytes on disk are byte-identical before and after (only index/refs
    /// move). Covers a tracked-modified file AND untracked additions.
    #[test]
    fn apply_does_not_touch_workdir() {
        let dir = init_scratch();
        let p = dir.path();
        write(p, "tracked.txt", "orig\n");
        stage(p, &["tracked.txt"]);
        create_commit(p, "base", None, false).expect("commit");

        // A tracked modification + two untracked additions.
        write(p, "tracked.txt", "modified body\n");
        write(p, "new1.txt", "new one\n");
        write(p, "new2.txt", "new two\n");
        let before: Vec<(String, String)> = ["tracked.txt", "new1.txt", "new2.txt"]
            .iter()
            .map(|f| (f.to_string(), std::fs::read_to_string(p.join(f)).expect("read")))
            .collect();

        let plan = ComposePlan {
            groups: vec![
                group(&["tracked.txt", "new1.txt"], "g1"),
                group(&["new2.txt"], "g2"),
            ],
        };
        apply_composed_commits(p, &plan).expect("apply");

        for (f, bytes) in &before {
            assert_eq!(
                &std::fs::read_to_string(p.join(f)).expect("read after"),
                bytes,
                "working-tree bytes of {f} must be byte-identical after apply"
            );
        }
    }

    /// §8.15: the result/plan/commit wire shapes are camelCase and match the TS
    /// types. `ComposePlan` DESERIALIZES (command input); the result/commit
    /// SERIALIZE (command output).
    #[test]
    fn apply_result_wire_shape_is_camel_case() {
        let v = serde_json::to_value(ComposeApplyResult {
            commits: vec![ComposeCommit {
                oid: "a".repeat(40),
                summary: "feat: x".to_string(),
            }],
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "commits": [{ "oid": "a".repeat(40), "summary": "feat: x" }] })
        );

        // ComposePlan deserializes from the exact JSON the TS `ComposePlan` sends.
        let plan: ComposePlan =
            serde_json::from_str(r#"{"groups":[{"files":["src/a.rs"],"message":"m"}]}"#)
                .expect("deserialize plan");
        assert_eq!(plan.groups.len(), 1);
        assert_eq!(plan.groups[0].files, vec!["src/a.rs".to_string()]);
        assert_eq!(plan.groups[0].message, "m");

        // ComposeCommit standalone casing.
        let c = serde_json::to_value(ComposeCommit {
            oid: "deadbeef".to_string(),
            summary: "s".to_string(),
        })
        .expect("json");
        assert_eq!(c, serde_json::json!({ "oid": "deadbeef", "summary": "s" }));
    }
}
