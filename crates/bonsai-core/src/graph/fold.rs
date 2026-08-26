//! Fold-span computation (spec-004).
//!
//! Rust decides *which rows are foldable*; the frontend merely applies the
//! resulting [`FoldSpan`] row mapping. The rule is defined over lanes/refs/
//! edges ONLY (no `parents` dependency) so ONE predicate serves the one-shot
//! layout, the live stream, and cached replay whose `StreamNode`s carry no
//! parents (plan §Approach).
//!
//! Conservative lane rule — row `r` is *foldable* iff ALL of:
//! 1. its refs are empty (covers branch/tag/HEAD pills AND stash labels) and
//!    `r != head_index` (detached-HEAD-no-label safety);
//! 2. exactly one edge leaves `r`, and it is `(r, r+1)` — one parent,
//!    contiguous (a truncated/absent parent ⇒ no outgoing edge ⇒ not
//!    foldable, so cap-adjacent rows are safe);
//! 3. exactly one edge arrives at `r`, and it is `(r-1, r)` on `r`'s lane
//!    (single child, contiguous);
//! 4. no other edge crosses the row (`e.from < r < e.to`);
//! 5. under `first_parent`, `r` is not a *real* merge (`merge_rows`) — the
//!    view truncates parents, so the edge model alone cannot see it.
//!
//! A [`FoldSpan`] is a maximal contiguous run of foldable rows with
//! `count >= MIN_FOLD_RUN`. Spans are computed per-request POST-redecorate and
//! never cached (refs move between requests).

use super::GraphChunk;

/// Minimum hidden rows for a run to fold ("⋯ 2 commits" is worse than the
/// rows). Mirrored in the frontend mock.
pub const MIN_FOLD_RUN: u32 = 5;

/// A maximal foldable run. `start..start+count` are the HIDDEN model rows;
/// the anchor rows `start-1` and `start+count` remain visible.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FoldSpan {
    /// First hidden row (model index).
    pub start: u32,
    /// Hidden rows == the pill's N (`>= MIN_FOLD_RUN`).
    pub count: u32,
    /// The run's lane (pill color).
    pub lane: u32,
}

/// Incremental fold accumulator. The stream core feeds it one row (+ that
/// row's finalized edges) at a time; [`finish`](FoldScan::finish) turns the
/// accumulated counters into spans. The batch form [`compute_fold_spans`]
/// wraps this same scan, so batch == incremental by construction.
#[derive(Debug, Default)]
pub struct FoldScan {
    lanes: Vec<u32>,
    refs_empty: Vec<bool>,
    /// Rows whose REAL parent count (pre-first-parent-truncate) is > 1.
    /// Sorted ascending by construction (rows arrive in order).
    merge_rows: Vec<u32>,
    out_cnt: Vec<u32>,
    /// Row has an edge `(r, r+1)`.
    out_contig: Vec<bool>,
    in_cnt: Vec<u32>,
    /// Row has an edge `(r-1, r)` on the row's own lane.
    in_contig_same_lane: Vec<bool>,
    /// Diff array for "an edge passes THROUGH the row": `+1` at `from+1`,
    /// `-1` at `to`; prefix-summed in `finish` (O(n+e) total).
    cross_diff: Vec<i64>,
}

impl FoldScan {
    pub fn new() -> Self {
        Self::default()
    }

    /// Rows walked so far.
    fn rows(&self) -> usize {
        self.lanes.len()
    }

    /// The real-merge rows accumulated so far (sorted). The cache store path
    /// persists these so cache-hit span recomputation never re-touches libgit2.
    pub fn merge_rows(&self) -> &[u32] {
        &self.merge_rows
    }

    /// Append the next row (rows MUST arrive in walk order).
    pub fn push_row(&mut self, lane: u32, refs_empty: bool, real_merge: bool) {
        if real_merge {
            self.merge_rows.push(self.rows() as u32);
        }
        self.lanes.push(lane);
        self.refs_empty.push(refs_empty);
        self.out_cnt.push(0);
        self.out_contig.push(false);
        self.in_cnt.push(0);
        self.in_contig_same_lane.push(false);
        self.cross_diff.push(0);
    }

