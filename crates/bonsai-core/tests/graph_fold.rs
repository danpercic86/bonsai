//! Spec-004 fold-linear-runs — integration tests over the public graph API:
//! `compute_graph_with` (one-shot `GraphLayout.fold_spans`) and
//! `stream_graph_from_repo` (`Done.fold_spans`).
//!
//! Locked semantics under test (plan.md §Approach):
//! - fold off == spec-003 output byte-identically, no spans (regression);
//! - a maximal linear run of >= MIN_FOLD_RUN refs-free rows folds (AC1);
//! - refs / HEAD / stash rows / merges / fork points never fold (AC3);
//! - a crossing edge blocks folding of the rows it passes through;
//! - real merges under first-parent are excluded via the merge bit (AC5);
//! - stream and one-shot produce identical spans under the same filter.

use bonsai_core::graph::{
    compute_graph, compute_graph_with, stream_graph_from_repo, FoldSpan, GraphChunk, GraphFilter,
    MIN_FOLD_RUN,
};

// ---- fixture helpers (mirrors tests/graph_filter.rs) ------------------------

fn init_repo() -> (tempfile::TempDir, git2::Repository) {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let repo = git2::Repository::init(dir.path()).expect("init repo");
    {
        let mut config = repo.config().expect("open config");
        config.set_str("user.name", "Test User").expect("set name");
        config
            .set_str("user.email", "test@example.com")
            .expect("set email");
    }
    (dir, repo)
}

fn commit(repo: &git2::Repository, msg: &str, parents: &[git2::Oid], t: i64) -> git2::Oid {
    let sig = git2::Signature::new("Test User", "test@example.com", &git2::Time::new(t, 0))
        .expect("signature");
    let blob = repo.blob(msg.as_bytes()).expect("blob");
    let mut tb = repo.treebuilder(None).expect("treebuilder");
    tb.insert("f.txt", blob, 0o100_644).expect("tree insert");
    let tree = repo
        .find_tree(tb.write().expect("write tree"))
        .expect("find tree");
    let parent_commits: Vec<git2::Commit> = parents
        .iter()
        .map(|p| repo.find_commit(*p).expect("find parent"))
        .collect();
    let parent_refs: Vec<&git2::Commit> = parent_commits.iter().collect();
    repo.commit(None, &sig, &sig, msg, &tree, &parent_refs)
        .expect("commit")
}

fn branch(repo: &git2::Repository, name: &str, oid: git2::Oid) {
    let c = repo.find_commit(oid).expect("find commit");
    repo.branch(name, &c, true).expect("create branch");
}

fn tag(repo: &git2::Repository, name: &str, oid: git2::Oid) {
    repo.reference(&format!("refs/tags/{name}"), oid, true, "tag")
        .expect("create tag ref");
}

fn set_head(repo: &git2::Repository, name: &str) {
    repo.set_head(&format!("refs/heads/{name}"))
        .expect("set head");
}

fn fold() -> GraphFilter {
    GraphFilter {
        fold_linear: true,
        ..Default::default()
    }
}

/// Linear chain of `n` commits; `main` on the tip, HEAD attached. Returns the
/// commits oldest-first.
fn chain_fixture(n: i64) -> (tempfile::TempDir, git2::Repository, Vec<git2::Oid>) {
    let (dir, repo) = init_repo();
    let mut oids = vec![commit(&repo, "c0", &[], 1)];
    for i in 1..n {
        let prev = *oids.last().expect("prev");
        oids.push(commit(&repo, &format!("c{i}"), &[prev], 1 + i));
    }
    branch(&repo, "main", *oids.last().expect("tip"));
    set_head(&repo, "main");
    (dir, repo, oids)
}

fn stream_spans(repo: &mut git2::Repository, filter: &GraphFilter) -> Vec<FoldSpan> {
    let mut spans: Vec<FoldSpan> = Vec::new();
    stream_graph_from_repo(repo, filter, |c| {
        if let GraphChunk::Done { fold_spans, .. } = c {
            spans = fold_spans;
        }
        true
    })
    .expect("stream");
    spans
}

// ---- AC1 + regression --------------------------------------------------------

