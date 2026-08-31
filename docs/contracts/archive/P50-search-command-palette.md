# P50 — Commit/Content Search + Command Palette + List Filtering

Three keyboard-first discovery features, all absent today: (1) commit/content search surfaced as
graph highlights + next/prev jump, (2) a Ctrl/Cmd-K command palette, (3) type-to-filter on the
sidebar Branches/Remotes/Tags lists.

References read (current state, verified — not guessed):
`crates/bonsai-core/src/graph.rs` (`compute_graph(&Path) -> Result<GraphLayout, AppError>`, no
filter params — stays that way), `crates/bonsai-core/src/git/remote.rs` (`credential_fill`: the
capture-stdout git shell-out idiom with `CREATE_NO_WINDOW` + never-prompt env), `external.rs`
(injected `CommandRunner` testability pattern), `src-tauri/src/commands/history.rs` (command house
shape `X → X_inner → repo_path(state,&id) → spawn_blocking(core)`), `src/components/
repoWorkspace/useReadOverlays.ts` (`revealCommitByOid`), `useWorkspaceKeyboard.ts` (Esc-layering +
shortcut gate model), `src/graph/GraphCanvas.tsx` + `draw.ts` (`Interaction`), `Combobox.tsx`,
`Sidebar.tsx`, `workspaceMenus.ts`, `App.tsx` (`globalModalOpen`), `src/ipc/{types,mock}.ts` +
`mock/handlers/{external,diff}.ts`. House pattern: P49-external-integrations, M2-graph.

**Command count: 128 → 129** (`search_commits`). Open questions in §11.

---

## 0. Key decisions (with rationale)

**D1 — Search backend split by cost, not by the task's suggested trio.** Verified: a git2
pathspec walk needs a diff-per-commit (same `O(N·diff)` jank as pickaxe), so **path belongs with
content, not with message/author**. Final split:
- **message / author / `all`** → **git2 revwalk** (header-only: read `commit.message()` /
  `commit.author()`; NO diff, NO subprocess). Cheap, cancel-friendly, common case.
- **path / content (`-S`/`-G`)** → **shell-out `git log`** (git's TREESAME-pruned path walk +
  optimized pickaxe; parity with the CLI is near-tautological). Reuse the `credential_fill`
  idiom: `current_dir(workdir)`, `CREATE_NO_WINDOW`, `GIT_TERMINAL_PROMPT=0`, `wait_with_output`.
- Both behind ONE command `search_commits`; the core dispatches on `query.field`. The git binary
  is already a hard dependency (fetch/clone/credentials), so leaning on it for the two modes that
  genuinely benefit is consistent, and it confines the arg-injection surface to two argv builders.

**D2 — Single command response, capped, `truncated` flag (no channel).** Matches are capped at
`MAX_SEARCH_RESULTS = 1000` compact rows (~120 KB worst case) — a single `invoke` like `get_graph`,
not a channel. Cap detection via the **cap+1 trick** (collect up to cap+1, `truncated = len > cap`,
slice to cap) — exact for both backends (shell: `--max-count=cap+1`; git2: stop at cap+1). git2
modes additionally bound the walk at `MAX_SEARCH_SCAN = 200_000` commits (`truncated=true` if hit).

**D3 — Results UX = graph highlight + compact results overlay, jump reuses `revealCommitByOid`.**
Highlight-only can't help when matches are off-screen or numerous; the overlay adds count + next/prev
+ click-to-reveal. The "current match" simply *is* the normal selection — `next/prev` call the
existing `revealCommitByOid(oid)` (which sets `selectedIndex` and scrolls the row in), so the
single-selection model is reused, not fought. `GraphCanvas` gets a new `matchRows` prop → a ring on
matching dots so matches stay visible while scrolling. **Cheap modes live-search (debounced);
content mode is submit-only** (Enter / button) — never live-pickaxe per keystroke.

**D4 — Command palette is its OWN increment (P50c), hosted in `RepoWorkspace`.** It needs `repoId`
+ every repo action handler + `graph` + branches, all of which live in `RepoWorkspace`; threading a
static `appCommands: PaletteAction[]` *down* from App is far cheaper than threading all repo
handlers *up*. Opened by Ctrl/Cmd-K on the active tab only. Entity rows (branch/tag/commit) = **jump**
(reveal in graph); mutating ops route through their existing confirm dialogs (never fire raw). With
no repo open the palette is unavailable (EmptyState has its own buttons) — see OQ3.

