# P10 — Stash as a graph node (+ context-menu icons)

> Contract for senior-dev. Implement strictly to the signatures and sequences below.
> Rust owns ALL git logic and layout math; React only renders. The IPC wire is UNCHANGED
> (no new fields on `GraphNode`/`GraphLayout`). Prior art: `docs/contracts/M2-graph.md`,
> `docs/contracts/P7-*.md`, `docs/contracts/P9-stash-management.md`.

## §0 Goals / non-goals

**Goal.** Render each stash as its OWN node in the commit graph (GitKraken/GitExtensions
style): a distinct stash node on a small offshoot lane, connected by a single edge to its
base commit, drawn with the stash glyph (not an author avatar) and its "WIP on <branch>: …"
summary. Plus: (T2) leading icons on every context-menu action; (T3) a working Apply/Pop/Drop
right-click menu on a stash in the graph.

**Non-goals.** No new stash git operations (P9 already ships list/create/apply/pop/drop).
No hunk staging. No wire-schema change to `GraphNode`/`GraphLayout`/`GraphEdge`. No change to
the M2d 20k perf gate.

**P9b → P10 behavior change (document in code comments too).**
- BEFORE (P9b): a stash was a violet `RefKind::Stash` pill (`stash@{n}`) attached to its
  BASE commit's row; step 6.5 in `layout_walk`. An orphaned base showed no pill.
- AFTER (P10): the stash commit `W` is emitted as its own node (offshoot lane), carrying the
  `stash@{n}` pill on ITS row; a single edge `W → base` connects it. The base row no longer
  carries the stash pill. Step 6.5 is deleted.
- **Orphan consequence (accepted, more correct):** an orphaned stash (base not reachable from
  any branch) now APPEARS — the stash node `W` is a walk tip, so it pulls its base `B` and
  `B`'s otherwise-unreachable ancestors back into the walk (the stash keeps them alive). This
  reverses the old "orphan → nothing" behavior; update tests accordingly.

Task 1 (orchestrator-owned) is already done and is out of scope here.

---

## §1 Rust — `src-tauri/src/graph.rs`

### 1.1 `RefKind` / `RefLabel` / `GraphNode` — UNCHANGED

Keep `RefKind::Stash` (pill_rank 4) and the existing `RefLabel`. **Do NOT add a stash marker
field to `GraphNode`.** The renderer detects a stash node purely via
`node.refs.some(r => r.kind === Stash)`. Update the `RefKind::Stash` doc comment: it is now
"Attached to a stash's OWN node `W`; name is `stash@{n}`." (was "base commit").

### 1.2 Replace `collect_stash_bases` → `collect_stashes`

Delete `collect_stash_bases`. Add:

```rust
/// One stash resolved for the walk. `stash_oid` (= commit `W`) is pushed as a
/// revwalk TIP so the stash appears as its own node; `hide` = the stash's
/// synthetic parents (index commit `I` = parent 1, untracked commit `U` =
/// parent 2 if present) which are `revwalk.hide()`-d so they never become
/// nodes. `W`'s FIRST parent (the base `B`) is intentionally NOT hidden — it is
/// reached naturally and yields the single `W → B` edge.
struct StashSeed {
    index: usize,
    stash_oid: git2::Oid,
    hide: Vec<git2::Oid>,
}

/// O(stashes). Enumerate the stash stack (ascending index, `stash@{0}` first);
/// for each, resolve `W` via the `refs/stash` reflog (entry `i`.`id_new()`), then
/// derive `hide` from `W`'s parents `[1..]` (skip parent 0 = base). A missing
/// `refs/stash` → empty; unresolvable entries are skipped. Requires `&mut` for
/// `stash_foreach`.
fn collect_stashes(repo: &mut git2::Repository) -> Result<Vec<StashSeed>, AppError>;
```

