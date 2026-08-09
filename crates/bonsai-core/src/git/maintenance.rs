//! Best-effort git commit-graph maintenance (P52). Shells
//! `git commit-graph write --reachable --changed-paths` to (re)write
//! `.git/objects/info/commit-graph`. libgit2 (v1.8, git2 0.21) consumes that
//! file UNCONDITIONALLY when present (no `core.commitGraph` gate), so the git2
//! revwalk in `graph::compute_graph` and the merge-base / ahead-behind in
//! `health` get faster for free — with NO behavioural change (the graph is a
//! pure ODB-level optimization: identical output, only fewer object inflates).
//!
//! Best-effort by design: git absent, a spawn failure, or a non-zero exit is
//! reported as [`CommitGraphOutcome::Skipped`] and NEVER surfaced as an error —
//! libgit2 still works without the file. Trigger sites fire this off the UI
//! path in an un-awaited `spawn_blocking` and discard the outcome (`let _ = …`).

use std::path::Path;

use crate::git::search::{GitRunner, SpawnGitRunner};

/// Repo-relative path of the single-file commit-graph (used by tests + the
/// fixture existence-guard). Non-`--split` writes land here.
pub const COMMIT_GRAPH_REL: &str = ".git/objects/info/commit-graph";

/// Outcome of a write attempt. Best-effort ⇒ never an `Err`. Trigger sites
/// discard it (`let _ = …`); tests assert `Written` under a `have_git` guard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitGraphOutcome {
    /// `git commit-graph write` ran and exited 0 (the file may still be absent
    /// for an unborn / commit-less repo — git writes nothing then).
    Written,
    /// git not on PATH, spawn failure, or non-zero exit. String = a short
    /// reason for optional debug logging; NOT surfaced to the user.
    Skipped(String),
}

/// The exact argv (injection-free — no user input). Pure; unit-tested:
/// `["commit-graph", "write", "--reachable", "--changed-paths"]`.
///
/// `--reachable` rewrites the whole graph from all refs (single file, no
/// `--split` chain in v1); `--changed-paths` adds Bloom filters that accelerate
/// the shelled `git log -- <pathspec>` search (libgit2 ignores them but reads
/// the base graph).
pub fn commit_graph_args() -> Vec<String> {
    ["commit-graph", "write", "--reachable", "--changed-paths"]
        .into_iter()
        .map(String::from)
        .collect()
}

/// Blocking. (Re)writes the commit-graph for the repo at `workdir` via `runner`
/// (`runner.run(&commit_graph_args(), workdir)`). NEVER returns `Err`:
/// `Ok(_)` → [`CommitGraphOutcome::Written`], `Err(_)` →
/// [`CommitGraphOutcome::Skipped`]. No panics, no `?`.
pub fn write_commit_graph(workdir: &Path, runner: &dyn GitRunner) -> CommitGraphOutcome {
    match runner.run(&commit_graph_args(), workdir) {
        Ok(_) => CommitGraphOutcome::Written,
        Err(e) => {
            // Best-effort optimization: a skip is never surfaced to the user
            // (libgit2 works without the file), but emit the reason to stderr
            // for optional debug logging instead of dropping it silently — the
            // "reason for optional debug logging" the `Skipped` doc promises.
            // (bonsai-core carries no `log`/`tracing` dep; eprintln! mirrors the
            // other git/ diagnostic sites.)
            let reason = e.to_string();
            eprintln!(
                "bonsai: commit-graph write skipped for {}: {reason}",
                workdir.display()
            );
            CommitGraphOutcome::Skipped(reason)
        }
    }
}