**D5 — List filtering = per-section inline filter box** (Branches / Remotes / Tags), not one shared
box (sections are independently collapsible; a shared box can't disambiguate). New tiny
`ListFilterInput.tsx` + a pure `filterByName` helper. `Combobox.tsx` is a dropdown-*select*, not an
inline list filter, so we reuse its **capture-phase-Esc idiom**, not the component.

**D6 — No new `AppError` variant.** Empty/whitespace `text` → `Ok(empty results)` (UI shows
nothing, no special-casing). Bad pathspec / invalid `-G` regex → git exits non-zero → `AppError::Git`
with stderr context. Reuse `noRepo` for an unknown id. (OQ2: add `invalidRegex` for a nicer toast?)

---

## 1. Module boundaries / files

**New**
- `crates/bonsai-core/src/git/search.rs` — wire types + `GitRunner` trait + `SpawnGitRunner` +
  `search_commits` + pure arg-builders/parsers + oracle tests.
- `src-tauri/src/commands/search.rs` — `search_commits` command + `_inner`.
- `src/components/CommitSearchBar.tsx` — query input + mode/regex/case controls + result summary
  + prev/next (own file).
- `src/components/SearchResultsList.tsx` — the compact results overlay list (own file).
- `src/components/repoWorkspace/useCommitSearch.ts` — search state hook (query/results/currentMatch
  /open, reqId last-wins guard, debounce, `matchRows` derivation).
- `src/components/CommandPalette.tsx` — palette overlay (presentational: input + fuzzy list + nav).
- `src/components/paletteActions.ts` — `PaletteAction` type + `buildPaletteActions()` + pure
  `fuzzyScore()`.
- `src/components/repoWorkspace/usePalette.ts` — palette open/close + Ctrl/Cmd-K effect.
- `src/components/ListFilterInput.tsx` — inline filter box + pure `filterByName()`.
- `src/ipc/mock/handlers/search.ts` — mock `searchCommits`.

**Edited**
- `crates/bonsai-core/src/git/mod.rs` — `pub mod search;`
- `src-tauri/src/commands/mod.rs` — `mod search; pub use search::*;`
- `src-tauri/src/lib.rs` — register `commands::search_commits` (after `read_reflog`; 128→129).
- `src/ipc/types.ts` — search wire types + `IpcApi.searchCommits`.
- `src/ipc/tauri.ts` — `searchCommits` wrapper.
- `src/ipc/mock.ts` — import + spread `searchHandlers`.
- `src/graph/GraphCanvas.tsx` — `matchRows?: readonly number[]` prop → `Interaction`.
- `src/graph/draw.ts` — `Interaction.matchRows` + a match-ring pass.
- `src/components/WorkspaceGraphPane.tsx` — render `CommitSearchBar` + pass `matchRows` to
  `GraphCanvas` + a search-toggle affordance.
- `src/components/RepoWorkspace.tsx` — wire `useCommitSearch` + `usePalette`; render
  `CommandPalette`; thread `matchRows`; feed `searchOpen`/`paletteOpen` into the keyboard gates.
- `src/components/repoWorkspace/useWorkspaceKeyboard.ts` — `searchOpenRef` in Esc-layering;
  `paletteOpen`/`searchOpen` in the shortcut gate; Ctrl/Cmd-F opens search.
- `src/components/Sidebar.tsx` — per-section filter inputs + filtering for Branches/Remotes/Tags.
- `src/App.tsx` — assemble `appCommands: PaletteAction[]`; new `RepoWorkspace` prop.
- `styles.css` — palette / search-bar / filter classes; a `--match-ring` color.

---

## 2. Wire types

### 2.1 Rust (`crates/bonsai-core/src/git/search.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SearchField { All, Message, Author, Path, Content } // All = message OR author (both header-only)

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchedField { Message, Author, Path, Content }

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub text: String,
    pub field: SearchField,
    #[serde(default)] pub regex: bool,          // CONTENT only: false = -S literal, true = -G regex. Ignored elsewhere in v1 (OQ2).
    #[serde(default)] pub case_sensitive: bool,  // default false => -i (grep/author/-G); -S literal is always case-sensitive.
    #[serde(default)] pub max_results: u32,      // 0 => MAX_SEARCH_RESULTS; clamped to that hard cap.
    #[serde(default)] pub scope_ref: Option<String>, // None => all refs (git log --all seeding); Some => walk only that ref.
    #[serde(default)] pub since: Option<i64>,    // author time >= (unix secs); None => unbounded.
    #[serde(default)] pub until: Option<i64>,    // author time <=.
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchMatch {
    pub oid: String,        // full 40-hex — feeds revealCommitByOid; row derived frontend-side.
    pub summary: String,    // first message line, capped 120 (reuse graph's cap helper).
    pub author_name: String,
    pub author_ts: i64,     // author time, secs since epoch.
    pub matched: MatchedField, // in All mode: Message wins over Author when both hit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>, // v1: the matched pathspec for Path mode; None otherwise (OQ4).
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub matches: Vec<SearchMatch>, // newest-first (commit-date desc, same as `git log`).
    pub truncated: bool,           // cap or scan-bound hit — "there may be more".
}
```

### 2.2 TypeScript (`src/ipc/types.ts`)

```ts
export type SearchField = 'all' | 'message' | 'author' | 'path' | 'content';
export type MatchedField = 'message' | 'author' | 'path' | 'content';

export interface SearchQuery {
  text: string;
  field: SearchField;
  regex: boolean;
  caseSensitive: boolean;
  maxResults: number;             // 0 => backend default cap
  scopeRef: string | null;
  since: number | null;
  until: number | null;
}
export interface SearchMatch {
  oid: string;
  summary: string;
  authorName: string;
  authorTs: number;
  matched: MatchedField;
  snippet?: string;               // absent when null (skip_serializing_if)
}
export interface SearchResults { matches: SearchMatch[]; truncated: boolean; }
```

`IpcApi` gains (near `blameFile`/`fileHistory`):
```ts
/** Commit/content search. Read-only, does NOT emit repo-changed. Rejects
 *  git (bad pathspec / invalid -G regex / binary missing on path+content) | noRepo.
 *  Empty/whitespace `text` resolves to `{ matches: [], truncated: false }`. */
searchCommits(repoId: string, query: SearchQuery): Promise<SearchResults>;
```

`tauri.ts`: `searchCommits: (repoId, query) => invoke('search_commits', { repoId, query })`.

---

## 3. Backend core — `crates/bonsai-core/src/git/search.rs`

```rust
pub const MAX_SEARCH_RESULTS: u32 = 1000; // default + hard cap
pub const MAX_SEARCH_SCAN: usize = 200_000; // git2 walk bound

/// Injected so arg-building + parsing are unit-testable without git; the oracle
/// tests use the real SpawnGitRunner against a fixture repo.
pub trait GitRunner {
    /// Runs `git <args>` in `cwd`; returns stdout (utf8-lossy). Spawn failure or
    /// non-zero exit -> AppError::Git (include a stderr tail).
    fn run(&self, args: &[String], cwd: &std::path::Path) -> Result<String, AppError>;
}
pub struct SpawnGitRunner; // never-prompt env + CREATE_NO_WINDOW + capture (remote.rs::credential_fill idiom)

/// Blocking. Opens repo at `workdir` (open_ext NO_SEARCH, like compute_graph).
/// Dispatches: All|Message|Author -> revwalk_search (git2); Path|Content -> git log via `runner`.
/// Empty/whitespace text -> Ok(empty). Never panics; non-utf8 -> lossy.
pub fn search_commits(workdir: &std::path::Path, runner: &dyn GitRunner, query: &SearchQuery)
    -> Result<SearchResults, AppError>;

// ---- pure helpers ----
fn effective_cap(q: &SearchQuery) -> u32;              // q.max_results==0 => MAX; else min(q.max_results, MAX)
fn build_log_args(q: &SearchQuery, cap: u32) -> Vec<String>;      // Path/Content only
fn parse_log_output(stdout: &str, cap: u32) -> (Vec<SearchMatch>, bool); // NUL/US-separated records
fn revwalk_search(repo: &git2::Repository, q: &SearchQuery, cap: u32)
    -> Result<(Vec<SearchMatch>, bool), AppError>;
fn seed_all_refs(repo: &git2::Repository, walk: &mut git2::Revwalk) -> Result<(), AppError>;
    // mirrors compute_graph's collect_refs seeding: heads + remotes(skip */HEAD) + tags(peeled) + HEAD.
    // => scope_ref==None matches `git log --all` for the standard ref namespaces.
```

### 3.1 `build_log_args` (Path / Content) — normative

```
args = ["log", "--format=%H%x1f%s%x1f%an%x1f%at", "--glob-pathspecs"]
cap+1 => args += ["--max-count", (cap+1).to_string()]
if !q.case_sensitive => args += ["-i"]              # affects --grep/--author/-G; harmless for -S
if let Some(s)=q.since => args += ["--since", unix_to_git_date(s)]
if let Some(u)=q.until => args += ["--until", unix_to_git_date(u)]
match q.field:
    Path    => args += [ scope_or_all(q) , "--", q.text.clone() ]   # text = one pathspec argv token
    Content => let flag = if q.regex {"-G"} else {"-S"};
               args += [ format!("{flag}{}", q.text) , scope_or_all(q) ]  # -S<text> ONE token, never a shell string
scope_or_all(q): q.scope_ref.clone().unwrap_or("--all".into())
```
- `%x1f` = US byte between the 4 fields; records newline-separated (`%s`/`%an` are single-line).
  `parse_log_output` splits lines, then `splitn(4, '\x1f')` → oid/summary/author/ts; `matched` =
  `Path`|`Content` per mode; `truncated = records > cap` (drop the +1th); snippet = `Some(q.text)`
  for Path else None.
- Injection-safe exactly like P49 D2: `text` is always a single argv element; no `sh -c`, no
  interpolation — a `;`/`&&` in `text` is literal.

### 3.2 `revwalk_search` (All / Message / Author) — normative

```
revwalk_search(repo, q, cap):
    let mut walk = repo.revwalk()
    walk.set_sorting(git2::Sort::TIME)?                 # commit-date desc ~ git log default order
    match q.scope_ref: Some(r) => walk.push_ref-or-oid(r)?  None => seed_all_refs(repo, &mut walk)?
    let needle = if q.case_sensitive { q.text.clone() } else { q.text.to_lowercase() }
    let (mut out, mut examined) = (Vec::new(), 0usize)
    for oid in walk:
        examined += 1
        if examined > MAX_SEARCH_SCAN: return Ok((out, true))
        let c = repo.find_commit(oid?)?
        let ts = c.author().when().seconds()
        if out_of_range(ts, q.since, q.until): continue
        let (hit, which) = match q.field {
            Message => (contains(c.message(), &needle, q.case_sensitive), Message),
            Author  => (contains(&ident(&c), &needle, q.case_sensitive), Author),
            All     => if contains(c.message(), &needle, cs) { (true, Message) }
                       else if contains(&ident(&c), &needle, cs) { (true, Author) }
                       else { (false, Message) },
            _ => unreachable(),                          # Path/Content never reach here
        }
        if hit:
            out.push(SearchMatch { oid: hex, summary: cap120(c.summary()),
                                   author_name: c.author().name lossy, author_ts: ts,
                                   matched: which, snippet: None })
            if out.len() as u32 > cap: return Ok((truncate_to(out, cap), true))
    Ok((out, false))
```
- `ident(c)` = `format!("{} <{}>", name_lossy, email_lossy)` — matches what git `--author` tests
  against (so the oracle lines up). `contains` folds case when `!case_sensitive`.
- `Message` searches the FULL message (subject+body) like git `--grep` (NOT just the summary).

---

## 4. Command — `src-tauri/src/commands/search.rs`

House shape (mirror `blame_file`/`file_history`). Read-only ⇒ **no `repo-changed` emit**.

```rust
use super::shared::*;
use bonsai_core::git::search::{self, SearchQuery, SearchResults, SpawnGitRunner};

#[tauri::command]
pub async fn search_commits(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    query: SearchQuery,
) -> Result<SearchResults, AppError> {
    search_commits_inner(state.inner(), &repo_id, query).await
}

pub(crate) async fn search_commits_inner(
    state: &AppState, repo_id: &str, query: SearchQuery,
) -> Result<SearchResults, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        search::search_commits(&workdir, &SpawnGitRunner, &query)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
```
Register in `lib.rs generate_handler!` after `read_reflog`. `commands/mod.rs`:
`mod search; pub use search::*;`.

---

## 5. Frontend — search UI + graph highlight/jump (P50b)

### 5.1 `useCommitSearch.ts` (state hook — keeps RepoWorkspace lean)

```ts
export function useCommitSearch(deps: {
  repoId: string;
  graphDataRef: { current: GraphLayout | null };   // oid -> row for matchRows
  revealCommitByOid(oid: string): void;             // reused for jump (single-selection model)
  pushToast(kind: ToastKind, msg: string): void;
}): {
  open: boolean; openSearch(initialText?: string): void; close(): void;
  openRef: { current: boolean };                    // for Esc-layering
  query: SearchQuery; patchQuery(patch: Partial<SearchQuery>): void;
  submit(): void;                                    // required for content mode; cheap modes auto-fire
  results: SearchResults | null; loading: boolean; error: string | null;
  currentMatch: number;                              // index into results.matches, -1 when none
  matchRows: number[];                               // node indices present in the layout (GraphCanvas)
  next(): void; prev(): void;                        // wrap-around; calls revealCommitByOid
};
```
Behavior: `reqId` last-wins guard + ~250 ms debounce (mirror `useReadOverlays`/`refetchStatus`).
Cheap modes (`all`/`message`/`author`/`path`) fire on debounced `patchQuery`; **`content` fires
only on `submit()`** (Enter or the Search button). On results, compute `matchRows` from
`graphDataRef` (oid→index map, ignore misses), reset `currentMatch` to 0 and `revealCommitByOid`
the first match. `next/prev` step `currentMatch` (wrap) and reveal; a match not in the layout →
`revealCommitByOid` already toasts "not in current view".

### 5.2 `CommitSearchBar.tsx` + `SearchResultsList.tsx`
- Bar (rendered at the top of the graph pane by `WorkspaceGraphPane`): text input (autofocus on
  open), a `field` segmented control (All/Message/Author/Path/Content), a `regex` toggle (enabled
  only when `field==='content'`), a `caseSensitive` toggle, a "Search" button (content mode), a
  `n / m` counter with ↑/↓ buttons, a "truncated — showing first 1000" note when
  `results.truncated`, and a close (×). All state via the hook props.
- `SearchResultsList.tsx`: compact list of `results.matches` (short oid • summary • author • rel
  date • a `matched`-field badge); current row highlighted; click → `revealCommitByOid`. Shown as a
  dropdown under the bar (toggle) so it never permanently steals graph width.

### 5.3 GraphCanvas / draw.ts highlight
- `GraphCanvasProps` gains `matchRows?: readonly number[]` → passed into `draw.ts` `Interaction`.
- `Interaction` gains `matchRows: Set<number> | null` (component builds the Set once per prop
  change). Draw pass (after dots, before/with the head-ring pass): for a visible row in
  `matchRows`, stroke a 1.5 px ring in a new `--match-ring` theme color at r≈6.5 (distinct from the
  head ring and the selection ring). The current match is already the `selectedIndex` highlight.

### 5.4 Keyboard (`useWorkspaceKeyboard.ts`)
- **Esc-layering:** add `searchOpenRef` just below the transient overlays and above `diffSlot`
  (order: aiPanel → blame → history → reflog → commitBrowser → **search** → diffSlot → compare →
  deselect). When the search INPUT is focused, the existing typing-guard already lets the bar's own
  capture-phase Esc (Combobox idiom) close it first.
- **Shortcut gate:** add `searchOpen || paletteOpen` to the early-return set (alongside
  `dialogOpen || abortConfirmOpen`) so graph-nav keys don't fire underneath.
- **Open search:** Ctrl/Cmd-F → `openSearch()` (`preventDefault`; note the browser-harness caveat
  in OQ1). Also reachable from the palette + a graph-pane button.

---

## 6. Command palette (P50c)

### 6.1 `paletteActions.ts`
```ts
export type PaletteGroup = 'action' | 'branch' | 'tag' | 'commit' | 'search';
export interface PaletteAction {
  id: string; title: string; hint?: string; group: PaletteGroup;
  keywords?: string; disabled?: boolean; run(): void;
}
/** Assemble the static registry from existing handlers + refs. Pure w.r.t. deps. */
export function buildPaletteActions(deps: BuildPaletteDeps): PaletteAction[];
/** Case-insensitive subsequence score with a contiguous-run bonus; -1 = no match. Pure, unit-tested. */
export function fuzzyScore(query: string, text: string): number;
```
`BuildPaletteDeps` (from RepoWorkspace): existing handlers + gate flags (`mutating`, `canPullPush`,
`repoPath`, staged presence), `branches: BranchesSnapshot`, `graph: GraphLayout | null`,
`revealCommitByOid`, and `appCommands: PaletteAction[]` (threaded from App).
- **group 'action' (safe/confirmed only):** Fetch, Pull, Push, Refresh, New branch…, New worktree…,
  Stash changes, Open in terminal / file manager / editor (on `repoPath`) — each `run` = the SAME
  handler the toolbar/menu uses; `disabled` mirrors the toolbar (`mutating`, `canPullPush`). Any
  op that mutates dangerously (delete/discard/reset/drop) is EXCLUDED or, if added later, its `run`
  MUST open the existing confirm dialog — never a raw destructive IPC.
- **group 'branch'/'tag':** one row per `branches.local`+`branches.remote` / `branches.tags`;
  `run` = resolve oid then `revealCommitByOid` (jump). Branch oid = `BranchInfo.tip`; tag oid =
  scan `graph.nodes` for a `tag` ref with that name (tags carry no oid on the wire).
- **appCommands (from App):** Open repository, Clone…, Init…, Settings, Toggle theme, Toggle list
  view, Keyboard shortcuts, AI Assets, Health — all group 'action'.

### 6.2 `CommandPalette.tsx` (presentational)
```ts
export interface CommandPaletteProps {
  open: boolean;
  actions: PaletteAction[];
  onClose(): void;
  onRunSearch(text: string): void;      // the dynamic "Search commits for '<text>'" row
  onJumpToCommit(prefix: string): void; // dynamic row when input matches /^[0-9a-f]{4,40}$/
}
```
Renders a centered modal: input + grouped filtered list (fuzzy over `title`+`keywords`, group
headers). Dynamic top rows built from the current input: "Search commits for '<text>'" (any
non-empty text) and "Jump to commit <hex>" (hex-looking text). Enter runs the highlighted row then
`onClose()`; ↑/↓ move; **Esc closes via a capture-phase window listener** (Combobox idiom) so it
beats the workspace Esc-layering. Destructive rows are never present (see 6.1).

### 6.3 `usePalette.ts` + wiring
- `usePalette({ active, globalModalOpen })` owns `open` + a Ctrl/Cmd-K effect (active tab only,
  suppressed while `globalModalOpen`); toggles `open`. Returns `{ open, close }`.
- RepoWorkspace: `paletteOpen = palette.open`; feed into the keyboard shortcut gate (§5.4); render
  `<CommandPalette open actions={buildPaletteActions(...)} onClose onRunSearch={openSearch}
  onJumpToCommit={...} />`. `onJumpToCommit` resolves a node whose `id` starts with the prefix →
  `revealCommitByOid`, else toasts.
- App: build `appCommands` from its existing handlers; pass as a new `RepoWorkspace` prop
  `appCommands: PaletteAction[]`.

---

## 7. List filtering (P50d)

- `ListFilterInput.tsx`: a small controlled `<input class="list-filter">` (placeholder "Filter…",
  a clear ×). Own capture-phase Esc → clear then blur (does not close other overlays). Props:
  `{ value; onChange(v); ariaLabel; count?; }`.
- Pure helper `filterByName(names: string[], query: string): string[]` — case-insensitive substring;
  empty query → identity. (For remote-tracking rows match the `origin/name` shorthand; for branch
  rows match `BranchInfo.name`.)
- `Sidebar.tsx`: three local `useState<string>` (`branchFilter`, `remoteFilter`, `tagFilter`). Each
  of the Branches / Remotes / Tags sections renders `ListFilterInput` under its `SectionHeader`
  **only when expanded AND row count ≥ 6** (avoid clutter on short lists); the section then renders
  the filtered subset. No matches → a muted `"No {branches|remotes|tags} match '<q>'"` row.
  Detached-HEAD row and section "+" actions are unaffected. Filtering is display-only (never touches
  git). The global typing-guard already prevents shortcuts from firing while a filter is focused.

---

## 8. Mock (`src/ipc/mock/handlers/search.ts`)

`searchHandlers satisfies Partial<IpcApi>` with `searchCommits(repoId, query)`:
- `await delay(120)`; resolve the current layout exactly as `mock/handlers/diff.ts::getGraph` does
  (`generateLayout20k()` / `buildMockGraphDetached()` / `prependCommits(buildMockGraph(), …)`).
  **Recommend extracting that resolution into a shared `resolveLayout(state)` helper in P50a** so
  the two handlers stay in lockstep (flag if reviewer prefers duplication).
- `#fail` sentinel in `query.text` → throw `{ kind: 'git', message: 'Mock: search failed' }` (drives
  the error-toast path, mirrors `external.ts`).
