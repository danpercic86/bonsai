# P4 — UX refinements (post-P3 user feedback)

**Summary:** Five frontend-only refinements — tab `+`-menu overflow fix, Fetch/Pull/Push relocated to a centered bar above the graph, merged "Changes" status section, sidebar auto-expand of the current branch only, and diff coloring (file-type chip + full syntax highlighting) — with **zero backend and zero IPC/type changes**.

---

## Sub-increment table

| ID | Goal (one line) | Files touched |
|----|-----------------|---------------|
| **P4a** | Fix `.tab-strip` overflow so pills scroll horizontally but the `+` menu escapes with no spurious vertical scrollbar | `src/components/TabStrip.tsx`, `src/styles.css` |
| **P4b** | Move Fetch/Pull/Push into a horizontally-centered bar above the graph; Refresh stays top-right | `src/components/RepoWorkspace.tsx`, `src/styles.css` |
| **P4c** | Merge Unstaged + Untracked into one presentation-only "Changes" section; untracked badge → `A` | `src/components/StatusPanel.tsx`, `src/components/DiffOverlay.tsx`, `src/styles.css` |
| **P4d** | Tree mode: ref folders collapsed by default except the path to the current branch, which auto-expands; current branch sorted first | `src/components/Tree.tsx`, `src/components/Sidebar.tsx`, `src/utils/pathTree.ts`, `src/styles.css` |
| **P4e** | Diff coloring: file-type accent chip + full per-line syntax highlighting via highlight.js (lazy, CSS-variable themed) | `src/utils/language.ts` (new), `src/utils/highlight.ts` (new), `src/components/DiffView.tsx`, `src/components/DiffOverlay.tsx`, `src/styles.css`, `package.json` |

No file outside `src/` (except `package.json` for the P4e dependency) is touched. No Rust, no `src/ipc/*` signature, no fixture-shape change.

---

## Invariants preserved

- **Rust owns all Git logic + graph math.** Every P4 change is pure presentation of data the backend already returns. **Syntax highlighting (P4e) is presentation of already-computed diff text** — it tokenizes the exact `DiffLine.content` strings the diff payload already carries, adds no new information, requires no per-commit round-trip, and touches neither `FileDiff`/`DiffLine` nor any command/event/channel. It therefore belongs client-side in React and is **explicitly NOT git logic**. Confirmed.
- **Zero IPC / zero backend change.** No command, event, channel, or wire type is added or altered in any of P4a–P4e. `WorkdirSection` (`staged|unstaged|untracked`) is unchanged; diff-slot key grammar is unchanged.
- **Mock harness stays a faithful twin.** Because no IPC signature or wire type changes, `src/ipc/mock.ts` needs **no code change**. For P4e visual verification the mock fixtures SHOULD already expose diffs across several extensions (`.ts`, `.rs`, `.json`, `.css`, `.md`); if they do not, senior-dev MAY add fixture diff entries (fixture data only) so highlighting is visible in `VITE_MOCK_IPC=1`. That is the only permitted mock touch, and it changes data, not types.
- **All new colors are CSS variables**, defined in both the `:root` (dark, default) and `[data-theme='light']` blocks — except language-accent + syntax-token palettes that are intentionally theme-shared, following the existing `--lane-*` precedent (documented inline).

---

## P4a — Tab `+` menu overflow fix

### Root cause
`.tab-strip` sets `overflow-x: auto`. Per CSS spec a non-`visible` `overflow-x` forces `overflow-y` to compute to `auto`; the absolutely-positioned `.repo-switcher-menu` (which sits at `top: calc(100% + 4px)`, extending below the strip) then makes the strip's scroll height exceed its client height, producing a spurious vertical scrollbar AND clipping the menu.

### Fix — structural split (no portal)
Move the horizontal scroll onto an **inner** wrapper that contains only the pills; keep `.tab-add-wrap` (button + menu) as a **sibling outside** the scroll container. `.tab-strip` itself carries no `overflow`, so the absolutely-positioned menu escapes freely.

**`TabStrip.tsx` — new JSX structure** (behavioural logic, refs, handlers all unchanged; only the wrapping element is added):

```tsx
return (
  <div className="tab-strip" ref={rootRef}>
    <div className="tab-scroll">
      {tabs.map((t) => ( /* ...existing .tab pill markup, unchanged... */ ))}
    </div>
    <div className="tab-add-wrap">
      {/* ...existing .tab-add button + {menuOpen && <div className="repo-switcher-menu">...} ... */}
    </div>
  </div>
);
```

- `rootRef` stays on the outer `.tab-strip` so the existing outside-mousedown close logic still covers both the scroll region and the menu.
- No prop, state, or handler changes.

**`styles.css` changes:**

