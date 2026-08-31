# P5 — Graph context menus

Right-click affordances on the commit-graph canvas. Two independent features that
ship together:

- **Part 1 — Merge/Rebase from branch pills.** Right-click a *branch ref pill* drawn
  beside a commit row → context menu mirroring the sidebar's merge/rebase actions.
- **Part 2 — Compare HEAD ↔ selected commit.** Right-click a commit row/dot → context
  menu with "Compare with HEAD" → tree-vs-tree diff shown in a new right-panel mode.

House style follows `M4-diff.md` (diff engine), `M5-branches.md`, `P3c-merge-conflicts.md`,
`P3d-rebase.md` (merge/rebase wording + gating), `P3a-diff-overlay.md` (overlay slot
grammar), and `M2-graph.md` (canvas geometry).

---

## §1 Scope + locked decisions

### 1.1 Part 1 gating (mirrors the sidebar EXACTLY)
- Merge/rebase items appear ONLY for `RefLabel.kind === 'localBranch'` (with `isHead === false`)
  and `kind === 'remoteBranch'`. Tag pills, the `head` pill, and the current branch's own
  pill (`localBranch` with `isHead === true`) get **no** merge/rebase items.
- Items exist only when `currentBranch != null` (i.e. `headBranch?.name`, null when
  detached/unborn). With no current branch → the ref menu has zero items → **the menu does
  not open** (mirrors the sidebar HIDING the buttons).
- The two items are **disabled** (shown greyed) when `mutating || opActive` — same as the
  sidebar's `disabled={busy || opActive}`.
- Handlers are the EXISTING `handleMergeBranch(name)` / `handleRebaseBranch(onto)` in
  `RepoWorkspace.tsx` — no new backend, no new IPC for Part 1.
- Labels/titles MATCH the sidebar verbatim (`Sidebar.tsx` `BranchRow`/`RemoteRow`):
  - Merge: **`Merge ${ref.name} into ${currentBranch}`**
  - Rebase: **`Rebase ${currentBranch} onto ${ref.name}`**

### 1.2 Part 2 direction (LOCKED; user may flip later — FLAG for orchestrator)
- **old = HEAD, new = selected commit** → `git diff HEAD <commit>`. The file list therefore
  shows what changed going from HEAD to the chosen commit. The Compare header labels BOTH
  endpoints: `HEAD (<branch>) <shortOid>` on the old side, `<shortOid> <summary>` on the new.
- HEAD is resolved **server-side** in Rust (authoritative; the frontend never passes a HEAD
  oid). The command takes only the selected commit oid.

### 1.3 Part 2 edge cases (specify precisely)
- **Right-click the HEAD commit itself** → `from.oid == to.oid` → diff is empty → backend
  returns a `CompareDiff` with `files: []` (NOT an error). UI shows a clear
  "No differences" state.
- **Detached HEAD** → allowed. `from.oid` = the detached HEAD commit oid; header shows
  `HEAD <shortOid>` with no branch name (`headBranchName === null`).
- **Unborn HEAD** → the "Compare with HEAD" item is **omitted** (unborn repos have no commits
  and the graph pane already shows "No commits yet", so this is unreachable in practice; the
  backend still defines it safely — see §2.2 — as compare-vs-empty-tree for robustness).

### 1.4 Architecture-invariant note
Ref-pill *pixel geometry* is a pure view-layer concern already computed in `draw.ts` for
rendering. Reusing it for right-click hit-testing is view-layer (same category as the
existing WIP-row compositing), **not** a graph-layout-math violation. The comparison diff
itself is Git logic → lives in Rust.

---

## §2 Backend (Rust)

### 2.1 New wire types — `src-tauri/src/git/diff.rs`
Add alongside `CommitDiff`. A comparison has **two** endpoints, so we do NOT reuse
`CommitDetails` (single-commit). New lean types:

```rust
/// One endpoint of a two-commit comparison (P5 §1.2).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareEndpoint {
    pub oid: String,     // full 40-char hex; "" when HEAD is unborn (old side)
    pub summary: String, // first line of that commit's message; "" when unborn
}

/// Tree-vs-tree comparison HEAD(old) → `to`(new). Headers only — hunks fetched
/// per file, exactly like `CommitDiff`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareDiff {
    pub from: CompareEndpoint,      // OLD = HEAD
    pub to: CompareEndpoint,        // NEW = the right-clicked commit
    /// Sorted path-ascending (byte-wise). Empty when from.oid == to.oid.
    pub files: Vec<FileDiffHeader>,
}
```

