# P1 — Polish: Implementation Contract

Status: authoritative for the Polish phase. Implementer: senior-dev in four fresh-context passes
(P1a/P1b/P1c/P1d — each section is self-contained; read §1–§2 plus your section). Builds on
`docs/contracts/M0-scaffold.md` (IPC conventions, AppError, spawn_blocking pattern),
`M2-graph.md` (GraphLayout wire format, GraphCanvas scroll model, frameStats),
`M4-diff.md` (DiffSlot mechanism), `M5-branches.md`, `M6-remotes.md` (remote-op feedback copy),
`ui-reference.md` (all visual tokens/metrics).

Invariants (unchanged, enforced in review): Rust owns Git logic + layout math; IPC carries
compact precomputed data; commands=req/resp, events=signals, channels=streaming; git2 under
`spawn_blocking`; canvas graph virtualized; watcher paired with manual refresh + focus rescan;
`src/ipc/mock.ts` updated with EVERY IpcApi change so the browser harness keeps working.

---

## 1. Scope decisions (IN / DEFER)

| # | Item | Verdict | Where |
|---|---|---|---|
| 1 | Keyboard shortcuts (small set + "?" overlay) | **IN** | P1c §6 |
| 1b | Arrow-key graph selection (Up/Down move selection) | **IN** (small: selection + scroll-into-view exist) | P1c §6.3 |
| 2 | Toast system (migrate remote notice/error + refresh failures; contextual banners stay inline) | **IN** | P1c §5 |
| 3 | Empty/loading-state audit | **IN** | P1c §7 |
| 4 | Styling pass (concrete refinements, no redesign) | **IN** | P1c §8 |
| 5 | WIP (uncommitted changes) row atop the graph | **IN** — frontend-composited render offset over the unchanged Rust layout (§9; design tension flagged in §12.1) | P1d §9 |
| 6 | Recent repos: persistence + reopen-last-on-launch + header repo switcher + empty-state list | **IN** — NOTE: no last-repo persistence exists today (flagged §12.2); this designs it from scratch | P1a §3, P1d §10 |
| 7 | Keep old diff visible during same-key refetch | **IN** | P1b §4.1 |
| 8 | `React.memo(DiffView)` | **IN** | P1b §4.2 |
| 9 | `messageBody` unconditional first-line strip | **IN** | P1b §4.3 |
| 10 | Commit textarea enabled during stage-in-flight | **IN** | P1b §4.4 |
| 11 | Id-based error dismissal + shared `isAppError`/`errorMessage` util | **IN** | P1b §4.5 |
| 12 | Refresh-failure path alignment + frame-log stream tagging | **IN** | P1b §4.6–§4.7 |
| 13 | Mock: prepend synthetic graph row on commit | **IN** | P1a §3.5 |
| 14 | `GraphLayout.truncated` banner (M2 §2.7 leftover) | **IN** (3 lines of JSX) | P1c §7 |
| — | Pane resizing | **DEFER** — nice-to-have, no backlog pressure, fixed widths sanctioned by ui-reference §1 |
| — | Light-theme toggle UI | **DEFER** — tokens exist but no toggle was ever requested; ship dark-only |
| — | Hunk staging / amend / WIP-row click-to-stage | **DEFER** — v2 features, locked out of v1 by product decisions |
| — | Graph keyboard nav beyond Up/Down (Home/End/PgUp, lane jumps) | **DEFER** — keep v1 discoverable and small |

---

## 2. Shared surface changes (wire types + utils — referenced by all passes)

### 2.1 New IPC types (`src/ipc/types.ts` — add verbatim)

```ts
export interface RecentRepo {
  /** Absolute workdir path as passed to openRepo. */
  path: string;
  /** Seconds since epoch (UTC) of the last successful open. */
  lastOpened: number;
}
```

`IpcApi` gains:

```ts
/** Recent successfully-opened repos, most recent first, max 10. Never rejects
 *  for a missing/corrupt settings file (returns []). */
getRecentRepos(): Promise<RecentRepo[]>;
/** Removes one entry; returns the updated list. */
removeRecentRepo(path: string): Promise<RecentRepo[]>;
```

Re-export `RecentRepo` from `src/ipc/index.ts`.

### 2.2 New util module `src/utils/errors.ts`

Single source for the three copies currently in `App.tsx`, `CommitBox.tsx`, `Sidebar.tsx`
(delete all three local copies; import from here):

```ts
import type { AppError } from '../ipc';
export function isAppError(e: unknown): e is AppError;   // body identical to today's copies
export function errorMessage(e: unknown): string;        // AppError.message | Error.message | String(e)
```

### 2.3 New frontend types (`src/components/Toasts.tsx`, §5)

```ts
export type ToastTone = 'error' | 'success' | 'warning';
export interface Toast {
  id: number;          // monotonic, App-owned counter
  tone: ToastTone;
  text: string;
  /** true => stays until dismissed (all 'error' toasts); false => auto-dismiss 5 s. */
  sticky: boolean;
}
```

### 2.4 IPC surface after P1 (complete list)

