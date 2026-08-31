# M4 — Diff View: Implementation Contract

Status: authoritative for M4. Implementer: senior-dev, two fresh-context passes (M4a Rust,
M4b frontend). Builds on `docs/contracts/M0-scaffold.md` (AppError, IPC conventions),
`M1-status.md` (StatusSnapshot, FileStatus), `M2-graph.md` (GraphLayout, `GraphNode.id` = full
40-char oid, selection groundwork §4.6), `M3-commit.md` (path convention §2.1, right-panel
structure, mock statefulness), `ui-reference.md` (§2 tokens, §3 mono font, §7 file colors,
§8 states).

Locked product scope (CLAUDE.md — do not relitigate):
- Two diff kinds: (a) working-dir diffs — unstaged (workdir vs index) AND staged (index vs
  HEAD); (b) commit diffs — selected graph node vs its **first parent** (root commit: vs the
  empty tree).
- Right panel: commit details (message/author/date + changes) when a graph node is selected;
  working-dir status + staging otherwise.
- No hunk staging, no diff editing — read-only view.

Architecture invariants: Rust computes all diff data; React renders precomputed structures.
git2 under `spawn_blocking`. Compact wire payloads with hard size caps (§2.6). Mock IPC updated
in the same change as every IPC addition.

---

## 0. Headline decisions (details in the referenced sections)

1. **Unified diff only in v1** (§4.1). Side-by-side is Polish. Rationale: the right panel is
   380px; unified is the only presentation that fits without a layout rework, and it matches the
   GitButler-minimal feel. Rendered as a React DOM list (NOT canvas) — diffs are small, capped,
   and benefit from native text selection/copy.
2. **Lazy per-file hunks, upfront headers** (§3). `get_commit_diff(oid)` returns commit details
   + a per-file header list (status, +/- counts, binary flag) in one response; hunks for ONE
   file are fetched only when its row is expanded (`get_commit_file_diff` /
   `get_workdir_file_diff`). One interaction pattern for both modes; the IPC never carries a
   whole multi-file hunk payload.
3. **Command-response, no channels** (§3.4). Per-file diffs are capped at 5 000 lines
   (≲ 0.5 MB); M2d measured 5.4 MB serializing in ~12 ms — streaming buys nothing at these
   sizes. Upgrade path: an additive `stream_file_diff` channel command if a future need
   (uncapped diffs) appears; do NOT build it now.
4. **Diffs expand inline (accordion) under the file row in the right panel**, one file expanded
   at a time, horizontal scroll inside the diff block (§4.2). A wide center-pane diff view is
   Polish.
5. **Per-file cap: 5 000 emitted lines → `tooLarge: true` and NO hunks** (all-or-nothing, §2.6).
6. Fixed diff options: 3 context lines, rename detection on (`find_similar`), no whitespace
   options, standard interhunk (§2.5).

---

## 1. New / changed files

```
src-tauri/
  src/git/diff.rs             # NEW: FileDiff model + workdir/commit diff fns + unit tests
  src/git/mod.rs              # + pub mod diff;
  src/git/stage.rs            # extract path validation to pub(crate) validate_rel_path (reused)
  src/commands.rs             # + get_workdir_file_diff, get_commit_diff, get_commit_file_diff
  src/lib.rs                  # register the 3 commands
  tests/diff_cli.rs           # NEW: CLI-oracle integration tests (§6)
src/
  ipc/types.ts                # + DiffLine, Hunk, FileDiff, FileDiffHeader, CommitDetails,
                              #   CommitDiff; IpcApi + 3 methods
  ipc/tauri.ts                # + 3 invoke wrappers
  ipc/mock.ts                 # + 3 mock methods (canned, §5)
  ipc/fixtures/diffs.ts       # NEW: canned FileDiff / CommitDiff fixtures
  components/DiffView.tsx     # NEW: pure unified-diff renderer for one FileDiff
  components/StatusPanel.tsx  # + expandable rows hosting DiffView (mode A)
  components/CommitPanel.tsx  # NEW: commit details header + file list + DiffView (mode B)
  App.tsx                     # selection→commit-diff fetch, diff expansion state, Esc handling
  styles.css                  # diff table, expansion, commit header styles
```

## 2. Rust — data model + diff engine (`src-tauri/src/git/diff.rs`)

### 2.1 Wire types (implement exactly)