### 2.2 New pure functions — `src-tauri/src/git/diff.rs`
Reuse the existing helpers verbatim: `head_tree` (already returns `None` for unborn),
`build_diff_options`, `apply_find_similar`, `collect_headers`, `collect_file_diff`,
`pathspecs`, `validate_rel_path`, `map_status`. No new diff machinery.

```rust
/// `git diff HEAD <to_oid>` as headers. HEAD (old side) is resolved internally:
/// attached or detached both work via `repo.head()`; unborn HEAD -> old tree is
/// the empty tree (everything shows Added) and `from` = CompareEndpoint{"",""}.
/// `from.oid == to_oid` (comparing HEAD to itself) -> empty `files`, not an error.
/// Bad/unknown/non-commit `to_oid` -> AppError::Git.
pub fn compare_head_diff(workdir: &Path, to_oid: &str) -> Result<CompareDiff, AppError>;

/// Hunks for ONE file of the HEAD → `to_oid` comparison (§2.2 shape mirrors
/// `commit_file_diff`). No matching delta -> AppError::Git.
pub fn compare_head_file_diff(
    workdir: &Path,
    to_oid: &str,
    path: &str,
    orig_path: Option<&str>,
) -> Result<FileDiff, AppError>;
```

Implementation shape (normative, no bodies):
- Open repo; `to_commit = repo.find_commit(Oid::from_str(to_oid)?)?`; `to_tree = to_commit.tree()?`.
- Resolve HEAD for the `from` endpoint: `match repo.head()` → attached/detached → peel to a
  commit → `from = { oid: commit.id().to_string(), summary: lossy(summary_bytes) }`, old tree
  = that commit's tree; unborn/`NotFound` → `from = { oid: "".into(), summary: "".into() }`,
  old tree = `None`. (Use `head_tree` for the tree; resolve the oid/summary separately so the
  endpoint metadata is populated.)
- `diff = repo.diff_tree_to_tree(old_tree.as_ref(), Some(&to_tree), Some(&mut opts))?` then
  `apply_find_similar`. For `compare_head_diff`: `opts = build_diff_options(&[])`, files =
  `collect_headers(&diff)?`. For `compare_head_file_diff`: `opts = build_diff_options(&pathspecs(path, orig_path))`,
  then `collect_file_diff(&diff)?.ok_or_else(|| AppError::Git(...))`.
- `to = { oid: to_commit.id().to_string(), summary: <first line> }`.

### 2.3 New commands — `src-tauri/src/commands.rs`
Follow the `get_commit_diff` / `get_commit_file_diff` pattern exactly: thin
`#[tauri::command]` + a runtime-free `_inner` that resolves `repo_path` then runs the pure fn
under `spawn_blocking`. Command names differ from the pure fns (house convention).

```rust
/// HEAD → `oid` tree comparison (P5 §1.2). Errors: noRepo | git.
#[tauri::command]
pub async fn compare_with_head(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
) -> Result<CompareDiff, AppError>;

/// Hunks for one file of the HEAD → `oid` comparison. Errors: noRepo | git.
#[tauri::command]
pub async fn compare_with_head_file_diff(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
    path: String,
    orig_path: Option<String>,
) -> Result<FileDiff, AppError>;
```
Update the `use crate::git::diff::{...}` line to import `compare_head_diff`,
`compare_head_file_diff`, `CompareDiff`, `CompareEndpoint`.

### 2.4 Registration — `src-tauri/src/lib.rs`
Add to `tauri::generate_handler![ … ]`:
```
            commands::compare_with_head,
            commands::compare_with_head_file_diff
```

---

## §3 IPC surface (TypeScript)

### 3.1 Types — `src/ipc/types.ts`
```ts
export interface CompareEndpoint {
  /** Full 40-char hex; "" when HEAD is unborn (old side). */
  oid: string;
  /** First line of that commit's message; "" when unborn. */
  summary: string;
}

export interface CompareDiff {
  /** OLD side = HEAD. */
  from: CompareEndpoint;
  /** NEW side = the right-clicked commit. */
  to: CompareEndpoint;
  /** Sorted path-ascending. Empty when from.oid === to.oid. Headers only. */
  files: FileDiffHeader[];
}
```

