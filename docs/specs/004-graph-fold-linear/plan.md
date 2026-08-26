# Plan: Fold Linear Runs — Collapse Uneventful History into Expandable Rows

**Spec:** ./spec.md
**UI contract:** ../../contracts/spec-004-ui.md (pending — ui-designer)
**Status:** draft

## Approach

**Expansion mechanism decided: Option C — Rust computes fold *spans*, the frontend collapses
rows as a pure view transform.** Three candidates were weighed:

- **A. Re-request with an `expandedRuns` exclusion param** (brief's sketch): every
  expand/collapse click costs an IPC round-trip + walk/cache churn; backend must model
  per-view expansion state. Rejected.
- **B. Backend emits synthetic folded nodes + a targeted-unfold command:** still one IPC
  round-trip per click, mutates the row model on the wire (row indices shift on every
  expand → selection/edge remapping churn), and AC4 ("off = identical view") requires a
  refetch. Rejected.
- **C (chosen). Backend emits the normal (filtered) node/edge stream unchanged, PLUS
  `foldSpans` metadata — which contiguous row ranges are safely foldable.** Frontend keeps
  a display-row ↔ model-row mapping and renders a placeholder row per collapsed span.
  Expand/collapse, transient expansion state, AC4, AC6 (selection preserved), and reveal
  auto-expand are all zero-IPC frontend-local state. The Rust-owns-topology invariant
  holds: Rust decides *which rows are foldable* (the topology/lane math); React only
  applies a row mapping — the same category of work as virtualization.
  Honest tradeoff, recorded: fold under C does not shrink the walk or the IPC payload,
  only rendered rows — exactly what the spec's perf goal asks ("fewer rows to render").

**FLAG (deviation from the brief/prompt): NO node-schema change.** `GraphNode`/`StreamNode`
gain no `kind`/`folded` field — under C there is no synthetic node on the wire. The
placeholder is a frontend row-model construct built from `FoldSpan`s.

**Decisions recorded:**

- **Minimum run length: `MIN_FOLD_RUN = 5` hidden commits.** Runs hiding < 5 rows never
  fold (a "⋯ 2 commits" pill is worse than the rows; 5 also keeps span counts tiny).
  Constant in Rust `graph/fold.rs`; mirrored in the mock.
- **Conservative lane rule (precise — defined over lanes/refs/edges ONLY, no `parents`
  dependency, so one rule serves the one-shot layout, the stream, and cached replay whose
  `StreamNode`s carry no parents):** row `r` (node `n`) is *foldable* iff ALL of:
  1. `n.refs` is empty (covers branch/tag/HEAD pills AND stash rows — stash `W` rows carry
     a stash label) and `r != head_index` (detached-HEAD-no-label safety);
  2. exactly one edge leaves `r` (`from == r`), and it is `(from: r, to: r+1)` — one
     parent, contiguous (its lane == `n.lane` automatically: the p0 edge inherits the
     child's lane per lane.rs). A truncated/absent parent ⇒ no outgoing edge ⇒ not
     foldable — cap-adjacent rows are safe;
  3. exactly one edge arrives at `r` (`to == r`), and it is `(from: r-1, to: r)` on
     `n.lane` (single child, contiguous);
  4. no other edge crosses the row: there is NO edge `e` with `e.from < r < e.to`
     (any long edge — same lane or another lane — passing through the row would be
     misrendered by a placeholder; when in doubt, don't fold);
  5. under `first_parent`, `n` is not a *real* merge (see merge-bit mechanism below) —
     AC3's "merge commits are never folded" holds even when the view truncates parents.
  A **fold span** is a maximal contiguous run of foldable rows with `count >= MIN_FOLD_RUN`.
  The anchor rows above/below the span stay visible; `count` == hidden rows == the pill's N.
- **`GraphFilter` gains `fold_linear: bool`** (serde-default — wire-compatible exactly as
  filter.rs anticipated). It gates span computation only; it does NOT change the walk.
  **Cache: toggling fold is a Hit, not a re-walk** — `classify` compares a *walk key*
  (`GraphFilter` with `fold_linear` cleared: `filter.walk_eq(&cached.filter)`); spans are
  produced per request when `fold_linear` is set.
- **Spans are computed AFTER redecorate (Trap: refs move).** Eligibility depends on refs, and
  the `HitRedecorate` path refreshes pills on a cached node set — a ref landing mid-run must
  break that run. Mechanism: spans are **computed per-request in the serve path, post-
  redecorate**, never cached: fresh walks accumulate them in the stream core (incremental
  `FoldScan`, below); cache Hit/HitRedecorate paths recompute from the cached rows/edges +
  refreshed refs via `compute_fold_spans` (O(n+e) over ≤100k rows — cheap).
- **Real-merge bit under first-parent (Trap: cached path has no walk).** `LaneWalker::step`
  sees `commit.parent_ids()` before the spec-003 `truncate(1)`; it records rows whose real
  parent count > 1. `CachedGraph` stores this as `merge_rows: Vec<u32>` (sorted; small) so
  cache-hit span recomputation never re-touches libgit2. When `first_parent` is false the
  edge model already encodes merges (rule 2's single-outgoing-edge check excludes them);
  the bitset is only consulted under `first_parent`.
- **Streaming timing:** `fold_spans` ride the terminal `Done` chunk (spans need the full
  edge set). The graph streams in unfolded and collapses once at `done` — accepted; the
  view is already in motion while streaming. One-shot `GraphLayout` gains `foldSpans`.
- **Composition with spec-003 (AC5):** spans are computed over the *filtered* walk output,
  so fold-after-filter is automatic. The 100k/1M caps are untouched; `truncated` still set.
- **Condensed indication is frontend truth for fold** (deliberate asymmetry vs spec-003's
  backend `Meta.filtered`): the chip/indicator ORs in the local `graphFoldLinear` toggle.
  `Meta` is untouched — fold never changes the walk, so there is no backend "did it apply"
  question; "no spans found" is not an error state.
- **Expansion state is transient** (spec-locked): a per-session `Set<number>` of expanded
  span `start` rows in a new `useGraphFold` hook; cleared on repo change, filter change, and
  graph reload (spans arriving from a new `done` reset it).
- **Selection preservation (AC6):** selection stays keyed by oid/model row (it already is);
  when spans arrive (fold enabled) and the selected model row falls inside a span, that span
  is added to the expanded set before the first folded paint — the commit stays visible and
  selected.
- **Reveal auto-expand (spec edge case):** `revealCommitByOid` resolves oid → model row; if
  inside a collapsed span, expand that span first, then scroll (mapping recomputes
  synchronously before scrollIntoView).
- **Toggle costs:** toggle-OFF clears spans + expansion state client-side — **no refetch**
  (the underlying rows never left the store), so AC4 holds by construction. Toggle-ON
  re-requests the stream (spec-003 filter-change pattern) — a cache-Hit **replay** of up to
  100k rows just to obtain spans. Considered and accepted for v1: it reuses the existing
  request path unchanged; do NOT add a separate get-spans command without revisiting this.
- **Persistence:** `graph_fold_linear: bool` (default false), exact `graph_first_parent`
  precedent — plain bool, no double-option: settings.rs + ui_settings.rs snapshot/patch,
  `useUiSettings`, `uiSettingsDefaults.json`/`defaults.ts`, mock persistence validation.

## Rust/TS boundary

Rust: the fold-eligibility rule, span computation (fresh + cached paths), the merge-row
bitset, `fold_linear` on the wire filter, spans on `Done`/`GraphLayout`. React only: the
persisted toggle, the display↔model row mapping + transient expansion set, placeholder
painting/hit-testing, chip indication, reveal/selection auto-expand. No client-side
topology decisions in the real path (mock approximates the same rule, fixture-side only).

## Files touched

- `crates/bonsai-core/src/graph/filter.rs` — `GraphFilter` gains `fold_linear: bool` +
  `pub fn walk_eq(&self, other: &GraphFilter) -> bool` (compares `first_parent` +
  `seed_refs` only) (~12 ln).
- `crates/bonsai-core/src/graph/lane.rs` — `step()` records real-merge info: new
  `pub(super) fn last_was_merge(&self) -> bool` (set from `commit.parent_ids().len() > 1`
  BEFORE the first-parent truncate) (~8 ln).
- `crates/bonsai-core/src/graph/stream.rs` — `stream_graph_from_repo_with` feeds a
  `FoldScan` accumulator per row (lane, refs-empty, finalized edges, merge bit) when
  `filter.fold_linear`; `Done` gains `fold_spans: Vec<FoldSpan>` (serde-default) and
  `merge_rows` is surfaced to the cache-store path (~30 ln).
- `crates/bonsai-core/src/graph.rs` (~473 ln, near cap — threading only): `GraphLayout`
  gains `fold_spans: Vec<FoldSpan>` (`skip_serializing_if empty`); one-shot path calls
  `compute_fold_spans` when `filter.fold_linear`; re-export `FoldSpan` from `fold.rs`
  (~15 ln).
- `src-tauri/src/graph_cache.rs` — `classify` uses `walk_eq`; `CachedGraph` stores
  `merge_rows: Vec<u32>`; Hit + HitRedecorate paths call `compute_fold_spans` post-
  redecorate when the *requested* filter has `fold_linear` (spans are per-request, never
  cached; replayed `Done` chunks get spans injected at replay time) (~35 ln).
- `src-tauri/src/commands/status.rs` — no signature change (`Option<GraphFilter>` already
  carries the new field via serde default) (~0 ln; verify only).
- `src/ipc/types/graph.ts` — `FoldSpan` interface; `GraphFilter.foldLinear: boolean`;
  `done` chunk + `GraphLayout` gain `foldSpans?: FoldSpan[]` (~12 ln).
- `src/components/repoWorkspace/graphStreamApply.ts` — surface `done.foldSpans` through the
  assembler setter bundle (~8 ln).
- `src/components/RepoWorkspace.tsx` (>500 exception; delta small) — wire `useGraphFold`
  (spans from stream state, toggle from settings, expansion set), pass fold row-model to
  the graph pane; AC6 initial-expand on spans arrival (~20 ln).
- `src/graph/GraphCanvas.tsx` (frontend in flux — line refs approximate) — consume the
  display-row model: virtualization window, hit-testing, and selection go through
  `displayToModel`/`modelToDisplay`; fold-pill rows hit-test to an `onToggleSpan(start)`
  callback, are not selectable as commits; keyboard up/down skips over pills or toggles on
  Enter per the UI contract (~40 ln).
- `src/graph/draw.ts` — **AT the 500-line cap: no new painting here.** Only a dispatch
  branch: fold rows delegate to `drawFold.ts` (~6 ln).
- `src/components/repoWorkspace/useReadOverlays.ts` — `revealCommitByOid` gains the
  expand-before-scroll step (via a callback/ref from `useGraphFold`) (~10 ln).
- `src/hooks/useUiSettings.ts`, `src/settings/uiSettingsDefaults.json`,
  `src/settings/defaults.ts` — `graphFoldLinear: false` (~10 ln total).
- `src/ipc/mock/persistence.ts` — validate + round-trip `graphFoldLinear` (bool; malformed
  → false) (~6 ln).
- `src/ipc/mock/handlers/graphStream.ts` — when `filter.foldLinear`, run the fixture layout
  through the mock fold (below) and attach `foldSpans` to the `done` chunk (~8 ln).
- Spec-003 filter UI surfaces (`GraphFilterPopover.tsx` / `SettingsGraphDeclutterSection.tsx`
  / `useGraphFilter.ts` or per the spec-004 UI contract) — the "Fold linear runs" toggle
  joins the declutter controls; chip "condensed" state ORs in `graphFoldLinear` (~20 ln).

## New files

- `crates/bonsai-core/src/graph/fold.rs` (~170 ln — keeps graph.rs under the cap) —
  `FoldSpan`, `MIN_FOLD_RUN`, and the batch form used by one-shot + cache paths:
  `compute_fold_spans(lanes: &[u32], refs_empty: &[bool], edges: &[GraphEdge],
  head_index: Option<u32>, merge_rows: &[u32], first_parent: bool) -> Vec<FoldSpan>`
  (parameters chosen so BOTH `GraphNode` and cached `StreamNode` rows can feed it —
  the rule never needs `parents`), plus the incremental `FoldScan`
  (`push_row(...)` / `finish() -> Vec<FoldSpan>`) used by the stream core. Both implement
  the identical per-row predicate (shared fn); a unit test asserts batch == incremental
  on a fixture.
- `src/graph/foldModel.ts` (~120 ln) — pure row-model math: given
  `(totalRows, spans, expandedSet)` build sorted collapsed spans + prefix sums;
  `displayRowCount`, `displayToModel(d): { kind: 'commit'; row } | { kind: 'fold'; span }`,
  `modelToDisplay(row)` (binary search, O(log s)), `spanContaining(row)`. No canvas, no
  React — unit-testable.
- `src/hooks/useGraphFold.ts` (~90 ln) — toggle (via `useUiSettings`), spans from stream
  state, transient `expanded: Set<number>`, `toggleSpan`, `expandFor(modelRow)` (AC6 +
  reveal), reset-on-reload/filter-change; memoized `foldModel`.
- `src/graph/drawFold.ts` (~90 ln) — paints the placeholder row: "⋯ N commits" pill in the
  span's lane color, expand/collapse affordance, hover state; geometry per the UI contract.
  Sibling of draw.ts/drawBonsai.ts (draw.ts is at cap).
- `src/ipc/mock/handlers/graphFold.ts` (~80 ln) — fixture-side `computeFoldSpans` mirroring
  the Rust rule against the full mock `GraphLayout` (same edge-only predicate; the mock
  passes `merge_rows` derived from `parents.length > 1` since it has full parents).
  Approximation is fine; Rust remains the truth.
- `crates/bonsai-core/tests/graph_fold.rs` (~180 ln) — see Testing.
- `src/graph/foldModel.test.ts` + mock fold tests (vitest).

## Data model / types

```rust
// crates/bonsai-core/src/graph/fold.rs
pub const MIN_FOLD_RUN: u32 = 5;

/// A maximal foldable run. `start..start+count` are the HIDDEN model rows;
/// the anchor rows `start-1` and `start+count` remain visible.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FoldSpan {
    pub start: u32, // first hidden row (model index)
    pub count: u32, // hidden rows == the pill's N (>= MIN_FOLD_RUN)
    pub lane: u32,  // the run's lane (pill color)
}

// filter.rs
pub struct GraphFilter {
    pub first_parent: bool,
    pub seed_refs: Option<Vec<String>>,
    pub fold_linear: bool, // spec-004; gates span computation only, never the walk
}
impl GraphFilter { pub fn walk_eq(&self, o: &Self) -> bool /* first_parent + seed_refs */ }

// stream.rs Done variant (additive, serde-default):
//   fold_spans: Vec<FoldSpan>   // empty when fold off or none found
// graph.rs GraphLayout (additive): fold_spans: Vec<FoldSpan> (skip if empty)
// graph_cache.rs CachedGraph: merge_rows: Vec<u32> (sorted; rows with real parent_count > 1)
// settings.rs UiSettings: graph_fold_linear: bool (default false)
```

```ts
// src/ipc/types/graph.ts
export interface FoldSpan { start: number; count: number; lane: number }
// GraphFilter gains: foldLinear: boolean
// done chunk gains:  foldSpans?: FoldSpan[]   (absent == [])
// GraphLayout gains: foldSpans?: FoldSpan[]

// src/graph/foldModel.ts
export type DisplayRow =
  | { kind: 'commit'; row: number }                  // model row
  | { kind: 'fold'; span: FoldSpan; expanded: false }; // one display row per collapsed span
```

## Fold-span algorithm (pseudocode — batch form; FoldScan is the same rule online)

```
compute_fold_spans(lanes, refs_empty, edges, head_index, merge_rows, first_parent):
  n = lanes.len
  crossed  = bitset(n)      // any edge passes THROUGH the row — mark via
                            // diff-array +1 at from+1, -1 at to, prefix-sum (O(n+e))
  out_cnt  = count[n]; out_contig = bitset(n)   // edge (r, r+1)
  in_cnt   = count[n]; in_contig_same_lane = bitset(n)
  for e in edges:
    out_cnt[e.from] += 1
    if e.to == e.from + 1: out_contig.set(e.from)
    in_cnt[e.to] += 1
    if e.from == e.to - 1 and e.lane == lanes[e.to]: in_contig_same_lane.set(e.to)
    if e.to - e.from > 1: crossed.mark_range(e.from+1, e.to-1)

  foldable(r):
    refs_empty[r]                                  // also excludes stash rows (stash label)
    and Some(r) != head_index
    and out_cnt[r] == 1 and out_contig[r]          // rule 2: one parent, contiguous
                                                   // (lane inherited by p0 per lane.rs;
                                                   //  merges excluded: out_cnt would be >1)
    and in_cnt[r] == 1 and in_contig_same_lane[r]  // rule 3: one child, contiguous
    and !crossed[r]                                // rule 4: nothing passes through
    and !(first_parent and merge_rows.contains(r)) // rule 5: real merges under first-parent

  spans = maximal contiguous runs of foldable rows with len >= MIN_FOLD_RUN
          → FoldSpan { start, count: len, lane: lanes[start] }
```

## Testing

- **Regression (locked):** `fold_linear: false` ⇒ `fold_spans` empty and nodes/edges
  byte-identical to spec-003 output; `walk_eq` ⇒ cache Hit on fold toggle (no re-walk).
- Rule unit tests (fixture repos via git2): straight 10-commit run → one span, count/lane
  right (AC1); run of 4 → no span; ref mid-run splits it; HEAD mid-run splits it; stash
  row never inside a span; merge/fork rows never foldable (AC3); a second lane's long edge
  crossing the run → no fold (rule 4); run adjacent to the truncation cap folds, `truncated`
  still true.
- First-parent composition: real merge without refs under `first_parent` is NOT folded
  (merge_rows path), including via the cache-hit recompute (AC5).
- Batch (`compute_fold_spans`) == incremental (`FoldScan`) on a branching fixture.
- Redecorate: cached graph, then a branch is created on a mid-run commit → next request's
  spans exclude that row (post-redecorate computation).
- Stream/one-shot parity: `done.fold_spans == layout.fold_spans` under the same filter.
- Frontend (vitest): `foldModel` mapping (displayRowCount, displayToModel/modelToDisplay
  round-trip, expand/collapse, spanContaining); `useGraphFold` reset-on-reload + AC6
  initial-expand; mock fold rule; settings round-trip for `graphFoldLinear` (malformed →
  false).
- Harness/e2e: toggle fold in the mock → row count shrinks, "⋯ N" pill visible; click pill
  → rows appear in place + re-collapse affordance (AC2); toggle off → identical view (AC4);
  reveal to a hidden commit auto-expands; selection inside a run survives enabling fold
  (AC6); persistence across reload (AC7).
- Perf: span computation on the 20k fixture ≤ a few ms (timed test); scroll feel with fold
  on = USER CHECKPOINT (AC8).

## Risks / open questions

- **FLAG (for orchestrator): no GraphNode/StreamNode schema change** — the prompt/brief
  assumed a folded node kind; option C replaces it with `FoldSpan` metadata + a frontend
  row model. Confirm ui-designer designs against display rows, not wire nodes.
- **FLAG (minor): spans arrive only at `done`** — during streaming the graph shows
  unfolded, then collapses once. Accepted; avoiding it would require speculative spans per
  batch for no real gain.
- **FLAG (minor): keyboard semantics on fold pills** (skip vs focus+Enter-to-expand) —
  ui-designer's call; GraphCanvas supports either via the display-row model.
- Frontend files are concurrently in flux (another agent) — frontend line counts/hook
  names above are approximate; senior-dev must re-verify `GraphCanvas.tsx` /
  `useReadOverlays.ts` integration points at implementation time.
- Rule 4 (no crossing edge) is deliberately conservative: busy multi-lane regions fold
  rarely. Accepted per spec ("when in doubt, don't fold").
- `merge_rows` grows `CachedGraph` slightly (u32 per merge). Negligible.
- draw.ts is at the 500-line cap — enforced: all new painting in `drawFold.ts`; if the
  dispatch branch tips it over, move an existing helper out in the same increment.
