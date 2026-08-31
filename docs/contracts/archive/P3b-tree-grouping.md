# P3b — Tree-Grouped Sidebar & Status Lists: Implementation Contract

Status: authoritative for P3b. Scope: display-only tree grouping of (a) Sidebar branches /
remotes / tags by `/` namespace and (b) StatusPanel + CommitPanel file lists by directory,
plus ONE additive persisted setting (`listView`, default `tree`) with a global header toggle.
Builds on `P2-followups.md` (UiSettings/patch IPC pattern — reused verbatim, no new commands),
`P3a` diff-overlay (diffSlot-driven expanded rows must keep working on tree leaves),
`M3`/`M4`/`M5` row semantics (entryPaths rename expansion, stage/unstage, checkout/delete).

Invariants (unchanged): Rust owns Git logic + graph layout; **grouping is pure display math in
TS** — backend keeps returning flat `BranchesSnapshot`/`StatusSnapshot`/`CommitDiff` untouched.
`src/ipc/mock.ts` updated with the IPC change. No new dependencies, no state library.
`pnpm build` green after every sub-increment; `cargo check`/`cargo test` green for P3b-1.

---

## 1. Scope split (sub-increments — each a fresh-context senior-dev pass)

| # | Increment | Contents |
|---|---|---|
| 1 | **P3b-1** | `listView` setting end-to-end: `settings.rs` field + tests, `types.ts`, `tauri.ts` (no change beyond types), `mock.ts`, App state + header toggle button + CSS for the button. No tree rendering yet — toggle flips state that nothing consumes (harmless). |
| 2 | **P3b-2** | `src/utils/pathTree.ts` (builder) + `src/components/Tree.tsx` (renderer) + tree CSS; wire into **StatusPanel** and **CommitPanel** file lists behind `listView`. |
| 3 | **P3b-3** | Wire **Sidebar** (local branches, remotes, tags) onto the same Tree; harness verification pass. |

P3b-2 and P3b-3 both read §3–§5 plus their own section. P3b-1 is independent and lands first
so 2/3 can consume `listView` as an existing prop.

---

## 2. P3b-1 — the `listView` setting

### 2.1 Rust — `src-tauri/src/settings.rs` (the ONLY Rust change in P3b)

```rust
/// Flat vs tree-grouped list rendering for sidebar refs and file lists
/// (P3b contract §2). Pure UI preference; display-only, no Git effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListView {
    #[default]
    Tree,
    Flat,
}

// Settings struct gains (additive; container-level #[serde(default)] already
// covers it — SETTINGS_VERSION stays 1 per the P2 precedent documented on the
// Settings doc comment):
//   pub list_view: ListView,
// Settings::default() gains: list_view: ListView::default(),

// UiSettings (commands.rs or wherever it lives today) gains:
//   pub list_view: ListView,
// UiSettingsPatch gains:
//   pub list_view: Option<ListView>,
// get_ui_settings/set_ui_settings need no signature change — patch application
// gains one `if let Some(v) = patch.list_view { s.list_view = v; }` line.
```

Wire format addition: `"listView": "tree"` (default) / `"flat"`.

Rust tests (append to `settings.rs` `mod tests`, same style as P2):
1. `list_view_roundtrips_both_variants` — save/load `Tree` and `Flat`; raw JSON contains
   `"listView": "tree"` / `"listView": "flat"`.
2. `old_settings_file_without_list_view_loads_default` — extend (or mirror) the existing
   legacy-JSON test: a file without `listView` loads `ListView::Tree`.
3. Patch partiality: patching only `list_view` leaves `theme`/`pane_widths` untouched (extend
   the existing patch test if one exists in commands-adjacent code, else pure-fn test).

### 2.2 TypeScript — `src/ipc/types.ts`

```ts
export type ListView = 'tree' | 'flat';
// UiSettings gains:      listView: ListView;
// UiSettingsPatch gains: listView?: ListView;
```

`src/ipc/tauri.ts`: no change (generic invoke wrappers already pass the patch through).

### 2.3 Mock — `src/ipc/mock.ts`

- `DEFAULT_UI_SETTINGS` gains `listView: 'tree'`.
- `readUiSettings()` gains `const listView: ListView = parsed.listView === 'flat' ? 'flat' : 'tree';`
  (same corrupt-tolerant shape as `theme`).
- `setUiSettings` merge gains `listView: patch.listView ?? current.listView`.

### 2.4 App state + toggle UI (`App.tsx`, `styles.css`)