### 3.2 `IpcApi` methods — `src/ipc/types.ts`
```ts
  /** Tree-vs-tree diff between HEAD (old) and `oid` (new): `git diff HEAD <oid>`.
   *  HEAD is resolved server-side (detached ok; unborn -> empty old tree). Empty
   *  `files` when `oid` IS HEAD. Rejects {@link AppError} (`noRepo`, `git`). */
  compareWithHead(repoId: string, oid: string): Promise<CompareDiff>;
  /** Hunks for one file of the HEAD → `oid` comparison. `origPath`: pass the
   *  FileDiffHeader.origPath for renames. Rejects AppError (`noRepo`, `git`). */
  compareWithHeadFileDiff(
    repoId: string,
    oid: string,
    path: string,
    origPath: string | null,
  ): Promise<FileDiff>;
```

### 3.3 Real impl — `src/ipc/tauri.ts`
Add two wrappers mirroring `getCommitDiff` / `getCommitFileDiff` (same arg-key casing —
Tauri v2 maps camelCase JS keys to the snake_case Rust params):
```ts
  compareWithHead: (repoId, oid) => invoke('compare_with_head', { repoId, oid }),
  compareWithHeadFileDiff: (repoId, oid, path, origPath) =>
    invoke('compare_with_head_file_diff', { repoId, oid, path, origPath }),
```
Also re-export `CompareDiff` / `CompareEndpoint` through the `src/ipc/index.ts` barrel
(follow how `CommitDiff` is re-exported).

### 3.4 Mock — `src/ipc/mock.ts` + `src/ipc/fixtures/diffs.ts`
Import `CompareDiff` in `mock.ts`. Add two methods to `mockIpc`:

- `compareWithHead(repoId, oid)`: `await delay()`; `const state = requireRepo(repoId)`; build
  the active layout with the SAME routing block `getCommitDiff` uses (`20k` →
  `generateLayout20k()`, `detached` → `buildMockGraphDetached()`, else
  `prependCommits(buildMockGraph(), state.commits)`); `index = layout.nodes.findIndex(n => n.id === oid)`;
  `index === -1` → throw `{ kind: 'git', message: 'mock: unknown commit' }`. Return
  `structuredClone(mockCompareDiff(state.headOid, oid, index, layout))`.
- `compareWithHeadFileDiff(repoId, oid, path, origPath)`: `requireRepo`; return
  `structuredClone(mockCommitFileDiff(oid, path, origPath))` (reuse — a compare file diff has
  the same FileDiff shape; no new per-file fixture needed).

New fixture builder in `src/ipc/fixtures/diffs.ts`:
```ts
export function mockCompareDiff(
  fromOid: string,
  toOid: string,
  toIndex: number,
  layout: GraphLayout,
): CompareDiff;
```
Behavior:
- `fromSummary` = `layout.nodes.find(n => n.id === fromOid)?.summary ?? 'HEAD'`.
- `toSummary` = `layout.nodes[toIndex]?.summary ?? 'commit'`.
- If `fromOid === toOid` → `{ from, to, files: [] }` (drives the "No differences" state — a
  required harness path: right-click the top/HEAD row).
- Else reuse `mockCommitDiff(toIndex, toOid).files` for a believable header list, wrapped as
  `{ from: { oid: fromOid, summary: fromSummary }, to: { oid: toOid, summary: toSummary }, files }`.

---

## §4 Frontend — graph hit-testing

### 4.1 Pure pill-layout helper — `src/graph/draw.ts` (MANDATORY refactor)
There is currently NO pill hit-testing and pass 5a computes pill geometry inline. Extract a
single pure function that BOTH pass 5a and the hit-test consume, so geometry can never
diverge. Export `PillStyle` and `pillStyle` (currently private).