- Commands: everything from M0–M6 **+ `get_recent_repos`, `remove_recent_repo`**.
- Events: `repo-changed` (unchanged). Channels: none.
- Mock implements all of the above (recents via `localStorage`, §3.4).

---

## 3. P1a — Backend: recent-repos persistence + mock upgrades

### 3.1 New module `src-tauri/src/settings.rs` (+ `pub mod settings;` in `lib.rs`)

Persistence decision: **hand-rolled JSON file, no `tauri-plugin-store`** — one tiny struct, no
new plugin/capability, and path-parameterized functions stay unit-testable without an AppHandle.
Location: `<app_config_dir>/settings.json` where `app_config_dir` =
`app.path().app_config_dir()` (resolves under `%APPDATA%/com.bonsai.app` on Windows).

```rust
pub const MAX_RECENT_REPOS: usize = 10;
pub const SETTINGS_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentRepo {
    pub path: String,
    pub last_opened: i64, // seconds since epoch (UTC)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub version: u32,               // 1
    pub recent_repos: Vec<RecentRepo>,
}
impl Default for Settings { /* version: SETTINGS_VERSION, recent_repos: vec![] */ }

/// Missing file, unreadable file, or unparseable JSON -> Settings::default().
/// NEVER errors (settings are best-effort).
pub fn load_from(file: &std::path::Path) -> Settings;

/// Creates parent dirs; writes pretty JSON atomically (write to `settings.json.tmp`,
/// then rename over). Errors map to AppError::Io.
pub fn save_to(file: &std::path::Path, s: &Settings) -> Result<(), crate::error::AppError>;

/// `<app_config_dir>/settings.json`; errors map to AppError::Other.
pub fn settings_file(app: &tauri::AppHandle) -> Result<std::path::PathBuf, crate::error::AppError>;

/// Upsert `path` at the front (dedupe by case-insensitive path compare on Windows —
/// use `str::eq_ignore_ascii_case`; document the simplification), stamp `last_opened`,
/// truncate to MAX_RECENT_REPOS. Pure — unit-testable.
pub fn record_recent(s: &mut Settings, path: &str, now: i64);
```

Wire format on disk:

```json
{ "version": 1, "recentRepos": [ { "path": "D:\\Repos\\x", "lastOpened": 1753660800 } ] }
```

### 3.2 Commands (`src-tauri/src/commands.rs`, register both in `lib.rs`)

```rust
#[tauri::command]
pub async fn get_recent_repos(app: tauri::AppHandle) -> Result<Vec<RecentRepo>, AppError>;
// spawn_blocking(load_from(settings_file(&app)?)).recent_repos — never rejects for
// missing/corrupt file (load_from defaults); only settings_file resolution can error.

#[tauri::command]
pub async fn remove_recent_repo(app: tauri::AppHandle, path: String)
    -> Result<Vec<RecentRepo>, AppError>;
// load -> retain(|r| !r.path.eq_ignore_ascii_case(&path)) -> save -> return list.
```

**`open_repo` hook:** in the outer `open_repo` command (NOT `open_repo_inner`, which stays
runtime-free), after `open_repo_inner` returns `Ok(info)` with `info.is_repo && !info.bare`:
`spawn_blocking` → load, `record_recent(&mut s, &info.path, now)`, save. Save failure is
**non-fatal**: `eprintln!` and return `Ok(info)` anyway. Use `info.path` (the canonical workdir
root reported by `read_repo_info`), not the raw argument — dedupes "repo root" vs "subfolder"
opens.

### 3.3 Rust tests (in `settings.rs` `#[cfg(test)]`, `tempfile::TempDir`)

1. `roundtrip` — save_to then load_from == input.
2. `missing_file_defaults` / `corrupt_json_defaults` (write `"{nope"`).
3. `record_recent_upserts_and_caps` — insert 12 distinct paths → len 10, newest first; re-insert
   an existing path (different case) → moved to front, deduped, `last_opened` updated.
4. `atomic_write_leaves_no_tmp` — after save_to, `settings.json.tmp` does not exist.

### 3.4 Mock (`src/ipc/mock.ts`): recents via localStorage

- Key `bonsai.mockRecents`, value: `RecentRepo[]` JSON. Helpers `readRecents()` (corrupt/missing
  → `[]`) and `writeRecents(list)`.
- `openRepo`: on every successful usable open (isRepo && !bare) upsert `{path, lastOpened:
  Math.floor(Date.now()/1000)}` at front, dedupe case-insensitively, cap 10, write.
- `getRecentRepos` → `delay(150)` → `readRecents()`. `removeRecentRepo` filters + writes +
  returns. This makes the harness reopen-on-launch verifiable: open once, reload the page,
  the app auto-reopens the mock repo (§10.3).

### 3.5 Mock: synthetic graph row on commit (backlog item 13)

New helper in `src/ipc/fixtures/graph.ts`:

