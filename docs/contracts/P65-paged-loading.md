# P65 — Incremental / Paged Commit Loading: Implementation Contract

Status: authoritative for P65. Independent Phase-4 milestone. Implementer: senior-dev, in three
fresh-context passes (P65a Rust core+command, P65b frontend assembler+renderer, P65c mock+harness+
tests). Builds directly on `docs/contracts/M2-graph.md` (the deferred `stream_graph` sketch, §1.1/
§2.7/§8.3) — this contract turns that sketch into a spec. Does NOT relitigate the M2 wire types,
lane algorithm, edge geometry (§1.3), scroll model (§4.1), or virtualization (§4.2); it reuses them
verbatim and adds an incremental delivery skin.

Current file state (read before implementing):
- `crates/bonsai-core/src/graph.rs` — `compute_graph`, the sequential `layout_walk`, `GraphNode/
  Edge/Layout`, `RefLabel`, `PendingEdge`, `first_free`, `MAX_COMMITS = 100_000`, stash injection.
- `src-tauri/src/commands/status.rs` — `get_graph` / `get_graph_inner` (spawn_blocking pattern).
- `src-tauri/src/commands/repo.rs::clone_repo`, `src-tauri/src/commands/history.rs::
  history_index_build` — the canonical `tauri::ipc::Channel<T>` streaming commands to mirror.
- `src/ipc/tauri.ts` (cloneRepo/historyIndexBuild Channel bridge), `src/ipc/types.ts`
  (GraphLayout/IpcApi, CloneProgress/IndexProgress), `src/ipc/mock/handlers/diff.ts::getGraph`,
  `src/ipc/mock/handlers/layout.ts::resolveLayout`.
- `src/graph/edgeIndex.ts` (bucket index), `src/graph/GraphCanvas.tsx` (edgeIndex memo, spacer,
  virtualization), `src/components/RepoWorkspace.tsx::refetchGraph` (last-wins `graphReqId` guard).

---

## 0. Decision: `stream_graph` channel — NOT `loadMore(offset,count)` paging

**Recommendation: a single `stream_graph` Tauri channel command that streams the layout forward in
batches.** Reject stateless `loadMore(offset, count)` paging.

Justification (the lane algorithm forces this): lane assignment at row `R` depends on the
accumulated active-lane state (`lanes: Vec<Option<Oid>>` + `pending`) built from **every** row
`0..R`. There is no way to compute row `R`'s lane — hence its color — without having walked all
prior rows. Therefore:
- A stateless `loadMore(offset,count)` must either **re-walk `0..offset` on every page** (O(n²)
  total, catastrophic at 200k+), or **persist a paused walk + its lane state** in a server-side
  session registry keyed by repo+generation (with eviction, staleness-on-`repo-changed`,
  cross-restart lifecycle) — strictly more machinery for the identical result.
- A **channel** expresses the walk natively: one continuous forward `spawn_blocking` pass flushes
  batches as it goes. Lane-color stability becomes **true by construction** — the streamed walk is
  byte-for-byte the same sequential computation as today's `compute_graph`, merely flushed in
  pieces. Batch boundaries touch no lane state.
- It is the codebase's established primitive for incremental data (`clone_repo`,
  `history_index_build`) and the CLAUDE.md-designated canonical channel use case.

"Fetch-more-on-scroll" is satisfied **eagerly**: the first batch (first screenful + overscan) is
flushed immediately for instant paint; the remainder streams in the background so it is almost
always ahead of the user's scroll. Scrolling never blocks on IPC (best native scroll feel). A
bounded/resume-on-scroll variant is possible but reintroduces the parked-walk session — deferred
(OQ4).

---

## 1. Module boundaries & file responsibilities

### Rust (P65a)
- `crates/bonsai-core/src/graph.rs` — extract the per-row step of `layout_walk` into a reusable
  `LaneWalker` (state owner); add `StreamNode`, `GraphStreamEdge`, `GraphChunk`, batch consts, and
  `stream_graph_core`. `compute_graph` is refactored to drive `LaneWalker` too, so the one-shot and
  streamed paths share ONE lane implementation (the equivalence guarantee). No behavior change to
  `compute_graph`'s output.