```ts
/** One laid-out pill in canvas CSS-px space. `ref` is null for the "+n"
 *  overflow chip (no ref → never a right-click target). */
export interface LaidPill {
  ref: RefLabel | null;
  style: PillStyle;   // resolved fill/text/border + already-truncated label
  x: number;          // left edge (same coord space as the summary column)
  w: number;          // pill width incl. padding
}

/** Column geometry shared by pass 5 and the hit-test. `startX` = left edge of
 *  the first pill; `budget` = the 40% overflow budget (§ M2 pass 5). */
export function pillArea(vpWidth: number, laneCount: number): { startX: number; budget: number };

/** Lays out one row's pills left-to-right with the existing overflow rule:
 *  break before a pill that would exceed `startX + budget` (except the first),
 *  then append a "+n" chip when any were hidden. Sets ctx.font internally.
 *  PURE (no drawing). */
export function layoutRowPills(
  ctx: CanvasRenderingContext2D,
  node: GraphNode,
  theme: Theme,
  startX: number,
  budget: number,
): LaidPill[];
```
- `pillArea`: `startX = gutter + min(laneCount, maxRenderLanes) * laneWidth + textGap`;
  `authorLeft = vpWidth - dateColWidth - colGap*2 - authorColWidth`;
  `budget = max(0, 0.4 * (authorLeft - startX))`. These are the CURRENT pass-5 formulas —
  move them here verbatim.
- `layoutRowPills`: replicate the current pass-5a loop EXACTLY (same `pillWidth`/
  `truncateToWidth`, same break condition `shown > 0 && x + w > startX + budget`, same
  unconditional trailing `+n` chip with `{ fill: bg2, text: text2, border, label: '+n' }`).
- **Refactor pass 5a** to call `layoutRowPills`, then paint each `LaidPill` (extract a
  `drawPillAt(ctx, x, cy, style, w)` from the existing `drawPill`, OR keep `drawPill` and have
  it accept a precomputed width). The post-pill advance (`x = lastPill.x + lastPill.w +
  pillGap + 8`, only when the row had pills) must match today's output — the M2b harness
  screenshot must be pixel-identical. Reviewer verifies no geometry duplication remains.