```ts
const [listView, setListView] = useState<ListView>('tree');
// Mount load (§3.3 of P2 — the existing single getUiSettings call): setListView(s.listView).
const toggleListView = useCallback(() => {
  const next: ListView = listView === 'tree' ? 'flat' : 'tree';
  setListView(next);
  void ipc.setUiSettings({ listView: next }).catch((e) =>
    pushToast('error', `Could not save list view: ${errorMessage(e)}`));
}, [listView, pushToast]);
```

- **One global toggle**, header bar, next to the theme toggle (same 32×32 icon-button recipe,
  class `.list-view-toggle`). Glyph shows the mode you'd SWITCH TO (matches the theme-toggle
  convention): `☰` ("Switch to flat lists") when `listView === 'tree'`, `𝌆`-style tree glyph —
  use text `⌥` is unclear; **use `☰` for flat and `⋔` for tree**; if either glyph renders poorly
  in Segoe UI, an inline 12×12 SVG (three stacked lines / indented fork) is acceptable —
  senior-dev's call, `title` + `aria-label` are the contract, the glyph is not.
- `listView` is passed as a prop to `Sidebar`, `StatusPanel`, `CommitPanel` (added in P3b-1 and
  ignored until P3b-2/3 consume it, OR added in 2/3 — senior-dev may defer the prop plumbing to
  the consuming increments; the toggle + persistence must land in P3b-1 either way).
- No keyboard shortcut (same rationale as the theme toggle, P2 §4.3). Add the button to
  `ShortcutOverlay` only if it gains one later — not now.

### 2.5 P3b-1 acceptance

`cargo test` green (new tests §2.1), `cargo clippy -- -D warnings`, `pnpm build` green.
Harness: toggle button flips and persists across reload (mock localStorage roundtrip);
`settings.json`-equivalent mock value shows `"listView"`.

---

## 3. `src/utils/pathTree.ts` — generic path-tree builder (P3b-2)

### 3.1 Types & signature (exact)

```ts
export interface TreeLeaf<T> {
  kind: 'leaf';
  /** Basename (segment after the last '/'), display-only. */
  name: string;
  /** The full original path — stable React key material for callers. */
  path: string;
  item: T;
}

export interface TreeDir<T> {
  kind: 'dir';
  /** Display label. After chain-collapsing this may contain '/'
   *  (e.g. "src/git" for a collapsed src -> git chain). */
  name: string;
  /** Full prefix from the root INCLUDING trailing content up to this dir,
   *  WITHOUT a trailing '/' (e.g. "src/git"). Unique within one tree —
   *  used as the React key and the collapse-state key. */
  fullPrefix: string;
  children: TreeNode<T>[];
}

export type TreeNode<T> = TreeLeaf<T> | TreeDir<T>;

/**
 * Splits each item's path on '/' into a nested tree, collapses single-child
 * directory chains, and sorts deterministically (§3.3). Pure; O(n · depth).
 * Items whose paths are duplicated produce duplicate leaves (callers'
 * status lists never contain duplicates within one section — not defended).
 * Empty segments from leading/trailing/double slashes are skipped defensively.
 */
export function buildPathTree<T>(
  items: readonly T[],
  getPath: (item: T) => string,
): TreeNode<T>[];
```

### 3.2 Builder pseudocode (normative)

```
buildPathTree(items, getPath):
  root = { childrenByName: Map, leaves: [] }            // transient mutable shape
  for item in items:
    segments = getPath(item).split('/').filter(s => s !== '')
    if segments.length == 0: continue                    // defensive; never expected
    node = root
    for seg in segments[0 .. len-2]:
      node = node.childrenByName.getOrInsert(seg, new dir node)
    node.leaves.push({ kind:'leaf', name: segments.last, path: getPath(item), item })

  finalize(node, prefix):                                // recursive
    dirs = []
    for (name, child) of node.childrenByName:
      d = finalize(child, prefix === '' ? name : prefix + '/' + name)
      // chain-collapse: a dir with EXACTLY ONE child, and that child is a dir,
      // and the parent has NO leaves of its own, merges into the child:
      while d.children.length == 1 && d.children[0].kind == 'dir':
        only = d.children[0]
        d = { kind:'dir', name: d.name + '/' + only.name,
              fullPrefix: only.fullPrefix, children: only.children }
      dirs.push(d)
    sort dirs by name (§3.3); sort node.leaves by name (§3.3)
    return { kind:'dir', name: <caller-supplied>, fullPrefix: prefix,
             children: dirs ++ node.leaves }              // dirs FIRST, then leaves

  return finalize(root, '').children
```