```rust
use crate::git::status::FileStatus;   // reused: added|modified|deleted|renamed|typechange|conflicted|untracked

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LineKind { Context, Add, Del }

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: LineKind,
    /// Line number in the OLD file; None for Add lines.
    pub old_no: Option<u32>,
    /// Line number in the NEW file; None for Del lines.
    pub new_no: Option<u32>,
    /// Content WITHOUT the leading +/-/space and WITHOUT the trailing newline (§2.4).
    pub content: String,
    /// True when this is the last line of a file that has no trailing newline
    /// (the CLI's "\ No newline at end of file" marker — never emitted as its own line).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub no_newline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}
// No raw header string on the wire: the frontend renders "@@ -a,b +c,d @@" from the numbers.
// git2's function-context tail is dropped (Polish nicety).

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    /// NEW path for renames; repo-relative, forward slashes (StatusEntry convention).
    pub path: String,
    /// OLD path for renames; None otherwise.
    pub orig_path: Option<String>,
    pub status: FileStatus,
    pub binary: bool,      // true -> hunks empty
    pub too_large: bool,   // true -> hunks empty (§2.6)
    pub hunks: Vec<Hunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiffHeader {
    pub path: String,
    pub orig_path: Option<String>,
    pub status: FileStatus,
    pub additions: u32,    // count of Add lines (0 for binary)
    pub deletions: u32,    // count of Del lines (0 for binary)
    pub binary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetails {
    pub oid: String,             // full 40-char hex
    pub summary: String,         // first line
    /// Full message, trailing whitespace trimmed. Includes the summary line.
    pub message: String,
    pub author_name: String,     // lossy UTF-8, like GraphNode.author
    pub author_email: String,
    pub author_ts: i64,          // seconds since epoch (UTC), matches GraphNode.ts convention
    pub committer_ts: i64,       // shown by the UI only when it differs meaningfully (not v1; carried for free)
    /// Full parent oids, first parent first. len > 1 => merge commit.
    pub parents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDiff {
    pub details: CommitDetails,
    /// Sorted by path ascending (byte-wise). Headers only — no hunks (decision §0.2).
    pub files: Vec<FileDiffHeader>,
}
```

### 2.2 Public functions (blocking; repos opened with NO_SEARCH like `read_status`)

```rust
/// Diff of ONE working-dir file.
/// staged == false -> index vs workdir (git diff -- <path>); untracked files supported
///                    (include_untracked + show_untracked_content + recurse_untracked_dirs
///                    -> all-Add hunk, status Untracked).
/// staged == true  -> HEAD tree vs index (git diff --cached -- <path>); unborn HEAD -> old
///                    side is the empty tree (None), i.e. everything staged shows as Added.
/// `orig_path`: Some for renames (frontend passes StatusEntry.origPath); pathspec then
/// includes BOTH paths and find_similar pairs them into one Renamed delta.
/// Paths validated per M3 §2.1 rules via validate_rel_path (reject empty/absolute/`..`).
/// If the pathspec matches no delta (file racing to clean), return an empty FileDiff:
/// status Modified, binary false, too_large false, hunks empty — NOT an error (the UI shows
/// "No changes"; the next status refetch removes the row).
pub fn workdir_file_diff(workdir: &Path, path: &str, orig_path: Option<&str>, staged: bool)
    -> Result<FileDiff, AppError>;

/// Commit details + per-file headers for commit `oid` vs its FIRST parent.
/// Root commit -> vs empty tree (parent tree None). Merge commit -> first parent only.
/// Bad/unknown oid -> AppError::Git (message from git2). Non-commit oid -> AppError::Git.
pub fn commit_diff(workdir: &Path, oid: &str) -> Result<CommitDiff, AppError>;

/// Hunks for ONE file of the commit-vs-first-parent diff. Same pathspec/rename handling as
/// workdir_file_diff. No matching delta -> AppError::Git("path not changed in commit: {path}")
/// (unlike the workdir case this cannot be a benign race — the header list came from the same
/// immutable commit).
pub fn commit_file_diff(workdir: &Path, oid: &str, path: &str, orig_path: Option<&str>)
    -> Result<FileDiff, AppError>;
```

### 2.3 Diff construction (normative pseudocode)

```
build_diff_options(paths):
    opts = DiffOptions::new()
    opts.context_lines(3)                 # fixed for v1
    opts.include_untracked(true).show_untracked_content(true).recurse_untracked_dirs(true)
                                          # harmless for tree-to-tree; needed for untracked files
    for p in paths: opts.pathspec(p)      # path + orig_path when present
    return opts

make_diff(repo, kind, paths):
    match kind:
      Unstaged      -> repo.diff_index_to_workdir(None, opts)
      Staged        -> old = HEAD tree or None if unborn (head() err UnbornBranch/NotFound)
                       repo.diff_tree_to_index(old.as_ref(), None, opts)
      Commit(c)     -> old = c.parent(0).tree() or None if c.parent_count() == 0
                       repo.diff_tree_to_tree(old.as_ref(), Some(&c.tree()), opts)
    find_opts = DiffFindOptions::new(); find_opts.renames(true)
    diff.find_similar(Some(&mut find_opts))    # AFTER pathspec restriction; pairs old+new
    return diff
```

Single-file hunk collection (`collect_file_diff(diff) -> FileDiff`):

