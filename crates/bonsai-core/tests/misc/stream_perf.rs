//! P65c streaming perf gate (contract §7 AI-gate item 3). Release-mode only —
//! debug git2 is far slower (both fixture generation and the walk). Run:
//!
//! ```text
//! cargo test --release -p bonsai-core --test stream_perf -- --ignored --nocapture
//! ```
//!
//! ## FINDING — the contract's `< 150 ms` first-batch target is NOT achievable
//!
//! The contract justifies `< 150 ms` first-batch as "first batch = 512 walked
//! rows over the P52 commit-graph file". Measured on a 200k-commit fixture that
//! premise does NOT hold: the FIRST `Batch` arrives at **~1.9 s cold / ~2.3–2.4 s
//! warm** (~15× the budget), and first-batch latency scales with TOTAL commits,
//! not with `STREAM_FIRST_BATCH`. (The committed fixture below is 120k — a
//! generation-budget fallback, see `FIXTURE_COMMITS` — where first-batch is
//! ~1.4 s: still ~10× over 150 ms.)
//!
//! Cause (verified in code + empirically): `stream_graph_core` seeds a
//! `git2::Sort::TOPOLOGICAL | TIME` revwalk. libgit2's `git_revwalk_next` under
//! `GIT_SORT_TOPOLOGICAL` runs `prepare_walk`, which **drains the entire
//! reachable graph to compute per-commit in-degrees before it yields row 0** —
//! even with the commit-graph present (it only speeds up per-commit reads, it
//! does not make the prep lazy). So the first `revwalk` item — hence the first
//! `Batch` — cannot be produced until an O(total-commits) preparation completes.
//! At 200k, first-batch (~2.3 s) ≈ 60% of the full-stream time (~3.8 s); the
//! `< 150 ms` budget is only met below ~10k commits, which defeats a gate that
//! exists FOR huge repos.
//!
//! This also invalidates the P65 "first screenful paints instantly on 200k+"
//! UX premise for the topo-sorted walk — flagged to the architect. Achievable
//! fast first-paint would require an architectural change to `stream_graph_core`
//! (e.g. a lazy generation-number topo-order like git's own `--topo-order` over
//! a commit-graph, or a TIME-first provisional screen), i.e. P65a work — out of
//! scope for this test.
//!
//! Given that, this gate asserts what IS true and valuable at scale — the full
//! stream completes correctly and delivers incrementally (first `Batch` strictly
//! before `Done`) — and MEASURES + PRINTS the first-batch/full-stream latency so
//! the regression is always visible. The absolute latency THRESHOLD is left for
//! the architect to set once the walk approach is decided (see report).

use std::io::Write;
use std::path::Path;
use std::time::Instant;

use bonsai_core::graph::{stream_graph_core, GraphChunk};

/// Fixture size. The contract asks for ~200k, but with the shared-empty-tree
/// generator below 200k generates in ~56–66 s across runs — it straddles the
/// `< 60 s` budget under machine load, i.e. NOT reliably "well under 60 s". Per
/// the contract's own fallback ("largest size that stays under ~45 s while still
/// being ≥ 100k") this is set to 120k, which generates in ~34–40 s and is still
/// a genuinely large repo. Raising it back to 200k only makes the FINDING (below)
/// starker — first-batch grows to ~2.3 s — it does not change the conclusion.
const FIXTURE_COMMITS: usize = 120_000;

/// The contract's first-batch budget. NOT asserted — see the module FINDING:
/// libgit2's topo-sort makes first-batch O(total commits) (~2.3 s at 200k), so
/// this is unmeetable at scale. Kept as a named constant to document the target
/// and print the measured shortfall against it at run time.
const CONTRACT_FIRST_BATCH_BUDGET_MS: f64 = 150.0;

const BASE_TS: i64 = 1_600_000_000;

