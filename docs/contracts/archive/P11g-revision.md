# P11g-revision — DiffBrowser rework (Changes A–D)

> Addendum to `docs/contracts/P11-followup.md` §6. Supersedes §6.2 (DiffBrowser owning
> an internal tree + scope), §6.4 (IntersectionObserver loader), and §6.5 (mount/entry) where
> they conflict with this file. Everything else in §6 (lazy per-file strategy §6.1, scope-filter
> semantics §6.3, binary/tooLarge/error handling) stands.
>
> senior-dev implements strictly to the signatures below. NO Rust/IPC change (see §8). Mock must
> keep compiling. Files touched: `src/components/DiffBrowser.tsx`, `src/components/DiffFileTree.tsx`
> (NEW), `src/components/ComparePanel.tsx`, `src/components/CommitPanel.tsx`,
> `src/components/RepoWorkspace.tsx`, `src/styles.css`, `src/utils/pathTree.ts` (§9).

## §0 The four locked user decisions (do not re-litigate)

- **A** — DiffBrowser loses its internal left tree. It becomes ONLY: header + a vertical scroll of
  stacked per-file `DiffCard`s. It fills the graph/main pane.
- **B** — The right-hand "Changes" pane (ComparePanel/CommitPanel) is the SOLE scope navigator.
  Its file list becomes a shared `DiffFileTree` (extracted from DiffBrowser) that drives a lifted
  `scope`. Root click → all files; folder click → that subtree; file click → that file. Selected
  scope is visually indicated in that tree.
- **C** — In COMPARE mode the DiffBrowser auto-opens over the graph as soon as `compareData` has
  loaded (root scope, no click). In COMMIT mode it stays closed until the user clicks a
  root/folder/file in the CommitPanel tree (explicit open; NOT on every commit selection). This
  asymmetry is intentional.