```
state: hunks = [], cur_lines = [], line_budget = 5000, binary = false, aborted = false
diff.foreach(
  file_cb:   capture delta -> path (new_file, fallback old_file), orig_path
             (old_file path when delta.status == Renamed|Copied), status map (§2.7);
             delta.flags BINARY -> binary = true
  binary_cb: Some(|_,_| { binary = true; true })      # required so libgit2 reports binaries
  hunk_cb:   push previous hunk; start new Hunk from DiffHunk {old_start, old_lines,
             new_start, new_lines}
  line_cb:   match line.origin():
               ' ' -> Context (old_no+new_no from line.old_lineno/new_lineno)
               '+' -> Add (old_no None), '-' -> Del (new_no None)
               '=' | '>' | '<' (EOFNL markers) -> set no_newline = true on the LAST pushed
                     DiffLine; emit nothing
               other ('F','H','B') -> ignore
             content: §2.4; decrement line_budget; if budget exhausted:
               aborted = true; return false            # aborts foreach with GIT_EUSER
)
if foreach err: code == User && aborted -> proceed; else -> propagate as AppError::Git
if binary          -> FileDiff { binary: true,  too_large: false, hunks: [] }
else if aborted    -> FileDiff { binary: false, too_large: true,  hunks: [] }   # all-or-nothing
else               -> push final hunk; FileDiff { hunks }
```

`commit_diff` header collection: one `diff.foreach` over the UNRESTRICTED (no pathspec) diff
with file_cb + binary_cb + line_cb counting Add/Del per delta (keyed by the current file_cb
delta); no hunk storage, no line budget (counts only — content strings are never built). Sort
`files` by `path` at the end. Details from `repo.find_commit(Oid::from_str(oid)?)`: summary =
`commit.summary()` lossy (empty string if None), message = `String::from_utf8_lossy(commit
.message_bytes()).trim_end()`, author/committer per signature (lossy name/email, `.when()
.seconds()`), parents = `commit.parent_ids().map(to_string)`.

### 2.4 Line content policy (normative)

- Raw bytes → `String::from_utf8_lossy` (never error on non-UTF-8; replacement chars are fine —
  truly binary files are caught by the binary flag first).
- Strip exactly one trailing `"\n"` if present, then exactly one trailing `"\r"` if present
  (CRLF repos render clean; a lone mid-line `\r` is preserved as-is).
- No length cap per line (the 5 000-line budget bounds totals); the renderer scrolls
  horizontally.

### 2.5 Fixed options (v1 — no user settings)

Context 3 lines; rename detection `DiffFindOptions.renames(true)` only (no copies); no
whitespace-ignore flags; libgit2 default binary heuristics + default `max_size` (512 MB blob
guard is not our concern; the line budget is).

### 2.6 Size guards

- `pub const MAX_FILE_DIFF_LINES: usize = 5_000;` — total emitted DiffLines per file. Exceeded
  → abort iteration, `too_large: true`, `hunks: []` (all-or-nothing; truncated hunks would need
  a "truncated at line N" UI + partial-hunk semantics for zero v1 value — the UI shows
  "Diff too large to display (> 5000 lines)").
- Binary → `binary: true`, `hunks: []` (UI: "Binary file").
- Worst-case wire size: 5 000 lines × ~100 B ≈ 0.5 MB per response — trivially inside a single
  invoke (M2 measurement precedent).

### 2.7 Delta status mapping (git2 `Delta` → `FileStatus`)

`Added|Copied→Added` is WRONG for copies — map `Copied→Renamed` (copies disabled anyway;
defensive). Full map: `Added→Added`, `Deleted→Deleted`, `Modified→Modified`,
`Renamed→Renamed`, `Copied→Renamed`, `Typechange→Typechange`, `Untracked→Untracked`,
`Conflicted→Conflicted`, anything else (`Unmodified|Ignored|Unreadable`) → `Modified`
(unreachable in practice; never panic).

### 2.8 Commands (`src-tauri/src/commands.rs`) + registration

Same `_inner` + `spawn_blocking` pattern as `get_status` (state lock → clone PathBuf → drop
lock → spawn_blocking → join error to `AppError::Other`; `NoRepo` when nothing open):

```rust
#[tauri::command]
pub async fn get_workdir_file_diff(state: tauri::State<'_, AppState>,
    path: String, orig_path: Option<String>, staged: bool) -> Result<FileDiff, AppError>;

#[tauri::command]
pub async fn get_commit_diff(state: tauri::State<'_, AppState>, oid: String)
    -> Result<CommitDiff, AppError>;

#[tauri::command]
pub async fn get_commit_file_diff(state: tauri::State<'_, AppState>,
    oid: String, path: String, orig_path: Option<String>) -> Result<FileDiff, AppError>;
```

Register all three. Command surface after M4: `open_repo`, `get_status`, `get_graph`, `stage`,
`unstage`, `commit`, `get_workdir_file_diff`, `get_commit_diff`, `get_commit_file_diff`.
Events: `repo-changed` (unchanged). Channels: none (decision §0.3). No new AppError variants.

## 3. IPC layer (TypeScript)

`src/ipc/types.ts` (mirrors §2.1 exactly):

