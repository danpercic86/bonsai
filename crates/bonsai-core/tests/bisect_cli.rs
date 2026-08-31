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

use bonsai_core::error::AppError;
use bonsai_core::git::bisect::{
    bisect_mark, bisect_reset, bisect_skip, get_bisect_state, start_bisect, BisectOutcome,
};
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

/// Parses `<40hex> is the first bad commit` out of `git bisect good/bad`
/// output. Git >= ~2.5x quotes the verdict (`is the first 'bad' commit`);
/// older git omits the quotes (`is the first bad commit`) — match both so
/// this oracle isn't pinned to one git version's exact wording.
fn parse_first_bad(out: &str) -> Option<String> {
    for line in out.lines() {
        if line.contains("is the first bad commit") || line.contains("is the first 'bad' commit") {
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

// ============================================================ reset safety

/// The #1 safety property: `bisect_reset` from mid-bisect must re-attach the
/// ORIGINAL branch at the EXACT pre-bisect tip with a clean worktree. Cross-
/// checked with `git rev-parse` (oid) and `git rev-parse --abbrev-ref` (branch).
#[test]
fn reset_from_mid_bisect_restores_original_branch_and_tip() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    let n = 16;
    let bug_at = 9;
    let oids = build_linear_bug(d, n, bug_at);
    let bad = oids[n - 1].clone();
    let good = oids[0].clone();

    // Record the exact pre-bisect state (attached branch `main` at the tip).
    let orig_head = rev(d, "HEAD");
    let orig_branch = git(d, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(orig_head, oids[n - 1], "pre-bisect HEAD is the tip");
    assert_eq!(orig_branch, "main");

    // Start, then answer a couple of midpoints (detaching HEAD across each).
    let mut outcome = start_bisect(d, &bad, std::slice::from_ref(&good)).expect("start");
    let mut marks = 0;
    while marks < 2 {
        match outcome {
            BisectOutcome::Testing { .. } => {
                // We are on a DETACHED HEAD mid-bisect (the risky state).
                assert!(git(d, &["rev-parse", "--abbrev-ref", "HEAD"]) == "HEAD");
                let bad_here = worktree_is_bad(d);
                outcome = bisect_mark(d, !bad_here).expect("mark");
                marks += 1;
            }
            BisectOutcome::Found { .. } => break,
            BisectOutcome::CannotDetermine { skipped } => {
                panic!("unexpected cannotDetermine: {skipped:?}")
            }
        }
    }

    // Reset mid-bisect and cross-check the restore against git itself.
    bisect_reset(d).expect("reset");
    assert_eq!(rev(d, "HEAD"), orig_head, "HEAD oid restored to the pre-bisect tip");
    assert_eq!(
        git(d, &["rev-parse", "--abbrev-ref", "HEAD"]),
        orig_branch,
        "original branch re-attached (not left detached)"
    );
    assert_eq!(git(d, &["status", "--porcelain"]), "", "worktree is clean after reset");
    assert!(get_bisect_state(d).expect("query").is_none(), "no bisect state remains");
    // A fresh bisect can be started again after a clean reset.
    start_bisect(d, &bad, std::slice::from_ref(&good)).expect("restart after reset");
    bisect_reset(d).expect("final cleanup reset");
}

// ============================================================ ensure_on_current guard (e2e)

/// End-to-end `ensure_on_current` guard: if the user moves HEAD off the current
/// midpoint via a REAL `git checkout` mid-bisect, `mark`/`skip` must ERROR and
/// leave the on-disk state file byte-for-byte unchanged (no silent corruption of
/// the search). Uses the git CLI (not an internal git2 checkout) to mirror the
/// real-world footgun.
#[test]
fn mark_after_external_checkout_errors_and_leaves_state_unchanged() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    let n = 16;
    let bug_at = 9;
    let oids = build_linear_bug(d, n, bug_at);
    let bad = oids[n - 1].clone();
    let good = oids[0].clone();

    let start = start_bisect(d, &bad, std::slice::from_ref(&good)).expect("start");
    let midpoint = match start {
        BisectOutcome::Testing { current, .. } => current,
        other => panic!("expected Testing on start, got {other:?}"),
    };
    assert_eq!(rev(d, "HEAD"), midpoint, "engine detached HEAD onto the midpoint");

    let before = get_bisect_state(d).expect("state").expect("in progress");

    // Move HEAD OFF the midpoint with the real git CLI (clean detached checkout).
    // The good boundary (oids[0], hidden) and bad tip (oids[n-1], excluded) are
    // never a testable midpoint, so `good` here is guaranteed != midpoint.
    assert_ne!(good, midpoint);
    git(d, &["checkout", "--detach", &good]);
    assert_ne!(rev(d, "HEAD"), midpoint, "HEAD is off the midpoint");

    // Both mark and skip must refuse with the guard error.
    match bisect_mark(d, true).expect_err("mark off-midpoint must fail") {
        AppError::Git(m) => assert!(m.contains("not on the bisect commit"), "got: {m}"),
        other => panic!("expected Git guard error, got {other:?}"),
    }
    match bisect_skip(d).expect_err("skip off-midpoint must fail") {
        AppError::Git(m) => assert!(m.contains("not on the bisect commit"), "got: {m}"),
        other => panic!("expected Git guard error, got {other:?}"),
    }

    // The persisted search state is untouched by the rejected calls.
    let after = get_bisect_state(d).expect("state").expect("still in progress");
    assert_eq!(before, after, "rejected mark/skip left the bisect state unchanged");

    // Cleanup: never leave the scratch repo mid-bisect on a detached HEAD.
    bisect_reset(d).expect("cleanup reset");
    assert!(get_bisect_state(d).expect("query").is_none());
}

// ============================================================ untracked-collision guard

/// DATA-LOSS SAFETY: an untracked, non-ignored worktree file whose path collides
/// with a file present in the midpoint commit's tree must NOT be silently
/// clobbered by the force checkout onto that midpoint. Start must refuse,
/// preserve the untracked file byte-for-byte, and leave NO bisect state (HEAD
/// stays on the original branch/tip).
#[test]
fn start_refuses_when_untracked_file_would_be_clobbered_by_midpoint_checkout() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();

    // c0 = good; c1 = the sole testable midpoint (tracks foo.txt); c2 = bad
    // (removes foo.txt). The midpoint's tree carries foo.txt, HEAD (c2) does not.
    write(d, "a.txt", "a\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "c0");
    let c0 = rev(d, "HEAD");

    write(d, "foo.txt", "from-midpoint\n");
    write(d, "m1.txt", "m1\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "c1");

    git(d, &["rm", "foo.txt"]);
    write(d, "m2.txt", "m2\n");
    git(d, &["add", "-A"]);
    commit_fixed(d, "c2");
    let c2 = rev(d, "HEAD");

    // Untracked foo.txt in the worktree collides with the midpoint (c1) tree.
    write(d, "foo.txt", "UNTRACKED-LOCAL\n");

    match start_bisect(d, &c2, std::slice::from_ref(&c0)).expect_err("collision must refuse") {
        AppError::Git(m) => assert!(
            m.contains("would be overwritten by checkout") && m.contains("foo.txt"),
            "got: {m}"
        ),
        other => panic!("expected Git, got {other:?}"),
    }

    // The untracked file is untouched, no bisect started, HEAD/branch unmoved.
    assert_eq!(
        std::fs::read_to_string(d.join("foo.txt")).expect("read foo.txt"),
        "UNTRACKED-LOCAL\n",
        "untracked file was clobbered"
    );
    assert!(get_bisect_state(d).expect("query").is_none(), "a refused start left bisect state");
    assert!(
        !d.join(".git").join("bonsai-bisect").exists(),
        "a refused start must leave no bisect state dir"
    );
    assert_eq!(rev(d, "HEAD"), c2, "HEAD still at the original tip");
    assert_eq!(
        git(d, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "main",
        "still on the original branch (not detached)"
    );
}