```ts
export interface MockCommit { oid: string; summary: string }
/** Prepends `commits` (newest first) as lane-0 rows to `layout`:
 *  - every existing node's `parents` indices and every edge's from/to shift by commits.length;
 *  - new rows: node i = { id, lane: 0, parents: [i+1], summary, author: 'You',
 *    ts: now - i*60 }, edges (i, i+1, 0) prepended keeping (from,to) sort order;
 *  - moves the `⌂`/isHead LOCAL-branch pill from the old head row to row 0 (other pills —
 *    origin/main, tags — stay on the old row); headIndex = 0. */
export function prependCommits(layout: GraphLayout, commits: MockCommit[]): GraphLayout;
```

`mock.ts`: module state `let mockCommits: MockCommit[] = []` (reset when a different path is
opened, alongside the existing resets). `commit()` unshifts `{oid, summary}` and deletes the
current `TODO(polish)` comment. `getGraph()` for the DEFAULT fixture returns
`prependCommits(buildMockGraph(), mockCommits)`; the `20k` and `detached` fixtures stay as-is.
Harness proof: commit in the browser → new top row with the summary + `⌂ main` pill.

### 3.6 P1a acceptance

`cargo test` green (incl. §3.3), `cargo clippy -- -D warnings`, `pnpm build` green.
`src/ipc/tauri.ts` gains the two invoke wrappers; mock compiles and round-trips recents +
synthetic commit rows in the harness.

---

## 4. P1b — Frontend correctness/refactor pass (items 7–12; no visual changes)

### 4.1 Keep old diff visible during same-key refetch (item 7)

`DiffSlot` semantics change (`src/components/DiffView.tsx` — update the doc comment):
`diff` MAY be non-null while `state === 'loading'` (stale content shown during a refetch).

- `App.fetchDiffSlot(key, fetcher)`: when `diffSlotRef.current?.key === key` and its `diff` is
  non-null, set `{ key, state: 'loading', diff: diffSlotRef.current.diff, error: null }` instead
  of `diff: null`. First-time expansions keep `diff: null` (skeleton unchanged).
- `DiffSlotView`: `state === 'loading' && slot.diff !== null` → render the diff (add class
  `diff-stale` on `.diff-scroll`; CSS: `opacity: 0.6`); skeleton only when `diff === null`.
  Error/ready branches unchanged. This kills the skeleton flash on focus/watcher-tick refetches
  of an expanded workdir diff.

### 4.2 `React.memo(DiffView)` (item 8)

`export const DiffView = memo(function DiffView({ diff }: DiffViewProps) { ... });` — with §4.1
keeping the same `FileDiff` object reference for stale content, a 5000-row diff no longer
re-renders while its slot is loading or unrelated App state changes.

### 4.3 `messageBody` fix (item 9) — `src/components/CommitPanel.tsx`

Current bug: `startsWith(summary)` can cut mid-line (summary a prefix of a longer first line)
and duplicates the summary when the message doesn't start with it. Replace with the
unconditional first-line strip (the summary IS always derived from line 1 by git2):

```ts
/** Body = message minus its first line and the following blank separator lines. */
function messageBody(message: string): string {
  const nl = message.indexOf('\n');
  if (nl === -1) return '';
  return message.slice(nl + 1).replace(/^(\r?\n)+/, '').replace(/^\r/, '');
}
```

Drop the `summary` parameter at the call site.

### 4.4 Commit textarea during stage-in-flight (item 10) — `CommitBox.tsx`

Textarea: `disabled={submitting}` only (typing keeps focus while stage/unstage runs — fixes the
Windows focus-drop annoyance). The Commit button keeps the full
`stagedCount === 0 || message.trim() === '' || busy || submitting` gate; Ctrl+Enter path guards
via the same `disabled` computation (unchanged).

### 4.5 Id-based error dismissal + shared util (item 11)

- Delete the local `isAppError`/`errorMessage` copies in `App.tsx`, `CommitBox.tsx`,
  `Sidebar.tsx`; import from `src/utils/errors.ts` (§2.2).
- `App.tsx`: `statusError` becomes `{ id: number; message: string } | null` (id from a
  `useRef(0)` counter incremented on every set). All `setStatusError(errorMessage(e))` call
  sites become `setStatusError({ id: ++statusErrorId.current, message: errorMessage(e) })`.
- `StatusPanelProps.error: { id: number; message: string } | null`; StatusPanel stores
  `dismissedErrorId: number | null` and shows the banner when
  `error !== null && error.id !== dismissedErrorId`. Identical errors from distinct operations
  now re-surface after dismissal (the old string-compare swallowed them).
- `branchesError`/`graphError`/commit-diff errors stay plain strings (their surfaces reset on
  every fetch; no dismissal-vs-recurrence bug there). Do not churn them.

### 4.6 Refresh-failure path alignment (item 12a) — `App.tsx`

New single post-op refresh helper; all "re-open + refetch everything" call sites use it:

```ts
/** openRepo(current path) + refetch status/graph/branches. Errors -> error toast
 *  "Refresh failed: <message>" (P1c wires pushToast; in P1b use a temporary
 *  setStatusError -> replaced in P1c). Never throws. */
const refreshAll = useCallback(async (): Promise<void> => { ... }, [...]);
```