Notes:
- The collapse loop runs bottom-up naturally because `finalize` recurses first; a single-pass
  post-recursion `while` as written is sufficient (each merge step re-checks the merged node).
- A dir with one dir child AND ≥1 leaf does NOT collapse (the leaf pins it).
- `fullPrefix` after collapsing is the DEEPEST merged prefix (`"src/git"`, not `"src"`) so
  collapse-state keys stay unique and stable even as sibling sets change.
- Examples (lock these as behavior, they make good harness spot-checks):
  - `["a/b/c.rs"]` → one dir `name:"a/b"`, `fullPrefix:"a/b"`, one leaf `c.rs`.
  - `["feature/x", "feature/y"]` → dir `feature` → leaves `x`, `y`.
  - `["origin/feature/x", "origin/main"]` → dir `origin` → [dir `feature` → leaf `x`, leaf `main`].
  - `["README.md"]` → single root-level leaf, no dir.

### 3.3 Sort order (locked)

Within every dir's children: **directories first, then leaves; each group sorted by `name`
ascending, case-insensitive, with a case-sensitive tiebreak** — implement as
`a.name.toLowerCase() < b.name.toLowerCase()`-style comparison falling back to `a.name < b.name`
(plain code-unit compare, NOT `localeCompare` — locale-independent determinism matters more
than linguistic collation for paths/refs). This intentionally differs from the backend's flat
ordering; flat mode keeps the backend order exactly as today.

### 3.4 Testing