Implementation notes:
- Collect indices via `stash_foreach` (same pattern as `collect_stash_bases`).
- `let reflog = repo.reflog("refs/stash")` → `Err` returns `Ok(Vec::new())`.
- For each index: `stash_oid = reflog.get(index)?.id_new()`; `commit = find_commit(stash_oid)`;
  `hide = commit.parent_ids().skip(1).collect()` (parents 1.. = `I`, optional `U`).
  Skip the entry if `find_commit` fails.
- Return ascending by index.

### 1.3 `compute_graph` — new sequence

```rust
pub fn compute_graph(workdir: &std::path::Path) -> Result<GraphLayout, AppError> {
    let mut repo = /* open NO_SEARCH, as today */;
    let stashes = collect_stashes(&mut repo)?;          // was collect_stash_bases
    let (mut refs, mut tips, head_oid) = collect_refs(&repo)?;

    // Inject stash W nodes: label + tip. Keeps the wire stable (no GraphNode field).
    let mut hide: Vec<git2::Oid> = Vec::new();
    for s in &stashes {
        refs.entry(s.stash_oid).or_default().push(RefLabel {
            name: format!("stash@{{{}}}", s.index),
            kind: RefKind::Stash,
            is_head: false,
        });
        tips.push(s.stash_oid);      // pushed AFTER HEAD (see determinism, §1.6)
        hide.extend(s.hide.iter().copied());
    }
    // Re-dedupe tips (a stash W could coincide with an existing tip — unlikely
    // but keep deterministic first-occurrence order).
    { let mut seen = HashSet::new(); tips.retain(|o| seen.insert(*o)); }

    if tips.is_empty() {
        return Ok(GraphLayout::empty());
    }
    layout_walk(&repo, &tips, refs, head_oid, &hide)
}
```

Notes:
- `collect_refs` currently returns `(RefMap, Vec<Oid>, Option<Oid>)` with the RefMap already
  pill-sorted. Appending a `Stash` label (highest `pill_rank`) after that sort keeps a valid
  order WITHOUT re-sorting — but on a stash node `W` there are normally no other pills, so
  order is trivially correct either way. Do not re-sort.
- If two stashes share the same `W` oid (cannot happen for distinct stack entries) the dedupe
  keeps the first; the labels HashMap already holds both names on that oid. Not a concern in
  practice.

### 1.4 `layout_walk` — signature change: `stash_bases` → `hide`

```rust
fn layout_walk(
    repo: &git2::Repository,
    tips: &[git2::Oid],
    refs: RefMap,
    head_oid: Option<git2::Oid>,
    hide: &[git2::Oid],        // was: stash_bases: &[(usize, git2::Oid)]
) -> Result<GraphLayout, AppError>
```

Body changes:
1. **CORRECTION (as-built): do NOT use `revwalk.hide()`.** `revwalk.hide(I)` marks `I` AND ALL
   ITS ANCESTORS uninteresting — and `I`'s parent is the base `B`, so hiding the index commit
   would wrongly exclude `B` (and its whole history) from the walk, the opposite of the intended
   single `W → B` edge (empirically: `W` ends with 0 parents). Instead **skip-emit** the hidden
   oids: build `let hidden: HashSet<Oid> = hide.iter().copied().collect();`, and in the walk loop
   `if hidden.contains(&oid) { continue; }` BEFORE any lane/pending/index work. Take the row
   index from `nodes.len()` (not the revwalk enumerate counter) so a skipped oid leaves no gap,
   and filter `hidden` oids out of each commit's `parent_ids()` in step 4 so `W` keeps only `B`.
   `B` stays reachable because it is NOT hidden and is reached from the branch/stash tips. The
   `MAX_COMMITS` cap becomes `nodes.len() >= MAX_COMMITS` (equivalent for stash-free repos).
2. **DELETE step 6.5 entirely** (the `for &(idx, base_oid) in stash_bases { … }` block).
   Stash labels now ride into the node via the existing step-5 `refs.remove(&oid)`.
3. Everything else (lane assignment, edge routing, parent resolution, edge sort) is UNCHANGED.