Call sites: `handleRefresh` (drop its bespoke catch-to-statusError), post-`commit` refresh,
post-`checkoutBranch`, post-`pull`. `refetch*` helpers keep their per-pane error states
(pane-scoped fetch failures belong to their pane); only the *composite refresh* failure path is
unified. Order note: P1b lands `refreshAll` with the temporary statusError sink; P1c swaps the
sink to `pushToast('error', ...)` — sequencing documented so neither pass blocks the other.

### 4.7 Frame-log stream tagging (item 12b) — `src/graph/GraphCanvas.tsx`

Today one recorder mixes paint durations and scroll inter-frame gaps, making `avg` meaningless.
Split into two recorders and tag the log lines:

- `paintRecorderRef` — fed by `paintNow` body duration; logs every 120 frames:
  `[bonsai] frames kind=paint n=120 avg=1.2ms max=9.0ms >33ms=0`.
- `gapRecorderRef` — fed by the inter-frame gap while scrolling (existing §M2-4.7 logic); logs:
  `[bonsai] frames kind=gap n=120 avg=8.4ms max=21.0ms >33ms=0`.
- `scrollSweep` output line unchanged (`[bonsai] scroll-test {...}`) — the M2d gate procedure
  keeps working verbatim.

### 4.8 P1b acceptance

`pnpm build` + lint green. Harness checks: expand a workdir diff → focus-blur-refocus the window
→ diff stays visible (dimmed briefly), no skeleton flash; commit-panel body no longer shows a
mangled first line for a long-first-line fixture message; typing in the commit box while a
stage is in flight keeps focus; dismiss a status error, trigger the same error again → banner
reappears; console frame lines are `kind=paint` / `kind=gap` tagged.

---

## 5. P1c(1) — Toast system

### 5.1 Model (App-owned state)

```ts
const [toasts, setToasts] = useState<Toast[]>([]);            // §2.3 type
const toastId = useRef(0);
/** sticky = tone === 'error'; non-sticky auto-dismiss after 5000 ms (timeout captured per
 *  toast id — no shared-counter invalidation needed anymore). Stack cap: 5 — pushing the 6th
 *  drops the oldest NON-sticky toast (oldest sticky if none). */
const pushToast = useCallback((tone: ToastTone, text: string) => { ... }, []);
const dismissToast = useCallback((id: number) => { ... }, []);
```

### 5.2 Component `src/components/Toasts.tsx` (presentational)

```ts
export interface ToastsProps { toasts: Toast[]; onDismiss(id: number): void; }
export function Toasts({ toasts, onDismiss }: ToastsProps): JSX.Element | null;
```

- Fixed overlay, top-right, 12px below the header (`position: fixed; top: 52px; right: 12px;
  z-index` above panes, below ConfirmDialog). Column, newest on top, `gap: 8px`, width 360px.
- Per-toast: 6px radius, 8px/12px padding, 13px text, dismiss `✕` button (reuse
  `.error-dismiss` affordance). Tones: error = `--danger` @12% bg + `--danger` text (same
  recipe as `.error-banner`); success = `--success` @12% + `--success`; warning = `--warning`
  @12% + `--warning`. Container `aria-live="polite"`; error toasts `role="alert"`.
- Rendered unconditionally in App (also over the empty state).

### 5.3 Surface migration map (normative)

| Existing surface | P1 disposition |
|---|---|
| `remoteNotice` line (M6) | **→ toast** (`ok`→success, `warn`→warning). Delete `remoteNotice`, `noticeId`, `showNotice`, `dismissNotice`, both header-adjacent JSX blocks and their CSS. |
| `remoteError` banner (M6) | **→ sticky error toast**. Delete `remoteError` state + banner JSX. |
| `refreshAll` failures (§4.6) | **→ sticky error toast** `Refresh failed: <message>`. |
| Open-repo failure from the recents/switcher path (§10) | **→ sticky error toast** (the empty-state open error banner stays for empty-state opens — context matters there). |
| StatusPanel banner (`statusError`) | **stays inline** (id-based per §4.5) — stage/unstage/status errors belong next to the file list; M1/M3 checklists reference it. |
| CommitBox inline error | **stays inline** (M3 checklist: `configMissing` copy shown at the box). |
| Sidebar banner + create-branch inline error | **stay inline** (M5 checklist + contract §4.2/§4.3). |
| Graph pane banner (`graphError`) | **stays inline** (pane-scoped fetch failure). |
| Empty-state error banners (not-a-repo / bare / open failure) | **stay inline**. |

**Copy preservation (load-bearing — M6 checklist greps these):** the migrated toasts must emit
the byte-identical strings currently produced in `handleFetch`/`handlePull`/`handlePush`:
`Fetched N remote(s)[ — K ref(s) updated]`, `Already up to date`,
`Fast-forwarded <branch> to <short>`, the full `Cannot fast-forward: ...` warning, and
`Pushed <branch> → <remote>/<branch>[ (upstream set)]`. Backend AppError messages pass through
`errorMessage` untouched.

---

## 6. P1c(2) — Keyboard shortcuts + overlay

### 6.1 Binding table (normative)

