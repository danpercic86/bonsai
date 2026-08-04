//! Synthetic-history fixture generator for the M2d perf gate (contract §5.1).
//!
//! git2 only — no CLI, no working-directory churn: blobs via `repo.blob()`,
//! trees via `repo.treebuilder(None)`, commits via `repo.commit(None, ...)`;
//! refs are written at the end. Signature times are `1_600_000_000 +
//! counter * 60`, strictly increasing across the whole history.
//!
//! This module is `#[doc(hidden)]` but unconditionally compiled so both the
//! criterion bench and the `#[ignore]`d gate test can reach it without
//! feature juggling (contract §8.9).

use std::path::{Path, PathBuf};

use crate::error::AppError;

/// Shape of the synthetic history. Defaults produce the 31 000-commit gate
/// fixture: 20 000 main commits + 400 feature branches × 20 commits (forked
/// at every 50th main commit, merged 30 main-commits later) + 3 long-lived
/// unmerged 1 000-commit branches + 100 lightweight tags.
#[derive(Debug, Clone)]
pub struct FixtureSpec {
    pub main_len: usize,
    /// Fork a feature branch at every `branch_every`-th main commit.
    pub branch_every: usize,
    /// Feature commits per branch.
    pub branch_len: usize,
    /// The merge lands this many main-commits after the fork point.
    pub merge_after: usize,
    /// Long-lived, never-merged branches (fork from main commits 100, 200, …).
    pub long_branches: usize,
    pub long_branch_len: usize,
    /// Lightweight tag `v{i}` on every `tag_every`-th main commit.
    pub tag_every: usize,
    /// Every n-th merged feature keeps its `refs/heads/feat-{k}` ref.
    pub keep_branch_ref_every: usize,
}

impl Default for FixtureSpec {
    fn default() -> Self {
        FixtureSpec {
            main_len: 20_000,
            branch_every: 50,
            branch_len: 20,
            merge_after: 30,
            long_branches: 3,
            long_branch_len: 1_000,
            tag_every: 200,
            keep_branch_ref_every: 10,
        }
    }
}

impl FixtureSpec {
    /// Total number of commits the spec generates (main commits include the
    /// merge commits; every feature branch — merged or not — stays reachable).
    pub fn total_commits(&self) -> usize {
        let features = self.main_len.checked_div(self.branch_every).unwrap_or(0);
        self.main_len + features * self.branch_len + self.long_branches * self.long_branch_len
    }
}

const BASE_TS: i64 = 1_600_000_000;

struct CommitFactory<'r> {
    repo: &'r git2::Repository,
    counter: i64,
}

impl CommitFactory<'_> {
    /// One commit from an in-memory tree (`n.txt` = the global counter).
    fn commit(&mut self, parents: &[git2::Oid]) -> Result<git2::Oid, AppError> {
        self.counter += 1;
        let blob = self.repo.blob(self.counter.to_string().as_bytes())?;
        let mut tb = self.repo.treebuilder(None)?;
        tb.insert("n.txt", blob, 0o100_644)?;
        let tree = self.repo.find_tree(tb.write()?)?;
        let sig = git2::Signature::new(
            "Fixture Bot",
            "fixture@bonsai.local",
            &git2::Time::new(BASE_TS + self.counter * 60, 0),
        )?;
        let parent_commits = parents
            .iter()
            .map(|p| self.repo.find_commit(*p))
            .collect::<Result<Vec<_>, _>>()?;
        let parent_refs: Vec<&git2::Commit> = parent_commits.iter().collect();
        let oid = self.repo.commit(
            None,
            &sig,
            &sig,
            &format!("commit {}", self.counter),
            &tree,
            &parent_refs,
        )?;
        Ok(oid)
    }
}