```css
.tab-strip {
  display: flex;
  align-items: center;
  gap: 4px;
  flex: 1;
  min-width: 0;
  /* REMOVE overflow-x: auto; strip must not clip the menu */
}

.tab-scroll {
  display: flex;
  align-items: center;
  gap: 4px;
  flex: 1 1 auto;
  min-width: 0;
  overflow-x: auto;
  overflow-y: hidden; /* explicit hidden prevents the forced-auto vertical bar */
  scrollbar-width: thin;
}

/* .tab-add-wrap unchanged (position: relative; flex: none) — now a sibling of
   .tab-scroll, so the menu is outside the scroll box and renders in full. */
```

### Acceptance
- With enough open tabs to overflow, the pill row scrolls horizontally; the `+` button stays pinned and fully visible.
- Opening the `+` menu shows the full dropdown escaping below the header with **no vertical scrollbar** on the header.
- Harness: open `VITE_MOCK_IPC=1`, open ~12 tabs (or narrow the window), screenshot the header — pills scroll, `+` menu fully visible, no vertical scrollbar.

---

## P4b — Toolbar relocation

Split the current `.workspace-toolbar` row: **Refresh stays** there (top-right, keeps the `header-progress` bar beneath it); **Fetch/Pull/Push move** into a new horizontally-centered `.graph-toolbar` that mounts inside `.graph-pane` above `GraphCanvas`.

### Mount point (RepoWorkspace render tree)
`.graph-toolbar` becomes the **first flex child of `<main className="graph-pane">`**, before the graph error/truncated banners and the canvas:

```tsx
<main className="graph-pane">
  <div className="graph-toolbar">
    {/* the three Fetch / Pull / Push buttons, moved verbatim from workspace-toolbar */}
  </div>
  {graphError !== null && (<div className="error-banner graph-error-banner">…</div>)}
  {graph !== null && graph.truncated && (<div className="graph-truncated-banner">…</div>)}
  {head?.unborn ? (…) : graph !== null ? (<GraphCanvas … />) : null}
  {diffSlot !== null && overlayMeta !== null && (<DiffOverlay … />)}
</main>
```

- **Overlay:** `DiffOverlay` is `position:absolute; inset:0; z-index:5` inside `.graph-pane`; it fully covers the bar when open (intended full-pane takeover). The bar therefore never visually overlaps or fights the overlay. No z-index or stacking change needed.
- **Flex/scroll model:** `.graph-pane` is `display:flex; flex-direction:column; overflow:hidden`. `.graph-toolbar` is `flex:none` (fixed height); the canvas container keeps `flex:1; min-height:0` and remeasures normally. The bar is static-height and present at mount, so no extra remeasure trigger is required. Show the bar whenever the workspace renders (including unborn-HEAD and null-graph states) — Fetch/Pull/Push are repo-level.
- The three buttons move **verbatim** (same `className="toolbar-btn"`, same `disabled` gating using `refreshing`/`mutating`/`canPullPush`, same `title`s incl. shortcut hints, same `remoteOp` labels). Handlers/state stay in RepoWorkspace. Keyboard shortcuts (Ctrl+Shift+F/P/U) are untouched.

### `.workspace-toolbar` after the move
Contains only the Refresh `btn-icon`. It keeps `justify-content: flex-end` so Refresh stays top-right; `header-progress` remains its sibling directly below and its `remoteOp || refreshing` condition is unchanged.

### `styles.css`
```css
.graph-toolbar {
  flex: none;
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: center; /* horizontally centered, spans only the graph pane */
  gap: 8px;
  padding: 0 12px;
  background: var(--bg-0);
  border-bottom: 1px solid var(--border);
}
```
`.toolbar-btn` styling is reused as-is.

### Acceptance
- Fetch/Pull/Push appear centered in a bar spanning only the center pane, directly above the graph; Refresh remains top-right in the thin toolbar row with the progress bar under it.
- Disabled/busy states, titles, `remoteOp` labels, and Ctrl+Shift+F/P/U all behave as before.
- Harness: screenshot shows centered remote-op bar above the canvas + Refresh top-right; opening a diff overlay hides the bar (overlay covers pane) and closing restores it.

---

## P4c — Merge Unstaged + Untracked into one "Changes" section

**Presentation-only merge.** The `WorkdirSection` union, the diff-slot key grammar (`${section}:${path}` with `section ∈ staged|unstaged|untracked`), and all RepoWorkspace/overlayMeta parsing stay **exactly as-is**. Untracked and unstaged rows render under one "Changes" header but each row keeps its **origin section** in its key so `refetchStatus` (RepoWorkspace ~264-277) and `overlayMeta` (~171-181) still resolve the entry in the correct `snapshot[section]` array and still call `getWorkdirFileDiff(..., section==='staged')` correctly (false for both).

### Why keys must stay origin-tagged
If both origins shared one prefix, `refetchStatus` would look an untracked file up in `snapshot.unstaged`, miss it, and collapse the open diff on every refresh. So unstaged-origin rows keep key `unstaged:<path>` and untracked-origin rows keep `untracked:<path>`. Paths are unique across `unstaged ∪ untracked` in one snapshot (a file is either tracked-modified or untracked, never both), so keys remain globally unambiguous.

### StatusPanel.tsx changes

