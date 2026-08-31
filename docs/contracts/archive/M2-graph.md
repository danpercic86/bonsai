# M2 — Commit Graph: Implementation Contract

Status: authoritative for M2. Implementer: senior-dev, in four fresh-context passes
(M2a/M2b/M2c/M2d — each section below is self-contained; read §1 Wire Types + your section).
Builds on `docs/contracts/M0-scaffold.md` (IPC conventions, AppError, state, spawn_blocking
pattern), `docs/contracts/M1-status.md` (command pattern in `commands.rs`, mock-IPC rule),
`docs/contracts/ui-reference.md` §4–§6 (graph metrics, lane palette, ref pills — canonical
visual values; this contract repeats the load-bearing numbers).

Locked product requirements (CLAUDE.md — do not relitigate):
- Walk seeded from ALL local branches + remote-tracking branches + tags (+ HEAD if detached).
- Ordering: topological, then commit date (git2 `Sort::TOPOLOGICAL | Sort::TIME`).
- Ref pills: local branches, remotes as `origin/name`, tags, HEAD; detached HEAD = HEAD pill on
  the checked-out commit.
- Lane colors deterministic per lane (`lane % 10` into the ui-reference §5 palette), stable while
  scrolling — guaranteed by computing lanes once in Rust for the whole history.
- 20k+ commits without jank; layout < 500 ms for 20k commits.
- WIP/uncommitted row: Polish, NOT M2. No selection-driven detail panel yet (that is M4); M2 only
  lays the selection groundwork (§4.6).

Architecture invariants (enforced in review):
- Rust owns ALL layout math. The frontend receives a finished `GraphLayout` and only rasterizes it.
- Single compact command response (decision §2.7); no per-commit round-trips.
- git2 calls run under `tauri::async_runtime::spawn_blocking` (same pattern as `get_status`).
- Canvas rendering, virtualized to visible rows, all repaints inside rAF.

---

## 1. Wire types (shared by all sub-increments — implement exactly)