```ts
export type LineKind = 'context' | 'add' | 'del';

export interface DiffLine {
  kind: LineKind;
  oldNo: number | null;
  newNo: number | null;
  content: string;
  /** Present (true) only on the last line of a file lacking a trailing newline. */
  noNewline?: boolean;
}

export interface Hunk {
  oldStart: number; oldLines: number; newStart: number; newLines: number;
  lines: DiffLine[];
}

export interface FileDiff {
  path: string;
  origPath: string | null;
  status: FileStatus;
  binary: boolean;    // true -> hunks empty
  tooLarge: boolean;  // true -> hunks empty
  hunks: Hunk[];
}

export interface FileDiffHeader {
  path: string;
  origPath: string | null;
  status: FileStatus;
  additions: number;
  deletions: number;
  binary: boolean;
}

export interface CommitDetails {
  oid: string;
  summary: string;
  message: string;        // full, trailing whitespace trimmed
  authorName: string;
  authorEmail: string;
  authorTs: number;       // seconds since epoch
  committerTs: number;
  parents: string[];      // full oids, first parent first
}

export interface CommitDiff {
  details: CommitDetails;
  files: FileDiffHeader[];  // sorted by path
}
```

`IpcApi` gains:

```ts
/** Diff of one working-dir file. staged=false: index vs workdir; staged=true: HEAD vs index.
 *  origPath: pass StatusEntry.origPath (renames). Rejects AppError ('noRepo', 'git'). */
getWorkdirFileDiff(path: string, origPath: string | null, staged: boolean): Promise<FileDiff>;
/** Commit details + per-file headers vs first parent. Rejects AppError ('noRepo', 'git'). */
getCommitDiff(oid: string): Promise<CommitDiff>;
/** Hunks for one file of a commit's first-parent diff. */
getCommitFileDiff(oid: string, path: string, origPath: string | null): Promise<FileDiff>;
```

`src/ipc/tauri.ts`:

```ts
getWorkdirFileDiff: (path, origPath, staged) =>
  invoke<FileDiff>('get_workdir_file_diff', { path, origPath, staged }),
getCommitDiff:      (oid) => invoke<CommitDiff>('get_commit_diff', { oid }),
getCommitFileDiff:  (oid, path, origPath) =>
  invoke<FileDiff>('get_commit_file_diff', { oid, path, origPath }),
```

No capability changes.

## 4. Frontend

### 4.1 DiffView (`src/components/DiffView.tsx`) — pure renderer, no ipc imports

```ts
export interface DiffViewProps { diff: FileDiff; }
export function DiffView({ diff }: DiffViewProps): JSX.Element;
```

- `diff.binary` → centered `--text-3` message "Binary file". `diff.tooLarge` → "Diff too large
  to display (> 5000 lines)". `hunks.length === 0` otherwise → "No changes".
- Otherwise: one `<div class="diff-view">` with `overflow-x: auto`, mono font
  (ui-reference §3), 12px, line-height 1.5, `--bg-0` background, 6px radius, 1px `--border`.
- Per hunk: a hunk-header row `@@ -{oldStart},{oldLines} +{newStart},{newLines} @@` in
  `--text-3` on `--bg-2`, then line rows. Each line row (CSS grid, `white-space: pre`):
  - old line number (right-aligned, 40px min, `--text-3`, empty for Add),
  - new line number (same, empty for Del),
  - marker `+` / `−` / space (mono, 16px column),
  - content (`white-space: pre`, no wrap).
  - Row background: Add → `--success` at 12% alpha; Del → `--danger` at 12% alpha; Context →
    transparent. Marker + numbers inherit; content `--text-1`.
  - `noNewline` line: append a trailing `⏎̸`-substitute — render a small `--text-3` suffix
    `" ␀ no newline"`? NO — keep it simple: an extra pseudo-row after that line, `--text-3`
    italic, content `\ No newline at end of file` (matches CLI familiarity), not selectable
    as diff content (aria-hidden).
- Rendering perf: capped at 5 000 lines; plain DOM list is fine (no virtualization). Intraline
  (word-level) highlighting: **Polish**, not M4.

### 4.2 Mode A — working-dir diffs in StatusPanel (accordion)

Decision: diffs expand **inline under the file row** inside the existing right panel
(380px, `overflow-x` inside the diff block), one expanded row at a time across ALL sections.
Rejected alternatives: fixed diff area below the lists (steals space permanently, awkward with
CommitBox pinned at bottom); center-pane overlay (hides the centerpiece graph; GitKraken-style
panel expansion is a Polish upgrade).

New StatusPanel props (stays presentational):

```ts
export interface DiffSlot {
  /** Expansion key this slot belongs to, e.g. "staged:src/main.rs". */
  key: string;
  state: 'loading' | 'error' | 'ready';
  diff: FileDiff | null;      // when ready
  error: string | null;       // when error
}
export interface StatusPanelProps {
  // ...M3 props unchanged...
  /** Currently expanded diff (null = none). */
  diffSlot: DiffSlot | null;
  /** Toggle expansion for a row. section: 'staged' | 'unstaged' | 'untracked'. */
  onToggleDiff(section: 'staged' | 'unstaged' | 'untracked', entry: StatusEntry): void;
}
```

- Row key convention (shared with App): `` `${section}:${entry.path}` ``.
- Clicking a file row's TEXT area (not the +/− action button) calls `onToggleDiff`; the row
  gets `aria-expanded`, a subtle chevron (`--text-3`, rotates when open), and `--bg-2`
  background while expanded. Clicking the expanded row again collapses (App passes
  `diffSlot: null`).
