//! Spec-004 fold × layout-cache tests: a fold toggle classifies as a Hit (no
//! re-walk) with spans recomputed per request; a redecorate recomputes spans
//! from the FRESH refs (a branch landing mid-run splits it); real merges under
//! first-parent stay unfolded on the cache-hit path (`merge_rows`, no libgit2).

use super::*;
use bonsai_core::graph::{FoldSpan, GraphChunk, GraphFilter};

// ---- fixtures (mirrors tests.rs; kept local — those helpers are private) ----

fn init_repo() -> (tempfile::TempDir, git2::Repository) {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let repo = git2::Repository::init(dir.path()).expect("init");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com")
        .expect("email");
    (dir, repo)
}

fn commit(repo: &git2::Repository, msg: &str, parents: &[git2::Oid], t: i64) -> git2::Oid {
    let sig =
        git2::Signature::new("Test User", "test@example.com", &git2::Time::new(t, 0)).expect("sig");
    let blob = repo.blob(msg.as_bytes()).expect("blob");
    let mut tb = repo.treebuilder(None).expect("treebuilder");
    tb.insert("f.txt", blob, 0o100_644).expect("insert");
    let tree = repo
        .find_tree(tb.write().expect("write tree"))
        .expect("tree");
    let parent_commits: Vec<git2::Commit> = parents
        .iter()
        .map(|p| repo.find_commit(*p).expect("parent"))
        .collect();
    let refs: Vec<&git2::Commit> = parent_commits.iter().collect();
    repo.commit(None, &sig, &sig, msg, &tree, &refs)
        .expect("commit")
}

/// Linear chain of `n` commits, `main` on the tip, HEAD attached. Oldest first.
fn chain_fixture(n: i64) -> (tempfile::TempDir, git2::Repository, Vec<git2::Oid>) {
    let (dir, repo) = init_repo();
    let mut oids = vec![commit(&repo, "c0", &[], 1)];
    for i in 1..n {
        let prev = *oids.last().expect("prev");
        oids.push(commit(&repo, &format!("c{i}"), &[prev], 1 + i));
    }
    {
        let tip = repo.find_commit(*oids.last().expect("tip")).expect("tip");
        repo.branch("main", &tip, true).expect("branch");
    }
    repo.set_head("refs/heads/main").expect("set head");
    (dir, repo, oids)
}

fn fold() -> GraphFilter {
    GraphFilter {
        fold_linear: true,
        ..Default::default()
    }
}

fn run(
    path: &std::path::Path,
    cache: &GraphCache,
    perf: &PerfState,
    filter: &GraphFilter,
) -> Vec<GraphChunk> {
    let mut out = Vec::new();
    stream_graph_cached(path, cache, perf, filter, |c| {
        out.push(c);
        true
    })
    .expect("stream_graph_cached");
    out
}

fn done_spans(chunks: &[GraphChunk]) -> Vec<FoldSpan> {
    match chunks.last() {
        Some(GraphChunk::Done { fold_spans, .. }) => fold_spans.clone(),
        other => panic!("last chunk must be Done, got {other:?}"),
    }
}

// ---- tests -------------------------------------------------------------------

/// Toggling fold on/off is a cache HIT (walk_eq excludes fold_linear), with the
/// spans recomputed per request: on → spans, off again → none (never stale).
#[test]
fn fold_toggle_is_a_hit_with_per_request_spans() {
    let (dir, _repo, _) = chain_fixture(12);
    let cache: GraphCache = Mutex::new(None);
    let perf = PerfState::default();

    let off = run(dir.path(), &cache, &perf, &GraphFilter::default());
    assert!(done_spans(&off).is_empty(), "fold off → no spans");

    let on = run(dir.path(), &cache, &perf, &fold());
    assert_eq!(
        done_spans(&on),
        vec![FoldSpan {
            start: 1,
            count: 10,
            lane: 0
        }],
        "fold on over the cached walk → fresh spans"
    );

    let off_again = run(dir.path(), &cache, &perf, &GraphFilter::default());
    assert!(
        done_spans(&off_again).is_empty(),
        "fold off must not leak the previous request's spans"
    );

    let c = perf.snapshot();
    assert_eq!(c.graph_walks, 1, "one cold walk only");
    assert_eq!(c.graph_cache_hits, 2, "both toggles are verbatim hits");
    assert_eq!(c.graph_redecorates, 0);
}

/// A branch created on a mid-run commit between requests: the next request is
/// a HitRedecorate and its spans are recomputed POST-redecorate — the new
/// pill's row must break the run.
#[test]
fn redecorate_recomputes_spans_from_fresh_refs() {
    let (dir, repo, oids) = chain_fixture(14);
    let cache: GraphCache = Mutex::new(None);
    let perf = PerfState::default();

    let first = run(dir.path(), &cache, &perf, &fold());
    assert_eq!(
        done_spans(&first),
        vec![FoldSpan {
            start: 1,
            count: 12,
            lane: 0
        }]
    );

    // Tip = row 0 … root = row 13; oids[7] sits at row 6, mid-run.
    let mid = repo.find_commit(oids[7]).expect("mid commit");
    repo.branch("feature", &mid, true).expect("branch mid-run");

    let second = run(dir.path(), &cache, &perf, &fold());
    assert_eq!(
        done_spans(&second),
        vec![
            FoldSpan {
                start: 1,
                count: 5,
                lane: 0
            },
            FoldSpan {
                start: 7,
                count: 6,
                lane: 0
            },
        ],
        "the new pill's row must split the span"
    );

    let c = perf.snapshot();
    assert_eq!(c.graph_walks, 1, "ref-only change never re-walks");
    assert_eq!(c.graph_redecorates, 1);
}

/// Real merges under first-parent never fold — including on the cache-hit
/// recompute path, which has no libgit2 access and must rely on the stored
/// `merge_rows`.
#[test]
fn first_parent_merges_stay_unfolded_on_cache_hit() {
    let (dir, repo) = init_repo();
    let mut tip = commit(&repo, "c0", &[], 1);
    for i in 0..7i64 {
        let t = 2 + i * 2;
        let s = commit(&repo, &format!("s{i}"), &[tip], t);
        tip = commit(&repo, &format!("m{i}"), &[tip, s], t + 1);
    }
    let tip_commit = repo.find_commit(tip).expect("tip");
    repo.branch("main", &tip_commit, true).expect("branch");
    repo.set_head("refs/heads/main").expect("set head");

    let cache: GraphCache = Mutex::new(None);
    let perf = PerfState::default();
    let f = GraphFilter {
        first_parent: true,
        fold_linear: true,
        ..Default::default()
    };

    let first = run(dir.path(), &cache, &perf, &f);
    assert!(done_spans(&first).is_empty(), "merges must not fold (walk)");

    let second = run(dir.path(), &cache, &perf, &f);
    assert!(
        done_spans(&second).is_empty(),
        "merges must not fold (cache-hit recompute via merge_rows)"
    );

    let c = perf.snapshot();
    assert_eq!(c.graph_walks, 1);
    assert_eq!(c.graph_cache_hits, 1);
}