| Binding | Action | Conflict analysis (Chrome/Edge harness + WebView2 native) | Handled in |
|---|---|---|---|
| `Ctrl+Enter` | Commit (existing) | none; scoped to the textarea | `CommitBox` (unchanged) |
| `Esc` | Deselect commit / close dialog / close create-input / close overlay & switcher menu (existing + new consumers) | none | per-component + App |
| `Ctrl+R`, `F5` | Refresh (`handleRefresh`) | Both reload the page in Chrome/Edge AND the WebView2 view; both are cancelable — `e.preventDefault()` on window keydown suppresses reload. MUST preventDefault even when the action is a no-op (no repo / already refreshing) so the native app never accidentally reloads its own UI. | App global handler |
| `Ctrl+O` | Open repository (folder picker) | Browser "open file" dialog — cancelable, preventDefault works | App |
| `Ctrl+Shift+F` | Fetch | Nothing at page level in Chrome/Edge/WebView2 (devtools-only binding). Avoids `Ctrl+F` find-bar muscle memory. | App |
| `Ctrl+Shift+P` | Pull | Chrome/Edge/WebView2: nothing at page level (command palette is devtools-focused only). Firefox: private window — RESERVED, not preventable; documented harness-only caveat, native target unaffected. | App |
| `Ctrl+Shift+U` | Push | Nothing on Windows Chrome/Edge/WebView2 (the IME Unicode binding is Linux-only). | App |
| `ArrowUp` / `ArrowDown` | Move commit selection −1/+1 (only when a commit is already selected) | Page scroll — preventDefault only when the shortcut actually fires (selection non-null, not typing) | App + `GraphCanvas` follow-scroll (§6.3) |
| `?` (Shift+`/`) | Toggle shortcut overlay | none | App |

Rejected: `Ctrl+P` (print — preventable but overloaded), `Ctrl+F` (find muscle memory),
`Ctrl+W/T/N`, `Ctrl+Shift+N` (browser-reserved, NOT cancelable — never bind these).

### 6.2 Global handler contract (one `useEffect` in App)

```ts
// window 'keydown', capture: false. Guard order:
// 1. if e.key === 'F5' || (ctrl && key==='r'): e.preventDefault(); if (canRefresh) void handleRefresh(); return;
// 2. typing guard: target is INPUT/TEXTAREA/SELECT or isContentEditable -> return (all remaining bindings);
// 3. ConfirmDialog open (App learns via a `dialogOpen` boolean lifted from Sidebar — add
//    optional SidebarProps.onDialogOpenChange(open: boolean)) -> return;
// 4. remaining table rows; each: check enablement (same gates as its button:
//    fetch/pull/push respect repoOpen/mutating/refreshing/canPullPush), preventDefault, run.
// Actions reuse the EXACT existing handlers — no new operation paths.
```

### 6.3 Arrow-key selection + follow-scroll

- App: on ArrowDown with `selectedIndex !== null` →
  `setSelectedIndex(Math.min(selectedIndex + 1, graph.nodes.length - 1))`; ArrowUp mirrors with
  `Math.max(0, ...)`. When `selectedIndex === null`, arrows do nothing (page scroll untouched).
- `GraphCanvas`: new effect — when `selectedIndex` changes to non-null and the row is outside
  `[scrollTop, scrollTop + viewportH - rowHeight]` (account for the WIP offset, §9.4), set
  `scroller.scrollTop` to bring it fully into view with one row of margin. The scroll event then
  repaints via the existing path. No new props needed.

### 6.4 `src/components/ShortcutOverlay.tsx`

```ts
export interface ShortcutOverlayProps { open: boolean; onClose(): void; }
export function ShortcutOverlay(props: ShortcutOverlayProps): JSX.Element | null;
```

Centered panel (max-width 480px, `--bg-1`, 6px radius, `--border`, dim backdrop like
ConfirmDialog) listing the §6.1 table (binding kbd-chips + action text; static content lives in
the component). Closes on Esc, backdrop click, `✕`, or pressing `?` again. Footer hint line in
the header bar is NOT added (keep the header clean); discoverability = the overlay itself +
button tooltips gaining `(Ctrl+Shift+F)`-style suffixes on the three remote buttons + refresh.

---

## 7. P1c(3) — Empty/loading-state audit (per-state disposition)

| State | Today | P1 treatment |
|---|---|---|
| No repo open | Centered Bonsai + Open button | Keep; ADD recent-repos list under the button (§10.2). Copy unchanged: "Bonsai" / "A tidy Git client" / "Open repository". |
| Unborn repo | "No commits yet" in graph pane; status panel usable | Keep. Verify the sidebar shows Branches(0) sections, not skeletons forever: Sidebar with `data !== null` and empty lists renders "No remotes"/"No tags" — ADD the missing equivalent for zero local branches: `<p class="branch-muted">No branches yet</p>`. |
| Graph first load | Blank canvas area | Keep (ui-reference §8: no spinners over canvas). |
| Graph refetch | Previous layout stays | Keep. |
| `layout.truncated` | Flag ignored | NEW slim banner at graph-pane top (item 14): `History truncated to the most recent 100,000 commits` — style like `.graph-error-banner` but `--warning` @12% bg / `--warning` text, non-dismissible. |
| Status/branches first load | Skeleton rows (6 / 3) | Keep; unify skeleton geometry (§8). |
| Status empty | "No changes" | Keep. |
| Diff slot loading | Skeleton (3) | First load: keep; refetch: stale diff (§4.1). |
| Empty diff (`hunks.length === 0`) | "No changes" placeholder | Keep. |
| Commit details loading | Skeleton (4) | Keep. |
| Remote op > 300 ms | Button label swap only | ADD the ui-reference §8 indeterminate 2px accent bar under the header whenever `remoteOp !== null || refreshing` (pure CSS animation, `.header-progress`). |