- Under the expanded row: `state === 'loading'` → 3 skeleton bars; `'error'` → inline
  dismissible error banner (§8 style); `'ready'` → `<DiffView diff={...}/>`. Max height 45% of
  the panel with `overflow-y: auto` inside, so the lists stay reachable.
- Conflicted rows: NOT expandable (no diff kind defined for conflicts in v1).
- Section determines the diff kind: `staged` rows → staged diff; `unstaged`/`untracked` rows →
  unstaged diff. A file in both staged and unstaged shows different diffs from its two rows.

### 4.3 Mode B — CommitPanel (`src/components/CommitPanel.tsx`)

Shown INSTEAD of StatusPanel + CommitBox when a commit is selected. Presentational:

```ts
export interface CommitPanelProps {
  node: GraphNode;                       // selected node (refs for pills-as-text if desired: v1 skips)
  data: CommitDiff | null;               // null while loading
  loading: boolean;
  error: string | null;
  diffSlot: DiffSlot | null;             // same accordion mechanism, key = `commit:${path}`
  onToggleDiff(file: FileDiffHeader): void;
  /** Parent short-oid clicked; App maps to a row via node.parents indices. */
  onSelectParent(parentOrdinal: number): void;
  onClose(): void;                       // "×" button -> deselect
}
```

Layout (top to bottom, panel scrolls as one column):
- Header block (12px padding, 1px bottom border): summary (13px, 600, `--text-1`, wraps);
  short oid (mono 12px, `--text-2`, `title` = full oid); author line
  `{authorName} <{authorEmail}>` (12px `--text-2`, email `--text-3`); date line: relative +
  absolute (`new Date(authorTs*1000).toLocaleString()`), 12px `--text-3`; parent links: label
  "Parents:" + one mono short-oid button per parent (accent color, underline on hover) →
  `onSelectParent(i)`; merge commits additionally show a `--text-3` 11px note
  "Showing changes vs first parent". Close button "×" top-right (icon button).
- Message body: when `message` has more than the summary line, the remainder in a
  `white-space: pre-wrap` block, 12px `--text-2`, collapsed to 8 lines with a "Show more"
  toggle (local state).
- File list: section header `Changes ({files.length})`, rows styled like StatusPanel rows
  (badge per §7 colors, dir/name split, rename `old → new`), plus right-aligned
  `+{additions}` in `--success` and `−{deletions}` in `--danger` (mono 11px; "bin" in
  `--text-3` for binary). Rows expand via the §4.2 accordion pattern (`onToggleDiff`).
- `loading && data === null` → skeleton rows; `error` → inline banner + retry not required
  (refetch happens on reselection).

### 4.4 App wiring (`App.tsx`)

New state:

```ts
const [commitDiff, setCommitDiff] = useState<CommitDiff | null>(null);
const [commitDiffLoading, setCommitDiffLoading] = useState(false);
const [commitDiffError, setCommitDiffError] = useState<string | null>(null);
const [diffSlot, setDiffSlot] = useState<DiffSlot | null>(null);   // shared by both modes
const commitDiffReqId = useRef(0);
const fileDiffReqId = useRef(0);
```

- **Selection → commit diff.** Effect on `[selectedIndex, graph]`: if `selectedIndex !== null
  && graph !== null` → `oid = graph.nodes[selectedIndex].id`; bump `commitDiffReqId`, set
  loading, `ipc.getCommitDiff(oid)`, apply result only if id still current (identical last-wins
  pattern as `refetchStatus`). Also reset `diffSlot` to null on every selection change.
  If `selectedIndex === null` → clear commitDiff/loading/error and any `commit:*` diffSlot.
  (`refetchGraph` already resets `selectedIndex` on new layouts — indices stay valid.)
- **Row expansion (both modes).** `toggleDiff(key, fetcher)`: if `diffSlot?.key === key` →
  `setDiffSlot(null)` (collapse). Else bump `fileDiffReqId`, `setDiffSlot({key, state:
  'loading', diff: null, error: null})`, await fetcher, apply if id current →
  `{state:'ready', diff}` / `{state:'error', error: errorMessage(e)}`.
  - Mode A: `onToggleDiff(section, entry)` → key `` `${section}:${entry.path}` ``, fetcher
    `() => ipc.getWorkdirFileDiff(entry.path, entry.origPath, section === 'staged')`.
  - Mode B: `onToggleDiff(file)` → key `` `commit:${file.path}` ``, fetcher
    `() => ipc.getCommitFileDiff(oid, file.path, file.origPath)`.
- **Snapshot changes invalidate mode-A expansion.** In `refetchStatus`'s success path (or an
  effect on `status`): if `diffSlot` is a mode-A key, look up the entry in the new snapshot's
  matching section by path — present → re-run its fetcher (content may have changed); absent →
  `setDiffSlot(null)`. Mode-B slots are untouched by status changes (commits are immutable).
- **Deselection:** existing `onSelect(null)` (empty canvas click) + NEW window `keydown`
  listener: `Escape` → if `selectedIndex !== null` `setSelectedIndex(null)` (subscribe in the
  existing subscription effect or its own effect; skip when focus is in a textarea/input).