    /// Record a finalized edge. Both `from` and `to` must already be pushed
    /// (`from < to < rows`) — true for the walk, which finalizes an edge when
    /// its parent row is emitted. Out-of-range edges are ignored (defensive;
    /// never produced by a real walk).
    pub fn push_edge(&mut self, from: u32, to: u32, lane: u32) {
        let n = self.rows() as u32;
        if from >= to || to >= n {
            return;
        }
        let (f, t) = (from as usize, to as usize);
        self.out_cnt[f] += 1;
        if to == from + 1 {
            self.out_contig[f] = true;
        }
        self.in_cnt[t] += 1;
        if from == to - 1 && lane == self.lanes[t] {
            self.in_contig_same_lane[t] = true;
        }
        if to - from > 1 {
            // Marks rows (from, to) exclusive as crossed.
            self.cross_diff[f + 1] += 1;
            self.cross_diff[t] -= 1;
        }
    }

    /// Compute the maximal spans. `head_index` and `first_parent` come from
    /// the finished walk / the requested filter.
    pub fn finish(self, head_index: Option<u32>, first_parent: bool) -> Vec<FoldSpan> {
        let n = self.rows();
        let mut spans: Vec<FoldSpan> = Vec::new();
        let mut crossed: i64 = 0;
        let mut merge_iter = self.merge_rows.iter().copied().peekable();
        let mut run_start: Option<usize> = None;

        for r in 0..n {
            crossed += self.cross_diff[r];
            let is_merge = merge_iter.peek() == Some(&(r as u32));
            if is_merge {
                merge_iter.next();
            }
            let foldable = self.refs_empty[r]
                && head_index != Some(r as u32)
                && self.out_cnt[r] == 1
                && self.out_contig[r]
                && self.in_cnt[r] == 1
                && self.in_contig_same_lane[r]
                && crossed == 0
                && !(first_parent && is_merge);
            match (foldable, run_start) {
                (true, None) => run_start = Some(r),
                (false, Some(s)) => {
                    push_span(&mut spans, s, r, &self.lanes);
                    run_start = None;
                }
                _ => {}
            }
        }
        if let Some(s) = run_start {
            push_span(&mut spans, s, n, &self.lanes);
        }
        spans
    }
}

/// Close the run `[s, end)` into a span when it meets [`MIN_FOLD_RUN`].
fn push_span(spans: &mut Vec<FoldSpan>, s: usize, end: usize, lanes: &[u32]) {
    let count = (end - s) as u32;
    if count >= MIN_FOLD_RUN {
        spans.push(FoldSpan {
            start: s as u32,
            count,
            lane: lanes[s],
        });
    }
}

/// Batch form used by the one-shot layout and the cache-hit recompute paths.
///
/// `edges` is an iterator of `(from, to, lane)` triples so BOTH `GraphEdge`
/// and cached `GraphStreamEdge` rows can feed it (signature deviation from the
/// plan's `&[GraphEdge]`, recorded — the rule never needs `ord`).
/// `merge_rows` must be sorted ascending (it is, by construction).
pub fn compute_fold_spans(
    lanes: &[u32],
    refs_empty: &[bool],
    edges: impl IntoIterator<Item = (u32, u32, u32)>,
    head_index: Option<u32>,
    merge_rows: &[u32],
    first_parent: bool,
) -> Vec<FoldSpan> {
    let mut scan = FoldScan::new();
    for (i, (&lane, &empty)) in lanes.iter().zip(refs_empty.iter()).enumerate() {
        scan.push_row(lane, empty, merge_rows.binary_search(&(i as u32)).is_ok());
    }
    for (from, to, lane) in edges {
        scan.push_edge(from, to, lane);
    }
    scan.finish(head_index, first_parent)
}