---

## 8. P1c(4) — Styling refinements (closed list — anything else is out of scope)

1. Skeletons: one `.skeleton-row` recipe everywhere — height 20px, radius 6, `--bg-2`, 1.2s
   pulse, 8px vertical gap (currently drifts between panels).
2. Buttons: `.toolbar-btn` aligned to the ui-reference button spec — 32px tall, 6px radius,
   `--bg-2` hover, 8px gap between toolbar buttons; icon glyphs vertically centered.
3. Banner rhythm: all `.error-banner` variants padding 8px 12px, margin 8px 12px, 6px radius
   (audit graph/sidebar/commit variants for drift).
4. Section headers: 12px horizontal padding in BOTH the sidebar and right panel (currently
   inconsistent); 11px/0.08em/uppercase per ui-reference §1.
5. Panel scrollbars (right panel, sidebar, `.diff-scroll`): `::-webkit-scrollbar` width 8px,
   thumb `--bg-3` radius 4, track transparent (WebView2 = Chromium, safe).
6. `:focus-visible` audit: every interactive element (rows, pills-area buttons, toolbar,
   dialog buttons) shows the 2px `--accent` outline; remove any `outline: none` without a
   replacement.
7. Commit-counter turns `--warning` when the first line exceeds 72 chars (today text-3 only).
8. New-in-P1 components (Toasts, ShortcutOverlay, RepoSwitcher, WIP row colors) use ONLY
   existing tokens — no new hex values in CSS or TS (WIP row uses `--warning` + lane palette).

### 8.1 P1c acceptance

`pnpm build` green. Harness: toasts appear/auto-dismiss/stack (drive via `?remote=authfail`
fetch → sticky error toast with the M6 message; successful mock fetch → success toast with
byte-identical M6 copy); every §6.1 binding fires its action and suppresses the browser default
(Ctrl+R does NOT reload the harness tab); `?` overlay opens/closes; truncated banner renders
when the mock layout is hand-flagged (add `?fixture=truncated` variant: `buildMockGraph()` with
`truncated: true`); progress bar shows during the 400 ms mock remote ops; screenshots for the
styling checklist items.

---

## 9. P1d(1) — WIP (uncommitted changes) row

### 9.1 Design decision (RECOMMENDED; tension flagged in §12.1)

**Frontend-composited render offset. The Rust `GraphLayout` wire format and `compute_graph` are
untouched.** Rationale: the WIP row's only inputs are (a) does the working dir have changes and
(b) how many files — both already delivered by `get_status`, which App fetches in parallel with
the graph on every trigger. A Rust-side synthetic row would either duplicate the status walk
inside `get_graph` (paying the most expensive git2 call twice per refresh) or couple the two
commands, and would shift every node index — breaking `parents`-as-indices, `headIndex`, edge
tuples, and every M2 consumer/test. The WIP row is not layout math: no lane assignment or edge
routing changes; it is a +1 row translation of an unchanged Rust layout plus one dashed marker
whose lane is *read from* Rust data (`nodes[headIndex].lane`). Precedent: M2 §8 item 4 (TS edge
bucket index = view plumbing). Rejected alternative documented in §12.1.

### 9.2 Data flow (no IPC change)

```ts
// App.tsx
export interface WipSummary { fileCount: number }   // export from GraphCanvas.tsx
const wip: WipSummary | null = useMemo(() => {
  if (status === null || repo?.head?.unborn === true) return null;
  const paths = new Set<string>();
  for (const s of [status.staged, status.unstaged, status.untracked, status.conflicted])
    for (const e of s) paths.add(e.path);
  return paths.size > 0 ? { fileCount: paths.size } : null;
}, [status, repo]);
// <GraphCanvas layout={graph} wip={wip} ... />
```

`GraphCanvasProps` gains `wip: WipSummary | null`.

### 9.3 Rendering contract (`GraphCanvas.tsx` + `draw.ts`)

Let `wipOffset = wip !== null ? 1 : 0` and `RH = METRICS.rowHeight` (28).

- **Spacer:** `(layout.nodes.length + wipOffset) * RH + 8`.
- **Layout translation:** compute `layoutScrollTop = scrollTop - wipOffset * RH` and pass THAT
  as `Viewport.scrollTop` to the unchanged `drawGraph` (every layout row renders 28px lower).
  Visible range: `firstRow = max(0, floor(layoutScrollTop / RH) - OVERSCAN)`,
  `lastRow = min(n - 1, ceil((layoutScrollTop + h) / RH) + OVERSCAN)` (negative
  `layoutScrollTop` is safe under the `max(0, …)`).
