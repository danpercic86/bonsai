// Spec-004: fixture-side fold-span computation for the browser harness.
// Mirrors the Rust rule in crates/bonsai-core/src/graph/fold.rs — a row is
// foldable iff it carries no refs, is not HEAD, has exactly one contiguous
// outgoing edge (r, r+1), exactly one contiguous same-lane incoming edge
// (r-1, r), no other edge crosses it, and (under first-parent) it is not a
// REAL merge. Maximal runs of >= MIN_FOLD_RUN foldable rows become spans.
// Approximation for `VITE_MOCK_IPC=1` only; Rust remains the truth.
import type { FoldSpan, GraphLayout } from '../../types';
import { MIN_FOLD_RUN } from '../../../graph/foldModel';

/** Compute fold spans over a (possibly filtered) fixture layout. `mergeRows`
 *  are rows whose REAL parent count (pre-first-parent-truncate) is > 1 — the
 *  filtered layout's own `parents` cannot show that under first-parent. */
export function computeMockFoldSpans(
  layout: GraphLayout,
  mergeRows: readonly number[],
  firstParent: boolean,
): FoldSpan[] {
  const n = layout.nodes.length;
  const outCnt = new Uint32Array(n);
  const inCnt = new Uint32Array(n);
  const outContig = new Uint8Array(n);
  const inContigSameLane = new Uint8Array(n);
  // Diff-array for "an edge passes THROUGH the row": +1 at from+1, -1 at to.
  const crossedDiff = new Int32Array(n + 1);
  for (const e of layout.edges) {
    outCnt[e.from]++;
    if (e.to === e.from + 1) outContig[e.from] = 1;
    inCnt[e.to]++;
    if (e.from === e.to - 1 && e.lane === layout.nodes[e.to].lane) inContigSameLane[e.to] = 1;
    if (e.to - e.from > 1) {
      crossedDiff[e.from + 1]++;
      crossedDiff[e.to]--;
    }
  }
  const merges = new Set(mergeRows);
  const spans: FoldSpan[] = [];
  let crossed = 0;
  let runStart = -1;
  const flush = (end: number): void => {
    if (runStart >= 0 && end - runStart >= MIN_FOLD_RUN) {
      spans.push({ start: runStart, count: end - runStart, lane: layout.nodes[runStart].lane });
    }
    runStart = -1;
  };
  for (let r = 0; r < n; r++) {
    crossed += crossedDiff[r];
    const node = layout.nodes[r];
    const foldable =
      (node.refs === undefined || node.refs.length === 0) &&
      layout.headIndex !== r &&
      outCnt[r] === 1 &&
      outContig[r] === 1 &&
      inCnt[r] === 1 &&
      inContigSameLane[r] === 1 &&
      crossed === 0 &&
      !(firstParent && merges.has(r));
    if (foldable) {
      if (runStart < 0) runStart = r;
    } else {
      flush(r);
    }
  }
  flush(n);
  return spans;
}