**Why the offshoot look emerges:** `W`'s parents are `[B, I, U?]`. `I`/`U` are hidden, so they
are never emitted; step 6 resolves `W.parents` via `index_of`, and only `B` is present → `W`
ends with a single parent `B` and a single edge `W → B`. `W` is a fresh tip, so it gets its
own lane via `first_free` (an offshoot), and `B` keeps its own lane.

### 1.5 Edge/lane shape (informative)

No new algorithm. `W` lands on the leftmost free lane at its row; the `W → B` edge routes with
the existing 3-segment rule. Because `W` sorts by TIME/TOPO among the tips, it typically appears
near the top; its base `B` appears lower, so `to > from` holds naturally (revwalk guarantees
parents at higher indices).

### 1.6 Determinism

`compute_graph` must remain deterministic (same repo → identical `GraphLayout`; a test asserts
it). Sources of order:
- Stash tips are appended to `tips` in `collect_stashes` ASCENDING index order, AFTER the
  branch/remote/tag/HEAD tips. `tips` is then deduped preserving first occurrence. Fixed.
- `hide` is built in the same ascending order. `revwalk.hide` order is irrelevant to output.
- The revwalk sort (`TOPOLOGICAL | TIME`) plus deterministic tip order ⇒ stable node order.

Keep the existing `determinism` test green and add a stash-specific determinism assertion
inside the new test (compute twice, assert equal — mirrors the old scenario-1 check).

### 1.7 Rust test matrix — rewrite `stash_pill_on_base_and_orphan_omitted`

Delete `stash_pill_on_base_and_orphan_omitted`. Keep helpers `init_worktree_repo`,
`commit_file`, `checkout_ref`, `stash_labels`. Add `stash_appears_as_own_node` with three
scenarios (reuse `crate::git::stash::create_stash`):

**Scenario 1 — stash on a branch tip → own node, single base edge, no synthetic nodes.**
- Build: `C0`, `C1` (HEAD `main` tip). Dirty `f.txt`, `create_stash(dir, None, false)` → created.
- `l = compute_graph(dir)`.
- Find the stash node: exactly one node whose `refs` contains a `RefKind::Stash` label
  `stash@{0}`; call its index `sr`.
- Assert `l.nodes[sr].summary.starts_with("WIP")` (git default stash message summary).
- Assert `l.nodes[sr].parents.len() == 1`; the single parent resolves to the base row whose
  `id == c1` (i.e. `l.nodes[l.nodes[sr].parents[0] as usize].id == c1.to_string()`).
- Assert the base row (`c1`) still exists AND carries NO `RefKind::Stash` label (pill moved off
  the base).
- Assert exactly one edge originates at `sr` and points to the base row, with
  `edge.lane == l.nodes[sr].lane` (offshoot lane).
- **No synthetic nodes:** with `C0`, `C1`, `W` reachable and `I` hidden, `l.nodes.len() == 3`.
  (The index commit `I` must not be emitted.) Assert node count == 3.
- Determinism: `compute_graph(dir)` again equals `l`.

**Scenario 2 — two stashes on the same base → two distinct stash nodes.**
- Build `C0`, `C1` (base, HEAD stays on it). Dirty→stash (a), dirty→stash (b). Now
  `stash@{0}` = b (newest), `stash@{1}` = a.
- `stash_labels(&l)` (any order) contains exactly `stash@{0}` and `stash@{1}`.
- Exactly two distinct nodes carry a `Stash` label; each has a single parent resolving to the
  same base row (`c1`).
- The base row carries no `Stash` label.

**Scenario 3 — orphaned base → stash node PRESENT and base now present (reversed P9b rule).**
- Reproduce the old scenario-2 fixture: branch `temp` off `c1`, commit `X` on it, stash on `X`,
  return to main, delete `temp` → `X` unreachable from any branch.