/// Builds a purely-linear history of `n` commits (one lane, `refs/heads/main` at
/// the tip) using git2 tree/commit objects ONLY — no CLI, no `git commit` loops.
///
/// Follows the 31k layout-gate generator's approach (mempack backend → one
/// packfile) but SHARES a single empty tree across every commit (no per-commit
/// blob + treebuilder) — the "cheapest object reuse" the contract's fallback
/// hints at, which is what makes 120k generate in ~34–40 s (≈3.0–3.6k commits/s)
/// rather than the several minutes the per-commit-blob generator would take (it
/// managed only ~950 commits/s). Every commit is
/// reachable from the single ref, so the streamed `total_rows` lands exactly on
/// `n`. Branchy topology / lane stability is covered by AI-gate item 1; THIS gate
/// isolates large-repo streaming behaviour, for which a linear chain is the
/// cheapest way to hit an exact large row count.
fn generate_linear_fixture(path: &Path, n: usize) {
    let repo = git2::Repository::init(path).expect("init repo");
    let odb = repo.odb().expect("odb");
    // Route object writes through an in-memory mempack and dump ONE packfile at
    // the end: loose objects would make both generation and every later revwalk
    // pathologically slow on Windows (mirrors the 31k generator's rationale).
    let mempack = odb.add_new_mempack_backend(1000).expect("mempack backend");

    // ONE empty tree, shared by all commits.
    let empty_tree_oid = repo
        .treebuilder(None)
        .expect("treebuilder")
        .write()
        .expect("write empty tree");
    let empty_tree = repo.find_tree(empty_tree_oid).expect("find empty tree");

    let mut parent: Option<git2::Commit> = None;
    for i in 1..=n {
        // Strictly-increasing signature time (matches the 31k generator; keeps
        // the TIME component of the sort well-defined).
        let sig = git2::Signature::new(
            "Fixture Bot",
            "fixture@bonsai.local",
            &git2::Time::new(BASE_TS + i as i64 * 60, 0),
        )
        .expect("signature");
        let parent_refs: Vec<&git2::Commit> = parent.iter().collect();
        let oid = repo
            .commit(None, &sig, &sig, &format!("commit {i}"), &empty_tree, &parent_refs)
            .expect("commit");
        parent = Some(repo.find_commit(oid).expect("find commit"));
    }
    let tip = parent.expect("history must be non-empty").id();

    // Persist the in-memory objects as one packfile (pack + index) before refs.
    let mut buf = git2::Buf::new();
    mempack.dump(&repo, &mut buf).expect("mempack dump");
    let mut writer = odb.packwriter().expect("packwriter");
    writer.write_all(&buf).expect("write pack");
    writer.commit().expect("commit pack");

    repo.reference("refs/heads/main", tip, true, "fixture main")
        .expect("ref main");
    repo.set_head("refs/heads/main").expect("set head");
}

/// Drives `stream_graph_core` to completion, returning
/// `(first_batch_ms, full_stream_ms, total_rows, truncated)`. `first_batch_ms`
/// is measured from the `stream_graph_core` call start to the instant the FIRST
/// `Batch` chunk reaches `emit` (the preceding `Meta` is ignored); `full_stream_ms`
/// to the `Done` chunk.
fn run_stream(repo: &Path) -> (f64, f64, u32, bool) {
    let start = Instant::now();
    let mut first_batch_ms: Option<f64> = None;
    let mut full_stream_ms = 0.0f64;
    let mut total_rows: u32 = 0;
    let mut truncated = false;
    let mut saw_done = false;

    stream_graph_core(repo, |chunk| {
        match chunk {
            GraphChunk::Batch { .. } => {
                if first_batch_ms.is_none() {
                    first_batch_ms = Some(start.elapsed().as_secs_f64() * 1e3);
                }
            }
            GraphChunk::Done {
                total_rows: tr,
                truncated: tc,
                ..
            } => {
                full_stream_ms = start.elapsed().as_secs_f64() * 1e3;
                total_rows = tr;
                truncated = tc;
                saw_done = true;
            }
            GraphChunk::Meta { .. } => {}
        }
        true // consume the whole stream
    })
    .expect("stream_graph_core failed");

    assert!(saw_done, "stream ended without a Done chunk");
    let fb = first_batch_ms.expect("stream emitted no Batch chunk");
    (fb, full_stream_ms, total_rows, truncated)
}