- `src-tauri/src/commands/status.rs` — add `stream_graph` (channel command; mirrors
  `history_index_build`). Register in `src-tauri/src/lib.rs` `generate_handler!`.

### TypeScript (P65b)
- `src/ipc/types.ts` — add `StreamNode`, `StreamEdge`, `GraphChunk`; extend `IpcApi` with
  `streamGraph`.
- `src/ipc/tauri.ts` — `streamGraph` Channel bridge (mirror `historyIndexBuild`).
- `src/graph/incrementalEdgeIndex.ts` — NEW. Growable bucket index with generation-stamped dedupe
  (order-independent, incremental-insert). `edgeIndex.ts` stays UNCHANGED (one-shot path + its
  tests).
- `src/graph/streamAssembler.ts` — NEW. Folds a `GraphChunk` stream into a growing `GraphLayout`
  (+ `oidToRow` map, + the incremental edge index, + progress fields). Pure, unit-testable.
- `src/graph/GraphCanvas.tsx` — two additive optional props (`edgeIndex?`, `totalRows?`); no
  behavior change when absent (one-shot path).
- `src/components/RepoWorkspace.tsx` — `refetchGraph` switches from `getGraph` to `streamGraph`;
  keeps the last-wins generation guard; progressive selection remap.

### Mock (P65c)
- `src/ipc/mock/handlers/diff.ts` (or a small new `src/ipc/mock/handlers/graphStream.ts` if diff.ts
  nears the 500-line limit — implementer's call) — add `streamGraph` that chunks `resolveLayout`.
  `getGraph` stays.

---

## 2. Wire protocol (channel messages — implement exactly)

### 2.1 Rust (`crates/bonsai-core/src/graph.rs`)

```rust
/// Streaming batch sizes (tunable; defaults chosen so the first paint is instant and the steady
/// event count is low: 200k rows => ~49 events).
pub const STREAM_FIRST_BATCH: usize = 512;   // first flush: first screenful + generous overscan
pub const STREAM_BATCH: usize = 4096;        // steady-state batch

/// Streaming walk cap. Larger than the one-shot MAX_COMMITS (100_000) because streaming exists for
/// huge repos; beyond it the stream ends with `truncated: true`. ~200 B/node => ~200 MB in the
/// frontend at the cap (OQ3 — memory vs a bounded/resume variant).
pub const STREAM_MAX_COMMITS: usize = 1_000_000;

/// A streamed commit row. Identical to `GraphNode` MINUS `parents`: parent row indices are not
/// known when a child is emitted (parents are always at HIGHER, not-yet-walked rows), so the
/// frontend reconstructs `parents` from edge ordinals (§4.2). Saves the per-node parents bytes.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamNode {
    pub id: String,
    pub lane: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<RefLabel>,
    pub summary: String,
    pub author: String,
    pub ts: i64,
    pub committer_ts: i64,
}

/// Logical edge as `GraphEdge` PLUS the child's parent ordinal (`ord`) so the frontend can rebuild
/// each node's ordered `parents`. `ord == 0` is the first parent (the lane-inheriting edge).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphStreamEdge {
    pub from: u32,   // child row (already delivered: from < to)
    pub to: u32,     // parent row == this batch's finalizing row
    pub lane: u32,   // vertical-run lane (M2 §1.3) — RUST-owned layout math
    pub ord: u16,    // parent ordinal on `from`
}

/// One channel message. Order on the wire: exactly one `Meta`, then N `Batch`, then exactly one
/// `Done`. On any error the command REJECTS (AppError) instead of sending `Done`.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum GraphChunk {
    /// First message. `total` = exact reachable-commit count IF cheaply known (OQ2: from the P52
    /// commit-graph file), else `None` (frontend grows the scroll extent as rows arrive).
    /// `head_oid` lets the frontend resolve `headIndex` the moment HEAD's row lands.
    Meta { total: Option<u32>, head_oid: Option<String> },
    /// A run of consecutive rows `[start_row, start_row + nodes.len())` plus the edges FINALIZED
    /// within them (every edge whose parent `to` falls in this batch; its child `from` was
    /// delivered earlier or in this same batch). `lane_count_so_far` is the running max
    /// (`lanes.len()`), monotonic — drives the graph-area width without ever shrinking.
    Batch { start_row: u32, lane_count_so_far: u32, nodes: Vec<StreamNode>, edges: Vec<GraphStreamEdge> },
    /// Terminal. Authoritative final scalars (redundant with the accumulated stream, for a clean
    /// close). `total_rows == nodes emitted`; `head_index` resolved; `truncated` set at the cap.
    Done { total_rows: u32, lane_count: u32, head_index: Option<u32>, truncated: bool },
}
```

Empty/unborn repo: emit `Meta { total: Some(0), head_oid: None }` then `Done { total_rows: 0,
lane_count: 0, head_index: None, truncated: false }` — never an error (parity with `compute_graph`).

### 2.2 TypeScript (`src/ipc/types.ts` — add verbatim)

```ts
export interface StreamNode {
  id: string;
  lane: number;
  refs?: RefLabel[];
  summary: string;
  author: string;
  ts: number;
  committerTs: number;
}

export interface StreamEdge {
  from: number;
  to: number;
  lane: number;
  ord: number;
}

export type GraphChunk =
  | { kind: 'meta'; total: number | null; headOid: string | null }
  | { kind: 'batch'; startRow: number; laneCountSoFar: number; nodes: StreamNode[]; edges: StreamEdge[] }
  | { kind: 'done'; totalRows: number; laneCount: number; headIndex: number | null; truncated: boolean };
```

`IpcApi` gains (place beside `getGraph`):

```ts
/** Stream the graph layout for a repo as ordered chunks (Meta -> Batch* -> Done). The frontend
 *  passes a plain callback; the Tauri impl bridges it through a `Channel`, the mock invokes it
 *  directly. Resolves when the stream completes (after the Done chunk). Rejects AppError
 *  ('noRepo' when the id is not open, 'git'). `getGraph` is retained (small-repo / tests). */
streamGraph(repoId: string, onChunk: (c: GraphChunk) => void): Promise<void>;
```

### 2.3 Tauri bridge (`src/ipc/tauri.ts` — mirror `historyIndexBuild`)

```ts
streamGraph(repoId: string, onChunk: (c: GraphChunk) => void): Promise<void> {
  const channel = new Channel<GraphChunk>();
  channel.onmessage = onChunk;
  return invoke<void>('stream_graph', { repoId, onChunk: channel });
}
```

---

## 3. Rust: the streaming walk (P65a — the lane-stability core)

### 3.1 `LaneWalker` extraction

Refactor `layout_walk`'s per-row body (graph.rs §2.4 steps 1–5) into a struct so BOTH paths share
it verbatim:

```rust
struct LaneWalker {
    lanes: Vec<Option<git2::Oid>>,                       // active-lane vector (grows only)
    pending: HashMap<git2::Oid, Vec<PendingEdge>>,       // edges awaiting their parent row
    index_of: HashMap<git2::Oid, u32>,                   // oid -> emitted row
    hidden: HashSet<git2::Oid>,                          // stash synthetic parents (unchanged)
}
// PendingEdge gains `ord: u16` (populated in routing: p0 -> 0, parents[k] -> k).

impl LaneWalker {
    /// Advance one commit at row `row`. Returns the node (WITHOUT parents) and the edges finalized
    /// AT this row (those whose parent == `oid`; each carries its child's `ord`). Mutates lane
    /// state EXACTLY as graph.rs §2.4 does today — this is the single source of lane truth.
    fn step(&mut self, repo: &git2::Repository, oid: git2::Oid, row: u32,
            refs: &mut RefMap) -> Result<(StreamNode, Vec<GraphStreamEdge>), AppError>;
    fn lane_count(&self) -> u32 { self.lanes.len() as u32 }
}
```

- `compute_graph` (unchanged output): drive `LaneWalker::step` for every row; collect nodes +
  edges; resolve `GraphNode.parents` at the end from `index_of` exactly as today; ignore `ord`.
  Cap stays `MAX_COMMITS = 100_000`.
- Determinism inputs (tip collection/sort, stash injection, `Sort::TOPOLOGICAL | Sort::TIME`) are
  UNCHANGED and shared — same repo state => same walk order => same lanes => same colors, streamed
  or not.

### 3.2 `stream_graph_core`

```rust
/// Blocking. Opens `workdir` (NO_SEARCH, same as compute_graph), collects refs + stash tips
/// identically, then walks forward flushing GraphChunk batches through `emit`. `emit` returns
/// false when the sink is gone (channel dropped / cancelled) -> stop promptly. Unborn/zero-ref =>
/// Meta(total:0) + Done. Never resolves node.parents (frontend does, §4.2).
pub fn stream_graph_core(
    workdir: &std::path::Path,
    mut emit: impl FnMut(GraphChunk) -> bool,
) -> Result<(), AppError>;
```

Pseudocode (the batching skin over the shared walk):

```
stream_graph_core(workdir, emit):
    repo = open(workdir, NO_SEARCH)
    stashes = collect_stashes(&mut repo); (refs, tips, head_oid) = collect_refs(repo)
    inject stash tips + hidden set        # identical to compute_graph
    if not emit(Meta { total: cheap_total(repo)?, head_oid: head_oid.map(to_hex) }): return Ok
    if tips empty: emit(Done { 0,0,None,false }); return Ok

    walker = LaneWalker::new(hidden)
    revwalk = TOPOLOGICAL|TIME, push each tip in the deterministic order
    buf_nodes=[]; buf_edges=[]; start_row=0; row=0; truncated=false
    limit = STREAM_FIRST_BATCH                       # small first flush = instant paint
    for oid in revwalk:
        if walker.hidden.contains(oid): continue     # stash I/U synthetic parents
        if row as usize >= STREAM_MAX_COMMITS: truncated=true; break
        (node, edges) = walker.step(repo, oid, row, &mut refs)
        buf_nodes.push(node); buf_edges.extend(edges)
        row += 1
        if buf_nodes.len() >= limit:
            if not emit(Batch { start_row, walker.lane_count(), buf_nodes, buf_edges }): return Ok
            start_row = row; buf_nodes=[]; buf_edges=[]; limit = STREAM_BATCH
    if not buf_nodes.is_empty():
        if not emit(Batch { start_row, walker.lane_count(), buf_nodes, buf_edges }): return Ok
    head_index = head_oid.and_then(|h| walker.index_of.get(h))
    emit(Done { total_rows: row, lane_count: walker.lane_count(), head_index, truncated })
    Ok(())
```

`cheap_total`: OQ2. Recommended v1 = `Ok(None)` (grow-as-you-go) unless the commit-graph-file entry
count is trivially available; a full pre-count walk is NOT worth doubling the work.

### 3.3 Command (`src-tauri/src/commands/status.rs`)

```rust
/// Streams the commit-graph layout of `repo_id` as GraphChunk batches over `on_chunk`
/// (channel command; mirrors history_index_build). Heavy git2 walk => spawn_blocking. Unborn/
/// zero-ref => a Meta+Done pair, not an error. Rejects git | noRepo.
#[tauri::command]
pub async fn stream_graph(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    on_chunk: tauri::ipc::Channel<GraphChunk>,
) -> Result<(), AppError> {
    let path = repo_path(state.inner(), &repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        stream_graph_core(&path, |chunk| on_chunk.send(chunk).is_ok())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
```

Register `commands::stream_graph` in `lib.rs` `generate_handler!`.

---

## 4. Frontend assembly (P65b)

### 4.1 Incremental edge index (`src/graph/incrementalEdgeIndex.ts`, NEW)

The existing `edgesInRange` dedupe relies on the global `(from,to)` sort (edgeIndex.ts §comment);
streamed edges arrive in ascending-`to`, NOT `(from,to)`, order, so that trick is invalid here. Use
an **order-independent, generation-stamped** index (leave `edgeIndex.ts` and its tests untouched):

```ts
export interface IncrementalEdgeIndex {
  bucketSize: number;
  /** Per-row-bucket edge-array indices, push-order (ascending by construction). */
  buckets: number[][];
  /** Backing edge store the assembler appends to; queries read from it. */
  edges: GraphEdge[];
  insert(edge: GraphEdge): void;               // append + push into floor(from/size)..floor(to/size)
  edgesInRange(firstRow: number, lastRow: number): GraphEdge[]; // overlap [first,last]
}
export function createIncrementalEdgeIndex(bucketSize?: number): IncrementalEdgeIndex; // default 256
```

- `insert`: append to `edges`; grow `buckets` if `to` crosses the last bucket; push the new index
  into every bucket `floor(from/size)..=floor(to/size)`.
- `edgesInRange`: union the buckets covering `[firstRow,lastRow]`, dedupe with a
  **generation-stamped `Int32Array seen`** (bump a `gen` counter per query; mark `seen[idx]=gen`;
  skip if already `gen`); final filter `e.from <= lastRow && e.to >= firstRow`. O(visible + k),
  independent of arrival order. Grow `seen` when `edges.length` outgrows it.

### 4.2 Stream assembler (`src/graph/streamAssembler.ts`, NEW)

Folds chunks into the SAME `GraphLayout` shape the renderer already consumes, so ALL downstream
consumers (drawGraph, search match-rings, tagOidMap, reveal, CommitPanel) work unchanged once
assembly reaches them, and degrade gracefully mid-stream.

```ts
export interface GraphStream {
  layout: GraphLayout;                 // grows in place; identity bumped per applied batch (§4.3)
  edgeIndex: IncrementalEdgeIndex;
  oidToRow: Map<string, number>;       // shared reveal/search index, built as rows arrive
  total: number | null;                // Meta.total (spacer sizing)
  loadedRows: number;
  done: boolean;
  truncated: boolean;
  version: number;                     // increments on every applied chunk (repaint trigger)
  apply(chunk: GraphChunk): void;
}
export function createGraphStream(): GraphStream;
```

`apply` semantics:
- `meta`: set `total`, stash `headOid` (resolve `layout.headIndex` when it appears in `oidToRow`).
- `batch`: assert `startRow === layout.nodes.length` (defensive; mismatch => throw/log-drop). For
  each `StreamNode` push `{...node, parents: []}` and set `oidToRow[node.id] = absRow`. For each
  `StreamEdge`: `edgeIndex.insert({from,to,lane})`; set `layout.nodes[from].parents[ord] = to`
  (sparse until all a node's parents' rows arrive — matches `CommitPanel`'s `parents[i] !==
  undefined` guard; on a COMPLETE walk it becomes dense and equals `compute_graph`'s array).
  Update `laneCount = max(laneCount, laneCountSoFar)`; `loadedRows`; bump `version`.
- `done`: set final `laneCount`, `headIndex`, `truncated`, `done=true`; bump `version`.

Assembled invariant: after `done` on a complete (non-truncated) walk, `layout` is deep-equal to
`ipc.getGraph`'s output (nodes incl. ordered `parents`, edges as a set, `laneCount`, `headIndex`,
`truncated`). This is the frontend half of the stability guarantee (§7 tests).

### 4.3 `GraphCanvas.tsx` — two additive optional props

```ts
// add to GraphCanvasProps:
/** Streamed path: the incremental edge index owned by the assembler. When present it REPLACES
 *  the internal `buildEdgeIndex(layout)` memo (which would be O(n) per batch). Absent => one-shot
 *  path, unchanged. */
edgeIndex?: IncrementalEdgeIndex;
/** Streamed path: total row count for the scroll extent while rows are still arriving. Absent =>
 *  spacer uses layout.nodes.length (one-shot / grow-as-you-go). */
totalRows?: number;
```

- Edge lookups: `const edges = props.edgeIndex ? props.edgeIndex.edgesInRange(firstRow, lastRow)
  : edgesInRange(layout, memoIndex, firstRow, lastRow);`. Build the `buildEdgeIndex` memo only when
  `edgeIndex` is absent.
- Spacer height: `(Math.max(layout.nodes.length, totalRows ?? 0) + wipOffset) * rowHeight + 8`.
- Repaint on new data: RepoWorkspace passes a `layout` whose object identity bumps per batch (a new
  wrapper around the same growing arrays, keyed off `version`), so the existing `useEffect([...,
  layout])` synchronous repaint fires per batch. Only visible rows are drawn (virtualized) — appends
  are cheap; no O(n) per batch.

### 4.4 `RepoWorkspace.tsx::refetchGraph` — stream instead of one-shot

Keep the `graphReqId` last-wins guard; it now gates chunk application (generation = the crux of
cancellation, §6):

```
refetchGraph():
    id = ++graphReqId.current
    prevSelectedId = current selected node id (as today)
    stream = createGraphStream(); setGraphLoading(true); firstBatchPainted = false
    try:
      await ipc.streamGraph(repoId, (chunk) => {
        if (id !== graphReqId.current) return           # stale stream (repo switch / re-stream)
        stream.apply(chunk)
        setGraph(wrap(stream.layout, stream.version))    # identity bump -> repaint
        setGraphEdgeIndex(stream.edgeIndex); setGraphTotal(stream.total)
        # progressive selection remap: as soon as prevSelectedId is in oidToRow, select it
        if (prevSelectedId && !remapped && stream.oidToRow.has(prevSelectedId))
            setSelectedIndex(stream.oidToRow.get(prevSelectedId)); remapped = true
        if (chunk.kind === 'batch' && !firstBatchPainted) firstBatchPainted = true
      })
      if (id !== graphReqId.current) return
      setGraphError(null)
      if (prevSelectedId && !remapped) setSelectedIndex(null)   # gone after full stream
      else if (!prevSelectedId) setSelectedIndex(null)
    catch e: if (id === graphReqId.current) setGraphError(errorMessage(e))
    finally: if (id === graphReqId.current) setGraphLoading(false)
```

Refetches keep showing the previous layout until the first batch of the new stream lands (no
flicker): only replace `graph` state on the first applied chunk of the new generation.

---

## 5. Mock parity (P65c)

`streamGraph` must chunk the SAME layout `getGraph` serves so the harness renders identically.

```ts
// src/ipc/mock/handlers/diff.ts (or graphStream.ts)
async streamGraph(repoId: string, onChunk: (c: GraphChunk) => void): Promise<void> {
  const layout = resolveLayout(requireRepo(repoId));   // full layout, as getGraph
  const total = layout.nodes.length;
  onChunk({ kind: 'meta', total, headOid: layout.headIndex !== null
    ? layout.nodes[layout.headIndex].id : null });
  // Slice into STREAM_FIRST_BATCH then STREAM_BATCH runs; for each run [s,e):
  //   nodes = layout.nodes[s..e] mapped to StreamNode (drop parents);
  //   edges = layout.edges with to in [s,e), each given `ord` = index of `to` in
  //           layout.nodes[from].parents  (parents are known in the full mock layout).
  // await delay(~30ms) between batches so the harness shows progressive paint + exercises the
  // assembler's multi-batch path. Bump a per-repo generation and STOP emitting if superseded.
  onChunk({ kind: 'done', totalRows: total, laneCount: layout.laneCount,
            headIndex: layout.headIndex, truncated: layout.truncated });
}
```

- `getGraph` mock stays (retained command; still feeds `searchCommits`' `resolveLayout`).
- The 20k fixture (`?fixture=20k`) flows through `streamGraph` unchanged → proves the streamed
  renderer + scroll target in-browser.
- Mock must respect supersede: track a per-repo stream generation; a newer `streamGraph` (or repo
  switch) makes the older loop stop before `done` (so the harness models cancellation, §6).

---

## 6. Back-pressure & cancellation

- **Producer flow control:** none needed. Batches are large (event count is tiny: 200k/4096 ≈ 49),
  the walk is CPU-bound inside `spawn_blocking`, and the frontend appends cheaply + coalesces rapid
  batches into one rAF repaint. We do NOT throttle the producer; we cancel it.
- **Fast scroll:** the eager stream is almost always ahead of the cursor. If scroll outruns the
  frontier, rows below `loadedRows` paint as background (blank) until the next batch lands — no
  stall, no IPC round-trip. A faint "loading history…" tail affordance is optional Polish.
- **Repo switch / re-stream (supersede):** the frontend `graphReqId` generation guard drops every
  chunk from a superseded stream (they never touch the assembler). The backend walk of the
  superseded stream stops as soon as its `emit` returns false — which happens when the frontend
  drops the `Channel` (component unmount / `close_repo`), because `on_chunk.send(...).is_ok()`
  becomes false. A superseded stream whose channel is still referenced (same tab, immediate
  re-stream) runs to completion in the background and its batches are discarded by the guard —
  correct, just some wasted CPU (bounded by the cap).
- **Escalation (flagged, +0 commands):** if profiling shows superseded walks waste too much CPU on
  huge repos, add a per-repo `Arc<AtomicBool>` cancel registry in `AppState`: `stream_graph`
  supersedes (trips) any prior flag for `repo_id` and `close_repo` trips it; the walk checks it each
  batch. This is additive and needs NO new command. See OQ1 (does Tauri v2 reliably error
  `Channel::send` on frontend drop? — the `is_ok()`-break depends on it).

---

## 7. Acceptance criteria (AI-gate vs USER CHECKPOINT)

### AI-gate (orchestrator verifies)
1. **Lane-color stability across page boundaries (the key test).** Rust unit test in graph.rs
   `#[cfg(test)]`: on each M2 §2.5 fixture (E1–E6) AND a mid-size generated fixture, run
   `stream_graph_core` capturing all chunks with SEVERAL batch sizes (`STREAM_FIRST_BATCH`/`BATCH`
   forced to `1, 2, 3, 7, 512`), assemble each chunk sequence into a `GraphLayout`, and assert every
   assembled layout is byte-identical to the others AND to `compute_graph` (nodes incl. ordered
   `parents`, sorted edges, `lane_count`, `head_index`, `truncated`). Varying the batch size proves
   batch boundaries never move a lane/color. A separate truncated case asserts equality on
   lanes/edges/`lane_count` (parents may differ only by truncation compaction — document).
2. **Frontend assembler equivalence.** Vitest: feed a fixture chunk-split at arbitrary boundaries to
   `streamAssembler`; assert the reconstructed `GraphLayout` deep-equals the un-chunked fixture
   (lanes, ordered parents, edges as a set, laneCount, headIndex). Plus an `incrementalEdgeIndex`
   test: incremental inserts + `edgesInRange` match a from-sorted brute-force oracle.
3. **First-paint latency on 200k+.** Extend the M2d generator (`FixtureSpec`/`generate_fixture` in
   `src-tauri/src/fixture.rs`) with a `main_len: 200_000` spec (git2 tree/commit objects only — NO
   CLI, NO `git commit` loops; < 60 s). `#[ignore]` release gate test `stream_first_batch_under_ms`:
   time from `stream_graph_core` start to the FIRST `Batch` emitted; assert < **150 ms** (justify:
   first batch = 512 walked rows over the P52 commit-graph file). Also assert total stream (to
   `Done`) completes and `truncated == false` at 200k.
4. **No jank at 20k+ scroll.** Harness: `VITE_MOCK_IPC=1`, `?fixture=20k`, open a mock repo, run
   `await window.__bonsai.scrollSweep(10000)`; PASS = `maxWindow5Avg <= 33 && over100 <= 3` (M2d
   §5.5 definition) — the streamed path must match the one-shot number.
5. **Mock renders paged data.** Harness screenshot of the streamed default + 20k fixtures: correct
   lanes/edges/pills, progressive fill visible, scrollbar reaches the last row.
6. `cargo test`, `cargo clippy -- -D warnings`, `pnpm build`, `vitest` green after every
   sub-increment.

### USER CHECKPOINT (never self-declared)
- Open a REAL 200k+ commit repo via `pnpm tauri dev`: first screenful paints effectively instantly;
  scrolling the loaded region feels smooth; scrolling into still-loading regions fills without
  jank; switching repos mid-stream is instant (no stale rows, no lag from the abandoned walk).

---

## 8. IPC surface delta

- Commands: **+1** — `stream_graph` (a `tauri::ipc::Channel<GraphChunk>` command). `get_graph` is
  RETAINED (small-repo convenience, unit/integration tests, mock `searchCommits` reuse).
- Events: +0.
- Channels: +1 conceptual (`stream_graph` carries `Channel<GraphChunk>`; same mechanism as
  `clone_repo`/`history_index_build`).
- **Recount at implementation** against `src-tauri/src/lib.rs` `generate_handler!`: current absolute
  command count is 147 → 148 after P65. (Escalation OQ1's AtomicBool registry, if taken, adds 0
  commands.)

---

## 9. Open questions (flag to orchestrator)

- **OQ1 — Channel-drop detection.** The recommended cancellation leans on `on_chunk.send(...)`
  returning `Err` once the frontend drops the `Channel` (unmount / `close_repo`). Verify Tauri v2
  actually errors dropped-channel sends on all platforms. If NOT reliable, adopt the flagged
  per-repo `Arc<AtomicBool>` cancel registry in `AppState` (supersede-on-new-stream + trip-on-close;
  walk checks each batch). Recommendation: ship the `is_ok()`-break; add the registry only if a
  large-repo profile shows wasted CPU. **Needs a decision only if OQ1 proves unreliable.**
- **OQ2 — Cheap total for the spacer.** Recommend `Meta.total = None` (grow-as-you-go scroll extent)
  for v1 unless the P52 commit-graph file exposes a trivially-cheap reachable count. A full
  pre-count walk is not worth doubling the work. Confirm whether a stable scrollbar-from-the-start
  is a hard requirement (then we pay the count).
- **OQ3 — Streaming cap / memory.** `STREAM_MAX_COMMITS = 1_000_000` ⇒ ~200 MB in the frontend at
  the cap (full arrays held for search/reveal/edge-index). Fine for the 200k target; heavy at 1M.
  Options: (a) keep 1M + `truncated`; (b) lower the cap; (c) the OQ4 bounded/resume variant.
  Recommend (a) for v1.
- **OQ4 — Bounded / resume-on-scroll (deferred).** A truly windowed model (stop after N screens,
  resume near the frontier) would cap memory for 1M+ repos but reintroduces a server-side
  parked-walk session with its lane state — the very complexity §0 avoids. Recommend deferring past
  P65; revisit only if 1M-commit memory becomes a real user problem.
- **OQ5 — `laneCountSoFar` reflow.** The graph-area width tracks the monotonic running-max lane
  count, so `summaryStartX` can only shift RIGHT as new lanes appear mid-stream (typically settles
  within the first batch, since tips establish most active lanes). Acceptable. Alternative: reserve
  the 24-lane-clamp width up front (no reflow, wastes width on narrow repos). Recommend running-max.
- **OQ6 — Consumer unification (optional).** `CommitPanel`/`onSelectParent` currently jump via
  `node.parents[ordinal]`; the assembler fills those from edge `ord`, so no change is required.
  A cleaner long-term option is to resolve parent jumps via the shared `oidToRow` +
  `CommitDetails.parents` oids and drop `StreamEdge.ord` entirely — but that touches two consumers
  and weakens the "assembled == compute_graph" equivalence test. Recommend keeping `ord` for P65.
```