- Otherwise filter `layout.nodes`: `message`/`all` → `summary.includes(text)`; `author` →
  `author.includes(text)`; `path`/`content` → heuristic over `summary` (fixtures carry no file
  data) — document this is UI-plumbing only. Case flag honored. Map hits → `SearchMatch`
  (`matched` per mode, `snippet` null). Apply the cap+1/`truncated` slice. Import + spread
  `searchHandlers` in `mock.ts`.

---

## 9. CLI-oracle test plan (P50a — `#[cfg(test)]` in `search.rs`)

Established pattern (graph tests + `git/stale.rs` CLI usage). Fixture: `tempfile::TempDir`, local
`user.name`/`user.email`, commits with **strictly-increasing `git2::Time`** (deterministic order),
known messages/authors, and real file contents (blob edits) so pickaxe/path have signal.
**Windows:** the test-running subagent MUST set `TMP`/`TEMP` to `D:\Temp` (C: is full — MEMORY
note); guard every CLI compare with `have_git()` (skip if git absent, like `stale.rs`).

- **Arg/parse units (no git, `GitRunner` fake):** `build_log_args` exact vecs for Path and Content
  (`-S`/`-G`, `-i`, `--max-count cap+1`, `--all` vs `scope_ref`, `--`+pathspec) incl. a
  `;`-bearing/space-bearing `text` staying ONE token; `parse_log_output` splits US-separated
  records, fills fields, and sets `truncated` at cap+1.
