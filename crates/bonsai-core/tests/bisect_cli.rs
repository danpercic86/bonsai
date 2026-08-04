//! P39 CLI-oracle git-bisect tests (contract §7).
//!
//! Fixtures are built with the `git` CLI (fixed dates → deterministic oids). A
//! known "bug" marker (`bug.txt`) is introduced at a chosen commit; the predicate
//! is "the worktree contains bug.txt". We obtain git's AUTHORITATIVE first-bad by
//! driving a real `git bisect` and answering each checked-out midpoint with that
//! same predicate, then run Bonsai's engine over the same range and assert:
//!   (a) Bonsai's `Found.first_bad` == git's first-bad,
//!   (b) every Bonsai midpoint is a member of git's candidate set (`git rev-list
//!       bad ^good`),
//!   (c) the step count is within `ceil(log2(N)) + 1`.
//!
//! DEVIATION from the contract's literal wording: git's answer is obtained by a
//! manual `git bisect good/bad` loop (evaluating the SAME predicate ourselves)
//! rather than `git bisect run <script>` — this is the identical authoritative
//! result but avoids a shell-script dependency that is not hermetic on Windows.
//!
//! All scratch repos live under `D:\Temp\bonsai-scratch`. Each test skips
//! (passes with a note) if `git` is not on PATH.

mod common;

use std::collections::HashSet;
use std::path::Path;

use bonsai_core::git::bisect::{bisect_mark, bisect_skip, start_bisect, BisectOutcome};
use common::{commit_fixed, git, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

fn rev(dir: &Path, r: &str) -> String {
    git(dir, &["rev-parse", r])
}

/// Linear history c0..c{n-1} on main; from commit index `bug_at` onward each
/// commit carries the `bug.txt` marker. Returns the oids oldest-first.
fn build_linear_bug(d: &Path, n: usize, bug_at: usize) -> Vec<String> {
    let mut oids = Vec::new();
    for i in 0..n {
        write(d, &format!("f{i}.txt"), &format!("c{i}\n"));
        if i >= bug_at {
            write(d, "bug.txt", "boom\n");
        }
        git(d, &["add", "-A"]);
        commit_fixed(d, &format!("c{i}"));
        oids.push(rev(d, "HEAD"));
    }
    oids
}

/// The predicate: the checked-out worktree contains the bug marker → BAD.
fn worktree_is_bad(d: &Path) -> bool {
    d.join("bug.txt").exists()
}

/// Parses `<40hex> is the first bad commit` out of `git bisect good/bad` output.
fn parse_first_bad(out: &str) -> Option<String> {
    for line in out.lines() {
        if line.contains("is the first bad commit") {
            let oid = line.split_whitespace().next()?;
            if oid.len() == 40 && oid.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(oid.to_string());
            }
        }
    }
    None
}

/// Drives a REAL `git bisect`, answering each midpoint with `worktree_is_bad`,
/// and returns git's authoritative first-bad oid. Leaves the repo reset.
fn git_bisect_first_bad(d: &Path, bad: &str, good: &str) -> String {
    git(d, &["bisect", "start", bad, good]);
    let mut answer: Option<String> = None;
    for _ in 0..64 {
        let verdict = if worktree_is_bad(d) { "bad" } else { "good" };
        let out = git(d, &["bisect", verdict]);
        if let Some(oid) = parse_first_bad(&out) {
            answer = Some(oid);
            break;
        }
    }
    git(d, &["bisect", "reset"]);
    answer.expect("git bisect announced a first-bad commit")
}

/// Runs Bonsai's engine over `good..bad`, answering each midpoint with the same
/// predicate. Returns (first_bad, midpoints_tested).
fn bonsai_bisect(d: &Path, bad: &str, good: &str) -> (String, Vec<String>) {
    let mut midpoints = Vec::new();
    let mut outcome = start_bisect(d, bad, &[good.to_string()]).expect("start bisect");
    for _ in 0..64 {
        match outcome {
            BisectOutcome::Testing { current, .. } => {
                midpoints.push(current.clone());
                let bad_here = worktree_is_bad(d);
                outcome = bisect_mark(d, !bad_here).expect("mark");
            }
            BisectOutcome::Found { first_bad } => return (first_bad, midpoints),
            BisectOutcome::CannotDetermine { skipped } => {
                panic!("unexpected cannotDetermine: {skipped:?}")
            }
        }
    }
    panic!("bonsai bisect did not converge");
}