### 4.2 GraphCanvas — `src/graph/GraphCanvas.tsx`
New optional prop + discriminated union (exported):
```ts
export type GraphContextTarget =
  | { kind: 'ref'; ref: RefLabel }
  | { kind: 'commit'; index: number; oid: string };

export interface GraphCanvasProps {
  // …existing…
  /** Right-click on a ref pill or a commit row. Empty area / WIP row → not
   *  called (native menu suppressed regardless). clientX/clientY anchor the menu. */
  onContextMenu?(target: GraphContextTarget, clientX: number, clientY: number): void;
}
```
Add `onContextMenu={handleContextMenu}` to the `.graph-scroll` overlay div. Handler:
1. `e.preventDefault()` (always — suppress the browser's native menu over the graph).
2. `const scroller = scrollerRef.current; if null return;`
   `const rect = scroller.getBoundingClientRect(); const y = e.clientY - rect.top; const x = e.clientX - rect.left;`
   (the scroller overlays the canvas 1:1 and there is no horizontal scroll, so `x` is canvas
   CSS-px — the same space as `LaidPill.x`.)
3. Row: `const hit = hitTest(y, scroller.scrollTop, wipOffset, layout.nodes.length);`
   `if (hit === null || hit === 'wip') return;` (no menu over empty/WIP).
4. `const node = layout.nodes[hit];`
5. Pill: `const ctx = canvasRef.current?.getContext('2d'); const theme = themeRef.current;`
   if both present and `node.refs?.length`, `const { startX, budget } = pillArea(cssSizeRef.current.w, layout.laneCount);`
   `const pills = layoutRowPills(ctx, node, theme, startX, budget);`
   `const hitPill = pills.find(p => p.ref !== null && x >= p.x && x <= p.x + p.w);`
   if found → `onContextMenu?.({ kind: 'ref', ref: hitPill.ref! }, e.clientX, e.clientY); return;`
6. Else → `onContextMenu?.({ kind: 'commit', index: hit, oid: node.id }, e.clientX, e.clientY);`

Left-click (`handleClick`) selection behaviour is unchanged here — the compare-exit rule
lives in RepoWorkspace (§5.4).

---

## §5 Frontend — context menu + Compare panel

### 5.1 `ContextMenu` — new `src/components/ContextMenu.tsx`
Small reusable menu. All colors via CSS variables (`--bg-2`, `--border`, `--text-1`,
`--text-3`, `--selection`) so both themes work; no hard-coded hex.
```ts
export interface ContextMenuItem {
  label: string;
  onSelect(): void;
  disabled?: boolean;
}
export interface ContextMenuProps {
  x: number;            // clientX anchor
  y: number;            // clientY anchor
  items: ContextMenuItem[];
  onClose(): void;      // fired by every dismiss path AND after an item activates
}
```
Behaviour:
- Fixed-position (`position: fixed`) at `(x, y)`, then clamp into the viewport (shift left/up
  when it would overflow `window.innerWidth/innerHeight`).
- Dismiss on: outside `pointerdown`, `Escape`, `scroll` (capture), `resize`, window `blur`.
  Activating an enabled item calls `item.onSelect()` then `onClose()`.
- a11y (kept proportional to the app): `role="menu"`; items `role="menuitem"` as `<button>`;
  focus the first enabled item on mount; ArrowUp/ArrowDown move focus (skipping disabled);
  Enter/Space activate; Esc closes. Disabled items: `aria-disabled`, not activatable, muted
  styling.
- Mounted only while open (RepoWorkspace conditionally renders it). No portal required (fixed
  positioning is sufficient); it may render at the end of RepoWorkspace's tree.

### 5.2 RepoWorkspace menu state + item construction — `src/components/RepoWorkspace.tsx`
```ts
const [menu, setMenu] = useState<{ x: number; y: number; items: ContextMenuItem[] } | null>(null);
```
Pass to `GraphCanvas`: `onContextMenu={handleGraphContextMenu}`. Build items from the target,
then open ONLY if non-empty:
```
function handleGraphContextMenu(target, clientX, clientY) {
  const items = buildContextItems(target);
  if (items.length === 0) return;         // ref pill w/o valid actions → no menu
  setMenu({ x: clientX, y: clientY, items });
}
```
`buildContextItems(target)`:
- `target.kind === 'ref'`:
  - `const cur = headBranch?.name ?? null;` if `cur === null` → return `[]`.
  - `const r = target.ref;`
    - `r.kind === 'tag' || r.kind === 'head'` → return `[]`.
    - `r.kind === 'localBranch' && r.isHead` → return `[]` (current branch's own pill).
    - `r.kind === 'localBranch' (non-head) | 'remoteBranch'` → two items:
      - `{ label: `Merge ${r.name} into ${cur}`, disabled: mutating || opActive, onSelect: () => void handleMergeBranch(r.name) }`
      - `{ label: `Rebase ${cur} onto ${r.name}`, disabled: mutating || opActive, onSelect: () => void handleRebaseBranch(r.name) }`
- `target.kind === 'commit'`:
  - if `head === null || head.unborn` → return `[]` (Compare unavailable; §1.3).
  - else one item: `{ label: 'Compare with HEAD', disabled: false, onSelect: () => handleCompareWithHead(target.oid) }`.
    (Compare is a read-only view → not gated on `mutating`/`opActive`.)

Render at the end of the component tree:
```
{menu !== null && (
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />
)}
```

### 5.3 Compare right-panel mode — state + handlers
New state (mirrors the `commitDiff` cluster):
```ts
const [compare, setCompare] = useState<{ oid: string } | null>(null);
const [compareData, setCompareData] = useState<CompareDiff | null>(null);
const [compareLoading, setCompareLoading] = useState(false);
const [compareError, setCompareError] = useState<string | null>(null);
const compareReqId = useRef(0);
```
`handleCompareWithHead(oid)`:
1. `setMenu(null)`.
2. Collapse any open diff overlay that belongs to another mode: `fileDiffReqId.current += 1; setDiffSlot(null);`
3. `setCompare({ oid }); setCompareData(null); setCompareLoading(true); setCompareError(null);`
   `const id = ++compareReqId.current;`
4. `ipc.compareWithHead(repoId, oid).then(cd => { if (id !== compareReqId.current) return; setCompareData(cd); setCompareLoading(false); }, e => { … setCompareError(errorMessage(e)); setCompareLoading(false); });`

`clearCompare()`: `compareReqId.current += 1; setCompare(null); setCompareData(null); setCompareLoading(false); setCompareError(null);`
and if `diffSlotRef.current?.key.startsWith('compare:')` → `collapseDiffSlot()`.

`handleToggleCompareDiff(file: FileDiffHeader)` (mirrors `handleToggleCommitDiff`):
```
if (compare === null) return;
const key = `compare:${file.path}`;
if (diffSlotRef.current?.key === key) { collapseDiffSlot(); return; }
void fetchDiffSlot(key, () => ipc.compareWithHeadFileDiff(repoId, compare.oid, file.path, file.origPath));
```

Refresh coexistence: when `compare !== null`, the `repo-changed` / focus / `refreshAll`
paths SHOULD re-fetch `ipc.compareWithHead(repoId, compare.oid)` (HEAD may have moved). MUST:
a stale/removed commit oid must not crash — a `git`-error rejection → `clearCompare()` +
`pushToast('info', 'Compared commit is no longer in this repository')`. (Since `compare.oid`
is a full oid, no row-remap is needed across refetches.) Keep the re-fetch minimal; a plain
"clear compare on refresh" is an acceptable fallback if re-fetch proves fiddly — state the
choice in the PR.

### 5.4 Selection ↔ compare interaction
- Entering compare does **not** change `selectedIndex` (compare is independent state).
- Compare **takes precedence** in the right panel over CommitPanel/StatusPanel.
- Left-clicking any graph row exits compare: wrap the `onSelect` passed to `GraphCanvas`:
  ```
  onSelect={(i) => { if (compare !== null) clearCompare(); setSelectedIndex(i); }}
  ```
- Esc layering (extend the existing effect, in order): open diff overlay → `collapseDiffSlot()`;
  else `compare !== null` → `clearCompare()`; else selection → deselect. (Compare slots in
  between the overlay and selection layers.)

### 5.5 Right-panel render precedence — `RepoWorkspace.tsx`
```
compare !== null
  ? <ComparePanel … />
  : selectedIndex !== null && graph !== null
    ? <CommitPanel … />
    : <><StatusPanel …/><CommitBox …/></>
```
(The `OpBanner` above stays unconditionally.)

### 5.6 overlay meta — `src/components/RepoWorkspace.tsx` + `DiffOverlay.tsx`
- Add `'compare'` to `DiffOverlayMeta['kind']` (`DiffOverlay.tsx`) and
  `KIND_LABEL.compare = 'Compare'`.
- In `overlayMeta` (RepoWorkspace `useMemo`), add a branch BEFORE the workdir-section fallback:
  ```
  if (key.startsWith('compare:')) {
    const path = key.slice('compare:'.length);
    const file = compareData?.files.find(f => f.path === path) ?? null;
    return { path, origPath: file?.origPath ?? null, status: file?.status ?? null, kind: 'compare' };
  }
  ```
  Add `compareData` to the `useMemo` deps.

### 5.7 `ComparePanel` — new `src/components/ComparePanel.tsx`
Mirror `CommitPanel` (same `FileHeaderRow`, Tree/flat via `listView`, skeletons, `× close`).
```ts
export interface ComparePanelProps {
  data: CompareDiff | null;      // null while loading
  loading: boolean;
  error: string | null;
  /** HEAD branch name for the header ("HEAD (main)"); null when detached. */
  headBranchName: string | null;
  diffSlot: DiffSlot | null;     // keys = `compare:${path}`
  listView: ListView;
  onToggleDiff(file: FileDiffHeader): void;
  onClose(): void;
}
```
- Header ("old → new", direction explicit): title "Comparing"; a two-endpoint row —
  `HEAD${headBranchName ? ` (${headBranchName})` : ''} · ${shortOid(data.from.oid)}` →
  `${shortOid(data.to.oid)} · ${data.to.summary}`. Use `mono` for oids; `title` attrs carry
  full oids.
- When `data !== null && data.files.length === 0` (incl. HEAD-vs-itself) → render a clear
  **"No differences"** empty state instead of the file list.
- File list: same as `CommitPanel` but `leafKey`/keys use the `compare:` prefix and
  `onToggle` calls `onToggleDiff(file)`.
- Wire in RepoWorkspace: `headBranchName={headBranch?.name ?? null}`,
  `onToggleDiff={handleToggleCompareDiff}`, `onClose={clearCompare}`.

---

## §6 Acceptance criteria

### AI gate (orchestrator-verifiable)
1. `cargo check` + `cargo clippy` clean; `pnpm build` + `tsc` clean.
2. Rust tests in `diff.rs` on a scratch repo (git2-init + identity, like the existing tests):
   - `compare_head_diff(HEAD_oid, other_oid).files` matches `git diff --name-status HEAD <other>`
     (path set + statuses) for a linear history and a branch-tip pair.
   - HEAD-vs-itself (`to_oid == HEAD oid`) → `files` empty, `from.oid == to.oid`, no error.
   - Unborn HEAD → `from == {"",""}`, all files `Added` (compare-vs-empty-tree).
   - Bad/non-commit oid → `AppError::Git`.
   - `compare_head_file_diff` hunks match `git diff HEAD <other> -- <path>` for one file
     (reuse the normalize/rename assertions style of the M4 tests).
   - `commands.rs`: `compare_with_head_inner` / `compare_with_head_file_diff_inner` return
     `NoRepo` for an unknown id (extend the existing `_require_an_open_repo` tests).
3. Browser harness (`pnpm dev`, `VITE_MOCK_IPC=1`), both dark + light themes:
   - Right-click a `main`/`feat`/`origin/main` pill → menu with the two correctly-worded items;
     clicking Merge/Rebase fires the existing handler (toast appears, graph updates).
   - Right-click a tag pill / the HEAD pill / the current branch's own pill → **no menu**.
   - Right-click a commit row → "Compare with HEAD" → ComparePanel shows a file list; clicking a
     file opens the `compare:` DiffOverlay with correct content.
   - Right-click the top (HEAD) row → Compare shows the **"No differences"** state.
   - Esc layering: overlay → compare → selection closes in that order. Left-click a row exits
     compare.
   - `?fixture=detached` tab: Compare header shows `HEAD <shortOid>` (no branch name).
   - Menu dismisses on outside-click, Esc, and scroll; keyboard arrows/Enter work.
   - The M2b static-graph screenshot is unchanged after the pass-5a refactor (pill rendering
     pixel-identical).

### USER CHECKPOINT (native `pnpm tauri dev`)
- Right-click a branch pill in the graph → Merge/Rebase runs against the real repo.
- Right-click a commit → "Compare with HEAD" shows the correct `git diff HEAD <commit>` file
  list and per-file diffs; the two-endpoint header reads correctly; HEAD-vs-itself shows
  "No differences".

---

## §7 Suggested decomposition (implement → review → commit)

- **P5a — Backend compare.** `diff.rs` types + `compare_head_diff`/`compare_head_file_diff`
  + Rust tests; `commands.rs` two commands + inners + NoRepo tests; `lib.rs` handler wiring.
  Gate: cargo test/clippy green.
- **P5b — IPC + mock.** `types.ts` (`CompareEndpoint`, `CompareDiff`, two `IpcApi` methods),
  `tauri.ts` wrappers, `index.ts` re-exports, `mock.ts` two methods + `mockCompareDiff`
  fixture. Gate: `tsc`/build green; harness console can call the mock.
- **P5c — Graph hit-test + menu + merge/rebase.** `draw.ts` `pillArea`/`layoutRowPills` +
  pass-5a refactor (+ export `PillStyle`/`pillStyle`); `GraphCanvas.tsx` `GraphContextTarget`
  + `onContextMenu`; new `ContextMenu.tsx`; RepoWorkspace `menu` state + `buildContextItems`
  wiring the EXISTING merge/rebase handlers. Gate: harness Part-1 checks + unchanged M2b
  screenshot.
- **P5d — Compare panel + overlay.** RepoWorkspace compare state/handlers + Esc/selection
  interaction + render precedence + overlay-meta branch; `DiffOverlay` kind; new
  `ComparePanel.tsx`. Gate: harness Part-2 checks.

---

## §8 Open items to FLAG for the orchestrator
- **Direction (§1.2)** is locked to old=HEAD/new=selected but the user may flip it. If flipped,
  only `compare_head_diff` (swap trees + endpoints) and the ComparePanel header change; the IPC
  shape (two labelled endpoints) already supports either direction with no wire change.
- **Compare-on-refresh (§5.3):** re-fetch vs clear. Recommend re-fetch with graceful clear on
  `git` error; falling back to plain clear is acceptable for v1 — record the choice in the PR.