- `l = compute_graph(dir)`.
- Assert a stash node exists (`stash_labels(&l)` non-empty, `stash@{0}`).
- Assert `X` IS now present in the walk (`l.nodes.iter().any(|n| n.id == x.to_string())`) —
  pulled in as the stash node's base. (Old test asserted the opposite; that assertion is
  intentionally reversed.)
- Assert the stash node's single parent resolves to the `X` row.

Keep every existing `graph.rs` test (linear_chain … annotated_tag_to_blob_skipped) untouched —
they contain no stashes and are unaffected.

### 1.8 Perf

Stashes add O(few) tips + hides; the 20k perf fixture contains no stashes, so the M2d criterion
benchmark is unaffected. **No change to the perf test.** State this in the code comment on
`collect_stashes`.

---

## §2 Frontend graph render — `src/graph/draw.ts`

### 2.1 Pass-4 stash-node branch (replaces the avatar for stash rows)

In `drawGraph` pass-4 (lines ~682-728), at the top of the per-row loop compute:

```ts
const isStash = node.refs?.some((r) => r.kind === 'stash') ?? false;
```

- If `isStash`, draw a **stash node** instead of the author avatar; else keep the existing
  avatar block verbatim.
- Keep the shared surrounding logic for BOTH branches: the `x`/`y`/`laneColor`/`selected`
  computation, the bg-ring halo (unchanged), the HEAD ring, and the selection ring. A stash is
  never HEAD (`layout.headIndex === row` will be false) but CAN be selected — keep the sel-ring
  branch reachable.

Factor the stash disc into a helper (keep `drawGraph` readable):

```ts
/** P10 §2.1: a stash node — violet disc + centered white stash glyph + violet
 *  lane ring. Draws in place of the author avatar; caller has ALREADY drawn the
 *  bg-ring halo and will draw HEAD/selection rings afterwards. */
export function drawStashNode(ctx: CanvasRenderingContext2D, x: number, y: number): void;
```