/// AC1: a 12-commit straight run folds into one span; the tip (ref + HEAD) and
/// the root (no outgoing edge) anchor it. N == hidden rows.
#[test]
fn straight_run_folds_into_one_span() {
    let (dir, _repo, _) = chain_fixture(12);
    let layout = compute_graph_with(dir.path(), &fold()).expect("layout");
    assert_eq!(
        layout.fold_spans,
        vec![FoldSpan {
            start: 1,
            count: 10,
            lane: 0
        }]
    );
}

/// Locked regression: fold off ⇒ no spans AND nodes/edges byte-identical to
/// the default (pre-fold) output; fold on changes nothing but the spans.
#[test]
fn fold_off_is_identical_to_pre_fold_output() {
    let (dir, _repo, _) = chain_fixture(12);
    let base = compute_graph(dir.path()).expect("base layout");
    assert!(base.fold_spans.is_empty(), "default filter → no spans");

    let off = compute_graph_with(dir.path(), &GraphFilter::default()).expect("fold off");
    assert_eq!(off, base, "fold off must be byte-identical");

    let on = compute_graph_with(dir.path(), &fold()).expect("fold on");
    assert_eq!(on.nodes, base.nodes, "fold never changes nodes");
    assert_eq!(on.edges, base.edges, "fold never changes edges");
    assert_eq!(on.head_index, base.head_index);
    assert!(!on.fold_spans.is_empty());
}

/// A run hiding fewer than MIN_FOLD_RUN rows never folds.
#[test]
fn short_run_never_folds() {
    // n commits → n-2 foldable interior rows; make that MIN_FOLD_RUN-1.
    let (dir, _repo, _) = chain_fixture(i64::from(MIN_FOLD_RUN) + 1);
    let layout = compute_graph_with(dir.path(), &fold()).expect("layout");
    assert!(layout.fold_spans.is_empty(), "{:?}", layout.fold_spans);
}

// ---- AC3: structure always visible --------------------------------------------

/// A tag mid-run splits the run; each half folds only when long enough.
#[test]
fn ref_mid_run_splits_the_run() {
    let (dir, repo, oids) = chain_fixture(14);
    // Tip=row0 … root=row13. Tag the commit at row 6 (oids index 13-6=7).
    tag(&repo, "v1", oids[7]);
    let layout = compute_graph_with(dir.path(), &fold()).expect("layout");
    assert_eq!(
        layout.fold_spans,
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
        ]
    );
}

/// Detached HEAD mid-run splits the run (HEAD pill + head_index).
#[test]
fn detached_head_mid_run_splits_the_run() {
    let (dir, repo, oids) = chain_fixture(14);
    repo.set_head_detached(oids[7]).expect("detach");
    let layout = compute_graph_with(dir.path(), &fold()).expect("layout");
    assert_eq!(layout.head_index, Some(6));
    for s in &layout.fold_spans {
        assert!(
            !(s.start..s.start + s.count).contains(&6),
            "HEAD row inside a span: {:?}",
            layout.fold_spans
        );
    }
    assert_eq!(layout.fold_spans.len(), 2, "{:?}", layout.fold_spans);
}

/// A stash row is never inside a span (its stash label breaks the run).
#[test]
fn stash_row_never_folds() {
    let (dir, repo, _) = chain_fixture(12);
    // Materialize the worktree, dirty it, stash.
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .expect("checkout");
    std::fs::write(dir.path().join("f.txt"), "dirty").expect("write");
    let mut repo = repo;
    let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
    repo.stash_save(&sig, "wip", None).expect("stash");

    let layout = compute_graph_with(dir.path(), &fold()).expect("layout");
    let stash_row = layout
        .nodes
        .iter()
        .position(|n| n.refs.iter().any(|r| r.name.starts_with("stash@")))
        .expect("stash row present") as u32;
    for s in &layout.fold_spans {
        assert!(
            !(s.start..s.start + s.count).contains(&stash_row),
            "stash row {stash_row} inside a span: {:?}",
            layout.fold_spans
        );
    }
    assert!(!layout.fold_spans.is_empty(), "the chain still folds");
}

/// A long edge crossing the run blocks folding of every row it passes through.
#[test]
fn crossing_edge_blocks_folding() {
    let (dir, repo, oids) = chain_fixture(16);
    // A side commit branching from an early main commit, with the NEWEST
    // timestamp so its edge spans the whole main run.
    let s = commit(&repo, "side", &[oids[3]], 100);
    branch(&repo, "side", s);
    let layout = compute_graph_with(dir.path(), &fold()).expect("layout");
    assert!(
        layout.fold_spans.is_empty(),
        "crossed rows must not fold: {:?}",
        layout.fold_spans
    );
}