- **WIP row paint** — new pure fn in `draw.ts`, called AFTER `drawGraph` when `wip !== null`
  and `scrollTop < RH + 56`:

```ts
export function drawWipRow(ctx: CanvasRenderingContext2D, layout: GraphLayout,
  wip: WipSummary, vp: Viewport /* raw scrollTop */, theme: Theme, hovered: boolean): void;
```

  - `headLane = layout.headIndex !== null ? layout.nodes[layout.headIndex].lane : 0`;
    `x = laneX(headLane)`, `y = RH/2 - vp.scrollTop`.
  - Hover background `theme.bg2` full-width when `hovered`.
  - Dashed connector to the HEAD dot: `setLineDash([3,3])`, 2px, color
    `laneColors[headLane % 10]`, vertical from `(x, y)` to
    `(x, headIndex*RH + RH/2 - layoutScrollTop)` clamped to `[-56, height+56]`; skip when
    `headIndex === null`.
  - Marker: dashed circle r=4 (same dash), stroke `theme.warning`, fill `theme.bg0`; reset
    `setLineDash([])` after.
  - Text at the standard text column x: `Uncommitted changes` in `summaryFont`, `theme.text2`,
    italic; then `(${fileCount} file${s})` in `metaFont`, `theme.text3`.
- **Hit-testing:** replace `rowAtMouseY` with
  `hitTest(y, scrollTop): number | 'wip' | null` — `raw = floor((y + scrollTop) / RH)`;
  `raw < wipOffset` → `'wip'` (when offset 1); else `row = raw - wipOffset`, valid range check
  as today. Hover: `'wip'` highlights the WIP row (tracked in a ref like `hoverRow`; encode as
  `hoverRow = -1` internally). Click on `'wip'` → `onSelect(null)`.
- **Follow-scroll (§6.3)** and `rowY` math account for `wipOffset` (target y =
  `(row + wipOffset) * RH`).

### 9.4 Semantics

- The WIP row is **never selectable** — clicking it deselects any commit, which already routes
  the right panel to the working-dir status + staging view (mode A). No commit-diff fetch fires.
- Appears/disappears purely with `status` (already refetched on watcher/focus/manual/ops); no
  new refresh triggers, no flicker rules beyond React state.
- Not rendered for unborn repos (graph pane shows "No commits yet") or when `wip === null`.
- Mock: no fixture change needed — the harness's stateful `mockStatus` is non-empty by default,
  so the WIP row shows immediately; staging→committing everything makes it disappear
  (fileCount reaches 0), which is the harness verification story.

---

## 10. P1d(2) — Recent repos UI + reopen-on-launch

### 10.1 App state + launch

```ts
const [recents, setRecents] = useState<RecentRepo[]>([]);
const refreshRecents = useCallback(async () => {
  try { setRecents(await ipc.getRecentRepos()); } catch { /* non-fatal, keep [] */ }
}, []);

/** Open a specific path (no picker): shared by switcher, recents list, launch. */
const openPath = useCallback(async (path: string, opts: { fromRecents: boolean }) => {
  // same body as handleOpenRepository after the picker: openRepo -> setRepo ->
  // refetch-or-clear; then void refreshRecents().
  // Failure: if opts.fromRecents && AppError.kind === 'io' (path gone) ->
  //   void ipc.removeRecentRepo(path).then(setRecents);
  //   surface the error as a sticky error toast when a repo is already open,
  //   or via the empty-state `error` banner when none is.
}, [...]);

// Mount effect (once): await refreshRecents(); if (recents fetched)[0] exists and no repo is
// open -> void openPath(first.path, { fromRecents: true }).  This implements the locked
// "reopen last repo on launch" product decision (§12.2).
```

`handleOpenRepository` (picker path) delegates to `openPath(picked, { fromRecents: false })`.

### 10.2 Empty-state recents list

Under the "Open repository" button, when `recents.length > 0`:
section label `RECENT` (11px uppercase, `--text-3`), then up to 10 rows — button per row:
folder name (`--text-1`, 13px) + full path (`--text-3`, 12px, truncated, `title` attr), hover
`--bg-2`, radius 6, width 360px. Click → `openPath(r.path, { fromRecents: true })`. Disabled
while `loading`.

### 10.3 Header `src/components/RepoSwitcher.tsx`

Replaces the static `.header-repo` block (repo name/path stay visually identical, but the block
becomes a button with a `▾` affix).

```ts
export interface RepoSwitcherProps {
  repo: RepoInfo;                 // current (open) repo — name/path/HeadSummary render as today
  recents: RecentRepo[];          // App state; current repo filtered out INSIDE the component
  disabled: boolean;              // refreshing || mutating
  onOpenPath(path: string): void; // -> openPath(path, { fromRecents: true })
  onBrowse(): void;               // -> handleOpenRepository()
}
```