- **Parent links:** `onSelectParent(i)` → `const p = graph.nodes[selectedIndex].parents[i];
  if (p !== undefined) setSelectedIndex(p)`. (GraphNode.parents are already node indices —
  truncated-away parents are simply absent; guard with `parents[i] !== undefined` using the
  CommitDetails↔GraphNode ordinal match: both are first-parent-first. When
  `details.parents.length > node.parents.length` the missing ones render as plain text, not
  buttons.)
- **Right panel render:**

```tsx
<aside className="right-panel">
  {selectedIndex !== null && graph !== null ? (
    <CommitPanel node={graph.nodes[selectedIndex]} data={commitDiff}
                 loading={commitDiffLoading} error={commitDiffError}
                 diffSlot={diffSlot} onToggleDiff={...}
                 onSelectParent={...} onClose={() => setSelectedIndex(null)} />
  ) : (
    <> <StatusPanel ... diffSlot={diffSlot} onToggleDiff={...} /> <CommitBox ... /> </>
  )}
</aside>
```

GraphCanvas needs NO changes (selection props exist since M2). Optional NIT-level: keep
`cursor: default`.

## 5. Mock IPC (`src/ipc/mock.ts` + `src/ipc/fixtures/diffs.ts`)

Decision: **static per-path canned diffs** — the stateful M3 status mock keeps moving entries
between lists, and the diff for a path is the same canned object wherever it sits. This is
"consistent enough" for the harness (a staged file HAS a diff); simulating true
staged-vs-unstaged content splits is not worth the state machine.

`fixtures/diffs.ts` exports (all plain data + tiny builders):

```ts
export function mockWorkdirDiff(path: string, origPath: string | null, staged: boolean): FileDiff;
export function mockCommitDiff(index: number, oid: string): CommitDiff;   // index = row in buildMockGraph()
export function mockCommitFileDiff(oid: string, path: string, origPath: string | null): FileDiff;
```

Canned workdir diffs (keyed by exact path; anything unknown → generic 1-hunk modified diff):
- `src/main.rs` → modified, **2 hunks** (realistic Rust content, ~8 lines each, mixed
  context/add/del, correct oldNo/newNo).
- `src/app.rs` → added: single hunk `@@ -0,0 +1,12 @@`, all Add lines, oldNo null.
- `old-config.toml` → deleted: single hunk `@@ -1,9 +0,0 @@`, all Del, newNo null; last line
  `noNewline: true` (exercises the marker row).
- `docs/getting-started.md` (origPath `docs/intro.md`) → renamed + modified: 1 hunk with a few
  changed lines; `origPath` set.
- `notes/todo.txt`, `scratch.rs` → untracked: all-Add hunk, status `'untracked'`.
- NEW fixture entries appended to `INITIAL_STATUS.unstaged`:
  `{ path: 'assets/logo.png', origPath: null, status: 'modified' }` → diff `binary: true`, and
  `{ path: 'data/big-report.csv', origPath: null, status: 'modified' }` → diff
  `tooLarge: true` (both `hunks: []`). (Yes, this changes the M3 fixture — the M3 smoke list
  counts are updated implicitly; both files behave normally for stage/unstage.)
- `src/shared/util.rs` → two variants by `staged` flag (different single hunks) — the one
  path where the staged/unstaged distinction is made visible.

Canned commit diffs, routed by ROW INDEX (mock.ts builds `buildMockGraph()` once at call time
and finds `nodes.findIndex(n => n.id === oid)` — robust against id spelling):
- row 0 (octopus merge "Merge feat and exp") → details: multi-line message (summary + 2-line
  body), authorName "Ada Lovelace", email `ada@example.com`, `parents` = the FULL oids of rows
  3, 1, 2; files: 3 headers (one modified with +12/−4, one added +30/−0, one binary).
- row 1 ("feat: polish") → 1 parent, 2 files (modified, renamed).
- row 7 ("core work 1", tag v0.9) → 1 parent, 1 file.
- any other row → generic: 1 parent (first parents entry's node id when available, else a fake
  oid), 1 modified file `+3/−1`.
- unknown oid (not in the layout) → throw `{ kind: 'git', message: 'mock: unknown commit' }`.

`mockCommitFileDiff`: returns a canned FileDiff matching the header (same status/binary), by
path; generic fallback. `mock.ts` methods: `await delay(150)` first, then return
`structuredClone` of fixtures (callers own copies). Detached/20k fixtures: same routing (20k
nodes fall through to the generic commit diff — fine).

## 6. Testing (contract for tester)

**HARD RULE (M3 §6.0): all scratch/temp on D:.** Every test uses the existing
`scratch_dir()` helper (`src-tauri/src/testutil.rs`, re-exported via `tests/common/mod.rs`);
run sessions with `TMP`/`TEMP=D:\Temp`. Pin in every fixture repo: `core.autocrlf=false`,
`init.defaultBranch=main`, repo-local user.name/user.email — so CLI and git2 see identical
bytes and CRLF content stays byte-faithful on both sides.

### 6.1 Oracle normalization (the load-bearing part)