1. **`BADGES.untracked`: `'U'` → `'A'`** (new files read as adds). Apply the identical change to the `BADGES` map in `DiffOverlay.tsx` so the overlay header badge for an untracked file is also `A`.

2. **Generalize `Section`** to allow a per-entry origin resolver while staying backward-compatible for the Staged section:
   - Add optional prop `sectionForEntry?: (e: StatusEntry) => WorkdirSection`.
   - In `renderRow`, derive the row's section as `const rowSection = sectionForEntry ? sectionForEntry(entry) : section;` and use `rowSection` to build `key` (`${rowSection}:${entry.path}`) and in the `onToggle` call `onToggleDiff(rowSection, entry)`. The `expandable && section !== null` gate keeps using the representative `section` prop (non-null for Changes).
   - Staged section passes no `sectionForEntry` → unchanged behavior.

3. **Replace the two `<Section>` (Unstaged + Untracked)** with ONE Changes section. Build the combined list and origin map in the StatusPanel body:

```tsx
const changes = useMemo(
  () => [...snapshot.unstaged, ...snapshot.untracked],
  [snapshot.unstaged, snapshot.untracked],
);
const originByPath = useMemo(() => {
  const m = new Map<string, WorkdirSection>();
  for (const e of snapshot.unstaged) m.set(e.path, 'unstaged');
  for (const e of snapshot.untracked) m.set(e.path, 'untracked');
  return m;
}, [snapshot.unstaged, snapshot.untracked]);
```

```tsx
<Section
  label="Changes"
  section="unstaged"                 // representative (non-null) for gating only
  sectionForEntry={(e) => originByPath.get(e.path) ?? 'unstaged'}
  entries={changes}
  rowAction="stage"
  actionLabel="Stage all"            // one bulk button over the whole combined list
  disabled={disabled}
  expandable
  diffSlot={diffSlot}
  listView={listView}
  onAction={onStage}
  onToggleDiff={onToggleDiff}
/>
```
   - Both origins stage with the same `onStage` (semantics `getWorkdirFileDiff(..., staged=false)`), so `rowAction`, `actionLabel`, `onAction` are uniform — no per-row action branching needed. "Stage all" = `onAction(changes.flatMap(entryPaths))` (existing header-button code, now over the combined list).

4. **Tree mode (P3b):** `buildPathTree(changes, e => e.path)` groups the combined list fine. Leaf keys stay `${status}:${path}` (unique). Each leaf's origin section is recovered inside `renderRow` via `sectionForEntry(entry)` (the `originByPath` map), so the diff key is correct regardless of tree vs flat. `buildPathTree` itself is unchanged.

5. **`isEmpty`** check is unchanged (still keys off the four raw snapshot arrays).

### RepoWorkspace / overlayMeta
**No change.** Keys keep origin prefixes and the `WorkdirSection` union is intact, so `refetchStatus` section-parse + entry-lookup and `overlayMeta`'s `status?.[section]` lookup and `kind: section` (header label "Unstaged"/"Untracked") continue to work unchanged. (Overlay header still distinguishes the two origins in its kind label — acceptable and informative.)

### styles.css
Make untracked rows read as normal "add" changes inside the merged list:
```css
/* untracked now shows an "A" badge in green, like added; drop the muted/italic
   path treatment so Changes rows look uniform. */
.file-status-untracked .file-badge {
  color: var(--success);
  font-style: normal;
}
.file-status-untracked .file-path,
.file-status-untracked .file-name {
  color: var(--text-1);
  font-style: normal;
}
```

### Acceptance
- Right panel shows: **Staged**, **Changes** (unstaged + untracked concatenated), then **Conflicts** (if any). No separate Unstaged/Untracked headers.
- Untracked rows show a green **A** badge; modified rows show **M**. One "Stage all" button under Changes stages every listed file.
- Expanding an untracked row opens its diff; a subsequent status refresh keeps it open (key `untracked:<path>` still resolves). Expanding a modified row uses key `unstaged:<path>`. Both verified in flat AND tree mode.
- Harness: screenshot the merged section in both list views; toggle a mock untracked file's diff, trigger a refresh, confirm the overlay persists.

---

## P4d — Sidebar auto-expand current branch only