Dropdown (absolute, below the button, `--bg-1`, `--border`, 6px radius, shadow): recents rows
(name + truncated path) then a separator and `Browse…`. Closes on selection, Esc, or outside
click (document mousedown listener while open). `HeadSummary` stays inside the button. Keyboard
shortcut `Ctrl+O` (§6.1) triggers `onBrowse` regardless of dropdown state.

### 10.4 P1d acceptance

`pnpm build` green. Harness: open the mock repo → reload the page → repo auto-reopens (mock
localStorage recents); empty-state shows the recent entry after a manual `localStorage` seed +
reload; switcher dropdown lists recents + Browse; WIP row visible with the default dirty mock
status, disappears after staging+committing everything in the harness, and clicking it while a
commit is selected returns the right panel to the status view; ArrowUp/Down + follow-scroll
still correct with the WIP offset active.

---

## 11. Acceptance criteria — overall P1

AI gate (orchestrator verifies):
- `cargo test` green incl. new `settings.rs` tests (§3.3); `cargo clippy -- -D warnings`;
  `pnpm build` green after every sub-increment. NO changes to `graph.rs` or its tests (the WIP
  design requires none — a diff touching `compute_graph` is a contract violation).
- Existing M1–M6 Rust test suites still green (no backend behavior changed except the additive
  recents hook in `open_repo`).
- Harness verification (VITE_MOCK_IPC=1), per item: §4.8, §8.1, §10.4 lists — screenshots of:
  toast stack (error + success), shortcut overlay, WIP row (top + after scrolling down),
  empty state with recents, switcher dropdown, truncated banner, stale-diff refetch,
  unified skeletons.
- M2d scroll gate re-run over the 20k fixture (`window.__bonsai.scrollSweep(10000)`) with the
  WIP row forced visible — pass criterion unchanged (`maxWindow5Avg <= 33`, `over100 <= 3`).
- Copy checks: the five M6 strings (§5.3) grep-identical in the new toast call sites.

USER CHECKPOINT (native `pnpm tauri dev` — never self-declared):
1. Ctrl+R / F5 refresh the repo state and do NOT reload/blank the window; every other §6.1
   binding fires its action; `?` overlay opens.
2. Toasts appear for fetch/pull/push success and error against a real remote-less/scratch repo;
   errors stay until dismissed.
3. Close and relaunch the app → the last repo reopens; the switcher lists prior repos and
   Browse works; a deleted repo path shows a clear error and drops from recents.
4. Dirty scratch repo → WIP row visible at the graph top with the correct file count; clicking
   it shows the status panel; committing everything removes it.
5. Empty states look right natively: no-repo (with recents), unborn repo, empty status.
6. Typing in the commit box while stage/unstage runs no longer loses focus.

---

## 12. Ambiguities resolved here (flag to orchestrator if disagreed)

1. **WIP row is frontend-composited** (§9.1) against a strict reading of "Rust owns layout
   math". Judged view composition (render offset + marker derived from Rust-provided
   `headIndex`/lane), precedent M2 §8.4. Rejected option B: `get_graph(include_wip: bool)`
   returning a synthetic node — duplicates the status walk per refresh, shifts every node
   index/edge tuple (breaking M2 tests + all consumers), and couples two commands. If the
   orchestrator overrules, option B must ALSO add `wip_row: Option<WipRow>` as a sibling field
   (never a nodes[0] entry) to preserve the row==index invariant.
2. **Reopen-last-repo did not exist** despite being a v1 product decision — no persistence
   mechanism was ever built (verified: no store plugin, no fs persistence in `src-tauri`, no
   localStorage). P1 builds it (§3, §10) rather than "extending" anything. Flag to orchestrator
   as a gap discovered, not scope creep.
3. **Hand-rolled `settings.json` over `tauri-plugin-store`** — one struct, atomic tmp+rename
   write, no new capability surface, path-parameterized for tests.
4. **Case-insensitive path dedupe via `eq_ignore_ascii_case`** — correct for Windows drive
   letters/ASCII paths; non-ASCII case-folding deliberately not attempted (documented
   simplification).
5. **Toast placement top-right under the header, cap 5, errors sticky** — replaces only the
   remote notice/error + composite-refresh failures; contextual inline banners stay (§5.3 map
   is normative; checklist copy preserved byte-identically).
6. **Shortcut set excludes `Ctrl+F`/`Ctrl+P`** despite the milestone prompt suggesting them —
   find/print muscle-memory conflicts; `Ctrl+Shift+F/P/U` chosen instead. `Ctrl+Shift+P` is
   unpreventable in Firefox only (harness caveat, native target Chromium-based).
7. **Arrow-key nav minimal**: only moves an existing selection; no "select first commit on
   ArrowDown from null" (would fight page scrolling and Esc semantics).
8. **`statusError` becomes id-wrapped; other error strings unchanged** — only the surface with
   a dismissal-recurrence bug pays the churn.
9. **Stale-diff refetch reuses the DiffSlot shape** (`diff` non-null while loading) instead of
   a new `refetching` state — smallest change that both fixes the flash and enables the
   `React.memo` win.
10. **Frame-log split into `kind=paint` / `kind=gap` recorders**; `scroll-test` line format
    frozen (M2d gate procedure depends on it).
