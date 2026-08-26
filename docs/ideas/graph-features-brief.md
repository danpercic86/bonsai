# Bonsai — Commit-Graph Feature Briefs (for implementation)

**Audience:** an implementer (Fable) working directly in this repo.
**Written:** 2026-08-26. Grounded in the code as it stands on `main` / `feat/bonsai-graph-theme`.
**Status:** proposals — none of these are specced/approved yet. Each brief is self-contained.

These five briefs came out of a demand-vs-fit review of what users ask for in Git-client commit
graphs (GitKraken, GitLens, Git Graph, Sublime Merge, lazygit, GitUp) crossed with Bonsai's
architecture. Ranked by value; #1 is the highest-ROI.

---

## 0. Ground rules every brief assumes (read first)

Bonsai's non-negotiable invariants (from `CLAUDE.md` — enforce them):

- **Rust owns ALL Git logic AND the commit-graph layout math.** React only renders. Any
  layout/topology decision (which lanes, which edges, first-parent vs all-parents, folding) lives
  in Rust, not TypeScript.
- **IPC carries compact precomputed data.** Commands = request/response; events = small signals;
  **channels = streaming** large/incremental data (the graph already streams — see below).
- **git2 is blocking → wrap heavy calls in `spawn_blocking`.**
- **The canvas is virtualized to visible rows** and must stay smooth over 20k+ commits.
- **Lane colors are deterministic per lane and must stay stable while scrolling.** Several of these
  features re-walk the graph; when they do, lane assignment must remain stable/predictable or users
  perceive it as "the graph jumped."
- **File-size discipline:** ~500 lines/file soft limit. New UI/fixtures = new files. `draw.ts` is
  currently at 500 lines — new canvas painting goes in a sibling module (the Bonsai theme did this
  with `drawBonsai.ts`).
- **Browser harness is how you verify UI without the native window:** `pnpm dev` +
  `VITE_MOCK_IPC=1` (or the `bonsai-mock` launch config). Keep `src/ipc/mock.ts` /
  `src/ipc/mock/*` compiling and updated with every IPC change, and add fixtures for new data.
- **Gate before done:** `pnpm gate` (tsc, eslint `--max-warnings 40`, file-size ratchet, vitest,
  `cargo nextest`, clippy, playwright e2e). Note: run `cargo` steps without racing clippy/test on a
  shared target dir (the gate splits target dirs; a manual concurrent run can fail spuriously).
- **Frame-timing / motion feel and native scroll are USER CHECKPOINTs** — the headless harness
  pauses `requestAnimationFrame`, so you cannot self-verify animation smoothness; present evidence
  and ask the user to confirm in `pnpm tauri dev`.

### The graph pipeline as it exists today (shared by all briefs)

**Rust build:**
- `crates/bonsai-core/src/graph.rs` — `compute_graph()` (one-shot) → `collect_seed()` (seeds tips
  from all local branches, remote-tracking branches, tags, HEAD, stashes; `collect_refs()` at
  ~lines 290–390) → `seeded_revwalk()` (`git2::Sort::TOPOLOGICAL | TIME`, ~line 188) →
  `layout_walk()` (~lines 414–473) drives `LaneWalker.step()` per commit.
- `crates/bonsai-core/src/graph/lane.rs` — `LaneWalker` (lanes `Vec<Option<Oid>>`, `pending` edge
  map, `index_of`, `hidden`, `last_parents`). `step()` (~lines 48–160) picks a lane (reserved lane
  if a child already routed to it, else first-free) and routes an edge to **every** parent. **All
  parent oids are available via `commit.parent_ids()` before edges are routed** — this is why
  first-parent / folding is cheap to add.
- Cap: `MAX_COMMITS` = 100k; `truncated` flag when hit.

**Commands / IPC:**
- `src-tauri/src/commands/status.rs` — `get_graph(repo_id)` (one-shot) and **`stream_graph(repo_id,
  on_chunk: Channel<GraphChunk>)`** (primary path). **Neither takes any filter/paging param today —
  `repo_id` is the only input.** Internal `stream_graph_core_with` (in `.../graph/stream.rs`) has
  batch/max params but they're hardcoded.
- Types: `crates/bonsai-core/src/graph.rs` structs mirrored byte-for-byte (camelCase) in
  `src/ipc/types/graph.ts`.
  - `GraphNode { id, lane, parents: number[], refs?, summary, author, ts, committerTs }`
  - `GraphEdge { from, to, lane }` (always `to > from`; sorted by `(from,to)`)
  - `GraphLayout { nodes, edges, laneCount, headIndex, truncated }`
  - Stream: `GraphChunk = meta | batch | done`; `StreamEdge` carries `ord` (0 = first parent).

