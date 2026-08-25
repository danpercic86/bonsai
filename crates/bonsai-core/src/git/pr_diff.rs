//! P89 PR local-diff engine.
//!
//! Auto-fetch a PR's base + head endpoints into namespaced local refs
//! (`refs/bonsai/pr/<n>/*`), then compute the **three-dot** base…head diff
//! (`merge_base(base, head)` → head) entirely locally. Forge providers supply a
//! neutral [`FetchTarget`] pair (see `bonsai-forge`); this module does the git
//! work. Pure git2 logic, no Tauri types; the command layer wraps every call in
//! `spawn_blocking`.
//!
//! Reuses the shared diff engine (`build_diff_options`, `apply_find_similar`,
//! `collect_headers`, `collect_file_diff`, `maybe_annotate`) and the credential
//! ladder (`acquire_cred` / `map_remote_err` / `evict_fresh_on_auth_fail`), so
//! PR diffs behave identically to commit/compare diffs and remote fetches.

use std::cell::RefCell;
use std::path::Path;

use serde::{Deserialize, Serialize};


use crate::error::AppError;
use crate::git::cred::{acquire_cred, evict_fresh_on_auth_fail, map_remote_err, CredAttempts};
use crate::git::diff::{
    apply_find_similar, build_diff_options, collect_file_diff, collect_headers, maybe_annotate,
    FileDiff, FileDiffHeader,
};
use crate::git::remote::open_repo_at;
use crate::git::stage::validate_rel_path;

/// A neutral fetch instruction: bring `resolve` (an oid) reachable locally.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchTarget {
    /// Fork/head clone URL to fetch from; `None` = use the repo's origin remote.
    pub url: Option<String>,
    /// Refspec, e.g. "+refs/pull/42/head:refs/bonsai/pr/42/head". Empty when the
    /// oid is expected already-local (base on same repo, or cache hit).
    pub refspec: String,
    /// Revision to resolve to an oid AFTER the fetch — normally the commit SHA.
    pub resolve: String,
}

/// Resolved base + head oids (both full 40-char hex).
#[derive(Debug, Clone)]
pub struct PrEndpoints {
    pub base_oid: String,
    pub head_oid: String,
}

/// Result of the local base…head diff (three-dot). Wire type (camelCase).
/// Serialize-only: `FileDiffHeader` (reused verbatim) is a Serialize-only wire
/// struct, so `PrDiffStats` is produced backend-side and never deserialized.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrDiffStats {
    pub additions: u32,
    pub deletions: u32,
    pub changed_files: u32,
    /// merge-base(base,head) — the OLD side of the diff. "" if unrelated histories.
    pub merge_base_oid: String,
    pub base_oid: String,
    pub head_oid: String,
    /// Sorted path-ascending; headers only (hunks fetched per file).
    pub files: Vec<FileDiffHeader>,
}

/// The origin remote name — the only remote a `url == None` target fetches from.
const ORIGIN: &str = "origin";

/// Fetch ONE `FetchTarget`'s refspec (anonymous remote when `url` is `Some`,
/// else the origin remote), with the shared credential ladder. Errors mapped via
/// `map_remote_err`; a fresh-fill auth failure evicts the cached credential.
fn fetch_target(repo: &git2::Repository, t: &FetchTarget) -> Result<(), AppError> {
    let attempts = RefCell::new(CredAttempts::default());
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(|url, username_from_url, allowed| {
        acquire_cred(repo.workdir(), &attempts, url, username_from_url, allowed)
    });
    let mut opts = git2::FetchOptions::new();
    opts.remote_callbacks(callbacks);
    // PR base/head refs are branch/pull refs, not tags — don't drag tags along.
    opts.download_tags(git2::AutotagOption::None);

    let mut remote = match &t.url {
        Some(url) => repo.remote_anonymous(url)?,
        None => match repo.find_remote(ORIGIN) {
            Ok(r) => r,
            Err(e) if e.code() == git2::ErrorCode::NotFound => {
                return Err(AppError::NoRemote(format!("remote '{ORIGIN}' not found")));
            }
            Err(e) => return Err(e.into()),
        },
    };

    let refspecs = [t.refspec.as_str()];
    if let Err(e) = remote.fetch(&refspecs, Some(&mut opts), None) {
        return Err(evict_fresh_on_auth_fail(repo, &attempts, map_remote_err(e, "pr-diff")));
    }
    Ok(())
}