No frontend test runner exists in this repo (no vitest/jest); introducing one is OUT of scope.
The builder's correctness is pinned by: (a) the normative pseudocode + locked examples above,
(b) harness screenshots against the mock fixtures (mock status data must include at least one
nested path like `src/git/status.rs`, one single-child chain, and one root-level file — extend
`mock.ts` fixture paths in P3b-2 if they don't already exercise this), (c) reviewer verification
against §3.2. If the orchestrator later adds vitest, `pathTree.ts` is the first test target.

---

## 4. `src/components/Tree.tsx` — recursive collapsible renderer (P3b-2)

### 4.1 Interface (exact)

```ts
import type { ReactNode } from 'react';
import type { TreeLeaf, TreeNode } from '../utils/pathTree';

export interface TreeProps<T> {
  nodes: TreeNode<T>[];
  /** Renders a COMPLETE <li> for a leaf (reuse existing FileRow / BranchRow /
   *  tag-row markup unchanged — Tree never inspects leaf content). */
  renderLeaf(leaf: TreeLeaf<T>): ReactNode;
  /** React key for a leaf <li>'s wrapper position; must be unique per list
   *  (e.g. `${entry.status}:${entry.path}` for status rows, branch name for refs). */
  leafKey(leaf: TreeLeaf<T>): string;
  /** Optional extra class on nested <ul role="group"> levels (styling hook). */
  groupClassName?: string;
}

export function Tree<T>(props: TreeProps<T>): JSX.Element;
```

### 4.2 Rendering & behavior (normative)

- Root: `<ul className="tree" role="tree">`. Dir node: `<li role="treeitem"
  aria-expanded={expanded}>` containing a full-width twisty `<button type="button"
  className="tree-dir-toggle">` (chevron span reusing `.file-chevron`/`.file-chevron-open` +
  dir name in `.tree-dir-name`, color `var(--text-3)` like `.file-dir`), then, when expanded,
  `<ul role="group" className="tree-group">` with the children. Leaf nodes render
  `renderLeaf(leaf)` directly (existing `<li>` rows unchanged; they do NOT gain
  `role="treeitem"` — accepted ARIA simplification, see §7.4).
- **Expand/collapse state lives locally in each `Tree` instance**:
  `useState<Set<string>>(new Set())` holding **collapsed** `fullPrefix` keys. Default policy:
  **everything expanded** (empty set). New data arriving (refresh, stage/unstage) does NOT reset
  the set — prefixes that survive stay collapsed; prefixes that disappear leave harmless stale
  keys; brand-new prefixes appear expanded. Unmount (e.g. switching StatusPanel↔CommitPanel,
  or toggling tree→flat→tree) discards the state — accepted, not persisted.
- Indentation: `padding-left: 14px` per nesting level via CSS on `.tree-group` (matches the
  existing chevron+gap rhythm; do NOT compute inline styles per depth).
- Keyboard/AT: the twisty is a plain `<button>` (Tab + Enter/Space work natively). Full roving
  tabindex / Arrow-key tree navigation is explicitly NOT required in v1.
- Tree is memo-friendly but `React.memo` is not required; lists here are small (≤ a few hundred
  rows) — no virtualization.

### 4.3 CSS additions (`styles.css`)

New classes: `.tree` (reset like `.file-list`), `.tree-group` (nested indent, `margin:0;
padding:0 0 0 14px; list-style:none`), `.tree-dir-row` (height 24px, flex, hover `--bg-2` —
mirror `.file-row`), `.tree-dir-toggle` (unstyled full-width button, `cursor:pointer`,
inherits row layout), `.tree-dir-name` (`color: var(--text-3)`; ellipsis overflow like
`.file-path`). Reuse `.file-chevron` rotation for the twisty.

---

## 5. Consumers

### 5.1 StatusPanel (`src/components/StatusPanel.tsx`) — P3b-2

- `StatusPanelProps` gains `listView: ListView;` — passed down to each `Section`.
- `Section` renders its `<ul className="file-list">` body as today when `listView === 'flat'`;
  when `'tree'`:
  ```tsx
  const nodes = useMemo(() => buildPathTree(entries, (e) => e.path), [entries]);
  <Tree
    nodes={nodes}
    leafKey={(l) => `${l.item.status}:${l.item.path}`}
    renderLeaf={(l) => (
      <FileRow entry={l.item} ...same props as the flat branch... treeMode />
    )}
  />
  ```
- `FileRow` gains an optional `treeMode?: boolean` prop: when true, the non-rename `pathEl`
  renders ONLY `name` (no `.file-dir` prefix — the tree supplies the directory context); the
  `title` tooltip keeps the FULL path (and full `orig → path` for renames) unchanged.
- **Renames (locked):** a renamed entry is placed in the tree by its NEW `path`
  (`buildPathTree(..., (e) => e.path)` — `origPath` never affects placement). In tree mode the
  rename leaf renders `origPath → basename(path)`-style is NOT required — keep the existing
  full `origPath → path` text (`.file-rename` span) unchanged; it may be long, ellipsis handles
  it, tooltip shows the full pair. One display, both modes, zero new rename logic.
- **"Stage all"/"Unstage all" unchanged:** the section-header bulk button keeps operating on
  the flat `entries.flatMap(entryPaths)` — never derived from the tree, never affected by
  collapsed state.
- **Diff-overlay highlight:** the expanded/highlight computation
  (`diffSlot.key === \`${section}:${entry.path}\``) moves unchanged into the render-prop call —
  each leaf's `FileRow` receives the same `expanded` boolean as in flat mode. Collapsing a dir
  that contains the expanded row leaves the overlay open (App owns the overlay; the row is
  merely hidden) — accepted, no auto-close.
- Conflicts section: `expandable={false}` as today; still tree-grouped (grouping is orthogonal
  to expandability).

### 5.2 CommitPanel (`src/components/CommitPanel.tsx`) — P3b-2

- `CommitPanelProps` gains `listView: ListView;`.
- The `data.files` list: flat as today, or
  `buildPathTree(data.files, (f) => f.path)` + `Tree` with
  `leafKey={(l) => \`commit:${l.item.path}\`}` and `renderLeaf` → existing `FileHeaderRow`
  (gains the same optional `treeMode` prop to drop the `.file-dir` prefix; `+/−` counts,
  binary badge, expanded state all unchanged).

### 5.3 Sidebar (`src/components/Sidebar.tsx`) — P3b-3

- `SidebarProps` gains `listView: ListView;`.
- **Local branches:** `buildPathTree(data.local, (b) => b.name)`; `leafKey` = branch name;
  `renderLeaf` → existing `BranchRow` with a new optional `displayName?: string` prop
  (`leaf.name`, the last segment) shown in `.branch-name` while `title`, `onCheckout(name)`,
  `onDelete(name)`, badge, and head-glyph all keep using the FULL `branch.name` — action
  semantics byte-identical to flat mode. The detached-HEAD pseudo-row stays OUTSIDE the tree,
  rendered first as today.
- **Remotes:** `buildPathTree(data.remote, (r) => r.name)` — `origin/feature/x` naturally
  yields origin → feature → x (with chain-collapse producing `origin/feature` when `feature`
  has one child and origin has others — that is correct and intended). Read-only leaf row
  unchanged apart from `displayName`.
- **Tags:** `buildPathTree(data.tags, (t) => t)` (items are plain strings; `T = string`).
- Section headers (Branches/Remotes/Tags collapse, `+` create button, counts, empty states,
  create-row, ConfirmDialog) are all untouched; only each section's `<ul className="branch-list">`
  body swaps to `Tree` in tree mode.
- Nested-tree indent inside the already-indented sidebar sections must not starve
  `.branch-name` width — acceptable at the default 240px sidebar; deep namespaces rely on
  ellipsis + tooltip, and the user can widen the pane (P2a) or switch to flat.

---

## 6. Acceptance

### AI gate (orchestrator-verifiable)
- P3b-1: `cargo test` (new §2.1 tests) + `cargo clippy -- -D warnings` + `pnpm build` green;
  harness toggle persists across reload.
- P3b-2: `pnpm build` green. Harness (`VITE_MOCK_IPC=1`) screenshots: status sections render
  directory trees (mock fixtures include nested + chain-collapsed + root-level paths per §3.4);
  chain `a/b/c.rs` shows one `a/b` dir row; dirs sort before files, both alphabetical;
  collapsing a dir hides its rows; stage/unstage buttons on tree leaves work and "Stage all"
  stages files hidden inside collapsed dirs; opening a leaf's diff highlights the row
  (`.file-row-expanded`) exactly as in flat mode; commit file list shows the same tree +
  `+/−` counts; flat toggle restores today's rendering byte-for-byte.
- P3b-3: `pnpm build` green. Harness: `feature/x` + `feature/y` local branches group under one
  `feature` node with leaves `x`/`y`; checkout/delete/ahead-behind on a tree leaf behave
  identically (delete confirm shows the FULL branch name); remotes nest under `origin`; tags
  with `/` group; empty sections keep their "No …" copy; detached-HEAD row still renders.
- All increments: no change to any `src-tauri/src/git/*`, `graph.rs`, or command signatures
  other than the `list_view` field; `mock.ts` compiles and round-trips `listView`.

### USER CHECKPOINT (native `pnpm tauri dev` — never self-declared)
1. Toggle tree/flat in the native app; preference persists across relaunch (`settings.json`
   gains `"listView"`).
2. On a real repo with `feature/*` branches and nested source dirs: sidebar + status trees read
   naturally; stage/unstage/commit and branch checkout/delete from tree leaves behave exactly as
   before; diff overlay opens from a tree leaf.

---

## 7. Ambiguities resolved here (flag to orchestrator if disagreed)

1. **One global `listView` toggle** (header button) rather than per-list toggles — one setting,
   one button, lighter UI; per-list granularity would need 5+ persisted flags and per-section
   chrome for marginal benefit. If per-list is later wanted, `ListView` generalizes to a map
   without wire-format breakage (new field, old one ignored).
2. **Default `tree`** per the locked user requirement; flat mode preserves today's backend
   ordering exactly (tree mode re-sorts per §3.3 — orderings intentionally differ).
3. **Rename placement:** leaf under the NEW path's directory; row text stays the existing full
   `orig → path`; `origPath` never influences tree shape. Simplest rule, zero new rename logic,
   `entryPaths` bulk-stage expansion untouched.
4. **Minimal ARIA:** `role="tree"/"treeitem"/"group"` on the Tree's own structure with plain
   `<button>` twisties; existing leaf `<li>`s keep their current markup (no `treeitem` role, no
   roving tabindex). Full WAI-ARIA tree keyboard nav is explicitly out of scope for v1.
5. **Collapse state is local + ephemeral** (per Tree instance, survives data refreshes via a
   collapsed-key set, lost on unmount/mode-toggle). Persisting expand state per repo would need
   a keyed store and isn't worth it; default-expanded matches today's fully-visible lists.
6. **Chain-collapse keying:** a collapsed dir's `fullPrefix` is the deepest merged prefix, so
   collapse-state keys remain stable when a sibling later appears and the chain un-merges (the
   old key simply goes stale — the un-merged dirs default to expanded, which is acceptable).
7. **No frontend test runner added** (§3.4) — the repo has none and introducing vitest is a
   separate decision for the orchestrator; the builder is pinned by normative pseudocode +
   locked examples + harness fixtures instead.
8. **Deterministic code-unit sort, not `localeCompare`** — paths and ref names need stable,
   locale-independent ordering across machines; linguistic collation buys nothing here.
9. **Collapsing a dir does not close an open diff overlay** for a row inside it — the overlay
   is App-owned center-pane state (P3a); auto-closing on collapse would add coupling for a
   rare interaction.