Compare **parsed structures, not raw text**. Test helper `parse_cli_diff(output: &str) ->
Vec<ParsedFile>` where `ParsedFile { path, orig_path, hunks: Vec<ParsedHunk> }`,
`ParsedHunk { old_start, old_lines, new_start, new_lines, lines: Vec<(char, String, bool)> }`
(kind char, content, no_newline):

- Skip `diff --git`, `index`, `old/new mode`, `similarity`, `rename from/to`, `new file`,
  `deleted file`, `---`, `+++`, `Binary files ... differ` header lines (capture rename
  from/to → orig_path; capture `Binary files` → assert our `binary: true` instead of hunks).
- `@@ -a,b +c,d @@ tail` → parse the four numbers ONLY (omitted count = 1); ignore the
  function-context tail (we drop it by design).
- ` ` / `+` / `-` lines → (kind, content minus prefix, false); strip `\n` then `\r` from
  content (same policy as §2.4).
- `\ No newline at end of file` → set no_newline=true on the previous line.
- Compare against our FileDiff mapped to the same shape (DiffLine old/new numbers additionally
  recomputed from hunk starts in the test and asserted consistent).

CLI commands (always `--no-color -U3 -M`):
unstaged → `git diff -- <paths>`; staged → `git diff --cached -- <paths>`;
commit → `git diff <oid>^1 <oid> -- <path>` (merge: same, `^1` IS first-parent);
root commit → `git show --format= --no-color -U3 -M <oid>`;
untracked (CLI has no direct equivalent) → oracle is `git diff --no-index /dev/null <file>`
adjusted, OR simpler: assert structurally (one hunk `-0,0 +1,n`, all Add, contents equal the
file lines) — structural assertion is the contract; skip the no-index dance.

### 6.2 Rust test scenarios (`tests/diff_cli.rs` + unit tests in `git/diff.rs`)

Each builds a scratch repo (git2 or CLI, pinned config), applies edits, then asserts
our fn vs the parsed oracle:

1. `unstaged_modified_multi_hunk` — file with 40 lines, edit lines 3 and 30 → 2 hunks; exact
   hunk numbers, line kinds/numbers/contents match CLI.
2. `staged_modified` — same edit staged → `workdir_file_diff(staged=true)` vs
   `git diff --cached`; and the unstaged diff of that file is empty.
3. `staged_vs_unstaged_split` — stage an edit, edit again → the two diffs differ and each
   matches its oracle.
4. `untracked_file` — structural all-Add assertion (above), status `Untracked`.
5. `deleted_file` — fs-delete tracked file → unstaged all-Del; then stage → staged all-Del.
6. `renamed_modified_staged` — `git mv` + small edit + stage; call with
   `orig_path: Some(old)` → status Renamed, orig_path set, hunks match
   `git diff --cached -M -- old new`.
7. `no_trailing_newline` — file without final `\n`, modify last line → `no_newline: true` on
   the right lines, matches the CLI marker positions (add AND del sides).
8. `binary_file` — commit a small PNG-like blob (include NUL bytes), modify → `binary: true`,
   `hunks: []`; CLI prints `Binary files ... differ`.
9. `too_large_cap` — generate a 6 000-line file, delete it (unstaged) → `too_large: true`,
   `hunks: []`; and a 100-line file stays under cap (`too_large: false`).
10. `crlf_content` — commit a CRLF file (autocrlf=false), modify one line → contents match CLI
    byte-for-byte after §2.4 stripping; no phantom `^M`-only changes.
11. `commit_diff_simple` — 2-commit repo; `commit_diff(tip)`: details (oid, summary, full
    multi-line message trimmed, author name/email, ts == `git log -1 --format=%at`, parents ==
    `git rev-parse tip^@` order), headers' +/- match `git diff --numstat tip^1 tip` (numstat is
    the counts oracle), files sorted by path.
12. `commit_file_diff_matches_show` — hunks vs parsed `git diff tip^1 tip -- <path>`.
13. `root_commit` — `commit_diff(root)`: `parents: []`, all files Added; file diff matches
    parsed `git show --format= root`.
14. `merge_commit_first_parent` — build a merge; `commit_diff(merge)` matches
    `git diff merge^1 merge` (NOT `--cc`); details.parents has both oids in order.
15. `unborn_staged` — unborn repo, stage a file → staged diff all-Add vs
    `git diff --cached` output.
16. `bad_oid_and_path_validation` — garbage oid → `AppError::Git`; `../escape` path →
    `AppError::Other("invalid path: ...")` (reused validator); `commit_file_diff` for an
    untouched path → `AppError::Git`.
17. Command-level (`commands.rs` tests): all three `_inner` fns return `NoRepo` when nothing
    open (mirrors the M3 test).

### 6.3 Frontend smoke (browser harness, `VITE_MOCK_IPC=1 pnpm dev`)

1. Mode A: click `src/main.rs` (staged) row text → chevron rotates, skeleton, then a 2-hunk
   unified diff with old/new line numbers, green/red rows, hunk headers; click again →
   collapses. Only one row expanded at a time (expanding another collapses the first).