/// Convenience for the fire-and-forget trigger sites: runs with the real
/// [`SpawnGitRunner`] (capture output, `GIT_TERMINAL_PROMPT=0`,
/// `CREATE_NO_WINDOW` on Windows) so callers never import from `search`.
/// `SpawnGitRunner` sets `current_dir(workdir)`; git resolves `.git` from there
/// and Bonsai only opens non-bare working copies, so `workdir` is the repo root.
pub fn write_commit_graph_best_effort(workdir: &Path) -> CommitGraphOutcome {
    write_commit_graph(workdir, &SpawnGitRunner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::process::Command;

    fn have_git() -> bool {
        let ok = Command::new("git").arg("--version").output().is_ok();
        if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
            panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
        }
        ok
    }

    // ---------------------------------------------------------- fake runners

    /// Always errors — simulates `git` absent / a non-zero exit WITHOUT a real
    /// subprocess, so the clean-skip contract is provable offline.
    struct ErrRunner;
    impl GitRunner for ErrRunner {
        fn run(&self, _args: &[String], _cwd: &Path) -> Result<String, AppError> {
            Err(AppError::Git("boom".to_string()))
        }
    }

    /// Records the exact argv + cwd handed to `run` and reports success.
    #[derive(Default)]
    struct RecordingRunner {
        calls: RefCell<Vec<(Vec<String>, PathBuf)>>,
    }
    impl GitRunner for RecordingRunner {
        fn run(&self, args: &[String], cwd: &Path) -> Result<String, AppError> {
            self.calls
                .borrow_mut()
                .push((args.to_vec(), cwd.to_path_buf()));
            Ok(String::new())
        }
    }

    // ---------------------------------------------------------- pure / offline

    #[test]
    fn commit_graph_args_are_exact() {
        let expected: Vec<String> = ["commit-graph", "write", "--reachable", "--changed-paths"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(commit_graph_args(), expected);
    }

    #[test]
    fn git_absent_or_failure_skips_cleanly() {
        // A runner error (git absent / non-zero exit) maps to Skipped — never
        // an Err, never a panic. Proves the best-effort no-git degrade.
        let outcome = write_commit_graph(Path::new("/nonexistent/repo"), &ErrRunner);
        match outcome {
            CommitGraphOutcome::Skipped(msg) => assert!(msg.contains("boom"), "reason: {msg}"),
            other => panic!("expected Skipped, got {other:?}"),
        }
    }

    #[test]
    fn argv_passed_to_runner_is_exact() {
        // The runner receives EXACTLY commit_graph_args() in the given cwd.
        let runner = RecordingRunner::default();
        let workdir = Path::new("/tmp/some/repo");
        assert_eq!(
            write_commit_graph(workdir, &runner),
            CommitGraphOutcome::Written
        );
        let calls = runner.calls.borrow();
        assert_eq!(calls.len(), 1, "exactly one git invocation");
        assert_eq!(calls[0].0, commit_graph_args());
        assert_eq!(calls[0].1, workdir.to_path_buf());
    }

    // ---------------------------------------------------------- git fixtures

    /// Init a `main`-headed repo with a pinned identity + `core.autocrlf=false`
    /// (mirrors `search.rs`) so commit/layout ordering is deterministic.
    fn init_repo(dir: &Path) -> git2::Repository {
        let repo = git2::Repository::init_opts(
            dir,
            git2::RepositoryInitOptions::new().initial_head("main"),
        )
        .expect("init repo");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "Test User").expect("name");
            cfg.set_str("user.email", "test@example.com").expect("email");
            cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        }
        repo
    }

    /// Commit `files` (built from the FIRST parent's tree, or empty) onto
    /// `update_ref` with author=committer time pinned to `t`. Supports multiple
    /// parents (merge commits) so the fixture exercises a fork + merge.
    fn commit_on(
        repo: &git2::Repository,
        update_ref: &str,
        parents: &[git2::Oid],
        files: &[(&str, &str)],
        msg: &str,
        t: i64,
    ) -> git2::Oid {
        let sig = git2::Signature::new("Test User", "test@example.com", &git2::Time::new(t, 0))
            .expect("sig");
        let base_tree = parents
            .first()
            .and_then(|p| repo.find_commit(*p).ok())
            .and_then(|c| c.tree().ok());
        let mut tb = match &base_tree {
            Some(tree) => repo.treebuilder(Some(tree)).expect("treebuilder"),
            None => repo.treebuilder(None).expect("treebuilder"),
        };
        for (name, content) in files {
            let blob = repo.blob(content.as_bytes()).expect("blob");
            tb.insert(name, blob, 0o100_644).expect("insert");
        }
        let tree = repo.find_tree(tb.write().expect("write tree")).expect("tree");
        let parent_commits: Vec<git2::Commit> = parents
            .iter()
            .map(|p| repo.find_commit(*p).expect("parent"))
            .collect();
        let parent_refs: Vec<&git2::Commit> = parent_commits.iter().collect();
        repo.commit(Some(update_ref), &sig, &sig, msg, &tree, &parent_refs)
            .expect("commit")
    }

    /// Fork + merge fixture: `main` = c0→c1→c2→merge; `feature` = c1→c3; the
    /// merge has parents [c2, c3]. Non-trivial (≥5 nodes, ≥2 lanes, a
    /// multi-parent commit) so the before/after layout equality is meaningful.
    fn build_graph_fixture() -> tempfile::TempDir {
        let dir = crate::testutil::scratch_dir();
        let repo = init_repo(dir.path());
        let c0 = commit_on(&repo, "refs/heads/main", &[], &[("a.txt", "a0\n")], "root", 1000);
        let c1 = commit_on(&repo, "refs/heads/main", &[c0], &[("a.txt", "a1\n")], "second", 2000);
        repo.reference("refs/heads/feature", c1, false, "branch feature")
            .expect("feature ref");
        let c2 = commit_on(&repo, "refs/heads/main", &[c1], &[("a.txt", "a2\n")], "main work", 3000);
        let c3 = commit_on(&repo, "refs/heads/feature", &[c1], &[("b.txt", "b\n")], "feat work", 4000);
        commit_on(
            &repo,
            "refs/heads/main",
            &[c2, c3],
            &[("a.txt", "a2\n"), ("b.txt", "b\n")],
            "merge feature",
            5000,
        );
        dir
    }

    // ---------------------------------------------------------- git-guarded

    #[test]
    fn write_produces_commit_graph_file() {
        if !have_git() {
            eprintln!("skipping: `git` CLI not found");
            return;
        }
        let dir = build_graph_fixture();
        let workdir = dir.path();
        assert_eq!(
            write_commit_graph_best_effort(workdir),
            CommitGraphOutcome::Written
        );
        assert!(
            workdir.join(COMMIT_GRAPH_REL).exists(),
            "commit-graph file must exist after a successful write"
        );
    }

    #[test]
    fn revwalk_layout_identical_with_and_without_graph() {
        // Load-bearing correctness oracle: the commit-graph is a pure
        // optimization ⇒ compute_graph must return a byte-identical GraphLayout
        // before and after the write (same nodes/edges/lanes/refs/head).
        if !have_git() {
            eprintln!("skipping: `git` CLI not found");
            return;
        }
        let dir = build_graph_fixture();
        let workdir = dir.path();

        let before = crate::graph::compute_graph(workdir).expect("layout before");
        // Non-degenerate: the fixture really is a fork+merge (else equality is
        // trivially true and proves nothing).
        assert!(before.nodes.len() >= 5, "fixture should have >= 5 commits");
        assert!(before.lane_count >= 2, "fork+merge ⇒ at least 2 lanes");
        assert!(
            before.nodes.iter().any(|n| n.parents.len() >= 2),
            "fixture must contain a merge (multi-parent) commit"
        );

        assert_eq!(
            write_commit_graph_best_effort(workdir),
            CommitGraphOutcome::Written
        );
        assert!(
            workdir.join(COMMIT_GRAPH_REL).exists(),
            "commit-graph file must exist after the write"
        );

        let after = crate::graph::compute_graph(workdir).expect("layout after");
        assert_eq!(
            before, after,
            "commit-graph must not change layout output — pure optimization"
        );
    }

    #[test]
    fn branches_scan_identical_with_and_without_graph() {
        // Proves the merge-base / ahead-behind results behind the health
        // branches section are unchanged by the graph (only faster).
        if !have_git() {
            eprintln!("skipping: `git` CLI not found");
            return;
        }
        let dir = build_graph_fixture();
        let workdir = dir.path();

        let before = crate::health::collect_repo_health(workdir).branches.data;
        assert!(before.is_some(), "branches section should collect");
        assert_eq!(
            write_commit_graph_best_effort(workdir),
            CommitGraphOutcome::Written
        );
        let after = crate::health::collect_repo_health(workdir).branches.data;
        assert_eq!(
            before, after,
            "commit-graph must not change branch scan results — pure optimization"
        );
    }
}