/// Cache-hit recompute: derive spans from an already-walked (and freshly
/// redecorated) chunk stream. Reads lanes + refs-emptiness + edges from the
/// `Batch` chunks and `head_index` from the `Done` chunk — call it AFTER
/// `redecorate_chunks` so moved refs/HEAD split runs correctly.
pub fn fold_spans_of_chunks(
    chunks: &[GraphChunk],
    merge_rows: &[u32],
    first_parent: bool,
) -> Vec<FoldSpan> {
    let mut lanes: Vec<u32> = Vec::new();
    let mut refs_empty: Vec<bool> = Vec::new();
    let mut edges: Vec<(u32, u32, u32)> = Vec::new();
    let mut head_index: Option<u32> = None;
    for chunk in chunks {
        match chunk {
            GraphChunk::Batch {
                nodes,
                edges: batch_edges,
                ..
            } => {
                for n in nodes {
                    lanes.push(n.lane);
                    refs_empty.push(n.refs.is_empty());
                }
                edges.extend(batch_edges.iter().map(|e| (e.from, e.to, e.lane)));
            }
            GraphChunk::Done {
                head_index: hi, ..
            } => head_index = *hi,
            GraphChunk::Meta { .. } => {}
        }
    }
    compute_fold_spans(&lanes, &refs_empty, edges, head_index, merge_rows, first_parent)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(from, to, lane)` edge triples.
    type Edges = Vec<(u32, u32, u32)>;

    /// Straight chain of `n` rows: edges `(r, r+1)` on lane 0.
    fn chain(n: u32) -> (Vec<u32>, Vec<bool>, Edges) {
        let lanes = vec![0u32; n as usize];
        let refs_empty = vec![true; n as usize];
        let edges = (0..n - 1).map(|r| (r, r + 1, 0)).collect();
        (lanes, refs_empty, edges)
    }

    /// 12-row chain, tip has a ref: rows 1..=10 fold (root row 11 has no
    /// out-edge; row 0 has a pill).
    #[test]
    fn straight_run_folds() {
        let (lanes, mut refs_empty, edges) = chain(12);
        refs_empty[0] = false; // tip pill
        let spans = compute_fold_spans(&lanes, &refs_empty, edges, Some(0), &[], false);
        assert_eq!(
            spans,
            vec![FoldSpan {
                start: 1,
                count: 10,
                lane: 0
            }]
        );
    }

    /// A run hiding exactly MIN_FOLD_RUN-1 rows never folds; exactly
    /// MIN_FOLD_RUN does.
    #[test]
    fn min_run_boundary() {
        // 6 rows: foldable rows 1..=4 (4 rows) → below minimum.
        let (lanes, mut refs_empty, edges) = chain(6);
        refs_empty[0] = false;
        let spans = compute_fold_spans(&lanes, &refs_empty, edges, Some(0), &[], false);
        assert!(spans.is_empty(), "4 hidden rows must not fold");

        // 7 rows: foldable rows 1..=5 (5 rows) → exactly the minimum.
        let (lanes, mut refs_empty, edges) = chain(7);
        refs_empty[0] = false;
        let spans = compute_fold_spans(&lanes, &refs_empty, edges, Some(0), &[], false);
        assert_eq!(
            spans,
            vec![FoldSpan {
                start: 1,
                count: 5,
                lane: 0
            }]
        );
    }

    /// A ref mid-run splits it; both halves fold only if long enough.
    #[test]
    fn ref_mid_run_splits() {
        let (lanes, mut refs_empty, edges) = chain(14);
        refs_empty[0] = false;
        refs_empty[6] = false; // tag mid-run
        let spans = compute_fold_spans(&lanes, &refs_empty, edges, Some(0), &[], false);
        assert_eq!(
            spans,
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

    /// `head_index` splits a run even when the row carries no pill.
    #[test]
    fn head_index_splits_run() {
        let (lanes, mut refs_empty, edges) = chain(14);
        refs_empty[0] = false;
        let spans = compute_fold_spans(&lanes, &refs_empty, edges, Some(6), &[], false);
        assert_eq!(spans.len(), 2);
        assert_eq!((spans[0].start, spans[0].count), (1, 5));
        assert_eq!((spans[1].start, spans[1].count), (7, 6));
    }

    /// A long edge crossing the run (another lane) blocks folding of the rows
    /// it passes THROUGH — endpoints stay independently checked.
    #[test]
    fn crossing_edge_blocks_fold() {
        let (lanes, mut refs_empty, mut edges) = chain(16);
        refs_empty[0] = false;
        // A lane-1 edge from row 2 down to row 12 crosses rows 3..=11.
        edges.push((2, 12, 1));
        let spans = compute_fold_spans(&lanes, &refs_empty, edges, Some(0), &[], false);
        // Rows 2 and 12 now have out/in count 2 → not foldable either; the
        // only clean run left is 13..=14 (2 rows) → below minimum. Row 1 is a
        // singleton run. No spans.
        assert!(spans.is_empty(), "crossed rows must not fold: {spans:?}");
    }

    /// Merge/fork rows are excluded by the edge counts alone (rule 2/3).
    #[test]
    fn merge_and_fork_rows_excluded() {
        // Chain with a merge at row 3 (two in-edges... actually two OUT
        // edges: a merge commit has 2 parents → 2 outgoing edges).
        let (lanes, mut refs_empty, mut edges) = chain(12);
        refs_empty[0] = false;
        edges.push((3, 9, 1)); // second parent edge of row 3
        let spans = compute_fold_spans(&lanes, &refs_empty, edges, Some(0), &[], false);
        for s in &spans {
            for r in s.start..s.start + s.count {
                assert!(r != 3 && r != 9, "merge/fork row {r} inside a span");
                assert!(!(4..9).contains(&r), "crossed row {r} inside a span");
            }
        }
    }

    /// Rule 5: under first-parent a real merge (invisible to the edge model)
    /// is excluded via `merge_rows`; without first-parent the bitset is ignored.
    #[test]
    fn first_parent_merge_rows_excluded() {
        let (lanes, mut refs_empty, edges) = chain(12);
        refs_empty[0] = false;
        let merge_rows = [4u32, 5, 6];
        let spans = compute_fold_spans(
            &lanes,
            &refs_empty,
            edges.clone(),
            Some(0),
            &merge_rows,
            true,
        );
        // Foldable: 1..=3 (3, too short), 7..=10 (4, too short).
        assert!(spans.is_empty(), "merge rows must split runs: {spans:?}");
        let spans = compute_fold_spans(&lanes, &refs_empty, edges, Some(0), &merge_rows, false);
        assert_eq!(spans.len(), 1, "merge_rows ignored when !first_parent");
    }

    /// The last row (root or cap-truncated) has no out-edge → never foldable,
    /// so a run adjacent to the truncation cap keeps its anchor.
    #[test]
    fn last_row_never_folds() {
        let (lanes, mut refs_empty, edges) = chain(10);
        refs_empty[0] = false;
        let spans = compute_fold_spans(&lanes, &refs_empty, edges, Some(0), &[], false);
        assert_eq!(spans, vec![FoldSpan { start: 1, count: 8, lane: 0 }]);
    }

    /// Batch form == incremental FoldScan on a branching shape (guard for the
    /// wrapper staying a wrapper).
    #[test]
    fn batch_equals_incremental() {
        let lanes = [0u32, 0, 1, 0, 0, 0, 0, 0, 0, 0];
        let refs_empty = [false, true, false, true, true, true, true, true, true, true];
        let edges = [
            (0u32, 1u32, 0u32),
            (1, 3, 0),
            (2, 3, 1),
            (3, 4, 0),
            (4, 5, 0),
            (5, 6, 0),
            (6, 7, 0),
            (7, 8, 0),
            (8, 9, 0),
        ];
        let batch = compute_fold_spans(&lanes, &refs_empty, edges, Some(0), &[3], true);
        let mut scan = FoldScan::new();
        for (i, (&l, &e)) in lanes.iter().zip(refs_empty.iter()).enumerate() {
            scan.push_row(l, e, i == 3);
        }
        for &(f, t, l) in &edges {
            scan.push_edge(f, t, l);
        }
        assert_eq!(scan.merge_rows(), &[3]);
        assert_eq!(batch, scan.finish(Some(0), true));
    }

    /// Perf guard: 100k synthetic rows compute in well under the frame budget.
    #[test]
    fn span_computation_is_cheap_at_100k() {
        let n = 100_000u32;
        let (lanes, mut refs_empty, edges) = chain(n);
        for r in (0..n).step_by(50) {
            refs_empty[r as usize] = false;
        }
        let t0 = std::time::Instant::now();
        let spans = compute_fold_spans(&lanes, &refs_empty, edges, Some(0), &[], false);
        let elapsed = t0.elapsed();
        assert!(!spans.is_empty());
        assert!(
            elapsed.as_millis() < 250,
            "span computation too slow: {elapsed:?}"
        );
    }
}