- **Oracle (git2 vs real `git`) — the load-bearing assertions:**
  - `message` == `git log --all -i -F --grep=<t> --format=%H` (full-message substring). Assert the
    exact ordered oid list.
  - `author` == `git log --all -i -F --author=<t> --format=%H`.
  - `all` == the UNION of the message-only and author-only CLI oid sets (two `git log` runs — avoids
    git's --grep/--author AND/OR combination ambiguity).
  - `scope_ref = Some("<branch>")` == `git log <branch> -i -F --grep=<t>` (subset of `--all`).
  - case-sensitive (`--all -F --grep`, no `-i`) differs from the insensitive run.
- **Oracle (shell modes — `SpawnGitRunner` against the fixture):**
  - `path` == `git log --all --format=%H -- <path>`.
  - content `-S` == `git log --all --format=%H -S<t>`; content `-G` == `git log --all -G<re>`.
- **Edge/cap:** > cap matches ⇒ `truncated==true` && `len==cap`; zero matches ⇒ empty, `Ok`;
  empty/whitespace `text` ⇒ empty, `Ok` (no git spawned — assert via a fake runner that panics);
  invalid `-G` regex ⇒ `Err(AppError::Git)`.

---

## 10. Sub-increment split + acceptance

### P50a — Search backend + IPC + mock + oracle
Scope: `git/search.rs` (types, `GitRunner`+`SpawnGitRunner`, `search_commits`, pure
builders/parsers, oracle+unit tests); `git/mod.rs`; `commands/search.rs` + `mod.rs` + `lib.rs`
(129); `types.ts` + `tauri.ts`; `mock/handlers/search.ts` + `mock.ts` spread (+ optional
`resolveLayout` extract).
**Acceptance:** (1) `cargo test -p bonsai-core search` green incl. every §9 oracle (git2 == CLI) and
arg/parse/cap tests; (2) `cargo build` + `cargo clippy -- -D warnings` clean; `generate_handler!`
lists 129; (3) `tsc`/`pnpm build` clean; (4) harness console: `await ipc.searchCommits('r', {field:
'message', text:'commit', regex:false, caseSensitive:false, maxResults:0, scopeRef:null, since:null,
until:null})` resolves with matching oids; `text:'#fail'` rejects `{kind:'git'}`.

### P50b — Search UI + graph highlight/jump
Scope: `useCommitSearch.ts`, `CommitSearchBar.tsx`, `SearchResultsList.tsx`; `GraphCanvas`
`matchRows` + `draw.ts` ring; `WorkspaceGraphPane` render + affordance; `useWorkspaceKeyboard`
(searchOpen Esc-layer + gate + Ctrl/Cmd-F); RepoWorkspace wiring.
**Acceptance:** (1) `pnpm build` clean; no file over the ~500-line soft limit; (2) harness: run a
message search → matching dots show `--match-ring`; the counter shows `n/m`; ↑/↓ scroll+select each
match (reusing reveal); (3) content mode does NOT fire per keystroke (only on Enter/button);
`truncated` shows the "first 1000" note (drive via the 20k fixture); (4) Esc closes the bar before
deselecting; nav keys inert while the bar is open.

### P50c — Command palette
Scope: `paletteActions.ts` (+ `fuzzyScore` test), `CommandPalette.tsx`, `usePalette.ts`;
RepoWorkspace render + gate; App `appCommands` prop.
**Acceptance:** (1) `pnpm build` clean; (2) harness: Ctrl/Cmd-K opens; fuzzy typing filters; Enter
runs an action (e.g. Refresh) and closes; a branch row jumps the graph to its tip; "Search commits
for '<text>'" opens the bar prefilled; a hex prefix offers "Jump to commit"; (3) Esc closes only the
palette (overlays beneath stay); (4) no destructive row present, and any confirm-backed action still
opens its dialog; nav keys inert while open.

### P50d — List filtering
Scope: `ListFilterInput.tsx` + `filterByName`, Sidebar wiring for Branches/Remotes/Tags.
**Acceptance:** (1) `pnpm build` clean; (2) harness: with ≥6 rows each, typing filters Branches,
Remotes, Tags live; no-match shows the muted state; clearing restores; (3) Esc in a filter clears it
without touching other overlays; global shortcuts don't fire while a filter is focused.

(b/c/d are order-independent; c+d could merge if time-boxed — recommend keeping separate for small
review diffs.)

---

## 11. Open questions (flag to orchestrator)

- **OQ1 — Ctrl/Cmd-F vs browser find.** Recommend Ctrl/Cmd-F to open search (with `preventDefault`);
  in the plain-browser harness the webview may still steal it, so also expose a graph-pane button +
  the palette entry (both harness-testable). Confirm the accelerator, or pick another (e.g. `/`).
- **OQ2 — Regex for message/author + an `invalidRegex` AppError.** v1: `regex` flag applies ONLY to
  content (`-S`/`-G`); message/author/path are substring/pathspec. Adding message/author regex means
  a `regex` crate dep in bonsai-core (recommend defer). Adding an `invalidRegex` kind gives a nicer
  toast than reusing `git` (recommend defer — `git` is fine). Confirm both.
- **OQ3 — Palette with no repo open.** Recommend: unavailable when no tab is open (EmptyState covers
  Open/Clone/Init). If you want a global palette there, App must host a minimal variant — defer.
- **OQ4 — Result metadata richness.** Recommend: `snippet` = the pathspec for Path, `None` elsewhere
  (a real `-S` content snippet needs `-p` parsing — heavy). `matched` is a single field (Message
  wins in All). Confirm, or ask for `matched: MatchedField[]` + content snippets.
- **OQ5 — Date scope semantics.** `since`/`until` filter **author** time in git2 modes but map to
  git's `--since`/`--until` (**commit** date) in shell modes — a minor cross-mode nuance. Recommend
  shipping the fields as-is (branch scope is the important one); confirm or drop date scope from v1.