In **tree mode** the ref folders (branches, remotes, tags) render **collapsed by default**, except the folder chain leading to the current (HEAD) branch, which auto-expands; and the current branch sorts **first within its parent**. Section headers (Branches/Remotes/Tags) keep their existing default-open state (Sidebar's own `*Collapsed` state — untouched). Flat mode also sorts the current branch to the top.

### (a) `Tree.tsx` — add default-collapsed + seeded-expansion
Tree currently initializes `collapsed` to an empty Set (everything expanded). Add two optional props and a default-collapsed initializer:

```tsx
export interface TreeProps<T> {
  nodes: TreeNode<T>[];
  renderLeaf(leaf: TreeLeaf<T>): ReactNode;
  leafKey(leaf: TreeLeaf<T>): string;
  groupClassName?: string;
  /** P4d: when true, every dir starts COLLAPSED except those in initiallyExpanded. */
  defaultCollapsed?: boolean;
  /** P4d: dir fullPrefixes to leave expanded on first render (seed). */
  initiallyExpanded?: readonly string[];
}
```

```tsx
function collectDirPrefixes<T>(nodes: TreeNode<T>[], out: string[]): void {
  for (const n of nodes) {
    if (n.kind === 'dir') {
      out.push(n.fullPrefix);
      collectDirPrefixes(n.children, out);
    }
  }
}

const [collapsed, setCollapsed] = useState<Set<string>>(() => {
  if (props.defaultCollapsed !== true) return new Set();      // legacy: all expanded
  const all: string[] = [];
  collectDirPrefixes(props.nodes, all);
  const s = new Set(all);
  for (const p of props.initiallyExpanded ?? []) s.delete(p);
  return s;
});
```
`toggle` and the rest of the render are unchanged (a dir is expanded iff its `fullPrefix` is NOT in `collapsed`). The initializer runs once per mount; callers reseed via a React `key` (below). Existing Tree callers (StatusPanel changes/staged sections) omit the new props → behavior identical.

### (b) Seeding the expansion set + reseed on checkout (Sidebar.tsx)
Compute the ancestor folder prefixes of the current branch and remount the local Tree when the current branch changes:

```tsx
function ancestorPrefixes(name: string): string[] {
  const segs = name.split('/').filter(Boolean);
  const out: string[] = [];
  for (let i = 1; i < segs.length; i++) out.push(segs.slice(0, i).join('/'));
  return out; // "a/b/c" -> ["a", "a/b"]; root-level branch -> []
}
```
Chain-collapsed nodes have `fullPrefix` equal to their deepest prefix (e.g. `"a/b"`), which is in this set; non-collapsed intermediate dirs (`"a"`, `"a/b"`) are also in the set. Extra prefixes that don't correspond to a real node are harmless.

Local branches Tree:
```tsx
const expanded = currentBranch !== null ? ancestorPrefixes(currentBranch) : [];
// key remounts (reseeds) only when HEAD changes (checkout); watcher refreshes
// that keep the same HEAD preserve the user's manual toggles.
<Tree
  key={`local:${currentBranch ?? 'none'}`}
  nodes={localTree}
  leafKey={(l) => l.item.name}
  defaultCollapsed
  initiallyExpanded={expanded}
  renderLeaf={/* unchanged BranchRow */}
/>
```
Remotes and Tags trees: `defaultCollapsed` with `initiallyExpanded={[]}` (no current concept), stable `key`.

> **Known minor limitation (flag):** because the seed is computed once per mount, a branch/ref *created* while the tree is mounted (no HEAD change → no remount) lands in a folder that renders expanded (its new `fullPrefix` isn't in the initial collapsed set). Acceptable for v1; it self-corrects on the next checkout/reopen. DEFAULT — flag to orchestrator.

### (c) Current-first ordering
Add an optional `priorityPath` to `buildPathTree` so a chosen leaf sorts first within its parent, without disturbing the shared StatusPanel usage:

```ts
export function buildPathTree<T>(
  items: readonly T[],
  getPath: (item: T) => string,
  options?: { priorityPath?: string },
): TreeNode<T>[]
```
Inside `finalize`, the leaf sort becomes:
```ts
const pp = options?.priorityPath;
const leaves = [...node.leaves].sort((a, b) => {
  if (pp !== undefined) {
    if (a.path === pp && b.path !== pp) return -1;
    if (b.path === pp && a.path !== pp) return 1;
  }
  return compareNames(a.name, b.name);
});
```
Dir ordering is unchanged (alphabetical). Callers omitting `options` are unaffected.

Sidebar builds the local tree with the current branch as priority:
```tsx
const localTree = useMemo(
  () => (treeMode && data !== null
    ? buildPathTree(data.local, (b) => b.name, { priorityPath: currentBranch ?? undefined })
    : []),
  [treeMode, data, currentBranch],
);
```

**Flat mode:** sort the current branch to the very top, rest in existing backend order:
```tsx
const localFlat = useMemo(() => {
  if (data === null) return [];
  const head = data.local.filter((b) => b.isHead);
  const rest = data.local.filter((b) => !b.isHead);
  return [...head, ...rest];
}, [data]);
```
Render `localFlat` instead of `data.local` in the flat `<ul className="branch-list">`. Detached-HEAD row and empty states unchanged. All row actions (checkout/merge/rebase/delete) keep using full `branch.name` — unchanged.

### styles.css
No new rules required (reuses existing `.tree*`/`.branch-*`). If a subtle current-branch emphasis is wanted it already exists via `.branch-row-head`.

### Acceptance
- Tree mode: ref folders start collapsed; the chain to the current branch is expanded and the current branch is the first leaf in its folder.
- Checking out a branch in a different folder auto-expands that folder (Tree remounts on HEAD change).
- Flat mode: current branch is the top row; everything else unchanged.
- Section headers still default open. Harness: with a mock repo containing nested branch names (`feature/a`, `feature/b`, `release/1.x`) and HEAD on `feature/b`, screenshot tree mode showing only `feature/` expanded with `feature/b` first.

---

## P4e — Diff coloring by file type (accent chip + full syntax highlighting)

Two presentational parts. **No change to `FileDiff`/`DiffLine`/IPC** — confirmed; both parts operate on `FileDiff.path` and `DiffLine.content` that the payload already carries.

> **Split guidance:** kept as ONE sub-increment with two internal steps (Step 1 chip + language util + CSS vars; Step 2 highlighter). If a single senior-dev pass proves too large, the orchestrator MAY split into **P4e-1** (Step 1) and **P4e-2** (Step 2) at the natural boundary below. DEFAULT — flag to orchestrator.

### Shared: `src/utils/language.ts` (new)
```ts
export type LangId =
  | 'typescript' | 'javascript' | 'json' | 'html' | 'xml' | 'css' | 'scss'
  | 'markdown' | 'rust' | 'python' | 'csharp' | 'java' | 'go' | 'ruby'
  | 'php' | 'bash' | 'yaml' | 'toml' | 'sql' | 'c' | 'cpp' | 'kotlin' | 'swift';

export interface LangMeta {
  /** highlight.js language id used for both grammar load and highlight. */
  id: LangId;
  /** Short chip label shown to the user (may differ from id, e.g. tsx/jsx). */
  label: string;
}

const EXT_MAP: Record<string, LangMeta> = {
  ts:   { id: 'typescript', label: 'ts' },
  tsx:  { id: 'typescript', label: 'tsx' },
  mts:  { id: 'typescript', label: 'ts' },
  cts:  { id: 'typescript', label: 'ts' },
  js:   { id: 'javascript', label: 'js' },
  jsx:  { id: 'javascript', label: 'jsx' },
  mjs:  { id: 'javascript', label: 'js' },
  cjs:  { id: 'javascript', label: 'js' },
  json: { id: 'json',       label: 'json' },
  html: { id: 'xml',        label: 'html' },
  htm:  { id: 'xml',        label: 'html' },
  xml:  { id: 'xml',        label: 'xml' },
  svg:  { id: 'xml',        label: 'svg' },
  css:  { id: 'css',        label: 'css' },
  scss: { id: 'scss',       label: 'scss' },
  sass: { id: 'scss',       label: 'sass' },
  md:   { id: 'markdown',   label: 'md' },
  markdown: { id: 'markdown', label: 'md' },
  rs:   { id: 'rust',       label: 'rs' },
  py:   { id: 'python',     label: 'py' },
  cs:   { id: 'csharp',     label: 'cs' },
  java: { id: 'java',       label: 'java' },
  go:   { id: 'go',         label: 'go' },
  rb:   { id: 'ruby',       label: 'rb' },
  php:  { id: 'php',        label: 'php' },
  sh:   { id: 'bash',       label: 'sh' },
  bash: { id: 'bash',       label: 'sh' },
  zsh:  { id: 'bash',       label: 'sh' },
  yml:  { id: 'yaml',       label: 'yaml' },
  yaml: { id: 'yaml',       label: 'yaml' },
  toml: { id: 'toml',       label: 'toml' },
  sql:  { id: 'sql',        label: 'sql' },
  c:    { id: 'c',          label: 'c' },
  h:    { id: 'c',          label: 'h' },
  cpp:  { id: 'cpp',        label: 'cpp' },
  cc:   { id: 'cpp',        label: 'cpp' },
  cxx:  { id: 'cpp',        label: 'cpp' },
  hpp:  { id: 'cpp',        label: 'hpp' },
  kt:   { id: 'kotlin',     label: 'kt' },
  swift:{ id: 'swift',      label: 'swift' },
};

export function detectLanguage(path: string): LangMeta | null {
  const base = path.slice(path.lastIndexOf('/') + 1);
  const dot = base.lastIndexOf('.');
  if (dot <= 0) return null; // no extension / dotfile
  const ext = base.slice(dot + 1).toLowerCase();
  return EXT_MAP[ext] ?? null;
}
```

### Step 1 — file-type accent chip
Show a small language chip in the diff overlay header (primary location).

**`DiffOverlay.tsx`:** compute `const lang = detectLanguage(meta.path);` and render, after the path span and before `.diff-overlay-kind`:
```tsx
{lang !== null && (
  <span className="lang-chip" data-lang={lang.id}>{lang.label}</span>
)}
```
(File-row chips are an OPTIONAL stretch and out of scope for v1 — the overlay header chip satisfies the requirement.)

**`styles.css`** — chip base + a small theme-shared accent palette (following the `--lane-*` precedent: brand hues read acceptably on the neutral chip background in both themes, so they are defined once in `:root` and NOT duplicated in the light block; documented inline):
```css
.lang-chip {
  flex: none;
  font-family: var(--font-mono);
  font-size: 10px;
  line-height: 1;
  padding: 3px 6px;
  border-radius: 4px;
  text-transform: uppercase;
  letter-spacing: 0.3px;
  color: var(--text-1);
  background: var(--bg-2);
  border-left: 3px solid var(--lang-accent, var(--text-3));
}
/* theme-shared language accents (see --lane-* precedent) */
:root {
  --lang-ts: #3178c6;  --lang-js: #f1b542;  --lang-json: #a0a0a0;
  --lang-html: #e34c26; --lang-css: #563d7c; --lang-md: #4f8cff;
  --lang-rust: #dea584; --lang-py: #3572a5; --lang-cs: #68217a;
  --lang-go: #00add8;  --lang-shell: #89e051; --lang-default: #6b7280;
}
.lang-chip[data-lang="typescript"] { --lang-accent: var(--lang-ts); }
.lang-chip[data-lang="javascript"] { --lang-accent: var(--lang-js); }
.lang-chip[data-lang="json"]       { --lang-accent: var(--lang-json); }
.lang-chip[data-lang="xml"]        { --lang-accent: var(--lang-html); }
.lang-chip[data-lang="css"],
.lang-chip[data-lang="scss"]       { --lang-accent: var(--lang-css); }
.lang-chip[data-lang="markdown"]   { --lang-accent: var(--lang-md); }
.lang-chip[data-lang="rust"]       { --lang-accent: var(--lang-rust); }
.lang-chip[data-lang="python"]     { --lang-accent: var(--lang-py); }
.lang-chip[data-lang="csharp"]     { --lang-accent: var(--lang-cs); }
.lang-chip[data-lang="go"]         { --lang-accent: var(--lang-go); }
.lang-chip[data-lang="bash"]       { --lang-accent: var(--lang-shell); }
/* remaining ids fall back to --lang-default via the .lang-chip default */
```

### Step 2 — full syntax highlighting inside diff lines

#### Library choice — **highlight.js** (firm recommendation)
Evaluated:
- **Shiki** — rejected. Async + WASM, heavy runtime/memory, and it inlines VS Code theme colors as element styles, which fights our CSS-variable theming (can't cleanly remap to `--syn-*`). Wrong fit for a light, synchronous, variable-themed diff.
- **Prism** — viable and tiny core, but its `components/prism-*` grammars require **manual dependency ordering** (e.g. `tsx → jsx → javascript`, `cpp → c`, `php → markup+clike`) when lazy-loaded; getting that right per-language is fragile.
- **highlight.js** — **chosen.** Each `highlight.js/lib/languages/<lang>` module is **self-contained** (bundles its own sub-grammars), so lazy per-language registration is a one-liner with no dependency graph to manage — the decisive practical advantage. It is synchronous once loaded, we always know the language (from the extension, so no auto-detect cost), and its stable `.hljs-*` token classes map trivially to our CSS variables.
  - **Package:** `pnpm add highlight.js`. Import ONLY `highlight.js/lib/core` + per-language modules via dynamic import — never the barrel `highlight.js` (which pulls all languages).
  - **Bundle cost:** core ≈ 8 KB gz, loaded lazily on first diff; each language module ≈ 1–5 KB gz, loaded on demand and cached. Initial app bundle cost ≈ 0 (all dynamic imports; Vite code-splits them).

A hand-rolled tokenizer was considered and rejected: correctly covering ~20 languages by hand is more code and more bugs than a 8 KB well-tested core.

#### Line-by-line vs full-file — **per-line** (firm recommendation)
Highlight each `line.content` independently with the detected grammar. Rationale: diffs are fragmented and context-limited (we don't always have full file sides), reconstructing and mapping back is complex and error-prone, and per-line keeps the cost **O(visible/diff lines)** with no diff-render regression. Accept minor imperfection on multi-line constructs (block comments, template strings) for v1 — highlight.js v11 exposes no public cross-line continuation API anyway. The diff is already capped (`tooLarge` at 5000 lines) so the per-line pass is bounded.

#### `src/utils/highlight.ts` (new) — lazy registry + hook
```ts
import hljs from 'highlight.js/lib/core';
import type { LangId } from './language';

const loaders: Record<LangId, () => Promise<{ default: unknown }>> = {
  typescript: () => import('highlight.js/lib/languages/typescript'),
  javascript: () => import('highlight.js/lib/languages/javascript'),
  json:       () => import('highlight.js/lib/languages/json'),
  html:       () => import('highlight.js/lib/languages/xml'),   // alias
  xml:        () => import('highlight.js/lib/languages/xml'),
  css:        () => import('highlight.js/lib/languages/css'),
  scss:       () => import('highlight.js/lib/languages/scss'),
  markdown:   () => import('highlight.js/lib/languages/markdown'),
  rust:       () => import('highlight.js/lib/languages/rust'),
  python:     () => import('highlight.js/lib/languages/python'),
  csharp:     () => import('highlight.js/lib/languages/csharp'),
  java:       () => import('highlight.js/lib/languages/java'),
  go:         () => import('highlight.js/lib/languages/go'),
  ruby:       () => import('highlight.js/lib/languages/ruby'),
  php:        () => import('highlight.js/lib/languages/php'),
  bash:       () => import('highlight.js/lib/languages/bash'),
  yaml:       () => import('highlight.js/lib/languages/yaml'),
  toml:       () => import('highlight.js/lib/languages/ini'), // TOML handled by ini grammar
  sql:        () => import('highlight.js/lib/languages/sql'),
  c:          () => import('highlight.js/lib/languages/c'),
  cpp:        () => import('highlight.js/lib/languages/cpp'),
  kotlin:     () => import('highlight.js/lib/languages/kotlin'),
  swift:      () => import('highlight.js/lib/languages/swift'),
};

const ready = new Set<LangId>();
const inflight = new Map<LangId, Promise<boolean>>();

export function ensureLanguage(id: LangId): Promise<boolean> {
  if (ready.has(id)) return Promise.resolve(true);
  const existing = inflight.get(id);
  if (existing) return existing;
  const p = loaders[id]()
    .then((mod) => {
      // hljs registers under the grammar's canonical name; register under `id`
      // too so highlight(text,{language:id}) resolves for aliases (html->xml).
      if (!hljs.getLanguage(id)) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        hljs.registerLanguage(id, mod.default as any);
      }
      ready.add(id);
      return true;
    })
    .catch(() => false)
    .finally(() => inflight.delete(id));
  inflight.set(id, p);
  return p;
}

/** Returns highlighted HTML (entities escaped by hljs) or null if grammar not
 *  ready. Never throws — highlight is best-effort presentation. */
export function highlightLine(id: LangId, text: string): string | null {
  if (!ready.has(id)) return null;
  try {
    return hljs.highlight(text, { language: id, ignoreIllegals: true }).value;
  } catch {
    return null;
  }
}
```

React hook (co-located or in a small `useHighlighter.ts`):
```ts
export function useHighlighter(id: LangId | null): ((text: string) => string | null) | null {
  const [, force] = useReducer((n) => n + 1, 0);
  useEffect(() => {
    if (id === null) return;
    let cancel = false;
    void ensureLanguage(id).then((ok) => { if (ok && !cancel) force(); });
    return () => { cancel = true; };
  }, [id]);
  if (id === null) return null;
  return (text: string) => highlightLine(id, text);
}
```

#### `DiffView.tsx` wiring
```tsx
const lang = useMemo(() => detectLanguage(diff.path), [diff.path]);
const highlight = useHighlighter(lang?.id ?? null);
```
Per line, replace the plain content span:
```tsx
{(() => {
  const html = highlight ? highlight(line.content) : null;
  return html !== null
    ? <span className="diff-content" dangerouslySetInnerHTML={{ __html: html }} />
    : <span className="diff-content">{line.content}</span>;
})()}
```
- `binary`/`tooLarge`/empty-hunks short-circuits stay first (unchanged) — no highlighting attempted there.
- Unknown extension → `lang === null` → `highlight === null` → plain content (current behavior). Progressive: before the grammar loads, plain text renders; the hook re-renders once ready. `dangerouslySetInnerHTML` is safe here because hljs HTML-escapes all text; the markup is only `<span class="hljs-*">`.
- The `.diff-nonewline` sentinel line stays plain.
- DiffView remains `memo`'d on `diff`; the one extra render when a grammar finishes loading is bounded and acceptable.

#### Theming — token classes → CSS variables
Do NOT ship any highlight.js stock theme CSS (they hardcode colors). Define our own `--syn-*` variables in BOTH theme blocks and map the common `.hljs-*` classes, scoped under `.diff-content` so highlighting only styles diff bodies:

```css
/* dark (:root) */
--syn-keyword: #ff7b72;  --syn-string: #7ee787;  --syn-comment: #8b949e;
--syn-number: #79c0ff;   --syn-function: #d2a8ff; --syn-title: #d2a8ff;
--syn-type: #ffa657;     --syn-attr: #79c0ff;     --syn-tag: #7ee787;
--syn-operator: #a8adb8; --syn-punctuation: #a8adb8; --syn-meta: #6b7280;

/* [data-theme='light'] */
--syn-keyword: #cf222e;  --syn-string: #0a7d33;  --syn-comment: #6e7781;
--syn-number: #0550ae;   --syn-function: #8250df; --syn-title: #8250df;
--syn-type: #953800;     --syn-attr: #0550ae;     --syn-tag: #0a7d33;
--syn-operator: #4b515c; --syn-punctuation: #4b515c; --syn-meta: #8a919e;
```
```css
.diff-content .hljs-keyword,
.diff-content .hljs-built_in,
.diff-content .hljs-literal      { color: var(--syn-keyword); }
.diff-content .hljs-string,
.diff-content .hljs-regexp        { color: var(--syn-string); }
.diff-content .hljs-comment,
.diff-content .hljs-quote         { color: var(--syn-comment); font-style: italic; }
.diff-content .hljs-number        { color: var(--syn-number); }
.diff-content .hljs-title,
.diff-content .hljs-title.function_,
.diff-content .hljs-function .hljs-title { color: var(--syn-function); }
.diff-content .hljs-type,
.diff-content .hljs-class .hljs-title,
.diff-content .hljs-title.class_  { color: var(--syn-type); }
.diff-content .hljs-attr,
.diff-content .hljs-attribute,
.diff-content .hljs-property      { color: var(--syn-attr); }
.diff-content .hljs-tag,
.diff-content .hljs-name,
.diff-content .hljs-selector-tag  { color: var(--syn-tag); }
.diff-content .hljs-operator,
.diff-content .hljs-symbol        { color: var(--syn-operator); }
.diff-content .hljs-punctuation   { color: var(--syn-punctuation); }
.diff-content .hljs-meta,
.diff-content .hljs-doctag        { color: var(--syn-meta); }
```
The add/del line backgrounds (`.diff-line-add` / `.diff-line-del`), the `+`/`−`/space marker, and the line-number gutters are on ancestor/sibling spans and are **untouched** — token colors compose over the tinted line background. Confirm the marker + gutters still render for highlighted lines (they are separate grid cells).

### Acceptance
- Overlay header shows a language chip (e.g. `TS`, `RS`, `JSON`) with a per-language accent; unknown/dotfiles show no chip.
- Diff bodies show colored keywords/strings/comments/numbers while add=green / del=red backgrounds, markers, and gutters remain intact; binary/too-large/unknown fall back to plain uncolored text.
- Token colors adapt to light/dark (driven by `--syn-*`).
- No highlight.js stock CSS is imported; core + languages are dynamically imported (verify no `highlight.js` barrel import).
- Harness: open mock diffs for `.ts`, `.rs`, `.json`, `.css`, `.md`; screenshot each in dark AND light; confirm coloring, chips, and preserved line backgrounds/markers; confirm an unknown-extension diff renders plain.

---

## Overall acceptance criteria

1. `pnpm build` and `tsc` pass; `pnpm dev` with `VITE_MOCK_IPC=1` runs with no console errors.
2. **P4a:** overflowing tabs scroll horizontally; `+` menu fully visible; no header vertical scrollbar. (Screenshot header with many tabs + open menu.)
3. **P4b:** Fetch/Pull/Push centered above the graph; Refresh top-right with progress bar; all gating/titles/shortcuts intact. (Screenshot pane top; toggle a diff to confirm overlay covers the bar.)
4. **P4c:** Staged / Changes / Conflicts sections; untracked shows green `A`; "Stage all" over the combined list; expand-persist across refresh in flat AND tree mode; RepoWorkspace/overlayMeta unchanged. (Screenshots both list views; refresh-persist check.)
5. **P4d:** tree folders collapsed except current-branch chain (auto-expanded, branch first); reseed on checkout; flat mode current-first; section headers still open. (Screenshot nested-branch mock, tree mode.)
6. **P4e:** language chip + per-line syntax highlighting themed via `--syn-*`, add/del backgrounds+markers+gutters preserved, graceful fallback; lazy dynamic imports only. (Screenshots across extensions in both themes.)
7. **Invariants:** no Rust/IPC/wire-type change; `src/ipc/mock.ts` compiles unchanged (fixture-data-only additions permitted for P4e visuals); all new colors are CSS variables.

### Harness verification checklist (orchestrator)
- Run `pnpm dev` (`VITE_MOCK_IPC=1`), open the browser pane.
- Screenshot: crowded tab header + open `+` menu (P4a); graph-pane top with centered remote bar + top-right Refresh (P4b); right panel Staged/Changes/Conflicts in flat + tree (P4c); sidebar nested-branch tree with only the HEAD chain expanded (P4d); diff overlays for `.ts`/`.rs`/`.json`/`.css`/`.md` in dark + light with chips and highlighting (P4e).
- Inspect the network/module panel to confirm highlight.js languages load as separate lazy chunks and the barrel is not bundled into the entry.

---

## Open questions (resolved with defaults)

1. **Tags tree default-collapsed?** The user note names branches/remotes. **DEFAULT:** apply `defaultCollapsed` to all three ref trees (branches, remotes, tags) for visual consistency; auto-expand only the current-branch chain. Flag to orchestrator.
2. **Newly created ref appears expanded until remount** (P4d seed-once limitation). **DEFAULT:** accept for v1; self-corrects on next checkout/reopen. Flag to orchestrator.
3. **File-row language chips** (in addition to the overlay header). **DEFAULT:** overlay-header chip only for v1; row chips deferred as polish. Flag to orchestrator.
4. **P4e split.** **DEFAULT:** implement as one sub-increment (Step 1 then Step 2); split into P4e-1/P4e-2 only if a single pass is too large. Flag to orchestrator.
5. **TOML grammar.** highlight.js has no dedicated TOML grammar; **DEFAULT:** use the `ini` grammar (close enough) with chip label `toml`. Flag to orchestrator.
6. **`dangerouslySetInnerHTML` for highlighted lines.** Safe because highlight.js HTML-escapes all text and emits only `<span class="hljs-*">`; content originates from the user's own repo. **DEFAULT:** accepted. Flag to orchestrator.
