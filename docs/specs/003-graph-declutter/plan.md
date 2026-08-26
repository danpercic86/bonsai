# Plan: Graph Declutter Modes — First-Parent Toggle & Branch Solo/Hide

**Spec:** ./spec.md
**UI contract:** ../../contracts/spec-003-ui.md
**Status:** draft (amended for accepted UI-contract flags 1+2)

## Approach

Adopt the brief-#1 design: a `GraphFilter` struct threaded through the shared walk seed and the
lane walk, in Rust only. Two orthogonal knobs:

1. **`first_parent: bool`** — set `revwalk.simplify_first_parent()` on the shared
   `seeded_revwalk` AND truncate parent enumeration in `LaneWalker::step()` to parent 0 (after the
   stash-hidden filter). Both are required together: simplify shrinks the node set (perf), the
   step change stops routing edges to parents that will never be walked.
2. **`seed_refs: Option<Vec<String>>`** — restrict the walk seed. `None` = today's behavior.
   `Some(names)` = full ref names (`refs/heads/x`, `refs/remotes/origin/x`, `refs/tags/v1`);
   `collect_refs` keeps only matching refs as tips AND as pill labels (hidden refs' pills vanish
   for free), then **always** pushes `head_oid` as a tip. Fallback rule distinguishes intent from
   staleness:
   - `Some([])` — intentional empty whitelist (hide-all) → seed HEAD only (spec edge case:
     HEAD's ancestry still shows), `seed_refs_applied = true`.
   - `Some(non-empty)` where **zero** names match an existing ref — stale persistence → treat as
     `None` (full graph, locked), `seed_refs_applied = false`.

`fold_linear` is **omitted entirely** (spec-004). `GraphFilter` is `#[serde(default)]` +
camelCase, so adding the field later is wire-compatible; no reserved dead field now.

**Decisions recorded (flagged for orchestrator where noted):**

- **First-parent semantics = `git log --first-parent` over the (possibly restricted) seed set.**
  A merged side branch whose ref still exists keeps its tip + its own first-parent line; "side
  branches disappear" (spec AC1) is literally true only when the merged ref was deleted or is
  excluded via `seed_refs`. This matches the brief and every other client. **FLAG:** AC1 test
  fixtures must delete the merged branch ref (or the test must assert the standard semantics).
- **Stash tips under solo/hide:** when `seed_refs` is `Some`, stash tips are **excluded** from the
  seed (AC3 says "exactly X's ancestry plus HEAD's"; an always-seeded stash would drag in foreign
  ancestry). Under `first_parent` alone, stashes stay seeded (spec edge case). **FLAG** (minor).
- **HEAD pill when HEAD's branch is excluded:** `head_oid` is seeded, and `collect_refs`
  synthesizes a detached-style `RefKind::Head` label on it when its branch label was filtered out,
  so "the HEAD pill always resolves" holds.
- **Persistence is GLOBAL** (graphStyle precedent; there is no per-repo settings store — adding
  one is out of scope). Per UI-contract FLAG-1 (**accepted**), the persisted shape is the user's
  **intent**, not the derived whitelist: `graphRefFilter: { mode: 'solo'|'hide'; refs: string[] }
  | null` (+ `graphFirstParent: bool`). The **frontend** derives the wire `seedRefs` whitelist
  from `mode` + `refs` + the known ref list (branches/remotes/tags snapshots it already holds):
  solo → `refs` verbatim; hide → all known refs minus `refs`. The backend `GraphFilter
  { firstParent, seedRefs }` wire shape is **unchanged**. **Risk recorded below:** ref names
  match by name in every repo.
- **Cache:** single-slot per repo, keyed by storing the `GraphFilter` in `CachedGraph`; filter
  mismatch ⇒ Miss (full re-walk). Toggling back re-walks once — acceptable; determinism (not
  caching) guarantees the restored full graph is identical (AC2).
- **"Filtered" / stale indicator truth comes from the backend:** `GraphChunk::Meta` gains two
  additive serde-default fields — `filtered: bool` ("any filter took effect") and, per UI-contract
  FLAG-2 (**accepted**), `seed_refs_applied: bool` ("the seed-ref restriction specifically took
  effect"), so the UI detects stale-seedRefs fallback even when `firstParent` is also active.
  Set in the stream core (see Files touched); mock mirrors both.

## Rust/TS boundary

Rust owns all topology: seed restriction, first-parent walk, lane layout, stale-ref fallback,
HEAD-always-included, the `filtered`/`seedRefsApplied` flags. React only: the persisted intent
(`graphFirstParent`, `graphRefFilter`), deriving the `seedRefs` whitelist from intent + known
refs, passing the filter on `streamGraph`, the chip/popover/settings/context-menu UI (per the UI
contract), and the existing selection-remap/reveal on reload. No client-side graph filtering in
the real path (mock only).

## Files touched

- `crates/bonsai-core/src/graph.rs` (~473 ln, near cap — only signature threading here, ~25 ln):
  `compute_graph(workdir)` becomes a thin wrapper over new
  `compute_graph_with(workdir, &GraphFilter)`; `collect_seed(repo, &GraphFilter)`;
  `seeded_revwalk(repo, tips, first_parent: bool)`; re-export `GraphFilter` from `filter.rs`.
- `crates/bonsai-core/src/graph/lane.rs` — `LaneWalker::new(hidden, first_parent: bool)`; in
  `step()` after the hidden filter: `if self.first_parent { parents.truncate(1); }` (~8 ln).
- `crates/bonsai-core/src/graph/stream.rs` — `stream_graph_core` / `stream_graph_from_repo`
  (+ `_with` variants) gain `filter: &GraphFilter`; pass into `collect_seed`, `seeded_revwalk`,
  `LaneWalker::new`. `Meta` gains `filtered` + `seed_refs_applied`, **set where `Meta` is emitted
  in `stream_graph_from_repo_with` (immediately after `collect_seed`)**, copied from the seed:
  `seed_refs_applied = seed.seed_refs_applied` (per the fallback rule above);
  `filtered = seed.seed_refs_applied || filter.first_parent` (~30 ln).
- `crates/bonsai-core/src/graph/seed.rs` — `graph_seed`/`graph_seed_with` take `&GraphFilter`;
  `GraphSeed` gains `seed_refs_applied: bool` (computed in `filter.rs` during seed filtering)
  (~15 ln).
- `src-tauri/src/graph_cache.rs` — `CachedGraph` stores `filter: GraphFilter`; `classify` requires
  equality first (else Miss); probe (`graph_seed_with`), store re-probe (`seed_unchanged_with`),
  AND the **HitRedecorate** path all use the filtered seed so hidden pills cannot reappear from a
  cached redecorate; `stream_graph_cached_with(.., filter, ..)`. Replayed cached `Meta` chunks
  already carry the correct flags (cached under the same filter) (~40 ln).
- `src-tauri/src/commands/status.rs` — `get_graph(repo_id, filter: Option<GraphFilter>)`,
  `stream_graph(repo_id, filter: Option<GraphFilter>, on_chunk)`; `None → default` so existing
  callers/tests are untouched (~15 ln).
- `src-tauri/src/settings.rs` + `src-tauri/src/commands/ui_settings.rs` — persisted prefs
  `graph_first_parent: bool` (default false) and `graph_ref_filter: Option<GraphRefFilter>`
  (default None), where `GraphRefFilter { mode: RefFilterMode /* Solo|Hide */, refs: Vec<String> }`
  is a settings-layer type (serde camelCase) — snapshot + patch, exact graphStyle precedent. (Note:
  the UI prefs live in `settings.rs`/`ui_settings.rs`, not a `prefs.rs` — verified against the
  graphStyle chain.) The backend stores it opaquely; it never interprets it (~40 ln).
- `src/ipc/types/graph.ts` — `GraphFilter` TS type; `GraphChunkMeta.filtered?: boolean` +
  `seedRefsApplied?: boolean` (~15 ln).
- `src/ipc/types/ipc-api.ts` + `src/ipc/tauri/repo.ts` — `streamGraph(repoId, filter, onChunk)`
  and `getGraph(repoId, filter?)` signatures (~10 ln).
- `src/components/repoWorkspace/graphStreamApply.ts` — surface `meta.filtered` and
  `meta.seedRefsApplied` from the assembler (setter bundle) (~12 ln).
- `src/components/RepoWorkspace.tsx` (container, >500 exception — changes small): `refetchGraph`
  reads the derived `GraphFilter` from `useGraphFilter` (via ref to avoid dep churn) and passes
  it; effect re-runs `refetchGraph` on filter change; after `done`, existing prevSelectedId remap
  keeps selection — add scroll-into-view via the existing reveal path (`revealCommitByOid`) when
  remapped (~25 ln).
- `src/components/repoWorkspace/WorkspaceGraphPane.tsx` — render `GraphFilterChip` +
  `GraphFilterPopover` and thread their props (UI contract §wiring) (~20 ln).
- Sidebar context-menu builders for `src/components/sidebar/BranchesSection.tsx` /
  `RemotesSection.tsx` / `TagsSection.tsx` rows — add the solo/hide/unfilter item group after a
  separator (UI contract §3.1) + the trailing filtered-marker glyph (§3.3) (~15 ln each).
- `src/hooks/useUiSettings.ts` — `graphFirstParent` / `graphRefFilter` state + patch + hydrate,
  additive/optional like graphStyle; NO metricsVersion bump (~25 ln).
- `src/settings/uiSettingsDefaults.json` + `src/settings/defaults.ts` — new defaults
  (`graphFirstParent: false`, `graphRefFilter: null`) (~6 ln).
- `src/ipc/mock/persistence.ts` — validate + round-trip both prefs (`graphRefFilter` shape-checked:
  mode ∈ {solo,hide}, refs string[]; malformed → null) (~20 ln).
- `src/ipc/mock/handlers/graphStream.ts` — accept `filter`, run `resolveLayout` through the new
  mock filter (below) before chunking; emit `meta.filtered` + `meta.seedRefsApplied` with the same
  hide-all vs stale-fallback semantics as Rust (~12 ln).
- `src/components/settings/categories/GraphCategory.tsx` (or equivalent Commit-graph category
  file) — compose the new `SettingsGraphDeclutterSection` (~5 ln).

## New files (if any)

- `crates/bonsai-core/src/graph/filter.rs` (~130 ln) — `GraphFilter` struct + the seed-filtering
  logic: ref-name matching; `seed_refs_applied` computation (**stale detection = `Some(non-empty)`
  with zero matches only; `Some([])` is an intentional hide-all → HEAD-only seed, applied=true**);
  HEAD injection + synthesized Head label; stash exclusion under `Some`. Called from
  `collect_seed`/`collect_refs`. Keeps graph.rs under the cap.
- `src/ipc/mock/handlers/graphFilter.ts` (~100 ln) — fixture-side filter for the harness: from a
  full `GraphLayout`, compute the kept set (seedRefs → tip rows by ref name + headIndex; BFS over
  `parents`, first parent only when `firstParent`), remap node indices, drop edges to removed
  nodes, keep original lane numbers (`laneCount = max+1`). Approximation is fine for the mock;
  Rust remains the layout truth.
- `src/hooks/useGraphFilter.ts` (UI contract) — intent state (`graphFirstParent`,
  `graphRefFilter`), hydrate/persist via `useUiSettings`, memoized derived wire `GraphFilter`
  (solo → refs; hide → knownRefs − refs), actions `toggleFirstParent` / `soloRef` / `addToSolo` /
  `hideRef` / `unfilterRef` / `clearAll`, `activeSummary` chip label.
- `src/components/GraphFilterChip.tsx` (UI contract) — chip control + stale variant
  (presentational).
- `src/components/GraphFilterPopover.tsx` (UI contract) — anchored popover (presentational).
- `src/components/settings/SettingsGraphDeclutterSection.tsx` (UI contract) — the two Settings
  rows, composed by the Commit-graph category; registered in the settings catalog.
- `crates/bonsai-core/tests/graph_filter.rs` (or module in existing graph tests, ~150 ln) — see
  Testing.

## Data model / types

```rust
// crates/bonsai-core/src/graph/filter.rs — WIRE shape (unchanged by FLAG-1)
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GraphFilter {
    pub first_parent: bool,
    /// Full ref names; None = all refs (today's behavior).
    /// Some([]) = hide-all → HEAD-only seed. Some(non-empty) matching zero
    /// existing refs = stale → fallback to None semantics (seedRefsApplied=false).
    pub seed_refs: Option<Vec<String>>,
}

// src-tauri/src/settings.rs — PERSISTED intent (opaque to the backend)
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRefFilter {
    pub mode: RefFilterMode, // "solo" | "hide" (serde lowercase)
    pub refs: Vec<String>,   // full ref names
}
// UiSettings: graph_first_parent: bool (default false),
//             graph_ref_filter: Option<GraphRefFilter> (default None)
```

```ts
// src/ipc/types/graph.ts
export interface GraphFilter {
  firstParent: boolean;
  seedRefs: string[] | null;
}
// GraphChunk meta variant gains: filtered?: boolean; seedRefsApplied?: boolean (absent = false)

// settings types (useUiSettings / defaults)
export interface GraphRefFilter { mode: 'solo' | 'hide'; refs: string[] }
// graphFirstParent: boolean; graphRefFilter: GraphRefFilter | null
```

Commands: `get_graph(repo_id: String, filter: Option<GraphFilter>)`,
`stream_graph(repo_id: String, filter: Option<GraphFilter>, on_chunk: Channel<GraphChunk>)`.

Derivation (frontend, `useGraphFilter.ts`): `graphRefFilter` null → `seedRefs` null; solo →
`refs` verbatim; hide → known refs minus `refs` (may be `[]` = hide-all). Stale names simply
won't match backend refs — the backend fallback + `seedRefsApplied:false` is the truth signal.

## Testing

- **Regression (locked):** `get_graph(repo) == get_graph(repo, GraphFilter::default())` on a
  fixture with branches/merges/tags/stashes. (By construction after the wrapper refactor — keep
  it anyway as the guard against future divergence.)
- First-parent: fixture where a merged branch's ref is **deleted** → filtered graph = single lane,
  merge commits present, side nodes absent; toggle off → byte-identical full layout.
- First-parent with the merged ref still present → tip + its first-parent line remain (documents
  the chosen semantics).
- Solo: `seed_refs = [refs/heads/x]` with `y` checked out → node set = ancestry(x) ∪ ancestry(HEAD);
  no pills for other refs; HEAD pill present (including the synthesized-Head case).
- Hide-all: `seed_refs = Some([])` → graph = HEAD's ancestry only, `seedRefsApplied: true`,
  HEAD pill present.
- Meta flags: default → `filtered:false, seedRefsApplied:false`; firstParent only →
  `true/false`; valid seedRefs → `true/true`; seedRefs non-empty and all stale (± firstParent) →
  `seedRefsApplied:false` with `filtered == firstParent`.
- Stale refs: `seed_refs` non-empty, all nonexistent → output == full graph; partially stale →
  valid subset applied, `seedRefsApplied == true`.
- Stashes: excluded from seed when `seed_refs` is `Some`; render normally under `first_parent`.
- Cache: same filter twice → Hit; filter change → Miss + re-walk; HitRedecorate under an active
  filter never resurrects hidden pills.
- Stream/one-shot parity under each filter (existing parity test pattern, parameterized).
- Truncation: `truncated` still set under filters at the cap.
- Frontend (vitest): settings round-trip + hydration defaults for `graphRefFilter` (incl.
  malformed persisted shape → null); `useGraphFilter` derivation (solo/hide/hide-all/clear); mock
  graphFilter unit tests (BFS kept-set, index remap); graphStreamApply surfaces both flags.
- Harness/e2e (UI contract §smoke): toggle first-parent in the mock → row count shrinks, chip
  visible; solo via sidebar menu; stale persisted `graphRefFilter` → warning-chip state via
  `seedRefsApplied:false`; selection preserved/cleared per AC7.
- Perf: first-parent on the 20k fixture ≤ full-walk time (criterion or timed test). Native scroll
  feel = USER CHECKPOINT.

## Risks / open questions

- **Global persistence of `graphRefFilter` leaks across repos** (a persisted solo of
  `refs/heads/main` will silently apply in every repo with a `main`). Stale-fallback +
  `seedRefsApplied:false` make it non-fatal and now *visible* (warning chip), but still
  surprising. FLAGGED — accept for v1 (locked "global") or scope per-repo later.
- **Hide-mode derivation depends on the known ref list** (branches/remotes/tags snapshots): if
  the graph refetch races a branch-list refresh, the derived whitelist can be one refresh stale.
  Mitigation: derive at refetch time from the freshest snapshots; the next watcher/refresh tick
  self-heals.
- **AC1 fixture semantics** (live-merged-ref keeps its line) — flagged above; tests encode the
  `--first-parent` reading.
- **Stash-under-solo exclusion** — flagged above (minor).
- Search matches on filtered-out commits: rings/reveal no-op for absent rows — acceptable;
  search itself is out of scope (spec non-goal).
- `simplify_first_parent` + TOPOLOGICAL|TIME ordering: verify identical merge-commit ordering vs
  full walk on the fixture; if libgit2 orders differently, that's fine (different view) but AC2
  only requires the *full* graph to be restored identically, which is untouched.
- RepoWorkspace.tsx is already a >500-line container (allowed exception); filter wiring lives in
  `useGraphFilter.ts`, keeping the container delta ≤ ~25 lines.