fn candidate_set(d: &Path, bad: &str, good: &str) -> HashSet<String> {
    git(d, &["rev-list", bad, &format!("^{good}")])
        .lines()
        .map(String::from)
        .collect()
}

fn ceil_log2(n: usize) -> usize {
    if n <= 1 {
        return 0;
    }
    (usize::BITS - (n - 1).leading_zeros()) as usize
}

// ============================================================ oracle

#[test]
fn bonsai_first_bad_matches_git_on_linear_history() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    let n = 16;
    let bug_at = 9;
    let oids = build_linear_bug(d, n, bug_at);
    let bad = oids[n - 1].clone();
    let good = oids[0].clone();

    let cand = candidate_set(d, &bad, &good);
    let git_first_bad = git_bisect_first_bad(d, &bad, &good);
    assert_eq!(git_first_bad, oids[bug_at], "git names the bug-introducing commit");

    let (bonsai_first_bad, midpoints) = bonsai_bisect(d, &bad, &good);

    // (a) final first-bad equality.
    assert_eq!(bonsai_first_bad, git_first_bad, "Bonsai first-bad == git first-bad");

    // (b) every Bonsai midpoint is in git's candidate set.
    for m in &midpoints {
        assert!(cand.contains(m), "midpoint {m} not in git's candidate set");
    }

    // (c) step bound.
    let bound = ceil_log2(cand.len()) + 1;
    assert!(
        midpoints.len() <= bound,
        "steps {} exceed ceil(log2({}))+1 = {}",
        midpoints.len(),
        cand.len(),
        bound
    );

    // HEAD ends detached at the culprit; no bonsai-bisect dir escapes the repo.
    assert_eq!(rev(d, "HEAD"), oids[bug_at]);
}

#[test]
fn bonsai_first_bad_matches_git_with_bug_at_head() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    // Only the very last commit is bad (regression at HEAD).
    let n = 10;
    let bug_at = 9;
    let oids = build_linear_bug(d, n, bug_at);
    let bad = oids[n - 1].clone();
    let good = oids[0].clone();

    let git_first_bad = git_bisect_first_bad(d, &bad, &good);
    let (bonsai_first_bad, _mids) = bonsai_bisect(d, &bad, &good);
    assert_eq!(bonsai_first_bad, git_first_bad);
    assert_eq!(bonsai_first_bad, oids[bug_at]);
}

// ============================================================ skip path

#[test]
fn skip_a_midpoint_still_converges_to_git_first_bad() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    let n = 14;
    let bug_at = 8;
    let oids = build_linear_bug(d, n, bug_at);
    let bad = oids[n - 1].clone();
    let good = oids[0].clone();

    let git_first_bad = git_bisect_first_bad(d, &bad, &good);
    assert_eq!(git_first_bad, oids[bug_at]);

    // Skip the FIRST midpoint once, then answer normally.
    let mut skipped_once = false;
    let mut outcome = start_bisect(d, &bad, std::slice::from_ref(&good)).expect("start");
    let mut answer = None;
    for _ in 0..64 {
        match outcome {
            BisectOutcome::Testing { .. } => {
                if !skipped_once {
                    skipped_once = true;
                    outcome = bisect_skip(d).expect("skip");
                } else {
                    let bad_here = worktree_is_bad(d);
                    outcome = bisect_mark(d, !bad_here).expect("mark");
                }
            }
            BisectOutcome::Found { first_bad } => {
                answer = Some(first_bad);
                break;
            }
            BisectOutcome::CannotDetermine { skipped } => {
                panic!("unexpected cannotDetermine: {skipped:?}")
            }
        }
    }
    assert!(skipped_once, "a midpoint was skipped");
    assert_eq!(answer.expect("converged"), git_first_bad, "skip still finds git's first-bad");
}