- **D** — The lazy loader must NOT depend on `IntersectionObserver`/visibility (it does not fire in
  the user's WebView2 and is suspended by `document.hidden=true` in the browser harness). On mount
  and on every scope change, immediately enqueue the current scope's non-binary files into the
  existing bounded queue (`MAX_CONCURRENCY = 4`), draining top-to-bottom.

---

## §1 `DiffBrowser` — header + stacked scroll only (Change A + D)

`scope` is LIFTED to `RepoWorkspace` and passed in. DiffBrowser filters its files by the passed-in
`scope` prop internally (keep the existing `visibleFiles` memo, sourcing scope from props). It
exposes NO internal file-tree navigator — the sole scope navigator is the shared `DiffFileTree` in
ComparePanel/CommitPanel (§2/§3); that removal is what §1 is really about.

It DOES take a `listView` prop, whose SOLE purpose is to ORDER the stacked cards so they match the
right-hand Changes-panel tree. DiffBrowser renders no tree from it; it only sorts. See §1.1
`orderedFiles`.

### 1.1 New props

```ts
// DiffScope now lives in and is re-exported from DiffFileTree.tsx (§2); import it here.
import type { DiffScope } from './DiffFileTree';

export interface DiffBrowserProps {
  repoId: string;
  /** Which commands to call + how to label the header (unchanged from §6.2). */
  source:
    | { mode: 'commit'; oid: string; title: string }
    | { mode: 'compare'; oid: string; fromLabel: string; toLabel: string };
  /** Header list for the active source (RepoWorkspace: CommitDiff.files / CompareDiff.files). */
  files: FileDiffHeader[];
  /** Current scope (lifted to RepoWorkspace). Drives which cards render. */
  scope: DiffScope;
  /**
   * Card-ordering ONLY (matches the right-pane Changes tree). NOT a tree toggle inside the
   * overlay — DiffBrowser has no internal tree. 'tree' → dirs-first pre-order; else raw `files`.
   */
  listView: ListView;
  onClose(): void;
}
export function DiffBrowser(props: DiffBrowserProps): JSX.Element;
```

`orderedFiles` — compute BEFORE scope filtering so the stacked cards sit in the same order as the
Changes-panel tree:

```ts
// 'tree' → dirs-first tree pre-order (same leaf order the right-pane DiffFileTree shows);
// otherwise → the raw `files` order as received.
const orderedFiles = useMemo(
  () => (listView === 'tree' ? flattenTreeLeaves(buildPathTree(files, (f) => f.path)) : files),
  [files, listView],
);
```

`visibleFiles` then filters `orderedFiles` (not the raw `files`) by the `scope` prop (§6.3 filter).
`flattenTreeLeaves` is the new pure helper in `src/utils/pathTree.ts` (§9).

REMOVED vs current: `initialScope`, the internal `const [scope, setScope]` state,
`DiffFileTree`/`DiffTreeNodes`/`DiffTreeFileRow` (moved to §2), the `observer` state, the
IntersectionObserver effect, and the `data-diff-path` observe wiring. NOTE: `listView` is NOT
removed — it is retained for card ordering only (see above).

### 1.2 Kept unchanged

- Component-local cache `cacheRef: Map<string, CardState>` keyed `` `${source.oid}:${path}` ``, the
  `bump` reducer, `queueRef`, `inFlightRef`, `MAX_CONCURRENCY = 4`, the `pump` callback,
  `sourceRef`/`filesRef`/`repoIdRef`, `cancelledRef` unmount guard, `enqueue` (idempotent per cache
  key), `retry`.
- `visibleFiles` memo (§6.3 filter), now reading the `scope` PROP and filtering `orderedFiles`
  (§1.1) instead of the raw `files` array.
- The header markup (`.diff-browser-header`, title, `×` close → `onClose`).
- `DiffCard` / `DiffCardBody` rendering: idle/loading → `SkeletonRows`; ready → `<DiffView>`;
  error → inline banner + Retry; binary short-circuit (never fetched); `DiffView` owns
  tooLarge/binary/empty-hunks/`Applied` placeholders.

### 1.3 The loader change (Change D — precise)

Delete the IntersectionObserver entirely: the `observer` state, its `useEffect`, `setObserver`, the
`observer` prop on `DiffCard`, the `DiffCard` observe/unobserve effect, its `ref`, and the
`data-diff-path` attribute. `DiffCard` no longer needs a `ref`/`observer`; it renders purely from
its `entry`/`header`.

Add a single mount-and-scope-change enqueue effect:

```ts
// Change D: no visibility events. Eagerly enqueue every non-binary file in the
// current scope (top-to-bottom order == visibleFiles order == orderedFiles order),
// so the first cards paint first. `enqueue` is idempotent per cache key, so
// re-running on scope change never double-fetches already-loaded/queued files;
// narrowing to a folder/file simply enqueues fewer.
useEffect(() => {
  for (const f of visibleFiles) {
    if (!f.binary) enqueue(f.path);
  }
  pump(); // §9 bug 1: resume any drain that stalled across a StrictMode remount.
}, [visibleFiles, enqueue, pump]);
```

`enqueue` stays `[pump]`-stable; `pump` stays `[]`-stable — the effect's real dep is `visibleFiles`.

`DiffCard`'s `entry` prop is still read from `cacheRef.current.get(\`${source.oid}:${f.path}\`)`
during render; each `pump` resolution `bump()`s so the card repaints.

**Tradeoff (state it in a code comment):** at root scope this issues one bounded (max 4 in-flight)
fetch per file. The per-file `MAX_FILE_DIFF_LINES = 5000` cap keeps each response cheap, and scoping
to a folder/file naturally reduces load. Acceptable at desktop-repo scale (the user's 125-file case
is fine); a batched command remains a future additive optimization, out of scope here.

---

## §2 `src/components/DiffFileTree.tsx` — NEW shared scope navigator (Change B)

Extract the current `DiffFileTree` + `DiffTreeNodes` + `DiffTreeFileRow` from `DiffBrowser.tsx`
verbatim into this new module, plus the `BADGES` map they need (self-contained; DiffBrowser keeps
its own `BADGES` for card headers — matches the existing CommitPanel/DiffBrowser duplication).
Export `DiffScope` from here (it is the canonical home now; DiffBrowser and RepoWorkspace import it).

```ts
export type DiffScope =
  | { kind: 'root' }
  | { kind: 'dir'; prefix: string }   // TreeDir.fullPrefix (no trailing '/')
  | { kind: 'file'; path: string };

export interface DiffFileTreeProps {
  files: FileDiffHeader[];
  listView: ListView;
  scope: DiffScope;
  onSelect(scope: DiffScope): void;
}
export function DiffFileTree(props: DiffFileTreeProps): JSX.Element;
```

Behavior (identical to the current internal component — this is a move, not a rewrite):

- Renders a top **root button** ("All files" + total count); `diff-tree-selected` when
  `scope.kind === 'root'`; click → `onSelect({ kind: 'root' })`.
- `listView === 'tree'` → `buildPathTree(files, f => f.path)` structure. Each **dir row** has an
  independent chevron button (collapse/expand, local `useState<Set<string>>` on `fullPrefix`, NOT
  tied to selection) AND a separate name button → `onSelect({ kind:'dir', prefix: node.fullPrefix })`;
  `diff-tree-selected` when `scope.kind==='dir' && scope.prefix===fullPrefix`. Each **leaf** →
  `onSelect({ kind:'file', path })`; selected when `scope.kind==='file' && scope.path===path`.
- `listView === 'list'` → flat `.diff-tree-flat` of file rows, same file-click selection.
- Collapse/expand state is ephemeral and independent of scope selection (keep exactly as-is).

Rationale (already flagged in §6.2/§8.3): the shared `Tree.tsx` binds a dir click to
collapse/expand and only exposes double-click `onActivateDir`, so it cannot express single-click
select-folder-as-scope. `DiffFileTree` reuses `buildPathTree` for STRUCTURE only. Do NOT modify
`Tree.tsx`.

---

## §3 `ComparePanel` / `CommitPanel` — render `DiffFileTree` (Change B)

Both drop `onViewAll` and `onOpenFile`; both drop the `Tree` + `FileHeaderRow`-list + "View all
changes" button; both render `<DiffFileTree>` as their file list, driven by a lifted `scope`.

**`onViewAll` decision:** DROP it in BOTH modes. The `DiffFileTree` root button ("All files")
already IS the select-all affordance, and compare auto-opens anyway. Keep the static
`Changes (N)` section-label header (no button) for context.

### 3.1 ComparePanel

```ts
export interface ComparePanelProps {
  data: CompareDiff | null;
  loading: boolean;
  error: string | null;
  headBranchName: string | null;
  listView: ListView;
  /** P11g-rev: current diff scope (selection highlight) + its setter. */
  scope: DiffScope;
  onSelectScope(scope: DiffScope): void;
  onClose(): void;
}
```

Replace the `data.files.length > 0` block's `<Tree>`/`<ul>` with:

```tsx
<section className="status-section commit-files">
  <div className="section-header section-label"><span>Changes ({data.files.length})</span></div>
  <DiffFileTree files={data.files} listView={listView} scope={scope} onSelect={onSelectScope} />
</section>
```

Keep the "No differences" empty state and the loading skeleton unchanged.

### 3.2 CommitPanel

Same edit: props lose `onViewAll`/`onOpenFile`, gain `scope: DiffScope` + `onSelectScope(scope)`.
Keep `node`, `data`, `loading`, `error`, `listView`, `onSelectParent`, `onClose`, and all the
commit-details/message-body markup. Replace the `commit-files` section's `<Tree>`/`<ul>` with the
same `<DiffFileTree ... />` block (header text `Changes ({data.files.length})`, no button).

`FileHeaderRow` and `SkeletonRows` remain exported from `CommitPanel.tsx` (DiffBrowser and
StatusPanel still import `SkeletonRows`; StatusPanel still uses `FileHeaderRow`).

---

## §4 `RepoWorkspace` — lifted scope, compare auto-open, commit explicit-open (Changes B + C)

### 4.1 State (replaces the current `diffBrowser` object at ~178–189)

```ts
import type { DiffScope } from './DiffFileTree';

// P11g-rev: ONE lifted scope drives BOTH the right-pane DiffFileTree highlight
// AND the DiffBrowser's visible cards. Reset to root whenever the active source
// (compare target / selected commit) changes.
const [scope, setScope] = useState<DiffScope>({ kind: 'root' });
// Commit mode ONLY: explicit-open flag (compare mode auto-opens, needs no flag).
const [commitBrowserOpen, setCommitBrowserOpen] = useState(false);
const commitBrowserOpenRef = useRef(commitBrowserOpen);
commitBrowserOpenRef.current = commitBrowserOpen;
```

Delete the `diffBrowser` state, `diffBrowserRef`, `openCommitBrowser`, `openCompareBrowser`.

### 4.2 Scope reset on source change

```ts
// Reset scope + close the commit browser whenever the active source changes
// (new compare target, or a different commit selected). Compare auto-open then
// renders at root; commit mode returns to closed.
useEffect(() => {
  setScope({ kind: 'root' });
  setCommitBrowserOpen(false);
}, [compare?.oid, selectedIndex]);
```

### 4.3 Right-pane wiring (~1658–1680)

- ComparePanel: `scope={scope}` and `onSelectScope={setScope}` (compare browser is already open;
  clicking just refilters). Remove `onViewAll`/`onOpenFile`.
- CommitPanel: `scope={scope}` and
  `onSelectScope={(s) => { setScope(s); setCommitBrowserOpen(true); }}` (clicking opens + scopes).
  Remove `onViewAll`/`onOpenFile`.
- `listView` continues to be passed to both panels AND (§4.5) to `DiffBrowser`, so the overlay's
  card order matches the panel tree.

### 4.4 DiffBrowser source memo (replaces `diffBrowserView` ~1497–1520)

```ts
const diffBrowserView = useMemo(() => {
  // Compare mode: AUTO-OPEN once data has loaded and there is at least one file.
  if (compare !== null && compareData !== null && compareData.files.length > 0) {
    const fromLabel = `HEAD${headBranch?.name != null ? ` (${headBranch.name})` : ''}`;
    const toLabel = `${shortOid(compareData.to.oid)} · ${compareData.to.summary}`;
    return {
      source: { mode: 'compare' as const, oid: compare.oid, fromLabel, toLabel },
      files: compareData.files,
      onClose: clearCompare, // × in compare mode exits compare (compare IS the diff)
    };
  }
  // Commit mode: EXPLICIT-open only.
  if (selectedIndex !== null && graph !== null && commitBrowserOpen && commitDiff !== null) {
    const oid = graph.nodes[selectedIndex].id;
    return {
      source: { mode: 'commit' as const, oid, title: `${shortOid(oid)} · ${commitDiff.details.summary}` },
      files: commitDiff.files,
      onClose: () => setCommitBrowserOpen(false),
    };
  }
  return null;
}, [compare, compareData, selectedIndex, graph, commitBrowserOpen, commitDiff, headBranch, clearCompare]);
```

Empty-compare note: when `compareData.files.length === 0` the browser does NOT auto-open — the graph
stays visible and ComparePanel shows "No differences".

### 4.5 DiffBrowser render (~1632–1641)

```tsx
{diffBrowserView !== null && (
  <DiffBrowser
    key={`${diffBrowserView.source.mode}:${diffBrowserView.source.oid}`}
    repoId={repoId}
    source={diffBrowserView.source}
    files={diffBrowserView.files}
    scope={scope}
    listView={listView}
    onClose={diffBrowserView.onClose}
  />
)}
```

The `key` on `source.oid` makes a DIFFERENT compare target / commit remount fresh (clears the
per-file cache + queue); a refetch of the SAME `compare.oid` (refetchCompare) keeps the same key, so
the cache survives a repo refresh. `listView` is passed for card ordering only (§1.1); no
`initialScope` prop.

### 4.6 Graph `onSelect` (~1611–1617)

```tsx
onSelect={(i) => {
  if (compare !== null) clearCompare();
  setSelectedIndex(i);
  // scope reset + commit-browser close handled by the §4.2 effect (selectedIndex dep).
}}
```

Selecting a new commit does NOT auto-open the browser (Change C asymmetry): the effect closes it and
resets scope; the CommitPanel then shows the summary, and the user opens the overlay by clicking in
the tree.

### 4.7 Esc layering (~1384–1397) + clearCompare

New order (topmost first):

```ts
if (commitBrowserOpenRef.current) { setCommitBrowserOpen(false); return; }  // commit overlay
if (diffSlotRef.current !== null) { collapseDiffSlot(); return; }           // workdir single-file
if (compareRef.current !== null)  { clearCompare();     return; }           // compare (also closes its auto-open browser)
setSelectedIndex((cur) => (cur !== null ? null : cur));                     // deselect commit
```

`clearCompare` (~280): drop the old `if (diffBrowserRef.current?.mode === 'compare') setDiffBrowser(null)`
line — the compare browser is now derived from `compare`/`compareData`, so setting `compare = null`
closes it automatically. Add `setCommitBrowserOpen(false); setScope({ kind: 'root' })` is NOT needed
here (the §4.2 effect covers selection changes; clearCompare is compare-only).

---

## §5 CSS deltas (`src/styles.css`, `.diff-browser` block ~1615)

- **Remove** `.diff-browser-body` and `.diff-browser-tree` (the two-column flex layout no longer
  exists).
- **`.diff-browser`** stays `position:absolute; inset:0; z-index:6; display:flex; flex-direction:column`.
  Its children are now just `.diff-browser-header` and `.diff-browser-scroll`.
- **`.diff-browser-scroll`** becomes the pane-filling column child: `flex:1; min-height:0;
  overflow-y:auto; padding:12px; display:flex; flex-direction:column; gap:12px` (drop the
  `min-width:0` row-child leftover; keep the rest).
- **Keep** all `.diff-tree*` rules (`.diff-tree`, `.diff-tree-root(:hover)`, `.diff-tree-root-label`,
  `.diff-tree-count`, `.diff-tree-dir-row`, `.diff-tree-chevron`, `.diff-tree-dir-name-btn`,
  `.diff-tree-file`, `.diff-tree-flat`, `.diff-tree-selected`) — they now style the right-pane
  navigator. Re-comment the block header from "Left column: the single-click file tree" to
  "Right-pane scope navigator (DiffFileTree)".
- Add if needed for right-pane fit: `.commit-files .diff-tree { padding: 0; }` and ensure
  `.diff-tree-file`/`.diff-tree-root` width:100% within the narrower right column (they already are).
  No fixed 260px width — the tree flows in the right panel.
- `.diff-card*` rules unchanged.

---

## §6 Acceptance criteria — AI gate (browser harness, `VITE_MOCK_IPC=1`)

- **KEY regression:** enter compare mode → the stacked DiffBrowser auto-opens over the graph and the
  FIRST file's actual hunk lines are visible on first paint, with NO scrolling and NO visibility
  event (the harness preview runs `document.hidden=true`). Screenshot must show real diff content,
  not just skeletons. (This is the exact bug Change D fixes.)
- Clicking the root ("All files"), a folder, and a file in the right-hand Changes pane refilters the
  DiffBrowser to all / that subtree / that one file; the clicked node shows the selected style;
  already-loaded files are not refetched on scope change.
- No left tree remains inside the overlay (`.diff-browser-tree`/`.diff-browser-body` gone; overlay =
  header + single scroll column).
- Stacked-card order matches the right-pane Changes tree in BOTH `listView` modes (§9 bug 2): in
  'tree' view the cards follow the dirs-first tree pre-order; in flat/'list' view they follow the
  raw `files` order.
- Commit mode: selecting a commit shows CommitPanel but NO overlay until a root/folder/file click in
  its tree; the × / Esc closes the overlay back to the summary; selecting a different commit closes
  it and resets scope.
- Compare × / Esc exits compare mode (overlay closes with it).
- Binary files show the placeholder without a fetch; per-file error shows Retry.
- `pnpm build` / `tsc` clean; `src/ipc/mock.ts` compiles; harness renders in a plain browser.

## §7 USER CHECKPOINT (native `pnpm tauri dev`, not self-declarable)

Open a real large comparison (the user's ~125-file case): the diff auto-opens and files actually
POPULATE (no infinite loading) in WebView2, and the stacked scroll feels smooth.

## §8 Backend / IPC

NO Rust, command, event, channel, `types.ts`, `tauri.ts`, or `mock.ts` wire change. The loader
reuses the existing `compareWithHeadFileDiff(repoId, oid, path, origPath)` and
`getCommitFileDiff(repoId, oid, path, origPath)` per-file commands (P11-followup §6.1 stands).
This revision is purely a frontend interaction/loader rework. If senior-dev finds the mock does not
already serve those two per-file commands, that is a pre-existing gap to flag — this contract adds
no new surface.

---

## §9 Post-revision fixes (2026-07-30)

Two bugs reported after P11g-revision landed in the all-files DiffBrowser, now fixed. This section
records what shipped so the contract matches the code; §1 above was reconciled to keep `listView`.

### 9.1 Bug 1 — files past the first ≤4 stuck in the loading skeleton forever

Root cause: under `React.StrictMode`, DiffBrowser mounts → unmounts → remounts. The `cancelledRef`
unmount guard effect only set `cancelledRef.current = true` on cleanup and never reset it, so after
the remount the ref stayed `true`, permanently short-circuiting `pump()`/`enqueue()`. The first
in-flight batch (≤ `MAX_CONCURRENCY` = 4) that started before cleanup could resolve, but no further
files were ever drained.

Fix:
- The guard effect now resets `cancelledRef.current = false` on (re)mount and sets it `true` on
  cleanup (was cleanup-only).
- The eager-enqueue effect (§1.3) calls `pump()` at the end, with `pump` in its dependency array, to
  resume a drain that stalled across the remount.

### 9.2 Bug 2 — stacked diff order did not match the right-hand Changes panel

Root cause: DiffBrowser rendered cards in raw `files` order while the Changes panel showed a
dirs-first tree, so the two lists disagreed in 'tree' view.

Fix:
- New pure helper `flattenTreeLeaves()` in `src/utils/pathTree.ts` — walks a `buildPathTree` result
  and returns its leaves in dirs-first pre-order (the same order the `DiffFileTree` renders):

  ```ts
  // src/utils/pathTree.ts
  export function flattenTreeLeaves<T>(tree: PathTree<T>): T[];
  ```

- DiffBrowser now takes the `listView` prop (§1.1) and computes
  `orderedFiles = listView === 'tree' ? flattenTreeLeaves(buildPathTree(files, f => f.path)) : files`,
  then filters `orderedFiles` by `scope` for `visibleFiles`. `listView` is used ONLY for this
  ordering — DiffBrowser still has no internal file-tree navigator.
- `RepoWorkspace` passes `listView` through to `DiffBrowser` (§4.5), the same value it already passes
  to ComparePanel/CommitPanel, so the overlay order tracks the panel tree.