/// Creates a repo at `path` with the synthetic history. Errors if `path`
/// already contains anything. Merge commits on main have parents
/// `[prev_main, feature_tip]`; unmerged feature branches keep their ref so
/// every generated commit stays reachable from the ref set.
pub fn generate_fixture(path: &Path, spec: &FixtureSpec) -> Result<(), AppError> {
    if path.exists() && std::fs::read_dir(path)?.next().is_some() {
        return Err(AppError::Other(format!(
            "fixture path is not empty: {}",
            path.display()
        )));
    }
    if spec.main_len == 0 {
        return Err(AppError::Other("fixture spec: main_len must be > 0".into()));
    }

    let repo = git2::Repository::init(path)?;
    // Route all object writes through an in-memory mempack backend and dump
    // one packfile at the end: ~93k loose-object files would make generation
    // AND every later revwalk pathologically slow on Windows (and real repos
    // are packed anyway, so the perf gate measures the realistic case).
    let odb = repo.odb()?;
    let mempack = odb.add_new_mempack_backend(1000)?;
    let mut factory = CommitFactory {
        repo: &repo,
        counter: 0,
    };

    let mut main_oids: Vec<git2::Oid> = Vec::with_capacity(spec.main_len);
    // main index (1-based) -> feature tip to merge there
    let mut pending_merges: std::collections::HashMap<usize, git2::Oid> =
        std::collections::HashMap::new();
    let mut feature_count = 0usize;
    let mut kept_refs: Vec<(String, git2::Oid)> = Vec::new();

    for i in 1..=spec.main_len {
        let prev = main_oids.last().copied();
        let oid = if let Some(feature_tip) = pending_merges.remove(&i) {
            let prev = prev.ok_or_else(|| {
                AppError::Other("fixture: merge scheduled before any main commit".into())
            })?;
            factory.commit(&[prev, feature_tip])?
        } else {
            match prev {
                Some(p) => factory.commit(&[p])?,
                None => factory.commit(&[])?,
            }
        };
        main_oids.push(oid);

        // Fork a feature branch at every branch_every-th main commit.
        if spec.branch_every > 0 && i % spec.branch_every == 0 {
            feature_count += 1;
            let k = feature_count;
            let mut tip = oid;
            for _ in 0..spec.branch_len {
                tip = factory.commit(&[tip])?;
            }
            let merge_at = i + spec.merge_after;
            if merge_at <= spec.main_len {
                pending_merges.insert(merge_at, tip);
                if spec.keep_branch_ref_every > 0 && k.is_multiple_of(spec.keep_branch_ref_every)
                {
                    kept_refs.push((format!("refs/heads/feat-{k}"), tip));
                }
            } else {
                // Never merged — keep the ref so the commits stay reachable.
                kept_refs.push((format!("refs/heads/feat-{k}"), tip));
            }
        }
    }

    // Long-lived unmerged branches, forked from main commits 100, 200, 300, …
    // (clamped to the chain length for small specs).
    for j in 0..spec.long_branches {
        let fork_idx = ((j + 1) * 100).min(spec.main_len);
        let mut tip = main_oids[fork_idx - 1];
        for _ in 0..spec.long_branch_len {
            tip = factory.commit(&[tip])?;
        }
        kept_refs.push((format!("refs/heads/long-{j}"), tip));
    }

    // Persist the in-memory objects as a single packfile (pack + index land
    // in .git/objects/pack) BEFORE writing refs.
    {
        use std::io::Write;
        let mut buf = git2::Buf::new();
        mempack.dump(&repo, &mut buf)?;
        let mut writer = odb.packwriter()?;
        writer
            .write_all(&buf)
            .map_err(|e| AppError::Io(e.to_string()))?;
        writer.commit()?;
    }

    // Lightweight tag v{t} on every tag_every-th main commit.
    if spec.tag_every > 0 {
        let mut t = 0usize;
        let mut i = spec.tag_every;
        while i <= spec.main_len {
            t += 1;
            repo.reference(
                &format!("refs/tags/v{t}"),
                main_oids[i - 1],
                true,
                "fixture tag",
            )?;
            i += spec.tag_every;
        }
    }

    let last_main = main_oids.last().copied().ok_or_else(|| {
        AppError::Other("fixture: main chain is empty after generation".into())
    })?;
    repo.reference("refs/heads/main", last_main, true, "fixture main")?;
    repo.set_head("refs/heads/main")?;
    for (name, oid) in kept_refs {
        repo.reference(&name, oid, true, "fixture branch")?;
    }
    Ok(())
}