2. `old-config.toml` → all-red diff ending with the italic "\ No newline at end of file" row.
3. `assets/logo.png` → "Binary file"; `data/big-report.csv` → "Diff too large..." message.
4. Stage an expanded unstaged file via its `+` button → after refetch the row moved sections
   and the expansion collapsed (entry left the section).
5. Mode B: click a graph row → right panel swaps to commit details (summary, mono short oid,
   author, dates, parents); StatusPanel + CommitBox gone. Row 0 shows the merge note + 3 files
   with +/- counts (one "bin"); expanding a file shows its hunks.
6. Parent link click → selection jumps to the parent row (canvas highlight moves, panel
   updates). Esc → back to mode A (status + commit box). Empty-canvas click → same.
7. Rapidly click two different commits → only the later one's details render (no flicker of
   the earlier response — request-id guard).
8. No `@tauri-apps/*` module executed; no console errors.

## 7. Sub-increment split for senior-dev

- **M4a — Rust diff core + commands + oracle tests.** `git/diff.rs` (§2), `validate_rel_path`
  extraction, commands + registration (§2.8), tests §6.1–§6.2. Gate: `cargo test` green,
  `cargo clippy -- -D warnings` clean, scratch dirs on D: only.
- **M4b — Frontend + IPC/mock.** `types.ts`/`tauri.ts`/`mock.ts`/`fixtures/diffs.ts` (§3, §5),
  `DiffView`, StatusPanel expansion, `CommitPanel`, App wiring (§4), styles. Gate:
  `pnpm build` green; §6.3 smoke passes in the harness.
  Fallback split if M4b reviews too large: **M4b1** = DiffView + StatusPanel mode A + mock
  workdir diffs; **M4b2** = CommitPanel + selection wiring + mock commit diffs. Orchestrator's
  call after seeing M4b's first diff.

## 8. Acceptance criteria — overall M4

AI gate:
- All §6.2 Rust tests pass with the CLI as oracle; `cargo check`/`clippy`/`test` and
  `pnpm build` green after each sub-increment.
- Harness renders BOTH diff kinds from mock data (screenshots: mode-A expanded 2-hunk diff,
  binary/too-large messages, no-newline row; mode-B commit details + file list + expanded
  file; Esc/parent-link navigation) — full §6.3 list.

USER CHECKPOINT (never self-declared): in the native app on a scratch repo — selecting a
commit in the graph shows its details (message/author/date/parents) and its changes; expanding
a file shows a correct unified diff; clicking a working-dir file shows its unstaged/staged
diff; a terminal `git show`/`git diff` agrees with what's displayed.

## 9. Ambiguities resolved here (flag to orchestrator if disagreed)

- **Unified diff only; DOM-rendered, not canvas** — 380px panel, capped sizes, native text
  selection; side-by-side + intraline highlighting + wide center-pane diff are Polish.
- **Lazy per-file hunks + upfront header list** over whole-snapshot/whole-commit hunk payloads
  — bounded responses, one accordion interaction for both modes; costs one extra command
  (`get_commit_file_diff`).
- **Command-response with a 5 000-line per-file cap; no channels** — M2's measured serialize
  numbers make streaming pointless at these sizes; `stream_file_diff` channel is the additive
  upgrade path.
- **tooLarge is all-or-nothing (no truncated hunks)** — partial-hunk semantics and a
  "truncated" UI aren't worth it in v1; the message tells the user why.
- **No raw hunk header string on the wire; function-context tail dropped** — numbers only,
  frontend formats `@@ -a,b +c,d @@`; oracle compares numbers only.
- **`\ No newline` as a `noNewline` flag on the preceding line** (never a DiffLine of its own);
  renderer shows the familiar CLI marker row.
- **Content = lossy UTF-8, strip one `\n` then one `\r`** — CRLF repos render clean; binary
  guard catches real byte soup first.
- **Accordion inline in the right panel, single expansion, 45%-height scroll area** — over a
  dedicated diff pane or center-pane takeover (graph stays visible; fixed-pane v1 layout
  untouched).
- **Mode B replaces StatusPanel + CommitBox entirely** (with an explicit "×" + Esc + empty-
  canvas-click to return) — a split panel showing both would be unusable at 380px.
- **Parent links navigate via `GraphNode.parents` indices** (ordinal-matched to
  `CommitDetails.parents`); parents dropped by truncation render as plain text.
- **Mock diffs are static per-path canned objects** (one `staged` variant for exactly one
  path); commit fixtures routed by row index of the 30-row graph — no id coupling. Two new
  fixture status entries (binary + too-large) knowingly extend the M3 mock snapshot.
- **`workdir_file_diff` on a no-longer-dirty path returns an empty FileDiff, not an error**
  (watcher race); `commit_file_diff` on an untouched path IS an error (immutable input).
- **Reuse `FileStatus` for diff statuses** (`Copied→Renamed` defensively) — no parallel enum.
- **No new AppError variants** — `git`/`other` cover bad oids and invalid paths.
- **Committer identity carried as `committerTs` only** (author is what the UI shows per product
  decision; the extra i64 is free, name/email dropped to keep the payload lean).
- **Scratch-on-D: enforcement continues** (`scratch_dir()` + `TMP`/`TEMP=D:\Temp`) — hard user
  mandate.