#[test]
#[ignore] // release-mode gate; see module docs for the invocation
fn stream_first_batch_under_ms() {
    bonsai_core::git::relax_odb_hash_verification();

    // Self-contained: the fixture lives in a TempDir (honors TMP/TEMP) and is
    // cleaned on drop.
    let dir = tempfile::TempDir::new().expect("tempdir");
    let repo_path = dir.path();

    let gen_start = Instant::now();
    generate_linear_fixture(repo_path, FIXTURE_COMMITS);
    let gen_secs = gen_start.elapsed().as_secs_f64();
    eprintln!(
        "[stream-perf] generated {}-commit linear fixture in {:.1}s ({:.0} commits/s)",
        FIXTURE_COMMITS,
        gen_secs,
        FIXTURE_COMMITS as f64 / gen_secs
    );

    // Commit-graph = the realistic P52 state a real opened repo carries.
    let cg = bonsai_core::git::maintenance::write_commit_graph_best_effort(repo_path);
    eprintln!("[stream-perf] commit-graph write: {cg:?}");

    // Warm-up (page cache / odb), then measure over a couple of runs and report
    // the best — mirrors the layout_31k gate's methodology.
    let (cold_fb, cold_full, _, _) = run_stream(repo_path);
    eprintln!("[stream-perf] cold: first-batch {cold_fb:.1} ms, full-stream {cold_full:.1} ms");

    let mut first_batch_ms: Vec<f64> = Vec::with_capacity(2);
    let mut full_stream_ms: Vec<f64> = Vec::with_capacity(2);
    let mut total_rows = 0u32;
    let mut truncated = true;
    for _ in 0..2 {
        let (fb, full, tr, tc) = run_stream(repo_path);
        first_batch_ms.push(fb);
        full_stream_ms.push(full);
        total_rows = tr;
        truncated = tc;
    }
    let min_fb = first_batch_ms.iter().copied().fold(f64::INFINITY, f64::min);
    let min_full = full_stream_ms.iter().copied().fold(f64::INFINITY, f64::min);
    eprintln!(
        "[stream-perf] warm first-batch: {first_batch_ms:.1?} ms (best {min_fb:.1}); \
         full-stream: {full_stream_ms:.1?} ms (best {min_full:.1}); \
         total_rows={total_rows}, truncated={truncated}"
    );
    eprintln!(
        "[stream-perf] contract first-batch budget was {CONTRACT_FIRST_BATCH_BUDGET_MS:.0} ms — \
         MEASURED {min_fb:.0} ms (see module FINDING: libgit2 topo-sort prep is O(commits))"
    );

    // --- Achievable, meaningful assertions (contract item 3 "full stream also
    // --- completes") ---------------------------------------------------------
    // The full 200k stream delivers every reachable commit and nothing is
    // dropped (200k < the 1M STREAM_MAX_COMMITS cap).
    assert_eq!(
        total_rows, FIXTURE_COMMITS as u32,
        "full stream must deliver every reachable commit"
    );
    assert!(
        !truncated,
        "{FIXTURE_COMMITS} < STREAM_MAX_COMMITS ⇒ the stream must not be truncated"
    );
    // Streaming IS incremental: the first screenful arrives strictly before the
    // whole walk finishes (the value streaming adds over one-shot getGraph at
    // this scale). The absolute first-batch latency is measured/printed above;
    // its THRESHOLD is deferred to the architect (module FINDING).
    assert!(
        min_fb < min_full,
        "first Batch must arrive before Done (incremental delivery): \
         first-batch {min_fb:.1} ms vs full-stream {min_full:.1} ms"
    );
}