`drawStashNode` body (exact order/colors/sizes):
1. Disc: `arc(x, y, METRICS.avatarRadius)`, `fillStyle = STASH_COLOR`, fill.
2. Lane ring: `arc(x, y, METRICS.avatarRadius)`, `strokeStyle = STASH_COLOR`,
   `lineWidth = METRICS.avatarRingWidth`, stroke. (Ring matches disc → a clean solid violet
   dot; this deliberately does NOT use the row's lane color, so the stash reads as "not a real
   branch commit". Document this choice.)
3. Glyph: reuse `drawStashIcon(ctx, bx, by, S)` with `S = METRICS.avatarRadius * 1.4` (≈14px),
   centered: `bx = x - S/2`, `by = y - S/2`. Set `ctx.strokeStyle` to the glyph color BEFORE
   the call. Use `theme.bg0`? No — `drawStashNode` has no theme; use white `'#ffffff'` for the
   glyph (legible on the violet disc, matches avatar `text:'#ffffff'`). If a theme color is
   preferred later, add a `glyphColor` param; for P10 hard-code white.

Import `STASH_COLOR` is already imported in `draw.ts` (line 6). `drawStashIcon` is already
exported in this file. Call site in pass-4:

```ts
// bg-ring halo (unchanged) …
if (isStash) {
  drawStashNode(ctx, x, y);
} else {
  // existing avatar disc + lane ring + initials block, verbatim
}
// HEAD ring + selection ring blocks (unchanged, run for both)
```

### 2.2 Pass-5 / pills / summary — UNCHANGED

- The `stash@{n}` violet pill STILL renders in the LEFT ref column of the stash node's own row
  via the existing `groupRefs` → `entityStyle`(stash) → `drawRefLabelAt` machinery (this is the
  T3 right-click target). No change.
- The stash node's `summary` ("WIP on <branch>: …") renders in the RIGHT summary column
  automatically (it is `node.summary`). No change.
- Relative time renders from `node.ts`. No change.
- Confirm: NO other draw-pass changes. Edges (pass-3) already draw the `W → B` offshoot from the
  wire `edges`.

---

## §3 Fixtures / mock — `src/ipc/fixtures/graph.ts`, `src/ipc/mock.ts`

### 3.1 `buildMockGraph` — remove the base-row stash pills

- Row 3 `core work 4`: drop the `stash@{0}` refs arg → `push('core work 4', 0, [5], undefined, 'torvalds')`.
  Keep the `'torvalds'` author override (still exercises initials "TO" on a NON-stash node).
- Row 6 `core work 2`: drop the two-stash refs arg → `push('core work 2', 0, [7])`.
- Update the neighbouring comments (P9 §6.6 references) to point at the new stash-node model.
- `laneCount` stays `3` here; the stash-node helper bumps it.

### 3.2 New helper `withStashNodes` (model on `prependCommits`)

Add to `fixtures/graph.ts`:

```ts
import type { StashEntry } from '../types';

/**
 * P10 §3.2: insert each stash as its OWN node at the TOP of `layout`, on a
 * fresh offshoot lane, connected by a single edge to its base row.
 *
 * For each stash (index order, stash@{0} first) whose `baseOid` matches a node
 * id in `layout`:
 *   - it becomes a new top row (like prependCommits): all existing node.parents
 *     and edge.from/to shift by k = number of INSERTED stash nodes;
 *   - new stash node i (0..k): { id: stash.oid, lane: layout.laneCount + i,
 *     parents: [baseRow + k], refs: [{ name:`stash@{${stash.index}}`,
 *     kind:'stash', isHead:false }], summary: stash.message, author:'', ts:
 *     stash.ts } where baseRow = index of the base node in the ORIGINAL layout;
 *   - new edge i: { from: i, to: baseRow + k, lane: layout.laneCount + i };
 *   - laneCount += k; headIndex (if non-null) shifts by k.
 * Stashes whose baseOid is not found are skipped (orphan → not rendered in the
 * mock; the real Rust path DOES render them, but the mock has no ancestor to
 * attach to). Edges are NOT required to be (from,to)-sorted for the mock.
 */
export function withStashNodes(layout: GraphLayout, stashes: StashEntry[]): GraphLayout;
```

Shift math (mirror `prependCommits` exactly):
- Resolve insertable stashes first: `const insertable = stashes.filter(s => baseIndex(s) !== -1)`
  where `baseIndex(s) = layout.nodes.findIndex(n => n.id === s.baseOid)`. `k = insertable.length`.
  If `k === 0` return `layout` unchanged.
- `shiftedNodes = layout.nodes.map(n => ({ ...n, parents: n.parents.map(p => p + k), refs: n.refs?.map(r => ({...r})) }))`.
- `shiftedEdges = layout.edges.map(e => ({ ...e, from: e.from + k, to: e.to + k }))`.
- `newNodes[i]` and `newEdges[i]` per the doc above, using `baseRow = baseIndex(insertable[i]) + k`.
- `headIndex = layout.headIndex === null ? null : layout.headIndex + k`.
- Return `{ nodes: [...newNodes, ...shiftedNodes], edges: [...newEdges, ...shiftedEdges],
  laneCount: layout.laneCount + k, headIndex, truncated: layout.truncated }`.

`baseRow + k` (= to) > `i` (= from) always holds (`baseRow >= 0`), so `to > from` is preserved.

### 3.3 Mock `getGraph` — rebuild from the live stash list (RECOMMENDED)

In `mock.ts` `getGraph`, wrap the default-fixture return so create/apply/pop/drop reflect
visually:

```ts
if (state.graphFixture === '20k') return generateLayout20k();
if (state.graphFixture === 'detached') return buildMockGraphDetached();
const base = prependCommits(buildMockGraph(), state.commits);
return withStashNodes(base, state.stashes);   // stash nodes reflect the live stack
```

Import `withStashNodes` alongside the existing `buildMockGraph`/`prependCommits` import (line 8).

- The seeded `state.stashes` (indices 0/1/2 with `baseOid = fixtureOid(3)` / `fixtureOid(6)`)
  resolve against the post-`prependCommits` layout by node id (the oids are stable across the
  shift), so three stash nodes appear on offshoot lanes off `core work 4` and `core work 2`.
- Because `createStash`/`dropStash`/`popStash` already mutate `state.stashes`, the next
  `getGraph` (the frontend refetches on `repo-changed`) shows/removes the stash node. This is
  the clean choice and matches the "built fresh per call" comment already there.
- `detached`/`20k`/non-default repos: no stash injection (their `state.stashes` is `[]` anyway).

If, and only if, the id-based resolution proves impractical during implementation, the fallback
is a static 2-stash-node fixture that at least RENDERS stash nodes for harness verification —
but the recommended live-rebuild path above is expected to work and is preferred.

---

## §4 Context-menu icons — `src/components/ContextMenu.tsx` + new `src/components/menuIcons.tsx`

### 4.1 `ContextMenuItem` gains an optional icon

```ts
export interface ContextMenuItem {
  label: string;
  onSelect(): void;
  disabled?: boolean;
  icon?: React.ReactNode;   // P10 T2: optional leading 16×16 monochrome glyph
}
```

Render in the button, before the label, keyboard/focus behavior UNCHANGED:

```tsx
<button …>
  {item.icon !== undefined && (
    <span className="context-menu-icon" aria-hidden="true">{item.icon}</span>
  )}
  {item.label}
</button>
```

- Do NOT change the `key`, `role`, `disabled`, `aria-disabled`, `tabIndex`, focus query, or
  arrow/enter handlers.
- CSS (add to the existing context-menu stylesheet — locate the `.context-menu-item` rule):
  ```css
  .context-menu-item { display: flex; align-items: center; gap: 8px; }
  .context-menu-icon {
    display: inline-flex; align-items: center; justify-content: center;
    width: 16px; height: 16px; flex: 0 0 16px; color: currentColor;
  }
  .context-menu-icon svg { width: 16px; height: 16px; display: block; }
  ```
  Keep whatever padding/min-width the rule already has; only add the flex row + icon box. If
  `.context-menu-item` already sets `display`, adapt (flex row) rather than duplicate.

### 4.2 `src/components/menuIcons.tsx` — inline SVG set (new file, no deps)

Monochrome, 16×16, `viewBox="0 0 16 16"`, `stroke="currentColor"` with
`stroke-width={1.4}`, `fill="none"`, `stroke-linecap="round"` `stroke-linejoin="round"`
(use `fill="currentColor"` only where a solid shape reads better, e.g. trash lid). Each is a
plain functional component returning an `<svg>` (no props needed). Export named components:

| Export | Action(s) | Rough SVG intent |
|---|---|---|
| `CheckoutIcon` | Checkout | a check-mark, or an arrow pointing into a circle (branch switch) |
| `CopyIcon` | Copy branch name | two overlapping rounded rectangles (classic copy) |
| `MergeIcon` | Merge …into… | two lines converging into one (git-merge glyph) |
| `RebaseIcon` | Rebase …onto… | a branch lifted onto another line (arc + baseline) |
| `CompareIcon` | Compare with HEAD (branch + commit row) | split/diff: two panes or a ⇄ arrows pair |
| `DeleteIcon` | Delete (branch), Drop (stash) | trash can (lid + body + two vertical strokes) |
| `StashApplyIcon` | Apply (stash) | drawer/tray + a down arrow INTO it (apply = pull out to worktree → arrow down-out is fine; pick one direction and keep consistent: down arrow leaving the tray) |
| `StashPopIcon` | Pop (stash) | drawer/tray + an up-and-out arrow (pop = remove from stack) |

- Reuse `DeleteIcon` for BOTH branch "Delete" and stash "Drop" (both are destructive-remove).
- Keep the aesthetic aligned with `draw.ts` glyphs (`drawStashIcon` tray box) — the stash
  apply/pop icons should echo that tray silhouette so the menu matches the graph node.
- No animation, no external icon library, no `currentColor` overrides inside (inherit from the
  `.context-menu-icon` span so disabled/hover colors flow through).

### 4.3 Assign an icon to every menu item — `src/components/RepoWorkspace.tsx`

Import the set from `./menuIcons`. Assign:
- `branchMenuItems`: Checkout→`CheckoutIcon`, Copy branch name→`CopyIcon`,
  `Merge …`→`MergeIcon`, `Rebase …`→`RebaseIcon`, Compare with HEAD→`CompareIcon`,
  Delete→`DeleteIcon`.
- `stashMenuItems`: Apply→`StashApplyIcon`, Pop→`StashPopIcon`, Drop→`DeleteIcon`.
- Commit-row item in `buildContextItems` ("Compare with HEAD")→`CompareIcon`.
- Set each via the new `icon:` field on the `ContextMenuItem` literals; add `icon: <XIcon />`.

No behavioral change to gating (`disabled`) or `onSelect`.

---

## §5 Task 3 — `buildContextItems` stash branch — `src/components/RepoWorkspace.tsx`

Current (line ~1224) early-returns `[]` for `r.kind === 'stash'`. Change:

```ts
function buildContextItems(target: GraphContextTarget): ContextMenuItem[] {
  if (target.kind === 'ref') {
    const r = target.ref;
    if (r.kind === 'stash') {
      const m = /^stash@\{(\d+)\}$/.exec(r.name);
      if (m === null) return [];              // malformed name → no menu (defensive)
      return stashMenuItems(Number(m[1]));    // reuse the existing builder (with icons)
    }
    if (r.kind === 'tag' || r.kind === 'head') return [];  // stash REMOVED from this line
    return branchMenuItems(r.name, r.kind === 'remoteBranch' ? 'remoteBranch' : 'localBranch');
  }
  // commit row → Compare with HEAD (unchanged, now with CompareIcon)
  …
}
```

- `stashMenuItems(index)` already builds Apply/Pop/Drop with the correct gating and the Drop→
  ConfirmDialog route (`setPendingDropStash`). No change to that function beyond §4.3 icons.
- The graph hit-test (`GraphCanvas.handleContextMenu` + `targetRefOf`) already emits
  `{ kind:'ref', ref }` for a stash pill (`targetRefOf` returns `entity.ref` for a stash
  entity). No GraphCanvas change is required for T3.

---

## §6 Acceptance / AI gate

**Rust (`cargo test --lib`)**
- New `stash_appears_as_own_node` (3 scenarios) green; all pre-existing `graph.rs` and
  `stash.rs` tests green. `collect_stash_bases` fully removed (no dead-code warning).
- `cargo clippy` clean (run sequentially with tests — never concurrently; target-dir race).

**Frontend (`pnpm build`)**
- `tsc`/build clean. New `menuIcons.tsx` and `withStashNodes` type-check.
- `p7SelfTest` still green. Its existing "groupRefs stash not collapsed, sorts last" assertion
  already covers the stash entity — NO new assertion is strictly required. OPTIONAL: add one
  assertion that a node with only a stash ref yields a single `stash` entity from `groupRefs`
  (already implied); if added, bump the expected pass count in whatever reads it. Prefer leaving
  the count unchanged unless the orchestrator wants the extra guard — FLAGGED below.

**Browser harness (`pnpm dev`, `VITE_MOCK_IPC=1`) — orchestrator screenshots**
- Default fixture: three stash nodes render as violet discs with the white drawer glyph on
  offshoot lanes, each linked by a single curved edge to its base commit (`core work 4`,
  `core work 2`), each showing its "WIP on main: …" summary and a `stash@{n}` violet pill in
  the left column. The base rows no longer carry stash pills.
- Right-click a stash node's `stash@{n}` pill → an Apply/Pop/Drop menu opens; Drop opens the
  confirm dialog. Right-click a branch/tag/commit still behaves as before.
- Every context-menu item shows a leading 16px monochrome icon (Checkout/Copy/Merge/Rebase/
  Compare/Delete; Apply/Pop/Drop). Disabled items dim icon+label together.
- Console `window.__bonsai.p7SelfTest()` → `{ pass: N, fail: 0, failures: [] }`.

**USER CHECKPOINT (native `pnpm tauri dev`, human perception — orchestrator must NOT self-pass)**
- Create a real stash in a scratch repo → a stash node appears on an offshoot lane linked to
  its base; scrolling stays smooth.
- Right-click the stash node in the native window → Apply/Pop/Drop work end-to-end; Drop
  confirm prevents accidental loss.
- Icons look crisp on HiDPI in both light and dark themes; menu spacing feels clean.

---

## §7 Sub-increment split (each independently compilable & committable)

- **P10a — Rust (`src-tauri/src/graph.rs`).** `collect_stashes` + `StashSeed`, `compute_graph`
  injection sequence, `layout_walk` `hide` param, delete step 6.5, rewrite the test to
  `stash_appears_as_own_node`. Gate: `cargo test --lib` + clippy green. Wire unchanged, so the
  frontend keeps compiling against the old fixtures until P10b.
- **P10b — Frontend graph + fixtures/mock + T3 menu.** `drawStashNode` + pass-4 branch
  (`draw.ts`); remove base-row pills + add `withStashNodes` (`fixtures/graph.ts`); wire
  `withStashNodes` into `getGraph` (`mock.ts`); `buildContextItems` stash branch
  (`RepoWorkspace.tsx`). Gate: `pnpm build` + `p7SelfTest` green; harness shows stash nodes and
  the stash right-click menu (unstyled/no icons yet is fine).
- **P10c — Context-menu icons.** `ContextMenuItem.icon` + render + CSS (`ContextMenu.tsx`); new
  `menuIcons.tsx`; assign icons across `branchMenuItems`/`stashMenuItems`/commit-row
  (`RepoWorkspace.tsx`). Gate: `pnpm build` green; harness shows icons on every item, keyboard
  nav unchanged.

Order dependency: P10a is independent (wire stable). P10b depends on nothing in P10a (mock is
frontend-only) but SHOULD land after P10a so the real backend and mock tell the same story.
P10c depends on P10b only for the `buildContextItems`/`stashMenuItems` call sites it decorates;
it can also land standalone.

---

## §8 Risks / flags for the orchestrator

1. **`p7SelfTest` pass-count.** If anything asserts an exact pass count elsewhere, adding a new
   self-test assertion would break it. Recommendation: DO NOT add a new assertion in P10 (the
   existing stash-entity assertion suffices). Flagged so you can decide.
2. **Orphan-stash behavior reversal is a real semantic change** (orphan base now rendered).
   Confirmed acceptable per the design decisions; the rewritten Scenario 3 encodes it. If a
   user later wants orphaned stashes hidden, that becomes a filter on `collect_stashes`, not a
   layout change.
3. **Mock `withStashNodes` id-resolution** relies on stash `baseOid` matching a node id in the
   post-`prependCommits` layout. The seeded oids (`fixtureOid(3)`/`fixtureOid(6)`) do match, and
   `prependCommits` preserves node ids. A freshly `createStash`-ed mock entry uses
   `randomOid()` for its `baseOid`? — CHECK: the mock `createStash` (mock.ts ~1280) sets the new
   entry's `baseOid`. If it is a random oid with no matching node, that stash is skipped by
   `withStashNodes` (no visual node) but still appears in the sidebar. If visual feedback on
   mock-created stashes matters for the harness, set the mock `createStash` `baseOid` to the
   current head node's id (`state.headOid` / row-0 oid). Flagged — recommend aligning the mock
   `createStash.baseOid` to the head node id so a created stash shows a node.
4. **Glyph color** in `drawStashNode` is hard-coded white (matches avatar text). If dark/light
   contrast on the violet disc looks off at USER CHECKPOINT, promote to a themed param — noted
   as a follow-up, not a P10 blocker.