/// Resolve a target to a full oid, peeling through refs/tags to a commit.
/// Errors if the revision is not present locally (used for the offline-fallback
/// probe: a local hit means we can skip a failed network fetch).
fn resolve_local(repo: &git2::Repository, rev: &str) -> Result<String, AppError> {
    let obj = repo.revparse_single(rev)?;
    let commit = obj.peel_to_commit()?;
    Ok(commit.id().to_string())
}

/// Bring ONE endpoint local and resolve it. If `refspec` is empty the oid is
/// expected to already exist. Otherwise fetch; on fetch failure fall back to a
/// cached local oid when present (offline / already-fetched), else propagate.
fn resolve_target(repo: &git2::Repository, t: &FetchTarget) -> Result<String, AppError> {
    if t.refspec.is_empty() {
        return resolve_local(repo, &t.resolve);
    }
    match fetch_target(repo, t) {
        Ok(()) => resolve_local(repo, &t.resolve),
        Err(fetch_err) => match resolve_local(repo, &t.resolve) {
            Ok(oid) => {
                eprintln!(
                    "bonsai: pr-diff fetch failed ({fetch_err}); using cached local oid for '{}'",
                    t.resolve
                );
                Ok(oid)
            }
            Err(_) => Err(fetch_err),
        },
    }
}

/// Blocking. Fetch base+head endpoints into the local object DB and resolve both
/// to oids. Offline fallback per [`resolve_target`].
pub fn fetch_pr_endpoints(
    workdir: &Path,
    base: &FetchTarget,
    head: &FetchTarget,
) -> Result<PrEndpoints, AppError> {
    let repo = open_repo_at(workdir)?;
    let base_oid = resolve_target(&repo, base)?;
    let head_oid = resolve_target(&repo, head)?;
    Ok(PrEndpoints { base_oid, head_oid })
}

/// merge-base(base, head) as a full oid, or `""` for unrelated histories.
fn merge_base_oid(
    repo: &git2::Repository,
    base: git2::Oid,
    head: git2::Oid,
) -> Result<String, AppError> {
    match repo.merge_base(base, head) {
        Ok(mb) => Ok(mb.to_string()),
        Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(String::new()),
        Err(e) => Err(e.into()),
    }
}

/// The old-side tree for the diff: the merge-base's tree, or `None` (empty tree)
/// when `merge_base_oid` is `""` (unrelated histories).
fn old_tree<'r>(
    repo: &'r git2::Repository,
    merge_base_oid: &str,
) -> Result<Option<git2::Tree<'r>>, AppError> {
    if merge_base_oid.is_empty() {
        return Ok(None);
    }
    let commit = repo.find_commit(git2::Oid::from_str(merge_base_oid)?)?;
    Ok(Some(commit.tree()?))
}

/// Blocking. Compute the base…head (three-dot) diff headers + counts. Bad oid →
/// `AppError::Git` (via `Oid::from_str` / `find_commit`).
pub fn pr_diff_headers(
    workdir: &Path,
    base_oid: &str,
    head_oid: &str,
) -> Result<PrDiffStats, AppError> {
    let repo = open_repo_at(workdir)?;
    let base = repo.find_commit(git2::Oid::from_str(base_oid)?)?;
    let head = repo.find_commit(git2::Oid::from_str(head_oid)?)?;

    let mb = merge_base_oid(&repo, base.id(), head.id())?;
    let old = old_tree(&repo, &mb)?;
    let new_tree = head.tree()?;

    let mut opts = build_diff_options(&[], false);
    let mut diff = repo.diff_tree_to_tree(old.as_ref(), Some(&new_tree), Some(&mut opts))?;
    apply_find_similar(&mut diff)?;
    let files = collect_headers(&diff)?;

    let additions = files.iter().fold(0u32, |a, f| a.saturating_add(f.additions));
    let deletions = files.iter().fold(0u32, |a, f| a.saturating_add(f.deletions));
    let changed_files = u32::try_from(files.len()).unwrap_or(u32::MAX);

    Ok(PrDiffStats {
        additions,
        deletions,
        changed_files,
        merge_base_oid: mb,
        base_oid: base.id().to_string(),
        head_oid: head.id().to_string(),
        files,
    })
}