/// Merge commits and fork points are never foldable (edge counts), even when
/// they carry no refs.
#[test]
fn merges_and_forks_never_fold() {
    let (dir, repo, oids) = chain_fixture(16);
    // Fork at oids[7], merged back at a new tip commit.
    let s = commit(&repo, "side", &[oids[7]], 17);
    let tip = *oids.last().expect("tip");
    let m = commit(&repo, "merge", &[tip, s], 18);
    // `main` is checked out — advance the ref directly (branch() would refuse).
    repo.reference("refs/heads/main", m, true, "advance main")
        .expect("advance main");
    let layout = compute_graph_with(dir.path(), &fold()).expect("layout");
    let fork_row = layout
        .nodes
        .iter()
        .position(|n| n.id == oids[7].to_string())
        .expect("fork row") as u32;
    let merge_row = layout
        .nodes
        .iter()
        .position(|n| n.id == m.to_string())
        .expect("merge row") as u32;
    for s in &layout.fold_spans {
        let range = s.start..s.start + s.count;
        assert!(!range.contains(&fork_row), "fork row folded");
        assert!(!range.contains(&merge_row), "merge row folded");
    }
}

// ---- AC5: composition with spec-003 -------------------------------------------

/// Consecutive-merges fixture: `m1..m7`, each merging an (unrefed) side commit.
fn merge_train_fixture() -> (tempfile::TempDir, git2::Repository) {
    let (dir, repo) = init_repo();
    let mut tip = commit(&repo, "c0", &[], 1);
    for i in 0..7 {
        let t = 2 + i * 2;
        let s = commit(&repo, &format!("s{i}"), &[tip], t);
        tip = commit(&repo, &format!("m{i}"), &[tip, s], t + 1);
    }
    branch(&repo, "main", tip);
    set_head(&repo, "main");
    (dir, repo)
}

/// Under first-parent, real merges LOOK linear (one in-edge, one out-edge) but
/// must never fold — the merge_rows bit excludes them (AC3 + AC5).
#[test]
fn first_parent_real_merges_never_fold() {
    let (dir, _repo) = merge_train_fixture();
    let f = GraphFilter {
        first_parent: true,
        fold_linear: true,
        ..Default::default()
    };
    let layout = compute_graph_with(dir.path(), &f).expect("layout");
    // Every interior row is a real merge → nothing may fold.
    assert!(
        layout.fold_spans.is_empty(),
        "merges folded under first-parent: {:?}",
        layout.fold_spans
    );
}

/// Solo composition: spans are computed over the FILTERED walk, so soloing a
/// branch makes its run fold even though the full graph would be crossed.
#[test]
fn fold_composes_with_seed_refs_solo() {
    let (dir, repo, oids) = chain_fixture(16);
    let s = commit(&repo, "side", &[oids[3]], 100);
    branch(&repo, "side", s);
    // Full graph: the side edge crosses the run → no fold (asserted above).
    // Solo main: side vanishes → the run folds again.
    let f = GraphFilter {
        seed_refs: Some(vec!["refs/heads/main".to_string()]),
        fold_linear: true,
        ..Default::default()
    };
    let layout = compute_graph_with(dir.path(), &f).expect("layout");
    assert_eq!(
        layout.fold_spans,
        vec![FoldSpan {
            start: 1,
            count: 14,
            lane: 0
        }]
    );
}

// ---- stream / one-shot parity --------------------------------------------------

/// `done.fold_spans == layout.fold_spans` under the same filter, on a branching
/// fixture (the batch and incremental scans share one predicate).
#[test]
fn stream_and_one_shot_spans_are_identical() {
    let (dir, repo) = merge_train_fixture();
    let mut repo = repo;
    for f in [
        fold(),
        GraphFilter {
            first_parent: true,
            fold_linear: true,
            ..Default::default()
        },
    ] {
        let layout = compute_graph_with(dir.path(), &f).expect("layout");
        let spans = stream_spans(&mut repo, &f);
        assert_eq!(layout.fold_spans, spans, "parity under {f:?}");
    }
}