**Frontend render/scroll:**
- `src/graph/GraphCanvas.tsx` — container: streaming assembly, virtualization, scroll, selection,
  theme resolve, painting orchestration. Native scrollbar; right inset computed at ~lines 359–363.
- `src/graph/viewport.ts` — `visibleRowRange()`, `scrollRowIntoView()`, `visibleRowCount()`.
- `src/graph/draw.ts` (+ `drawBonsai.ts`) — canvas painters. Match rings painted from `matchRows`
  at ~draw.ts:456.
- `src/graph/geometry.ts` — `laneX()`, `rowY()`, `avatarColor(name)` (HSL by name hash, ~line 90),
  `avatarHit()`.
- Reveal/select: `src/components/repoWorkspace/useReadOverlays.ts` — `revealCommitByOid(oid)`
  (~lines 93–109) finds the node index and sets `selectedIndex`; a `GraphCanvas` effect
  (~lines 586–602) scrolls it into view. **This is the shared "jump to a commit" mechanism.**
- Settings pattern: `src/components/settings/categories/AppearanceCategory.tsx` (SettingsRow +
  SettingsSegmented; `graphStyle`/`graphSeason` live here). There is also a **"Commit graph"**
  settings category (graph-specific prefs like date basis) — new graph toggles can live in either;
  prefer the Commit-graph category for topology toggles, Appearance for purely visual ones.
- Settings persistence: `src/hooks/useUiSettings.ts` + `src/ipc/mock/persistence.ts` (mock) +
  `src-tauri/src/settings/prefs.rs` + `src-tauri/src/commands/ui_settings.rs` +
  `src/settings/uiSettingsDefaults.json` + `src/settings/defaults.ts` (native + oracle). Follow the
  `graphStyle` precedent added in spec-002 for any new persisted pref.

**Recommended delivery for each brief:** use the repo's `/specify` → `/plan` → `/tasks` flow
(writes to `docs/specs/<NNN>-<slug>/`), or the milestone loop for the larger ones. Bring in
`ui-designer` before implementation for anything the user sees.

---

## 1. Declutter modes — first-parent toggle, merge-folding, branch solo/hide  ★ highest ROI

### Why
The single most-requested commit-graph capability across every client: users drown in tangled
histories. GitKraken "Hide merged commits," GitLens #1399, Git Graph #162/#387, Sublime Merge
"Commit Folding," lazygit fold/solo. Bonsai's Rust-owned layout is the ideal place to do this
*correctly* (server-side re-walk), which is Bonsai's structural advantage over clients that filter
in the view layer.

### Scope (three independently shippable sub-features; do them in this order)
1. **First-parent-only view** (cheapest, highest value): follow only each commit's first parent, so
   merged-in side branches collapse out of the mainline. A toggle.
2. **Branch solo / hide**: restrict the walk's seed tips to a chosen subset (solo one branch's
   ancestry; or hide selected branches/remotes/tags).