### 1.1 Rust (`src-tauri/src/graph.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RefKind {
    LocalBranch,
    RemoteBranch, // name already includes remote: "origin/main"
    Tag,
    Head,         // ONLY emitted when HEAD is detached
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefLabel {
    pub name: String,   // shorthand: "main", "origin/main", "v1.0", "HEAD"
    pub kind: RefKind,
    /// true on the local branch HEAD points at (attached), or on the Head label (detached).
    pub is_head: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,          // full 40-char hex oid (M4 needs it for commit diffs)
    pub lane: u32,
    /// Indices into GraphLayout.nodes (parents always appear at a HIGHER index —
    /// topological order guarantees it). First entry = first parent.
    /// Truncated walks (§2.8) silently drop parents that were not emitted.
    pub parents: Vec<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<RefLabel>, // omitted from JSON when empty (saves ~200 KB at 20k nodes)
    pub summary: String,     // first line of message, char-safe cap at 120 chars
    pub author: String,      // author name only (no email)
    pub ts: i64,             // author commit time, seconds since epoch (UTC)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub from: u32, // child node index == child ROW (see invariant below)
    pub to: u32,   // parent node index == parent row; always to > from
    pub lane: u32, // lane of the vertical run between the rows (§1.3 render rule)
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphLayout {
    pub nodes: Vec<GraphNode>,
    /// Sorted ascending by (from, to) — required for the frontend edge index (§4.4).
    pub edges: Vec<GraphEdge>,
    pub lane_count: u32,          // max lanes ever active; drives graph-area width
    pub head_index: Option<u32>,  // node index of the HEAD commit (None if unborn/no HEAD)
    pub truncated: bool,          // walk stopped at MAX_COMMITS (§2.8)
}
```

**Row invariant (load-bearing, saves 20k×`"row":n` bytes):** `nodes` is in walk order and
**row == node index**. There is no `row` field on the wire. `GraphEdge.from/to` double as row
numbers. Every consumer (renderer, tests) relies on this.

**Wire-size decision:** full 40-char ids + parents-as-indices + refs-skip-empty + short field
names (`ts`, `summary`, `author`). Estimated ~4 MB JSON / ~31k nodes; parsed once per layout.
Accepted for v1; M2d measures it (§5.4) — if serialize+parse ever exceeds 250 ms total, the
fallback is an additive channel-based `stream_graph` command; do NOT build it preemptively.

### 1.2 TypeScript mirrors (`src/ipc/types.ts` — add verbatim)

```ts
export type RefKind = 'localBranch' | 'remoteBranch' | 'tag' | 'head';

export interface RefLabel {
  name: string;
  kind: RefKind;
  isHead: boolean;
}

export interface GraphNode {
  id: string;
  lane: number;
  parents: number[];
  refs?: RefLabel[];      // absent when empty
  summary: string;
  author: string;
  ts: number;             // seconds since epoch
}

export interface GraphEdge {
  from: number;           // child row/index
  to: number;             // parent row/index; to > from
  lane: number;
}

export interface GraphLayout {
  nodes: GraphNode[];
  edges: GraphEdge[];
  laneCount: number;
  headIndex: number | null;
  truncated: boolean;
}
```

`IpcApi` gains:

```ts
/** Full graph layout for the open repo. Rejects AppError ('noRepo' when nothing open). */
getGraph(): Promise<GraphLayout>;
```

### 1.3 Edge geometry contract (Rust produces, frontend interprets — normative)

An edge occupies exactly one vertical-run lane (`edge.lane`) between its endpoint rows. The
layout algorithm (§2.4) guarantees `edge.lane ∈ {fromLane, toLane}` for simple cases, but the
renderer MUST handle the general case (criss-cross can produce lane ≠ both). Rendering rule,
with `cx(l) = gutter + min(l, MAX_RENDER_LANES-1) * 16 + 8` and `cy(r) = r * 28 + 14` (CSS px,
before scroll translation; `MAX_RENDER_LANES = 24`, §6.2):

```
fromLane = nodes[e.from].lane; toLane = nodes[e.to].lane
if e.to == e.from + 1:
    one segment cy(e.from) -> cy(e.to):
        bezier if fromLane != toLane else straight line
else:
    top:    cy(e.from)   -> cy(e.from+1): bezier fromLane -> e.lane (line if equal)
    middle: straight vertical in e.lane from cy(e.from+1) to cy(e.to-1)
    bottom: cy(e.to-1)   -> cy(e.to):     bezier e.lane -> toLane (line if equal)
```

Bezier between (x1,y1) and (x2,y2) one row apart: control points `(x1, y1+14)` and
`(x2, y2-14)` — vertical tangents both ends (ui-reference §4). Stroke: 2px, round caps, color
`palette[e.lane % 10]`.

This is O(visible)-queryable: `from`/`to` are rows, edges are sorted by `from`, and the frontend
builds a bucket interval index once per layout (§4.4). No per-row edge segments on the wire —
logical commit→parent edges with precomputed run lane is the smallest representation that still
allows exact culling.

---

## 2. M2a — Layout engine (pure Rust, no UI)

New file `src-tauri/src/graph.rs` (+ `pub mod graph;` in `lib.rs`). Types from §1.1. No Tauri
types in this module except none — the command lives in `commands.rs`.

### 2.1 Public surface

```rust
pub const MAX_COMMITS: usize = 100_000;

/// Blocking. Opens the repo at `workdir` (Repository::open_ext with NO_SEARCH, same as
/// read_status) and computes the full layout. Unborn HEAD / zero refs -> empty layout
/// (all vecs empty, lane_count 0, head_index None, truncated false), NOT an error.
/// Bare repos: works if given a git dir, but callers never pass one (open_repo gates bare);
/// no special handling required.
pub fn compute_graph(workdir: &std::path::Path) -> Result<GraphLayout, AppError>;
```

Internal decomposition (recommended, for testability):

```rust
fn collect_refs(repo: &git2::Repository)
    -> Result<(std::collections::HashMap<git2::Oid, Vec<RefLabel>>, Vec<git2::Oid>, Option<git2::Oid>), AppError>;
    // -> (labels per commit oid, deterministic tip list for the walk, head oid)
fn layout_walk(repo: &git2::Repository, tips: &[git2::Oid],
    mut refs: HashMap<git2::Oid, Vec<RefLabel>>, head: Option<git2::Oid>)
    -> Result<GraphLayout, AppError>;
```

### 2.2 Ref collection + seeding (pseudocode)

```
collect_refs(repo):
    labels = HashMap<Oid, Vec<RefLabel>>       # insertion order preserved per commit by sort below
    tips_ordered = Vec<Oid>                     # deterministic push order
    head_oid = None; head_branch = None; detached = false

    if not repo.head_unborn():
        head = repo.head(); head_oid = head.target commit oid
        detached = repo.head_detached()
        if not detached: head_branch = head.shorthand()      # e.g. "main"

    # 1. local branches, sorted by name ascending (byte-wise)
    for (branch, _) in repo.branches(Some(Local)) sorted by shorthand:
        oid = branch tip commit oid (skip if unresolvable)
        labels[oid].push(RefLabel { name, kind: LocalBranch,
                                    is_head: !detached && Some(name) == head_branch })
        tips_ordered.push(oid)

    # 2. remote-tracking branches, sorted by shorthand ("origin/main"), skip "*/HEAD"
    for (branch, _) in repo.branches(Some(Remote)) sorted:
        if shorthand ends with "/HEAD": continue
        labels[oid].push(RefLabel { name: shorthand, kind: RemoteBranch, is_head: false })
        tips_ordered.push(oid)

    # 3. tags, sorted by name; peel annotated tags to the target commit; skip tags that
    #    do not peel to a commit (tag->blob/tree)
    for ref in repo.references_glob("refs/tags/*") sorted by shorthand:
        obj = ref.peel(ObjectType::Commit) or skip
        labels[obj.id()].push(RefLabel { name: shorthand, kind: Tag, is_head: false })
        tips_ordered.push(obj.id())

    # 4. HEAD last: detached gets its own label; attached is already covered by (1)
    if head_oid is Some(oid):
        if detached: labels[oid].push(RefLabel { name: "HEAD", kind: Head, is_head: true })
        tips_ordered.push(oid)

    # sort each commit's labels for pill order: detached Head first, then LocalBranch
    # (is_head first, then name asc), then RemoteBranch name asc, then Tag name asc
    for v in labels.values_mut(): v.sort_by(pill_order)

    return (labels, dedupe_preserving_first(tips_ordered), head_oid)
```

Walk setup: `revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?` then `push(tip)` for each
deduped tip **in the deterministic order above**. git2/libgit2 semantics: TOPOLOGICAL guarantees
every commit appears before all of its parents; TIME orders by commit time within that
constraint — this is exactly the locked "topological, then commit date". Equal timestamps fall
back to push order, which the sorted-tip rule makes deterministic (same repo state → identical
layout → identical colors; this is the lane-color stability rule).

### 2.3 Lane-assignment state model

- `lanes: Vec<Option<Oid>>` — the active-lanes vector. `lanes[i] == Some(p)` means "an edge is
  running down lane i, waiting for commit p to appear". Multiple lanes may wait for the same
  oid (two children of one parent → their lines run in parallel and converge at the parent).
  The vector only grows (`lane_count = lanes.len()` at the end == max ever active); freed slots
  become `None` and are reused by `first_free`.
- `pending: HashMap<Oid, Vec<PendingEdge>>` with `PendingEdge { from: u32, lane: u32 }` —
  edges created at child time, finalized (given `to`) when the parent row is emitted.
- `index_of: HashMap<Oid, u32>` — filled as rows are emitted; used at the end to resolve
  `parents` oids → indices.
- `first_free(lanes) -> usize`: lowest index with `None`, else push and return new index.
  Scanning always starts at 0 — simple and deterministic (see §8 for the tradeoff note).

### 2.4 Lane assignment + edge routing (normative pseudocode)

```
layout_walk(repo, tips, refs, head_oid):
    lanes = []; pending = {}; index_of = {}
    nodes = []; edges = []; raw_parents = []   # Vec<Vec<Oid>> parallel to nodes
    truncated = false

    for (row, oid) in revwalk.enumerate():
        if row >= MAX_COMMITS: truncated = true; break
        commit = repo.find_commit(oid)

        # 1. which lanes were waiting for this commit?
        reserved = [i for i in 0..lanes.len() if lanes[i] == Some(oid)]   # ascending

        # 2. pick this commit's lane
        if reserved.is_empty():
            lane = first_free(lanes)          # tip / new branch head / orphan root
        else:
            lane = reserved[0]                # leftmost waiting lane wins (deterministic)
            for i in reserved[1..]: lanes[i] = None    # converging lines free their lanes

        # 3. finalize every edge that was waiting for this commit
        for pe in pending.remove(oid).unwrap_or_default():
            edges.push(GraphEdge { from: pe.from, to: row, lane: pe.lane })

        # 4. route edges to parents / update reservations
        parents = commit.parent_ids()          # order preserved: first parent first
        if parents.is_empty():
            lanes[lane] = None                                  # root: line ends here
        else:
            p0 = parents[0]
            lanes[lane] = Some(p0)                              # first parent inherits the lane
            pending[p0].push(PendingEdge { from: row, lane })   # even if p0 is ALSO reserved
                                                                # elsewhere (convergence at p0)
            for pk in parents[1..]:                             # merge parents (octopus-safe)
                if let Some(j) = lowest i where lanes[i] == Some(pk):
                    pending[pk].push(PendingEdge { from: row, lane: j })   # join existing line
                else:
                    j = first_free(lanes); lanes[j] = Some(pk)
                    pending[pk].push(PendingEdge { from: row, lane: j })

        # 5. emit the node
        index_of[oid] = row
        nodes.push(GraphNode { id: oid.to_string(), lane: lane as u32,
                               parents: [] /* resolved below */,
                               refs: refs.remove(oid).unwrap_or_default(),
                               summary: first_line_capped(commit.summary(), 120),
                               author: commit.author().name lossy,
                               ts: commit.author().when().seconds() })
        raw_parents.push(parents)

    # 6. resolve parent oids -> indices; drop parents outside the emitted set (truncation only —
    #    a complete walk emits every ancestor). pending edges never finalized are dropped too.
    for (i, ps) in raw_parents.enumerate():
        nodes[i].parents = ps.filter_map(|p| index_of.get(p)).collect()

    edges.sort_unstable_by_key(|e| (e.from, e.to))     # REQUIRED wire order (§1.1)
    head_index = head_oid.and_then(|h| index_of.get(h))
    lane_count = lanes.len() as u32
    return GraphLayout { nodes, edges, lane_count, head_index, truncated }
```

Cases covered by construction: octopus merges (step 4 loops over all extra parents), orphan
roots (step 2 empty-reserved → free lane; step 4 root → lane freed for reuse), criss-cross
(multi-reservation + leftmost-wins), parent already on a lane (merge edge joins lane `j`;
first-parent edge still runs down the child's lane and converges at the parent via
leftmost-wins). Non-UTF-8 summaries/authors: `from_utf8_lossy`, never error.

### 2.5 Worked examples (ASCII; these are the unit-test fixtures)

Notation: `rN name(lane)`; edges as `(from,to,lane)`. Rows top→bottom in walk order.

**E1 — linear chain (3 commits, branch `main` on tip):**

```
r0 C2(0) ⌂main      lanes: [C1]
r1 C1(0)            lanes: [C0]
r2 C0(0)            lanes: [None]  (root)
edges: (0,1,0) (1,2,0)      lane_count 1
```

**E2 — fork + merge:**
history: `M{C3,F2}  F2{F1}  C3{C2}  F1{C1}  C2{C1}  C1{C0}  C0{}` (walk order as listed).

```
r0 M (0)   p0 C3 keeps lane0; p1 F2 -> new lane1
r1 F2(1)
r2 C3(0)
r3 F1(1)
r4 C2(0)   p0 C1: C1 already reserved at lane1 -> ALSO reserve lane0 (parallel lines)
r5 C1(0)   reserved {0,1} -> lane0, lane1 freed
r6 C0(0)   root
edges: (0,1,1) (0,2,0) (1,3,1) (2,4,0) (3,5,1) (4,5,0) (5,6,0)   lane_count 2
```
`(0,1,1)`: adjacent rows, single bezier lane0→lane1 (the merge curve). `(3,5,1)`: vertical in
lane1 to r4, bottom bezier into lane0 at r5 (the fork-point convergence).

**E3 — two parallel branches, no merge** (`main`: C3→C2→C1; `topic`: T2→T1→C1; walk order
C3,T2,C2,T1,C1 by timestamps):

```
r0 C3(0)  r1 T2(1)  r2 C2(0)  r3 T1(1)  r4 C1(0) [reserved {0,1} -> 0; lane1 freed]
edges: (0,2,0) (1,3,1) (2,4,0) (3,4,1)      lane_count 2
```

**E4 — criss-cross:** `A2{A1,B1}  B2{B1,A1}  A1{R}  B1{R}  R{}` walk order as listed.

```
r0 A2(0)  p0 A1 -> lane0; p1 B1 -> new lane1
r1 B2(2)  no reservation -> lanes 0,1 busy -> new lane2; p0 B1 also reserves lane2;
          p1 A1 joins existing lane0 -> edge (1,2,0)
r2 A1(0)
r3 B1(1)  reserved {1,2} -> lane1, lane2 freed
r4 R (0)  reserved {0,1} -> lane0, lane1 freed
edges: (0,1,1)* (0,2,0) (1,2,0) (1,3,2) (2,4,0) (3,4,1)      lane_count 3
```
\* `(0,1,1)` sorted first: A2→B1 edge — careful: A2's p1 edge targets B1 at r3, so it is
actually `(0,3,1)`; full sorted list: `(0,2,0) (0,3,1) (1,2,0) (1,3,2) (2,4,0) (3,4,1)`.
Edge `(1,2,0)` (B2→A1): fromLane 2, e.lane 0 == toLane 0, adjacent rows → single bezier 2→0.
Edge `(1,3,2)` (B2→B1): fromLane 2 == e.lane 2, toLane 1 → vertical then bottom bezier — the
general `lane ∉ {fromLane==toLane}` shapes both appear here; assert exact tuples.

**E5 — octopus (3 parents):** `M{A,B,C}` then A,B,C linear to a common root R.
`M` at lane0; A keeps lane0, B → lane1, C → lane2; at R all converge to lane0, lanes 1,2 freed.
Assert `M.parents.len() == 3` and three edges out of r0 with lanes 0,1,2.

**E6 — two orphan roots** (main chain + disconnected `pages`: P1→P0, older timestamps):

```
r0..rk  main chain on lane0 ... rk root -> lane0 freed
rk+1 P1(0)   # reuses lane0 (free-scan from 0) — same color as main; accepted for v1
rk+2 P0(0)
```
Assert P1/P0 get lane 0 and there is NO edge between the two components.

### 2.6 Unit tests (in `graph.rs` `#[cfg(test)]`; fixtures built with git2 in `tempfile::TempDir`)

Fixture helper (extend/reuse the M0/M1 pattern): init repo, set local `user.name`/`user.email`,
commits created from in-memory trees (`repo.treebuilder` + one blob) with **explicit,
strictly-increasing `git2::Time` values** on every `Signature` (walk-order determinism depends
on distinct timestamps). Branch/tag helpers: `repo.branch(name, &commit, true)`,
`repo.tag_lightweight` / `repo.tag` (annotated), manual reference
`repo.reference("refs/remotes/origin/main", oid, ...)` for remote-tracking fixtures,
`repo.set_head_detached(oid)`.

Required tests — each asserts the EXACT node order, lanes, sorted edge tuples, and
`lane_count` from §2.5:
1. `linear_chain` (E1) — also asserts `head_index == Some(0)`, `refs` on r0 =
   `[{main, localBranch, isHead: true}]`, `parents == [[1],[2],[]]`.
2. `fork_merge` (E2).
3. `parallel_branches` (E3).
4. `criss_cross` (E4).
5. `octopus_merge` (E5) — build the 3-parent commit with `repo.commit(... &[&a,&b,&c])`.
6. `two_orphan_roots` (E6) — second root via a commit with no parents on an orphan branch
   (`repo.commit(Some("refs/heads/pages"), ..., &[])`).
7. `ref_pills_stacking` — one commit that is simultaneously: local branch tip (HEAD attached),
   remote-tracking `origin/main`, lightweight tag `v1.0`, annotated tag `v1.1-notes`. Assert
   label vector order per §2.2 pill_order and `is_head` only on the local branch.
8. `detached_head` — detach onto a mid-history commit; assert a `{name:"HEAD", kind:Head,
   is_head:true}` label on that row and `head_index` pointing at it; no Head label anywhere else.
9. `unborn_repo` — `Repository::init` only → empty layout, `Ok`, not `Err`.
10. `determinism` — build E4, call `compute_graph` twice, assert full struct equality.
11. `annotated_tag_to_blob_skipped` — a tag object pointing at a blob is ignored, no panic.

### 2.7 Command + IPC (part of the M2a pass)

`commands.rs` (mirror `get_status` exactly — state lock → clone path → drop lock →
`spawn_blocking` → join-error to `AppError::Other`):

```rust
#[tauri::command]
pub async fn get_graph(state: tauri::State<'_, AppState>) -> Result<GraphLayout, AppError>;
// inner: spawn_blocking(move || compute_graph(&path)); NoRepo when nothing open
```

Register in `lib.rs` `generate_handler![...]`. **Decision: single command response, no channel,
no version envelope** — ~4 MB once per refresh is well inside a single invoke; evolution path is
an additive `stream_graph` channel command if M2d measurements demand it (§1.1). Cap:
`MAX_COMMITS = 100_000`; beyond it the walk stops, `truncated: true`, dangling parents/edges
dropped (§2.4 step 6). UI treatment of `truncated` (banner) is Polish; M2 just carries the flag.

`src/ipc/tauri.ts`: `getGraph: () => invoke<GraphLayout>('get_graph')`.
`src/ipc/mock.ts`: MUST be extended in the same change (CLAUDE.md invariant) — return the §3.5
fixture after `delay(150)`; the M2a pass may ship a minimal placeholder fixture if M2b's full one
is not built yet, but `getGraph` must exist and typecheck.

### 2.8 M2a acceptance gate

`cargo test` green (all §2.6 tests), `cargo clippy -- -D warnings` clean, `pnpm build` green
(types + mock stub compile). No UI change required yet.

---

## 3. M2b — Canvas rendering of a static layout

New frontend module `src/graph/`. No virtualization yet (the mock fixture is 30 rows; draw the
whole layout on a canvas sized `rows*28` CSS px). Everything here is reused verbatim by M2c.

### 3.1 Files

```
src/graph/metrics.ts      # METRICS constant (single source for all geometry numbers)
src/graph/colors.ts       # resolveTheme(): reads CSS custom properties ONCE per mount/theme
src/graph/draw.ts         # pure draw functions — no React imports, unit-testable
src/graph/GraphCanvas.tsx # React component: canvas lifecycle, events, HiDPI (M2c extends)
src/ipc/fixtures/graph.ts # buildMockGraph(): GraphLayout (30-row fixture, §3.5)
```

### 3.2 Metrics + theme (exact values, from ui-reference §4–§6)

```ts
// metrics.ts
export const METRICS = {
  rowHeight: 28, laneWidth: 16, gutter: 12,
  dotRadius: 4, dotRingWidth: 2, edgeWidth: 2,
  textGap: 12,                 // gap between graph area and pills/summary column
  authorColWidth: 120, dateColWidth: 72, colGap: 12,
  pillHeight: 18, pillPadX: 8, pillGap: 4, pillMaxWidth: 160, pillFont: '600 11px',
  summaryFont: '400 13px', metaFont: '400 12px',
  maxRenderLanes: 24,          // §6.2 lane clamp
} as const;
// Font family strings appended at draw time:
// UI: '"Segoe UI Variable", "Segoe UI", system-ui, -apple-system, sans-serif'

// colors.ts
export interface Theme {
  laneColors: string[];   // 10 entries, --lane-0..--lane-9 (add these custom props to
                          // styles.css :root using the ui-reference §5 hex values)
  bg0: string; bg2: string; border: string;
  text1: string; text2: string; text3: string;
  selection: string; accent: string; accentText: string; danger: string; warning: string;
}
export function resolveTheme(el: HTMLElement): Theme; // one getComputedStyle pass; callers
                                                      // cache the result — NEVER per frame
```

Add to `styles.css`: `--lane-0: #4f8cff; --lane-1: #f2994a; --lane-2: #9b6dff; --lane-3: #43b97f;
--lane-4: #e5534b; --lane-5: #3ec6c0; --lane-6: #e8c341; --lane-7: #f26d9c; --lane-8: #7a86ff;
--lane-9: #8fbf4d;` (same in both themes, per ui-reference §5).

### 3.3 draw.ts surface (pure; ctx already DPR-transformed, coordinates in CSS px)

```ts
export interface Viewport {
  firstRow: number;   // inclusive; M2b: 0
  lastRow: number;    // inclusive; M2b: nodes.length - 1
  scrollTop: number;  // CSS px; M2b: 0
  width: number;      // CSS px of the canvas
  height: number;
}
export interface Interaction { hoverRow: number | null; selectedIndex: number | null; }

export function laneX(lane: number): number;   // gutter + min(lane, maxRenderLanes-1)*16 + 8
export function rowY(row: number, scrollTop: number): number;  // row*28 + 14 - scrollTop
export function rowAtPoint(yCss: number, scrollTop: number): number; // floor((y+scrollTop)/28)

export function drawGraph(
  ctx: CanvasRenderingContext2D, layout: GraphLayout,
  visibleEdges: readonly GraphEdge[],           // M2b: layout.edges; M2c: culled set
  vp: Viewport, theme: Theme, ix: Interaction,
): void;
```

Draw order inside `drawGraph` (one clear + four passes):
1. Clear `(0,0,width,height)` with `theme.bg0`.
2. Row backgrounds: `ix.hoverRow` → `theme.bg2`; `ix.selectedIndex` → `theme.selection`
   (selection wins over hover). Full-width rect, 28px tall.
3. Edges (under dots), per §1.3 rule. One `beginPath`/`stroke` per edge (grouping by color is a
   M2c-permitted optimization, not required). Long-edge clamp: when `e.to - e.from > 1`, clamp
   the middle vertical segment's y-range to `[-56, height+56]` — never emit path coordinates
   thousands of px off-canvas.
4. Dots: for each visible node — 2px ring of `theme.bg0` behind (circle r = 4+2 in bg0), then
   filled circle r=4 in `laneColors[lane % 10]`. `headIndex` row: extra 1.5px `text1` ring at
   r≈6.5. Selected row: r=5 dot + 1.5px `accent` ring (groundwork; selection set is wired in
   §4.6/M4).
5. Text row content, x cursor starts at `graphAreaWidth = gutter + min(laneCount, 24)*16 + textGap`:
   a. Ref pills (§3.4), left to right, advancing the cursor (`pillGap` apart), then +8px.
   b. Summary: `summaryFont`, `text1`, vertically centered (textBaseline 'middle', y = rowY),
      truncated with ellipsis to the space remaining before the author column.
   c. Author: right-aligned column of width 120 at `width - dateColWidth - colGap*2`,
      `metaFont`, `text3`, ellipsis-truncated.
   d. Date: right-aligned in the last 72px, `metaFont`, `text3`, relative format ("now",
      "5m", "3h", "4d", "2mo", "1y" — helper `relativeDate(ts: number, now: number)` in draw.ts,
      pure, unit-testable).

Ellipsis truncation helper: `truncateToWidth(ctx, text, maxPx): string` — binary search over
`measureText`, with a `Map<font+text, width>` cache module-level (cap 4096 entries, drop-all on
overflow). Used by summary, author, pills.

### 3.4 Ref pill rendering (canvas; ui-reference §6)

Pill: rounded rect (radius 9 = pillHeight/2), height 18, vertically centered in the row; text
`600 11px` UI font, `pillPadX` 8 each side; label truncated to `pillMaxWidth - 2*pillPadX`.
Variant styles (laneColor = `laneColors[node.lane % 10]`):

| kind / state | fill | text | border 1px | label prefix |
|---|---|---|---|---|
| localBranch, isHead | laneColor solid | `accentText` | none | `⌂ ` |
| localBranch | laneColor @18% alpha | laneColor | laneColor | — |
| remoteBranch | `bg2` | `text2` | `border` | — |
| tag | `#d4a72c` @18% | `#d4a72c` | `#d4a72c` | `# ` |
| head (detached) | `danger` solid | `#ffffff` | none | — (label is "HEAD") |

18% alpha: precompute `hexToRgba(hex, 0.18)` in colors.ts, once per theme. Pills render in the
order Rust provided (already sorted, §2.2). Overflow (M2c hardens, implement now): stop when the
next pill would push past `pillBudget = 40%` of the space between graph area and author column;
render a final compact chip `+n` (bg2/text2 style) counting the hidden refs.

### 3.5 Mock fixture (`src/ipc/fixtures/graph.ts`)

`buildMockGraph(): GraphLayout` — 30 rows, `laneCount: 3`, `headIndex: 0`, `truncated: false`.
Exact structure (ids: any 40-hex strings, e.g. `'a'.repeat(39) + n`; ts descending from a recent
base, 1h steps; authors alternate "Ada Lovelace" / "Grace Hopper"):

Rows 0–7 (hand-computed lanes/edges — copy exactly; this is the E-series geometry in one graph):

| row | name | lane | parents | refs |
|---|---|---|---|---|
| 0 | T "Merge feat and exp" | 0 | [3, 1, 2] (octopus) | main (localBranch, isHead), origin/main (remoteBranch), v1.0 (tag) |
| 1 | F2 "feat: polish" | 1 | [4] | feat (localBranch) |
| 2 | X1 "experiment" | 2 | [5] | exp (localBranch) |
| 3 | C4 "core work 4" | 0 | [5] | — |
| 4 | F1 "feat: start" | 1 | [6] | — |
| 5 | C3 "core work 3" | 0 | [6] | — |
| 6 | C2 "core work 2" | 0 | [7] | — |
| 7 | C1 "core work 1" | 0 | [8] | v0.9 (tag) |

Edges for rows 0–7: `(0,1,1) (0,2,2) (0,3,0) (1,4,1) (2,5,2) (3,5,0) (4,6,1) (5,6,0) (6,7,0)
(7,8,0)`.

Rows 8–26: linear chain `L19..L1` ("chore: history n"), lane 0, `parents [row+1]`, edges
`(r, r+1, 0)` for r in 8..=26 pointing down the chain; row 26's parent is row 27.
Row 27: `R0 "initial commit"`, lane 0, `parents []` (main root).
Row 28: `P1 "pages: update"`, lane 0, `parents [29]`, refs `gh-pages (localBranch)`; edge
`(28,29,0)`. Row 29: `P0 "pages: init"`, lane 0, `parents []` (second root, disconnected).

`mock.ts`: `getGraph()` → `delay(150)` → `buildMockGraph()` (20k variant added in M2d, §5.5).

### 3.6 GraphCanvas component (M2b scope)

```ts
export interface GraphCanvasProps {
  layout: GraphLayout;
  selectedIndex: number | null;
  onSelect(index: number | null): void;   // clicking a row toggles; empty area below rows -> null
}
export function GraphCanvas(props: GraphCanvasProps): JSX.Element;
```

M2b behavior: canvas sized `container width × rows*28` (fine at 30 rows), DPR-naive is NOT
allowed even here — apply the basic `canvas.width = cssW * dpr; ctx.setTransform(dpr,0,0,dpr,0,0)`
(M2c adds ResizeObserver + dpr-change handling). Mouse move → hoverRow (repaint via rAF only when
the row changed); mouse leave → clear hover; click → `onSelect(rowAtPoint(...))` if within
`nodes.length`, else `onSelect(null)`. Cursor: `default`.

App wiring (this pass): center pane renders `<GraphCanvas/>` when a graph is loaded — see §6.1
for the full App.tsx contract (fetch + refetch is part of M2b so the harness shows real mock
data end to end).

### 3.7 M2b acceptance gate

`pnpm build` green. Browser harness (`VITE_MOCK_IPC=1 pnpm dev`): screenshot shows — 3 colored
lanes; octopus merge at the top with two curved merge edges (lane1, lane2) and curved
convergences at rows 5/6; dots with bg rings; HEAD ring on row 0; pills: solid `⌂ main`,
muted `origin/main`, yellow `# v1.0`, outline `feat`/`exp`/`gh-pages`, `# v0.9`; summary/author/
date columns aligned; two disconnected roots with no edge between rows 27 and 28. Hover
highlights a row; clicking logs/sets selection without errors. No `@tauri-apps/*` executed.

---

## 4. M2c — Virtualization, scrolling, ref pills hardening, HiDPI

Extends `src/graph/GraphCanvas.tsx` + adds `src/graph/edgeIndex.ts` and
`src/graph/frameStats.ts`. `draw.ts` unchanged except consuming culled edges.

### 4.1 Scroll model (decision: fixed viewport canvas + overlay scroller)

```html
<div class="graph-pane-inner" style="position:relative; height:100%">
  <canvas style="position:absolute; inset:0" />                 <!-- output only -->
  <div class="graph-scroll" style="position:absolute; inset:0; overflow-y:auto">
    <div class="graph-spacer" style="height: {nodes.length * 28 + 8}px" />
  </div>
</div>
```

- The scroller sits ON TOP and owns all input (wheel, drag-scrollbar, mousemove, click); the
  canvas never receives pointer events. Native scrollbar for free; no synthetic wheel math.
- Scroll handler ONLY records `scrollTopRef.current = el.scrollTop` and calls
  `schedulePaint()`; `schedulePaint` sets a dirty flag and requests one rAF if none pending.
  ALL painting happens inside the rAF callback. Zero repaints outside rAF (invariant).
- Hit-testing from scroller events: `row = rowAtPoint(e.clientY - rect.top, scrollTop)`.
- Alternative considered and rejected: one tall canvas translated via CSS — breaks at
  20k*28 = 560k px (canvas size limits) and repaints everything; sticky-canvas-inside-scroller
  works but complicates event coordinates. Overlay model is the contract.

### 4.2 Visible range

```
firstRow = max(0, floor(scrollTop / 28) - OVERSCAN)
lastRow  = min(n - 1, ceil((scrollTop + viewportH) / 28) + OVERSCAN)
OVERSCAN = 4 rows
```
`drawGraph` receives this Viewport; node/text passes iterate `firstRow..=lastRow` only.

### 4.3 HiDPI + resize

- `ResizeObserver` on `.graph-pane-inner`: on size change, set `canvas.style.width/height` to
  CSS size, `canvas.width/height = round(css * dpr)`, then `ctx.setTransform(dpr,0,0,dpr,0,0)`,
  then `schedulePaint()`.
- DPR change (monitor move / zoom): listen via
  `matchMedia(`(resolution: ${devicePixelRatio}dppx)`)` `change` (re-arm after each fire);
  re-run the resize logic.
- All METRICS are CSS px; only the backing store is scaled. Reuse the theme cache; re-resolve
  theme only when `data-theme` changes (fine to leave dark-only until Polish).

### 4.4 Edge culling (`src/graph/edgeIndex.ts`)

Built ONCE per layout object (memoized on the layout reference):

```ts
export interface EdgeIndex { bucketSize: number; buckets: number[][]; } // edge indices per bucket
export function buildEdgeIndex(layout: GraphLayout, bucketSize = 256): EdgeIndex;
// edge e occupies buckets floor(e.from/256) ..= floor(e.to/256)
export function edgesInRange(layout: GraphLayout, ix: EdgeIndex,
                             firstRow: number, lastRow: number): GraphEdge[];
// union of buckets covering [firstRow, lastRow]; dedupe with a monotonic "lastEmitted" check
// (bucket lists are ascending because layout.edges is sorted by (from,to) — §1.1);
// final filter: e.from <= lastRow && e.to >= firstRow
```

O(visible + k) per frame; build is O(edges × spanBuckets) once (~31k edges, worst edge spans
~80 buckets — sub-millisecond). This TS index is view plumbing over Rust-computed geometry, not
layout math (flagged in §8).

### 4.5 Ref pill hardening

Pill budget + `+n` overflow chip per §3.4 (implement fully if M2b stubbed it). Pills must render
correctly at every scroll position (they are part of the per-frame row pass — no DOM pills).

### 4.6 Selection/hover groundwork

`GraphCanvas` keeps `hoverRow` internal (state in a ref + rAF repaint on change — never
`setState` per mousemove). `selectedIndex`/`onSelect` stay controlled props; App stores
`selectedIndex` in `useState` (right panel does nothing with it until M4). Keyboard: none in M2.

### 4.7 Frame-timing instrumentation (`src/graph/frameStats.ts`)

Enabled when `import.meta.env.DEV || import.meta.env.VITE_MOCK_IPC === '1'`; compiled out of
release paths behind that check.

```ts
export interface FrameStats { frames: number; avgMs: number; maxMs: number;
                              over33: number; over100: number; maxWindow5Avg: number; }
export function createFrameRecorder(): { record(durMs: number): void;
                                         flushSummary(): FrameStats; }
```

- The paint rAF wraps its body in `performance.now()` deltas AND records inter-frame gaps while
  scroll activity is ongoing (last scroll event < 200 ms ago).
- Every 120 recorded frames, log: `[bonsai] frames n=120 avg=2.1ms max=14.0ms >33ms=0`.
- Mock mode only: expose `window.__bonsai = { scrollSweep(durationMs = 10000): Promise<FrameStats> }`
  — animates `scroller.scrollTop` from 0 → max → 0 via rAF over `durationMs`, records every
  frame delta, then logs one line `[bonsai] scroll-test {"frames":N,"avgMs":..,"maxMs":..,
  "over33":..,"over100":..,"maxWindow5Avg":..}` and resolves. `maxWindow5Avg` = max over all
  windows of 5 consecutive frames of the window's average duration.

### 4.8 M2c acceptance gate

`pnpm build` green. Harness: scrolling the 30-row fixture is correct at top/middle/bottom
(screenshots); pills/dots/edges stable while scrolling (no smearing, no drift — lane colors
identical at any scroll position by construction); resize + browser zoom (dpr change) keeps the
canvas crisp; console shows `[bonsai] frames` lines while scrolling and none while idle.

---

## 5. M2d — Perf gate: fixture generator + criterion + harness scroll test

### 5.1 Fixture generator (Rust)

Location decision: `src-tauri/src/fixture.rs`, an unconditional `#[doc(hidden)] pub mod`
(registered in `lib.rs`) so both the criterion bench and the `#[ignore]`d gate test reach it
without feature juggling (flagged in §8). git2 only — no CLI, no working-dir churn: blobs via
`repo.blob()`, trees via `repo.treebuilder(None)` (one file `n.txt` containing the commit
counter), commits via `repo.commit(None, ...)` with explicit parents; refs written at the end.
Signature times: `1_600_000_000 + counter * 60`, strictly increasing. Target: full generation
< 60 s (expect single-digit seconds).

```rust
pub struct FixtureSpec {
    pub main_len: usize,        // 20_000
    pub branch_every: usize,    // 50   — fork a feature branch at every 50th main commit
    pub branch_len: usize,      // 20   — feature commits per branch
    pub merge_after: usize,     // 30   — merge lands 30 main-commits after the fork point
    pub long_branches: usize,   // 3    — long-lived, never merged
    pub long_branch_len: usize, // 1_000
    pub tag_every: usize,       // 200  — lightweight tag v{i} on every 200th main commit
    pub keep_branch_ref_every: usize, // 10 — every 10th merged feature keeps refs/heads/feat-{k}
}
impl Default for FixtureSpec { /* values above */ }

/// Creates (or errors if path non-empty) a repo at `path` with the synthetic history.
/// Total commits = 20_000 + 400*20 + 3*1_000 = 31_000. refs: main, 3 long-{j}, ~40 feat-{k},
/// 100 tags. Merge commits on main have parents [prev_main, feature_tip].
pub fn generate_fixture(path: &std::path::Path, spec: &FixtureSpec) -> Result<(), AppError>;

/// Returns the shared cached fixture, generating on first use:
/// <target-dir>/graph-fixture/repo  (locate via env!("CARGO_MANIFEST_DIR") + "../target" or
/// std::env::var("CARGO_TARGET_DIR") fallback). Prints generation time to stderr.
pub fn ensure_default_fixture() -> Result<std::path::PathBuf, AppError>;
```

Long branches fork from main commits 100, 200, 300; refs `long-0..2`. Sanity test (regular
`#[test]`, small spec e.g. `main_len: 200`): commit count, ref count, and `compute_graph`
succeeds with `truncated == false`.

### 5.2 Criterion benchmark (`src-tauri/benches/graph_layout.rs`)

`Cargo.toml`: `[dev-dependencies] criterion = "0.5"` (or current), plus
`[[bench]] name = "graph_layout" harness = false`.

- Setup: `ensure_default_fixture()`.
- `c.bench_function("compute_graph_31k", |b| b.iter(|| compute_graph(&path)))` with
  `sample_size(10)`.
- Second bench `serialize_31k`: precompute layout once, bench `serde_json::to_string(&layout)`;
  also `eprintln!` the byte size once. This is the §1.1 wire-size measurement.

### 5.3 Automated gate test (`src-tauri/tests/perf_gate.rs`)

Criterion reports but does not assert; the gate is a test:

```rust
#[test]
#[ignore] // run explicitly: cargo test --release --test perf_gate -- --ignored --nocapture
fn layout_31k_under_500ms() {
    // ensure_default_fixture(); warm-up run; then 3 timed compute_graph runs via Instant;
    // assert the MINIMUM of the 3 < 500 ms; print all three timings.
}
#[test]
#[ignore]
fn serialize_31k_report() {
    // time serde_json::to_string, print ms + bytes; assert < 250 ms (soft ceiling from §1.1).
}
```

Gate is release-mode only (debug git2 is far slower — orchestrator runs with `--release`).

### 5.4 Harness 20k layout (decision: procedural TS generator, no JSON file)

`src/ipc/fixtures/graph20k.ts`: `generateLayout20k(): GraphLayout` — 20 000 rows generated
procedurally in TS. This intentionally does NOT reproduce the Rust algorithm (mock fixtures are
canned data, not a second layout engine); it emits a pattern whose lanes are known by
construction and geometrically valid per §1.3:

- Lane 0: main spine, every row not otherwise assigned; `parents [next main row]`.
- Lane 2: long-lived branch — a node every 40 rows starting row 20, chained to each other by
  lane-2 edges, forked from main near the bottom (single bottom-curve edge), ref `long-0` pill
  on its first node.
- Lane 1: per 50-row block b: fork node at row `b*50+35` … merge target consistency: rows
  `b*50+5` is a merge commit on lane 0 with `parents [main_next, b*50+8]`; rows `b*50+8`,
  `b*50+16`, `b*50+24` are lane-1 feature nodes chained down, the last with a bottom-curve edge
  into the main row `b*50+30`. Edges exactly per the §1.3 shape rules; `edges` array kept
  sorted by `(from, to)`.
- Pills: `⌂ main` + `origin/main` on row 0; `# v{i}` every 200 rows; `feat-{b}` on every 10th
  block's fork row. `laneCount: 3`, `headIndex: 0`, `truncated: false`. Summaries
  `"commit #{n}"`, two alternating authors, ts descending 60 s steps.

`mock.ts` `getGraph()`: if `new URLSearchParams(window.location.search).get('fixture') === '20k'`
→ `generateLayout20k()` else `buildMockGraph()`; both after `delay(150)`.

### 5.5 Harness scroll test procedure (orchestrator-executable)

1. `VITE_MOCK_IPC=1 pnpm dev`, open `http://localhost:1420/?fixture=20k`, open a mock repo.
2. In devtools console: `await window.__bonsai.scrollSweep(10000)`.
3. Read the `[bonsai] scroll-test {...}` line.

**Pass criterion (normative):** `maxWindow5Avg <= 33` (no sustained window of 5 consecutive
frames averaging > 33 ms) AND `over100 <= 3` (isolated GC spikes tolerated) over the ~10 s sweep.

### 5.6 M2d acceptance gate

Fixture generation < 60 s (printed); `layout_31k_under_500ms` passes in `--release`;
`serialize_31k_report` passes and its numbers are recorded in TODO.md by the orchestrator;
criterion runs produce reports; harness scroll test passes per §5.5; screenshot of the 20k
fixture mid-scroll looks correct (3 lanes, periodic merges, pills).

---

## 6. Cross-cutting — App wiring + layout (implemented across M2b/M2c as noted)

### 6.1 App.tsx contract (M2b pass)

- New state: `graph: GraphLayout | null`, `graphError: string | null`, `graphLoading: boolean`,
  `selectedIndex: number | null` — plus a request-id last-wins guard identical to
  `refetchStatus` (`graphReqId` ref). `refetchGraph()` mirrors `refetchStatus()`.
- Fetch triggers (exactly the status triggers — graph and status refetch together):
  successful non-bare open → `refetchGraph()`; `repo-changed` event → refetch; window focus →
  refetch; refresh button → refetch after `openRepo`. Full relayout per change is accepted for
  v1 (the 300 ms backend debounce absorbs storms). `clearGraph()` on failed/bare open, and reset
  `selectedIndex` to null on every new layout.
- Center pane rendering: unborn HEAD → keep "No commits yet"; `graph === null` (loading first
  layout) → nothing over the canvas area (ui-reference §8: no spinners on the graph);
  `graphError` → inline error banner at the top of the pane; else
  `<GraphCanvas layout={graph} selectedIndex={selectedIndex} onSelect={setSelectedIndex} />`.
  Refetches keep showing the previous layout until the new one arrives (no flicker).
- `console.debug('[bonsai] repo-changed → refetch status+graph')` replaces the M1 log line.

### 6.2 Graph pane geometry

- Graph area width: `12 + min(laneCount, 24) * 16 + 12`. Lanes ≥ 24 clamp their x to lane 23's
  center (`laneX` in §3.3) — degenerate ultra-wide histories overlap at the right edge of the
  gutter instead of pushing text off-screen. No horizontal scroll in v1.
- Text columns per §3.3 pass 5. Pane min width 480px (ui-reference §1) — columns shrink
  summary first (it takes the flex remainder), author/date are fixed.

### 6.3 IPC surface after M2 (complete list)

- Commands: `open_repo(path)`, `get_status()`, `get_graph()`.
- Events: `repo-changed` (unchanged).
- Channels: none (decision §2.7).
- Mock (`src/ipc/mock.ts`) implements all of the above; `getGraph` has two fixtures (30-row
  default, 20k behind `?fixture=20k`).

---

## 7. Acceptance criteria — overall M2 (restated from CLAUDE.md)

AI gate (orchestrator verifies):
- M2a: lane/edge unit tests pass on all §2.6 fixture histories; clippy clean.
- M2b/M2c: harness screenshots show correct lanes, curved fork/merge edges, dots, ref pills;
  virtualized scrolling correct; HiDPI crisp.
- M2d: scripted git2 generator builds the 31k-commit fixture (< 60 s, no CLI commits);
  criterion + gate test show `compute_graph` < 500 ms in release; harness scroll test over the
  20k layout logs frame timings with no sustained frames > 33 ms (§5.5 definition).
- `cargo test`, `cargo clippy -- -D warnings`, `pnpm build` green after every sub-increment.

USER CHECKPOINT (never self-declared): scrolling the 20k+ fixture repo (opened for real via
`pnpm tauri dev`) feels smooth; the graph of a real repository looks correct (lanes, merges,
pills, HEAD).

## 8. Ambiguities resolved here (flag to orchestrator if disagreed)

1. **Row field dropped from the wire** — `row == nodes index` invariant instead. Saves bytes;
   every consumer documented to rely on it.
2. **Parents as node indices, edges as index pairs, full 40-char ids kept** — ids are needed by
   M4 (commit diff) and are the only string cost that matters; ~4 MB / 31k nodes accepted,
   measured by `serialize_31k_report` with a 250 ms soft ceiling that triggers the (additive)
   channel-streaming fallback only if breached.
3. **Single command response, no version envelope** — a future `stream_graph` channel command is
   purely additive; an envelope today is speculative. `truncated`/`head_index` live on
   `GraphLayout` itself.
4. **Logical commit→parent edges with one precomputed run lane** (not per-row segments) + the
   §1.3 three-segment render rule. Smallest exact representation; culling stays O(visible) via
   the TS bucket index (§4.4) — building that index in TS is judged view plumbing, not layout
   math (all lanes/rows come from Rust). If the reviewer disagrees, the index moves to Rust as a
   `buckets: Vec<Vec<u32>>` field — additive change.
5. **`first_free` scans lanes from index 0** (tips, merge parents alike). Simplest deterministic
   rule; can place a merge branch left of its merge commit, which GitKraken avoids by preferring
   rightward lanes. Accepted for v1 — revisit in Polish if graphs look tangled.
6. **Orphan roots may reuse a freed lane (same color as the ended line)** — deterministic and
   standard; a "skip one lane between components" nicety is Polish.
7. **Edges may pass under unrelated dots** (no detour routing) — sanctioned by ui-reference §4's
   bg-ring-behind-dots rule.
8. **MAX_COMMITS = 100_000**, silent truncation (flag on the wire, banner is Polish).
9. **Fixture generator ships as `#[doc(hidden)] pub mod fixture` in the lib crate** — needed by
   both benches and integration tests; feature-gating it costs more ceremony than the ~150 lines
   it adds to the (dev-only anyway) binary. 
10. **Perf gate as `#[ignore]`d release-mode test** asserting < 500 ms (criterion measures,
    tests assert).
11. **20k harness fixture generated procedurally in TS** (not a checked-in JSON, not a mirror of
    the Rust algorithm) — zero repo bloat, lanes valid by construction; it exists only to
    exercise the renderer.
12. **Scroll model: fixed viewport canvas + transparent overlay scroller with spacer** — native
    scrollbar, one repaint per rAF, avoids the 560k-px canvas-height limit a translated tall
    canvas would hit.
13. **"Sustained > 33 ms" defined as**: any 5-consecutive-frame window averaging > 33 ms fails;
    up to 3 isolated frames > 100 ms (GC) tolerated per 10 s sweep.
14. **Graph refetches on every `repo-changed`/focus/refresh alongside status** — full relayout
    per change accepted for v1 (< 500 ms worst case, debounced upstream).
15. **Lane render clamp at 24 lanes** (x positions clamp, no horizontal scroll) — pathological
    histories stay usable; real repos rarely exceed ~15 active lanes.