/// Returns the shared cached gate fixture (default spec), generating it on
/// first use under `<target-dir>/graph-fixture/repo`. A `COMPLETE` marker
/// guards against reusing a half-generated repo from an interrupted run.
/// Prints the generation time to stderr.
pub fn ensure_default_fixture() -> Result<PathBuf, AppError> {
    // Benches/gate tests must measure the app's real configuration.
    crate::git::relax_odb_hash_verification();
    // Tests within one binary run on parallel threads; serialize generation
    // so two callers cannot race on the shared cache directory.
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK
        .lock()
        .map_err(|_| AppError::Other("fixture lock poisoned".into()))?;

    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("target"));
    let dir = target_dir.join("graph-fixture");
    let repo_path = dir.join("repo");
    let marker = dir.join("COMPLETE");
    if marker.exists() && repo_path.exists() {
        return Ok(repo_path);
    }
    if repo_path.exists() {
        std::fs::remove_dir_all(&repo_path)?;
    }
    if marker.exists() {
        std::fs::remove_file(&marker)?;
    }
    std::fs::create_dir_all(&repo_path)?;

    let spec = FixtureSpec::default();
    let start = std::time::Instant::now();
    generate_fixture(&repo_path, &spec)?;
    std::fs::write(&marker, b"ok")?;
    eprintln!(
        "[fixture] generated {}-commit fixture in {:.1}s at {}",
        spec.total_commits(),
        start.elapsed().as_secs_f64(),
        repo_path.display()
    );
    Ok(repo_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::compute_graph;

    /// Small-spec sanity check (contract §5.1): commit count, ref count, and
    /// a successful non-truncated layout.
    #[test]
    fn small_fixture_generates_and_lays_out() {
        let spec = FixtureSpec {
            main_len: 200,
            branch_every: 50,
            branch_len: 10,
            merge_after: 30,
            long_branches: 2,
            long_branch_len: 40,
            tag_every: 100,
            keep_branch_ref_every: 2,
        };
        // 200 main + 4 features (forks at 50/100/150/200) * 10 + 2 * 40 long.
        assert_eq!(spec.total_commits(), 320);

        let dir = tempfile::TempDir::new().expect("tempdir");
        generate_fixture(dir.path(), &spec).expect("generate");

        let repo = git2::Repository::open(dir.path()).expect("open");
        // Refs: main, long-0, long-1, feat-2 (merged, kept), feat-4 (unmerged,
        // kept — forks at 200, merge point 230 > main_len), tags v1 + v2.
        let mut names: Vec<String> = repo
            .references()
            .expect("refs")
            .filter_map(|r| r.ok().and_then(|r| r.name().ok().map(str::to_owned)))
            .filter(|n| n != "HEAD")
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "refs/heads/feat-2",
                "refs/heads/feat-4",
                "refs/heads/long-0",
                "refs/heads/long-1",
                "refs/heads/main",
                "refs/tags/v1",
                "refs/tags/v2",
            ]
        );

        let layout = compute_graph(dir.path()).expect("layout");
        assert!(!layout.truncated);
        assert_eq!(layout.nodes.len(), 320);
        assert!(layout.lane_count >= 2, "expected parallel lanes");
        // HEAD is refs/heads/main = the newest main commit; some feature/long
        // commits have later timestamps, so HEAD need not be row 0 — but it
        // must be present.
        assert!(layout.head_index.is_some());
        // Merge commits exist: at least one node with two parents.
        assert!(layout.nodes.iter().any(|n| n.parents.len() == 2));
    }

    #[test]
    fn generate_into_non_empty_dir_errors() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        std::fs::write(dir.path().join("junk.txt"), b"x").expect("write");
        let err = generate_fixture(dir.path(), &FixtureSpec::default());
        assert!(err.is_err());
    }
}