/// Blocking. Hunks for ONE file of the merge_base…head diff. `merge_base_oid`
/// `""` ⇒ empty-tree old side. No matching delta ⇒ `AppError::Git`. Mirrors
/// `commit_file_diff`.
pub fn pr_file_diff(
    workdir: &Path,
    merge_base_oid: &str,
    head_oid: &str,
    path: &str,
    orig_path: Option<&str>,
    full_context: bool,
    intraline: bool,
) -> Result<FileDiff, AppError> {
    validate_rel_path(path)?;
    if let Some(op) = orig_path {
        validate_rel_path(op)?;
    }
    let repo = open_repo_at(workdir)?;
    let head = repo.find_commit(git2::Oid::from_str(head_oid)?)?;
    let new_tree = head.tree()?;
    let old = old_tree(&repo, merge_base_oid)?;

    let mut paths = vec![path];
    if let Some(op) = orig_path {
        if op != path {
            paths.push(op);
        }
    }
    let mut opts = build_diff_options(&paths, full_context);
    let mut diff = repo.diff_tree_to_tree(old.as_ref(), Some(&new_tree), Some(&mut opts))?;
    apply_find_similar(&mut diff)?;
    let fd = collect_file_diff(&diff)?
        .ok_or_else(|| AppError::Git(format!("path not changed in PR diff: {path}")))?;
    Ok(maybe_annotate(fd, intraline))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::branches::{checkout_branch, create_branch};
    use crate::git::commit::create_commit;
    use crate::git::stage::stage_paths;
    use crate::git::status::FileStatus;

    fn init_scratch() -> tempfile::TempDir {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        dir
    }

    /// A feature branch diverges from base: base…head is the THREE-DOT diff, so
    /// a commit landed on base AFTER the fork must NOT appear in the PR diff.
    #[test]
    fn pr_diff_is_three_dot_from_merge_base() {
        let dir = init_scratch();
        let p = dir.path();

        // O: base commit with file1.
        std::fs::write(p.join("file1.txt"), "one\n").expect("write");
        stage_paths(p, &["file1.txt".into()]).expect("stage");
        let o = create_commit(p, "O", None, false).expect("commit O");
        let main_name = o.branch.expect("base branch");

        // feat forks at O and adds file_feat.
        create_branch(p, "feat").expect("create feat");
        checkout_branch(p, "feat").expect("checkout feat");
        std::fs::write(p.join("file_feat.txt"), "feat\n").expect("write");
        stage_paths(p, &["file_feat.txt".into()]).expect("stage");
        let head = create_commit(p, "H", None, false).expect("commit H").oid;

        // base advances past the fork with file_base (must NOT show in base…head).
        checkout_branch(p, &main_name).expect("checkout base");
        std::fs::write(p.join("file_base.txt"), "base\n").expect("write");
        stage_paths(p, &["file_base.txt".into()]).expect("stage");
        let base = create_commit(p, "B", None, false).expect("commit B").oid;

        let stats = pr_diff_headers(p, &base, &head).expect("pr diff");
        assert_eq!(stats.merge_base_oid, o.oid, "merge-base is the fork point");
        let paths: Vec<&str> = stats.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["file_feat.txt"], "only the PR's own change: {paths:?}");
        assert_eq!(stats.changed_files, 1);
        assert_eq!(stats.additions, 1);
        assert_eq!(stats.deletions, 0);
        assert_eq!(stats.files[0].status, FileStatus::Added);

        // Per-file hunks for the changed file.
        let fd = pr_file_diff(p, &stats.merge_base_oid, &head, "file_feat.txt", None, false, false)
            .expect("file diff");
        assert_eq!(fd.path, "file_feat.txt");
        assert_eq!(fd.hunks.len(), 1);

        // A path not in the diff errors.
        let err = pr_file_diff(p, &stats.merge_base_oid, &head, "file_base.txt", None, false, false)
            .expect_err("unchanged path must error");
        assert!(matches!(err, AppError::Git(_)), "got {err:?}");
    }

    /// Unrelated histories → empty merge-base string; everything in head shows
    /// as Added against the empty tree.
    #[test]
    fn pr_diff_unrelated_histories_empty_merge_base() {
        let dir = init_scratch();
        let p = dir.path();

        std::fs::write(p.join("a.txt"), "a\n").expect("write");
        stage_paths(p, &["a.txt".into()]).expect("stage");
        let base = create_commit(p, "base", None, false).expect("commit").oid;

        // An orphan commit with no shared ancestry, created directly via git2.
        let repo = git2::Repository::open(p).expect("open");
        let head = {
            let mut idx = repo.index().expect("index");
            std::fs::write(p.join("b.txt"), "b\n").expect("write");
            idx.add_path(std::path::Path::new("b.txt")).expect("add");
            let tree_oid = idx.write_tree().expect("write tree");
            let tree = repo.find_tree(tree_oid).expect("tree");
            let sig = git2::Signature::now("T", "t@e.com").expect("sig");
            repo.commit(None, &sig, &sig, "orphan", &tree, &[])
                .expect("orphan commit")
                .to_string()
        };

        let stats = pr_diff_headers(p, &base, &head).expect("pr diff");
        assert_eq!(stats.merge_base_oid, "", "unrelated ⇒ empty merge-base");
        // vs empty tree, head's b.txt is Added.
        assert!(stats.files.iter().any(|f| f.path == "b.txt" && f.status == FileStatus::Added));

        let fd = pr_file_diff(p, "", &head, "b.txt", None, false, false).expect("file diff");
        assert_eq!(fd.path, "b.txt");
    }

    // ---- ground-truth integration tests (vs the real `git` CLI) ----

    /// Run `git <args>` in `dir` and return trimmed stdout; panics on failure.
    fn git_out(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim_end().to_string()
    }

    /// Sum numeric additions/deletions from `git diff --numstat` (binary rows are
    /// "-\t-\t…" and contribute 0, matching the engine).
    fn numstat_totals(numstat: &str) -> (u32, u32) {
        let mut adds = 0u32;
        let mut dels = 0u32;
        for line in numstat.lines() {
            let mut f = line.split('\t');
            let a = f.next().unwrap_or("-");
            let d = f.next().unwrap_or("-");
            adds += a.parse::<u32>().unwrap_or(0);
            dels += d.parse::<u32>().unwrap_or(0);
        }
        (adds, dels)
    }

    /// KEY CORRECTNESS PROPERTY: `pr_diff_headers` matches `git diff base...head`
    /// (three-dot) exactly — additions/deletions/changed-files AND the file set —
    /// on a history where base advanced past the fork. A two-dot implementation
    /// would wrongly fold in the base-side changes and FAIL this test.
    #[test]
    fn pr_diff_matches_git_three_dot_ground_truth() {
        let dir = init_scratch();
        let p = dir.path();

        // O: base commit with several tracked files.
        std::fs::write(p.join("common.txt"), "line1\nline2\nline3\n").expect("w");
        std::fs::write(p.join("oldname.txt"), "rename me\nkeep\n").expect("w");
        std::fs::write(p.join("todelete.txt"), "bye\n").expect("w");
        stage_paths(
            p,
            &["common.txt".into(), "oldname.txt".into(), "todelete.txt".into()],
        )
        .expect("stage");
        let o = create_commit(p, "O", None, false).expect("commit O");
        let main_name = o.branch.expect("base branch");

        // feat forks at O: modify common, add a file, rename oldname, delete a file.
        create_branch(p, "feat").expect("create feat");
        checkout_branch(p, "feat").expect("checkout feat");
        std::fs::write(p.join("common.txt"), "line1\nCHANGED\nline3\nline4\n").expect("w");
        std::fs::write(p.join("file_feat.txt"), "brand new\n").expect("w");
        std::fs::rename(p.join("oldname.txt"), p.join("newname.txt")).expect("rename");
        std::fs::remove_file(p.join("todelete.txt")).expect("rm");
        // git add -A picks up the rename/delete/add/modify set.
        git_out(p, &["add", "-A"]);
        let head = create_commit(p, "H", None, false).expect("commit H").oid;

        // base advances PAST the fork — these must NOT appear in base...head.
        checkout_branch(p, &main_name).expect("checkout base");
        std::fs::write(p.join("common.txt"), "line1\nline2\nline3\nBASE_ONLY\n").expect("w");
        std::fs::write(p.join("file_base.txt"), "base side\n").expect("w");
        stage_paths(p, &["common.txt".into(), "file_base.txt".into()]).expect("stage");
        let base = create_commit(p, "B", None, false).expect("commit B").oid;

        // Ground truth from the git CLI (three-dot, with rename detection).
        let numstat = git_out(p, &["diff", "--numstat", "-M", &format!("{base}...{head}")]);
        let name_status = git_out(p, &["diff", "--name-status", "-M", &format!("{base}...{head}")]);
        let (git_adds, git_dels) = numstat_totals(&numstat);
        // name-status renders renames as `R100\told\tnew` (tab-separated), so the
        // last tab field is always the NEW path git attributes the change to.
        let git_files: std::collections::BTreeSet<&str> = name_status
            .lines()
            .map(|l| l.rsplit('\t').next().unwrap_or(""))
            .collect();

        // Engine.
        let stats = pr_diff_headers(p, &base, &head).expect("pr diff");
        assert_eq!(stats.merge_base_oid, o.oid, "merge-base is the fork point O");
        assert_eq!(stats.additions, git_adds, "additions vs git three-dot\n{numstat}");
        assert_eq!(stats.deletions, git_dels, "deletions vs git three-dot\n{numstat}");
        assert_eq!(
            stats.changed_files as usize,
            git_files.len(),
            "changed-files count vs git three-dot\n{name_status}"
        );

        // File set: engine reports new paths; must equal git's three-dot set and
        // must EXCLUDE base-side files (file_base.txt, base-only common change kept
        // via the same path but its content matches head only through merge-base).
        let engine_files: std::collections::BTreeSet<&str> =
            stats.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(engine_files, git_files, "file set mismatch");
        assert!(
            !engine_files.contains("file_base.txt"),
            "base-side file leaked into three-dot diff: {engine_files:?}"
        );
        assert!(engine_files.contains("newname.txt"), "rename target present");
        assert!(engine_files.contains("file_feat.txt"));

        // Rename is detected as Renamed with the original path preserved.
        let renamed = stats
            .files
            .iter()
            .find(|f| f.path == "newname.txt")
            .expect("renamed entry");
        assert_eq!(renamed.status, FileStatus::Renamed, "{name_status}");
        assert_eq!(renamed.orig_path.as_deref(), Some("oldname.txt"));

        // Deleted / added statuses.
        assert_eq!(
            stats.files.iter().find(|f| f.path == "todelete.txt").unwrap().status,
            FileStatus::Deleted
        );
        assert_eq!(
            stats.files.iter().find(|f| f.path == "file_feat.txt").unwrap().status,
            FileStatus::Added
        );

        // Per-file hunks for the modified file resolve against the merge-base.
        let fd = pr_file_diff(p, &stats.merge_base_oid, &head, "common.txt", None, false, false)
            .expect("file diff");
        assert_eq!(fd.path, "common.txt");
        assert!(!fd.hunks.is_empty(), "modified file has hunks");
    }

    /// A binary file added on the head branch is flagged binary with 0/0 line
    /// counts, matching git's "-" numstat rows.
    #[test]
    fn pr_diff_binary_file_is_flagged() {
        let dir = init_scratch();
        let p = dir.path();

        std::fs::write(p.join("seed.txt"), "seed\n").expect("w");
        stage_paths(p, &["seed.txt".into()]).expect("stage");
        let o = create_commit(p, "O", None, false).expect("commit O");
        let main_name = o.branch.expect("branch");

        create_branch(p, "feat").expect("create feat");
        checkout_branch(p, "feat").expect("checkout feat");
        // NUL bytes ⇒ git + libgit2 treat as binary.
        std::fs::write(p.join("blob.bin"), [0u8, 1, 2, 0, 255, 0, 42]).expect("w");
        stage_paths(p, &["blob.bin".into()]).expect("stage");
        let head = create_commit(p, "H", None, false).expect("commit H").oid;

        checkout_branch(p, &main_name).expect("checkout base");
        let base = o.oid.clone();

        let stats = pr_diff_headers(p, &base, &head).expect("pr diff");
        let bin = stats
            .files
            .iter()
            .find(|f| f.path == "blob.bin")
            .expect("binary entry");
        assert!(bin.binary, "binary flag set");
        assert_eq!(bin.additions, 0);
        assert_eq!(bin.deletions, 0);
        assert_eq!(bin.status, FileStatus::Added);

        // git agrees it is binary ("-\t-\tblob.bin").
        let numstat = git_out(p, &["diff", "--numstat", &format!("{base}...{head}")]);
        assert!(
            numstat.lines().any(|l| l.starts_with("-\t-\t") && l.ends_with("blob.bin")),
            "git numstat binary row: {numstat}"
        );
    }

    /// A bad head oid maps to `AppError::Git`.
    #[test]
    fn pr_diff_bad_oid_errors() {
        let dir = init_scratch();
        let p = dir.path();
        std::fs::write(p.join("a.txt"), "a\n").expect("write");
        stage_paths(p, &["a.txt".into()]).expect("stage");
        let base = create_commit(p, "base", None, false).expect("commit").oid;

        let err = pr_diff_headers(p, &base, "notahexoid").expect_err("bad oid");
        assert!(matches!(err, AppError::Git(_)), "got {err:?}");
    }
}