3. **Fold linear runs / fold merges** (hardest): collapse long no-branch stretches (and/or a
   merge's second-parent subtree) into a single expandable placeholder row.

### Rust / React split
- **All topology work is Rust.** React only gains: the toggle UI, passing the chosen options over
  IPC, and re-requesting + repainting the returned layout.

### Rust changes
- **New options struct** (in `crates/bonsai-core/src/graph.rs`), e.g.:
  ```rust
  #[derive(Debug, Clone, Default, serde::Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct GraphFilter {
      pub first_parent: bool,               // sub-feature 1
      pub seed_refs: Option<Vec<String>>,   // sub-feature 2: None = all (current behavior)
      pub fold_linear: bool,                // sub-feature 3 (optional, later)
  }
  ```
- **Thread it through** `compute_graph` / the stream core. Two touch points:
  - **Seeding** (`collect_seed`/`collect_refs`, ~graph.rs:290–390): when `seed_refs` is `Some`,
    push only those tips (still dedup, still include HEAD if in the set). Default `None` keeps
    today's "all refs" behavior byte-for-byte.
  - **First-parent** (`LaneWalker.step()` in `lane.rs`, the parent-enumeration around ~135–160):
    when `first_parent`, route an edge only to `commit.parent_ids().next()` (parent 0) and ignore
    the rest. The lane state machine is unchanged; only parent enumeration changes. Also set the
    revwalk to `Sort::TOPOLOGICAL | TIME` and call `revwalk.simplify_first_parent()` so the walk
    itself skips the side history (keeps the node set small — important for perf).
  - **Fold linear** (later): post-walk pass that coalesces maximal chains where each node has
    exactly one parent and one child and carries no refs, into a synthetic "folded" node with a
    `foldedCount`. Emit as a special node kind the painter can render as a "⋯ N commits" pill;
    expanding re-requests unfolded. Keep this OUT of the first increment.
- **Commands** (`src-tauri/src/commands/status.rs`): add an optional `filter: GraphFilter` param to
  `get_graph` and `stream_graph` (defaulted so existing callers/tests are unaffected). Keep
  `spawn_blocking` wrapping. Invalidate/parameterize the `graph_cache` by the filter (today it
  caches per repo; it must now cache per `(repo, filter)` or clear on filter change).

### IPC / types
- Add `GraphFilter` to `src/ipc/types/graph.ts` and pass it: `invoke('streamGraph', { repoId,
  filter, onChunk })`. Update `src/ipc/mock/*` to honor at least `firstParent` and `seedRefs`
  against the fixture layout (a simple fixture-side filter is fine for the harness).
- If sub-feature 3 ships, add a `folded?: number` field (or a `kind`) to `GraphNode`/`StreamNode`.

### Frontend
- **Toggles** in the "Commit graph" settings category (persisted like `graphStyle`), AND a quick
  inline control near the graph (a small toolbar button) since these are used interactively, not
  set-and-forget. First-parent is a boolean; solo/hide is a branch multi-select (reuse the sidebar
  branch list selection model).
- On filter change: re-run the stream, reset scroll to top (or preserve selection by oid via
  `revealCommitByOid` after reload), repaint. Show a subtle "filtered" indicator so users know why
  commits are missing.
- **Lane stability caveat (important):** a first-parent walk yields fewer lanes; that's expected.
  But make sure toggling back restores the identical full layout. Don't try to animate between the
  two — just re-stream.

### Gotchas / invariants
- Keep default (`GraphFilter::default()`) byte-for-byte identical to today's output — guard with a
  test that `get_graph(repo)` == `get_graph(repo, default)`.
- Solo/hide changes the seed, so `headIndex` and ref pills must still resolve correctly when HEAD's
  branch is hidden (decide: always keep HEAD, or show an empty-ish graph — recommend always keep
  HEAD's ancestry).
- Performance: `simplify_first_parent()` makes first-parent *faster* than full (smaller walk) — good.

### Acceptance criteria
1. First-parent toggle on → merges' side branches disappear; mainline is a single lane where the
   history is linear; toggling off restores the exact full graph.
2. Solo a branch → only that branch's ancestry (+ HEAD) is walked; ref pills for hidden branches
   are gone; performance is equal or better.
3. Default (no filter) output is unchanged vs today (regression test).
4. Filter choice persists across restart (if placed in settings) and re-streams correctly.
5. 20k-commit repo stays smooth under every mode (USER CHECKPOINT for feel).

### Effort
Medium (sub-features 1–2). Sub-feature 3 (folding) is Medium-large — treat as a separate milestone.

---

## 2. In-graph search: match distribution on the scroll rail  (mostly an EXTENSION)

### Current state (already built — do not rebuild)
- UI: `src/components/CommitSearchBar.tsx` (fields: all/message/author/path/content; live for cheap
  fields, submit for pickaxe `-S`/`-G`), state in `src/components/repoWorkspace/useCommitSearch.ts`.
- Rust: `src-tauri/src/commands/search.rs` → `crates/bonsai-core/src/git/search.rs` (`SearchQuery`
  with `field`, `regex`, `case_sensitive`, `max_results`, `scope_ref`; returns `SearchResults`).
- Canvas: matches already paint as outer **match rings** from `matchRows` (draw.ts ~456); `next()`/
  `prev()`/`goToMatch()` reuse `revealCommitByOid` to select+scroll.
- **Blame→graph jump already exists** (`BlameView.onRevealCommit` → `revealCommitByOid`), so that
  frequently-requested ask is effectively done.

### The gap worth building
A **match-distribution track**: a thin vertical rail along the scrollbar edge showing where in the
whole history the matches are (like a browser Find's scrollbar ticks), so users can see clustering
and click a tick to jump — without scrolling blindly. GitLens does this on its minimap; users
specifically liked matches distributed on the rail.

### Rust / React split
Frontend-only. All data already present: `matchRows` (indices) + total row count.

### Files
- New component/painter, e.g. `src/graph/MatchRail.tsx` (or extend the minimap in brief #3 — they
  share the rail geometry). Hang it on the right inset (`GraphCanvas.tsx` ~359–363).
- Map each match row → `y = row / totalRows * railHeight`; draw a tick in `--match-ring` color.
  Click → `scrollRowIntoView`/set scrollTop (see `viewport.ts`), or select via the row's oid.
- Show the current match tick highlighted (tie to `currentMatch` from `useCommitSearch`).

### Gotchas
- Coalesce ticks when many matches map to the same pixel (draw density, not 10k overlapping ticks).
- Only render while search is open (keep it out of the idle paint path).

### Acceptance criteria
1. With search open and N matches, the rail shows ticks at proportional positions; clicking a tick
   scrolls there and selects the match.
2. The current match (next/prev) is visually distinct on the rail.
3. Rail disappears when search closes; no cost when search is closed.

### Effort
Small–Medium (frontend only). Natural to build together with #3.

---

## 3. On-demand minimap / overview rail

### Why
Rising ask; GitLens rebuilt its minimap (density, refs, HEAD markers, jump). Key lesson from their
tracker (#5598): **always-on wastes vertical space — default to on-demand (show during search /
hover), toggleable.** Great for orienting in big histories.

### Rust / React split
Frontend-only. Uses the already-streamed `GraphLayout` (nodes, lanes, refs, headIndex). No Rust.

### Approach
- A thin vertical rail (share geometry with the match rail in #2) that renders a **downsampled**
  overview of the whole history:
  - lane density / branch structure as a compressed heat/column (bucket rows into `railHeight`
    pixels; per bucket, encode how many lanes are active or commit density).
  - ref markers (branch/tag/HEAD) as small pips at their row positions.
  - a **viewport thumb** showing the currently-visible window (from `visibleRowRange` in
    `viewport.ts`); drag the thumb or click the rail to jump (set `scrollTop`).
  - overlay the #2 match ticks when search is open.
- **On-demand policy:** hidden by default; reveal on hover near the right edge and while search is
  open; a settings toggle for "always show." Follow GitLens's default of auto/search-only.

### Files
- `src/graph/Minimap.tsx` (new) + a small painter (its own canvas or SVG). Read the assembled
  layout from `GraphCanvas` (lift the streamed nodes/refs, or expose via a ref/prop).
- Hook scroll sync to `GraphCanvas`'s scroller (`scrollRowIntoView` / direct `scrollTop`).

### Gotchas
- Downsample cheaply — precompute buckets once per layout (or per stream `done`), not per paint.
- Keep it virtualization-friendly: the minimap needs the *whole* row count and ref positions, which
  the stream already provides at `done`; don't force loading extra data.
- Respect the native scrollbar inset so the minimap and scrollbar don't overlap.

### Acceptance criteria
1. Minimap shows the full history compressed with visible branch density + ref pips + HEAD.
2. A viewport thumb reflects the visible window and updates on scroll; clicking/dragging jumps.
3. Hidden by default; appears on hover/search per policy; "always show" setting works and persists.
4. No measurable idle cost when hidden; no jank on a 20k repo (USER CHECKPOINT for feel).

### Effort
Medium (frontend only). Build with #2 (shared rail).

---

## 4. Author coloring + parent-highlight on hover  (cheap delight)

### Current state
- Avatars are **already author-colored**: `geometry.ts:avatarColor(name)` → deterministic HSL by
  name hash. So per-author identity color already exists on the node disc.
- `GraphNode.parents` (indices into `nodes`) is already on the wire — parent relationships are known
  client-side without any Rust change.

### The two additions
1. **"Color lanes by author" toggle** — an alternate coloring mode where lane/edge color derives
   from the commit author (via `avatarColor`'s hue) instead of the branch-lane palette. A view
   toggle; purely a painter branch.
2. **Parent-highlight on hover/selection** — when a commit is hovered or selected, emphasize its
   parent edges + parent nodes (and optionally dim the rest), so you can trace lineage at a glance.
   lazygit does a version of this and users like it.

### Rust / React split
Frontend-only for both. (Author name is already on `GraphNode`; parents already provided.)

### Files
- `src/graph/draw.ts` / `drawBonsai.ts` — add a coloring-mode branch (branch-lane vs author) and a
  **highlight pass**: given the hovered/selected row, look up `node.parents`, and stroke those
  edges + node rings in an emphasis color; optionally lower global alpha on non-highlighted paint.
- `src/graph/GraphCanvas.tsx` — track hovered row (mousemove → row via `rowY` inverse / existing hit
  logic; there's already `avatarHit`), pass it into the interaction object like `selectedIndex`.
  Repaint on hover change (on-demand, same as selection — do NOT add a continuous loop).
- Settings toggle (Appearance or Commit-graph category) for the author-color mode, persisted.

### Gotchas
- Hover repaint must stay on-demand (repaint only when hovered row changes) to protect idle CPU and
  the 20k budget — mirror how selection repaints, not a rAF loop.
- Author color mode must still meet contrast on both app themes and both graph styles (standard +
  Bonsai) — reuse the theme's contrast approach; don't hand-pick hues that fail on light.
- Parent-highlight should be cheap: only the hovered node's direct parents (1–2 edges), not a full
  ancestry traversal (that would be expensive and visually noisy — offer "ancestry" as a separate
  opt-in later if wanted).

### Acceptance criteria
1. Toggle "color by author" recolors lanes/edges by author hue; toggling back restores branch-lane
   colors; persists across restart.
2. Hovering/selecting a commit highlights its parent edges + parent nodes distinctly; moving away
   restores normal paint; no idle repaints.
3. Both coloring modes are legible in light+dark and in standard+Bonsai styles.

### Effort
Cheap–Medium (frontend only).

---

## 5. Replay / "story" mode — animated history playback  (fun, secondary view)

### Why
The recurring "delight" ask (Gource, Gitlogue Show HN, git-story). Not a daily-driver view, but a
memorable, shareable mode that plays the repo's history building over time. Pairs naturally with the
Bonsai theme (the tree "grows").

### Scope (keep it a self-contained mode, not a change to the working graph)
A playback overlay/mode that animates commits appearing over time — either by author-date or in
topo order — revealing nodes/edges progressively, with play/pause/scrub and a speed control. MVP:
reveal existing rows top-to-bottom over time on the existing canvas; nicer: a dedicated
"grow the tree" animation in the Bonsai style.

### Rust / React split
Frontend-only. All required data is already streamed: `nodes` (with `ts`/`committerTs`), `edges`,
`refs`. No Rust change. (Optional later: a Rust endpoint to bucket commits by day for a smooth time
axis, but not needed for MVP — sort client-side by `ts`.)

### Approach
- New mode component, e.g. `src/graph/ReplayMode.tsx`, that takes the assembled layout and animates
  a "reveal cutoff" (row count or timestamp) increasing over wall-clock time; paint only nodes/edges
  up to the cutoff (reuse `draw.ts` painters with a max-row clamp).
- Transport UI: play/pause, scrub bar (maps to timestamp/row), speed (1×/2×/…). Escape exits back to
  the normal graph at the same selection.
- Bonsai-flavored variant: reuse the blossom/leaf painters; new leaves "sprout" as commits appear
  (respect the settle/reveal animation patterns already in `revealFlash*` / `useSway`).

### Gotchas / invariants
- This is the ONE feature with real animation. It **must** honor `prefers-reduced-motion`: offer a
  static scrubber (no auto-play motion) fallback, mirroring the reduced-motion handling in
  `revealFlash.ts`.
- **Frame-timing is a USER CHECKPOINT** (headless harness pauses rAF) — you can verify the
  state-machine (cutoff advances, scrub maps correctly, exit restores state) in the harness, but the
  animation *feel* and smoothness on a big repo must be confirmed in `pnpm tauri dev`.
- Keep the animation loop bounded/exitable (like `useSway`): stop scheduling frames when paused, at
  the end, or on exit — no perpetual idle loop.
- Don't degrade the normal graph: replay is a separate mode; entering/leaving must not mutate the
  working layout or selection.

### Acceptance criteria
1. Entering replay animates commits appearing over time; play/pause/scrub/speed all work; exit
   returns to the normal graph with prior selection intact.
2. Under `prefers-reduced-motion`, no auto-motion — a manual scrubber only.
3. No rAF scheduled when paused/stopped/exited.
4. Works with both standard and Bonsai styles; Bonsai variant sprouts leaves/blossoms.

### Effort
Medium–Large (frontend only). Ship after #1–#4.

---

## Suggested sequencing

1. **#1 first-parent + solo/hide** — biggest win, exercises the Rust layout seam (the real
   differentiator). Fold-linear as a later follow-up.
2. **#2 + #3 together** — shared right-rail geometry (match ticks + minimap); mostly frontend.
3. **#4** — cheap delight, frontend only.
4. **#5** — the fun capstone, self-contained mode.

Each should go through `/specify` → `/plan` → `/tasks` (or the milestone loop for #1 and #5), with
`ui-designer` engaged before implementation, and finish on a green `pnpm gate` plus the relevant
USER CHECKPOINT for anything with motion or large-repo scroll feel.
