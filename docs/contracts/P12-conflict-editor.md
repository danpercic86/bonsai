# P12 — Rich conflict-resolution editor

> Contract for senior-dev. Implement strictly to the signatures and sequences below.
> Rust owns ALL git logic; React only renders + does local text rewrites. IPC carries compact,
> already-computed data — the backend never does per-hunk logic; both view modes produce ONE
> resolved-text string saved via ONE new primitive. `src/ipc/mock.ts` MUST keep compiling and
> mirror every wire change; it is the harness's only data source (`VITE_MOCK_IPC=1`).
>
> Audience: the senior-dev implementing to file paths + these signatures, then reviewer/tester.
>
> Prior art (READ before implementing — these define the contracts P12 extends):
> `docs/contracts/P3c-merge-conflicts.md` (ConflictKind/Entry/File/Resolution model,
> `list_conflicts`/`get_conflict`/`resolve_conflict`, §3.2 resolution matrix, OpBanner,
> `conflict:<path>` diffSlot overlay routing); `docs/contracts/P8-merge-autostash.md` (MergeOutcome
> shape); `docs/contracts/M4-diff.md` (FileDiff hunk/line model + `MAX_FILE_DIFF_LINES` all-or-nothing
> cap convention); `docs/contracts/P3a-diff-overlay.md` (center-pane DiffOverlay / diffSlot host the
> editor mounts in). Current code: `src-tauri/src/git/conflict.rs`, `src-tauri/src/commands.rs`
> (~1009 `resolve_conflict`), `src/components/DiffOverlay.tsx` (`ConflictMarkerView` /
> `ConflictSlotView`), `src/components/RepoWorkspace.tsx` (`fetchConflictSlot`,
> `handleToggleConflictView`, `handleResolveConflict`, `refreshAll`), `src/ipc/{types,tauri,mock}.ts`.
>
> Plan of record: replace P3c's read-only `<pre>` marker view with a real CodeMirror 6 +
> `@codemirror/merge` conflict-resolution editor for TEXT conflict kinds; keep the P3c whole-file
> quick actions as the fallback for every other kind. Four sub-increments, each compiling and
> committable on its own: **P12a** backend + IPC; **P12b** unified editable editor + pure region
> helpers; **P12c** per-region accept widgets + overview ruler; **P12d** side-by-side MergeView +
> syntax highlighting + RepoWorkspace wiring.

## §0 Scope, locked decisions, invariants

Locked decisions (do NOT re-litigate — flagged items are in §7):

1. **Editor engine = CodeMirror 6 + `@codemirror/merge`** (net-new deps, user-approved). Frontend
   deps to add: `codemirror`, `@codemirror/view`, `@codemirror/state`, `@codemirror/language`,
   `@codemirror/merge`, `@codemirror/commands`, `@codemirror/language-data`.
2. **Side-by-side = 2-way `ours | theirs` ONLY.** Do NOT expose stage-1 (base/ancestor); no 3-way
   pane. (The merge engine already uses `conflict_style_merge(true)` — 2-way markers, no diff3 base.)
3. **Location markers = scrollbar overview-ruler ticks** (custom CM extension drawing on the
   scrollbar gutter), NOT a minimap thumbnail.
4. **Combination ordering = ours-block THEN theirs-block** (matches git's marker order:
   `<<<<<<<` ours `=======` theirs `>>>>>>>`).
5. **Rich editor scope = text kinds `bothModified` + `bothAdded` ONLY.** Every other kind
   (`deletedByUs`, `deletedByThem`, `addedByUs`, `addedByThem`, `bothDeleted`) and every text-less
   payload (`binary` | `tooLarge` | `missing`) keeps the existing P3c whole-file
   ours/theirs/resolved quick actions (StatusPanel `ConflictRow`) and the read-only marker/placeholder
   view — no editor. The editor is an ADDITIVE path for the two text-merge kinds.
6. **The two view modes edit ONE shared result document.** Toggling unified⇄side-by-side preserves
   in-progress edits — the shared result string is React-owned; each mode is (re)seeded from it.
7. **Both modes produce a single resolved-text string, saved via ONE new backend primitive
   `resolve_conflict_text`.** Per-region Accept Ours/Theirs/Both and the combination control are
   FRONTEND text rewrites (pure `src/utils/conflictRegions.ts`); the backend does NO per-hunk logic —
   it writes the given content to the worktree and stages it, exactly like `MarkResolved` /`git add`.

Invariants (unchanged): Rust owns all Git logic; git2 under `spawn_blocking` via the runtime-free
`*_inner` pattern; commands = req/resp (no new events/channels this milestone); every `IpcApi` change
mirrored in `types.ts` + `tauri.ts` + a stateful `mock.ts`; the mock always compiles and serves the
`?op=merge` fixture; no destructive op added (saving resolved text is non-destructive — it only
stages the user's own content).

Suggested build order: **P12a** (backend/IPC, self-contained) → **P12b** (editor shell + pure
helpers) → **P12c** (per-region widgets + ruler) → **P12d** (side-by-side + wiring). Each is one
fresh-context senior-dev pass (this file + the exact source paths).

Payload note: `ConflictFile` now carries `text` + `ours` + `theirs` — up to ~3×1 MiB for one
on-demand file. Acceptable (single-file, user-initiated). Per-side cap semantics are unchanged: if
the file is `tooLarge`/`binary`/`missing`, ALL THREE strings stay `""` (see §1.1). No new cap.

---

## §1 P12a — Backend `ours`/`theirs` + `resolve_conflict_text` + IPC

### 1.1 `src-tauri/src/git/conflict.rs` — extend `ConflictFile`

Add two fields (serde camelCase, already the struct's convention):

```rust
/// Read-only working-tree view of one conflicted file, markers included.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictFile {
    pub path: String,
    pub kind: ConflictKind,
    pub binary: bool,
    pub too_large: bool,
    pub missing: bool,
    /// Lossy UTF-8 of the worktree file WITH the <<<<<<< ======= >>>>>>> markers.
    pub text: String,
    /// Lossy UTF-8 of the stage-2 (OURS) blob content. "" when the ours side is
    /// absent (deletedByUs / addedByThem / bothDeleted) OR when binary/too_large/
    /// missing suppressed `text`.
    pub ours: String,
    /// Lossy UTF-8 of the stage-3 (THEIRS) blob content. "" when the theirs side
    /// is absent (deletedByThem / addedByUs / bothDeleted) OR when suppressed.
    pub theirs: String,
}
```

Populate in `get_conflict` (§ current impl lines 134–169). Rules:

1. Compute `binary`/`too_large`/`missing` exactly as today from the WORKTREE file. **If any of the
   three is true, set `ours = "" ` and `theirs = ""` and return** (the "text is suppressed → all
   three suppressed" rule — keeps the payload bounded and the mode selection simple).
2. Otherwise read each side blob via the existing pattern: `let idx = repo.index()?;
   idx.get_path(rel, 2)` → `Some(e)` ⇒ `repo.find_blob(e.id)?.content()` →
   `String::from_utf8_lossy(..).into_owned()`, else `""`. Same for stage `3` → `theirs`. `rel =
   Path::new(path)`. Do NOT apply the 1 MiB cap per side beyond the whole-file suppression in (1) —
   a text file under `MAX_CONFLICT_BYTES` has bounded sides.
3. Thread `ours`/`theirs` through the existing `make` closure (add two params) or construct the
   struct directly. Keep `path`/`kind` from the resolved `entry`.

Update the existing `wire_shapes_are_camel_case` unit test (conflict.rs ~289) so the `ConflictFile`
JSON assertion includes `"ours": ...` and `"theirs": ...` (use non-empty sample strings for a
`bothModified` value and confirm the `deletedByThem` sample keeps them `""`).

### 1.2 `src-tauri/src/git/conflict.rs` — new `resolve_conflict_text`

```rust
/// Blocking. Stages a user-authored resolution for one CURRENTLY CONFLICTED
/// path: writes `content` verbatim to the worktree file (creating parent dirs)
/// then `index.add_path(rel)` (clears all conflict stages → stage 0) +
/// `index.write()`. This is the single primitive behind BOTH editor view modes
/// (unified + side-by-side); per-region accept / combination happen in the
/// frontend before calling this. Same trust model as `MarkResolved` / `git add`:
/// leftover <<<<<<< markers are NOT rejected (see decision below).
/// Non-conflicted path -> `AppError::Git("path '<p>' has no conflict")`.
pub fn resolve_conflict_text(workdir: &Path, path: &str, content: &str)
    -> Result<(), AppError>;
```

Exact ordered algorithm (mirrors `resolve_conflict`'s guards + the `Action::Write` sequence):

1. `validate_rel_path(path).map_err(|_| AppError::InvalidName(format!("invalid path: {path}")))?`
   — identical guard/mapping to `resolve_conflict` (no absolute / `..` escapes).
2. `let repo = open_workdir_repo(workdir)?;`
3. `let mut index = repo.index()?;`
4. `let _entry = find_conflict(&index, path)?;` — REQUIRE the path is currently conflicted
   (reuses the private helper → `AppError::Git("path '<p>' has no conflict")` on miss). The return
   value is unused; the call is the guard.
5. `let wd = repo.workdir().ok_or_else(|| AppError::Git("repository has no workdir".into()))?;`
   `let file = wd.join(path); let rel = Path::new(path);`
6. `if let Some(parent) = file.parent() { std::fs::create_dir_all(parent)?; }`
7. `std::fs::write(&file, content.as_bytes())?;` — write the resolved text verbatim (bytes of the
   `&str`; UTF-8). No CRLF normalization (the user's editor produced these bytes; match `git add`).
8. `index.add_path(rel)?;` (clears conflict stages 1/2/3, records stage 0)
9. `index.write()?;`
10. `Ok(())`

**Leftover-marker decision (locked, state in rustdoc): do NOT reject content containing
`<<<<<<<`.** Rationale: same trust model as `MarkResolved` and `git add` (P3c §3.2 / flag §11.10);
the frontend already gates its Save button on `hasUnresolvedMarkers` (§3), so an unresolved doc
never reaches this fn through the UI — a backend rejection would be a redundant second gate with a
worse error surface. Trust the caller.

Enumerated error kinds: `invalidName` (escape path), `git` (non-conflicted path via `find_conflict`,
missing workdir, or any libgit2 error), plus `io` surfaced through `AppError` for the fs write
(existing `From<std::io::Error>` path — same as `resolve_conflict`). `noRepo` is added at the command
layer (below).

### 1.3 `src-tauri/src/commands.rs` + `src-tauri/src/lib.rs` — new command

Standard thin → `_inner` → `spawn_blocking` shape (mirror `resolve_conflict` at ~1009). Does NOT
emit `repo-changed` (frontend refetches imperatively).

```rust
/// Stages user-authored resolved text for one conflicted path (P12 §1.2).
/// Errors: `noRepo` | `git` | `invalidName`.
#[tauri::command]
pub async fn resolve_conflict_text(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    content: String,
) -> Result<(), AppError> {
    resolve_conflict_text_inner(state.inner(), &repo_id, path, content).await
}

/// Runtime-free core (unit-testable without a Tauri app).
async fn resolve_conflict_text_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    content: String,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        conflict::resolve_conflict_text(&workdir, &path, &content)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
```

Register `resolve_conflict_text` in the `generate_handler![]` list in `lib.rs` (next to
`resolve_conflict`). Extend the existing `commands.rs` NoRepo test group to cover
`resolve_conflict_text_inner` (same shape as the other conflict-command NoRepo assertions).

### 1.4 IPC — `src/ipc/types.ts` / `tauri.ts` / `mock.ts`

`types.ts` — extend `ConflictFile` (add the two fields, keep order after `text`):

```ts
export interface ConflictFile {
  path: string;
  kind: ConflictKind;
  binary: boolean;
  tooLarge: boolean;
  missing: boolean;
  /** Worktree contents INCLUDING <<<<<<< ======= >>>>>>> markers. */
  text: string;
  /** Stage-2 (OURS) blob text. '' when the ours side is absent or text is suppressed. */
  ours: string;
  /** Stage-3 (THEIRS) blob text. '' when the theirs side is absent or text is suppressed. */
  theirs: string;
}
```

Add to `IpcApi` (near `resolveConflict`):

```ts
  /** Stage user-authored resolved text for one conflicted path (P12).
   *  Rejects noRepo | git | invalidName. */
  resolveConflictText(repoId: string, path: string, content: string): Promise<void>;
```

`tauri.ts` — one invoke wrapper (camelCase arg keys, snake_case command name):

```ts
  resolveConflictText(repoId: string, path: string, content: string): Promise<void> {
    return invoke<void>('resolve_conflict_text', { repoId, path, content });
  },
```

`mock.ts`:
- Extend the `MERGE_AUTH_TEXT` `getConflict` fixture (the `src/auth.ts` `conflictTexts` entry,
  ~line 243/273) to include `ours` and `theirs` derived from the marker text: `ours` = the file with
  each conflict region collapsed to its OURS block (lines between `<<<<<<<` and `=======`), markers
  removed; `theirs` = same with the THEIRS block. Concretely, seed two module consts
  `MERGE_AUTH_OURS` / `MERGE_AUTH_THEIRS` (hand-written, matching `MERGE_AUTH_TEXT`'s single region)
  and set them on the fixture. The `README.md` (`deletedByThem`) fixture keeps `ours`/`theirs` = `""`
  (text is the ours-only body; theirs is deletion — both editor-irrelevant; it stays a quick-action
  kind per §0.5).
- Add `resolveConflictText(repoId, path, content)`: mirror `resolveConflict`'s state mutation exactly
  — reject `{ kind: 'git', message: \`path '${path}' has no conflict\` }` when `path` is not in
  `state.conflicts`; otherwise drop `path` from `state.conflicts`, `state.conflictTexts`, and
  `state.status.conflicted`. It does NOT need the `deletedByThem→staged deletion` special-case (text
  editor only runs for `bothModified`/`bothAdded`). `await delay(150)`.

---

## §2 P12b — Unified editable editor + pure region helpers

Two pieces: the pure `src/utils/conflictRegions.ts` module (fully testable, drives P12c/P12d too)
and a first `src/components/ConflictEditor.tsx` that renders a unified editable CodeMirror doc seeded
from the worktree `text`, with Save / Cancel. Per-region accept widgets and side-by-side arrive in
P12c/P12d — P12b lands the shell + helpers + self-test.

### 2.1 `src/utils/conflictRegions.ts` — pure module (new)

Region data shape and functions (verbatim):

```ts
/** One <<<<<<< ======= >>>>>>> block, located by line index within a document.
 *  Line indices are 0-based into `text.split('\n')`. Always parse FRESH from the
 *  current document before acting — indices shift as regions are resolved. */
export interface ConflictRegion {
  /** 0-based order within the file (0 = first region). */
  index: number;
  /** Line index of the `<<<<<<<` marker. */
  startLine: number;
  /** Line index of the `=======` separator. */
  sepLine: number;
  /** Line index of the `>>>>>>>` marker. */
  endLine: number;
  /** Text after `<<<<<<< ` on the start line (e.g. "HEAD"); '' if none. */
  oursLabel: string;
  /** Text after `>>>>>>> ` on the end line (e.g. "feature/login"); '' if none. */
  theirsLabel: string;
  /** Lines strictly between startLine and sepLine (the OURS body). */
  oursLines: string[];
  /** Lines strictly between sepLine and endLine (the THEIRS body). */
  theirsLines: string[];
}

/** Parse every well-formed conflict region in `text`. A region requires a
 *  `<<<<<<<` line, a later `=======` line, then a `>>>>>>>` line, in order, with
 *  no nested `<<<<<<<` between them. Malformed/partial markers are skipped (never
 *  throw). Regions are returned in document order with sequential `index`. */
export function parseConflictRegions(text: string): ConflictRegion[];

/** Return a NEW document with `region`'s block (startLine..endLine inclusive)
 *  replaced by the chosen body:
 *   - 'ours'   -> region.oursLines
 *   - 'theirs' -> region.theirsLines
 *   - 'both'   -> region.oursLines followed by region.theirsLines (ours-block
 *                 THEN theirs-block, matching git marker order — §0.4)
 *  `region` MUST have been parsed from the SAME `text` passed here (indices are
 *  used directly). Preserves all other lines and the doc's trailing newline. */
export function applyResolution(
  text: string,
  region: ConflictRegion,
  choice: 'ours' | 'theirs' | 'both',
): string;

/** True if `text` still contains any conflict marker line (`<<<<<<<`, `=======`,
 *  or `>>>>>>>` at line start). Drives the Save/Stage-resolved gate. */
export function hasUnresolvedMarkers(text: string): boolean;
```

Marker detection matches the existing `MARKER_RE = /^(<{7}|={7}|>{7})/` (line-start, 7 chars).
Implementation notes: split on `'\n'`; `parseConflictRegions` scans linearly tracking state
(seeking-start → seeking-sep → seeking-end); a stray `=======` or `>>>>>>>` without an open
`<<<<<<<` is ignored; a second `<<<<<<<` before a `=======` closes/abandons the partial and
restarts. `applyResolution` rebuilds via `lines.slice(0, startLine)` + chosen body +
`lines.slice(endLine + 1)` then `.join('\n')`.

### 2.2 Self-test hook — `conflictSelfTest`

Add `conflictSelfTest?(): P7SelfTestResult;` to `BonsaiDevHooks` in `src/graph/frameStats.ts` (§105).
Register a `conflictSelfTest` on `window.__bonsai` (mock/dev only) that exercises the three helpers
over inline fixtures and logs `[bonsai] conflictSelfTest {...}` (mirror `p7SelfTest` exactly:
`{ pass, fail, failures }`). Because `window.__bonsai` is created AND torn down by `GraphCanvas`
(frameStats §670–673), register `conflictSelfTest` via a NON-destructive merge from `ConflictEditor`'s
mount effect: `window.__bonsai = { ...(window.__bonsai ?? {}), conflictSelfTest };` and, on cleanup,
delete only that key if it is still ours. (Flag §7.4 — alternative is a standalone module-load
registration in `conflictRegions.ts` guarded by `import.meta.env.DEV`.)

P12b coverage: `parseConflictRegions` finds 1 region in `MERGE_AUTH_TEXT` (correct start/sep/end,
labels `HEAD`/`feature/login`, ours/theirs bodies); `parseConflictRegions('no markers')` → `[]`;
`hasUnresolvedMarkers` true on the fixture, false after resolving; `applyResolution(text, r, 'ours')`
has no markers and keeps the ours body. (P12c extends this to all three choices + combination order.)

### 2.3 `src/components/ConflictEditor.tsx` — editor shell (new)

Mounts in place of `ConflictMarkerView` for text kinds ONLY, inside the DiffOverlay conflict body
(`ConflictSlotView`, §5). Props:

```ts
export interface ConflictEditorProps {
  /** The conflicted file (already fetched by RepoWorkspace's fetchConflictSlot).
   *  Guaranteed kind ∈ {bothModified, bothAdded} and !binary && !tooLarge &&
   *  !missing by the mount guard (§5); other kinds never reach this component. */
  file: ConflictFile;
  /** Stage the given resolved text (RepoWorkspace → ipc.resolveConflictText →
   *  refreshAll). Rejects on backend error; the editor shows it inline. */
  onResolve(path: string, content: string): Promise<void>;
  /** Close the editor without staging (collapse the slot). */
  onCancel(): void;
  /** Busy flag (RepoWorkspace `mutating`) — disables Save while a mutation runs. */
  mutating: boolean;
}
export function ConflictEditor(props: ConflictEditorProps): JSX.Element;
```

P12b behavior (unified only):
- A single React-owned result string, `useState` seeded ONCE from `file.text` (keyed by
  `file.path` — a new file re-seeds). This is the shared result doc referenced in §0.6; P12d reads
  and reseeds it across mode toggles.
- A CodeMirror `EditorView` (editable) hosted in a `useRef` div; created in a mount effect,
  destroyed on cleanup. Doc = the result string. On every doc change, sync the string back to React
  state (an `updateListener` extension → `setResult(view.state.doc.toString())`). Base extensions:
  `lineNumbers`, `highlightActiveLine`, `EditorView.lineWrapping` (off — code), `history()` +
  `keymap.of([...defaultKeymap, ...historyKeymap])` from `@codemirror/commands`. Theme follows the
  app: read the current theme and apply a light/dark CM theme (a minimal `EditorView.theme` keyed on
  the document's `data-theme`/`.dark` — no hard dependency on a CM theme package). Syntax
  highlighting is deferred to P12d (lazy `@codemirror/language-data`).
- Header: file path (mono) + a `lang-chip` from `detectLanguage(file.path)` (reuse
  `src/utils/language.ts`); a spacer; the mode toggle placeholder (wired in P12d); **Cancel**
  (`.btn-secondary`) and **Stage resolved** (`.btn-primary`).
- **Stage resolved** is `disabled` when `mutating` OR `hasUnresolvedMarkers(result)` is true; on
  click → `void onResolve(file.path, result)`. Cancel → `onCancel()`.
- Inline error region (reuse the `.error-banner` recipe from `ConflictSlotView`) shows a rejection
  from `onResolve`.

No `ipc` import inside `ConflictEditor` — it calls `onResolve`/`onCancel` props only (RepoWorkspace
owns IPC + refresh, matching the diffSlot ownership pattern). No Esc handling inside (App/workspace
owns Esc-layering; the existing `conflict:<path>` slot Esc collapses it).

---

## §3 P12c — Per-region accept widgets + overview ruler

Extend `ConflictEditor` with CodeMirror decorations and a custom scrollbar ruler. All rewrites go
through `applyResolution` semantics; the doc stays the single source of truth.

### 3.1 Per-region accept widgets

- A CM `ViewPlugin` (or `StateField<DecorationSet>`) that, on every doc change, re-parses the current
  doc with `parseConflictRegions(view.state.doc.toString())` and places, for each region, a **block
  widget** at the `<<<<<<<` marker line (a `Decoration.widget` with `block: true`, `side: -1`). The
  widget DOM is a small toolbar: **Accept Ours**, **Accept Theirs**, **Accept Both** buttons
  (+ the `oursLabel`/`theirsLabel` as captions).
- Each button, on click, recomputes regions from the CURRENT doc (indices may have shifted), finds
  the region by `index`, computes `next = applyResolution(doc, region, choice)`, and dispatches a
  transaction replacing the whole doc range `{ from: 0, to: doc.length, insert: next }`. (Whole-doc
  replace is simplest and correct; a range-scoped replace over `line(startLine).from ..
  line(endLine).to` is an allowed optimization — either way re-parse from the post-change doc.)
- Optionally style the ours/theirs body line ranges with a tinted `Decoration.line` (`--accent`
  tints, reuse the `conflict-marker-line` color spirit) so regions are visually obvious. Keep it
  cheap — capped at 1 MiB per §0.
- Region widgets disappear naturally as regions are resolved (the re-parse finds fewer regions).

### 3.2 Overview ruler CM extension

- A custom "conflict overview ruler" extension (a `ViewPlugin` managing an absolutely-positioned
  overlay element on the editor's right edge, OR reuse `@codemirror/view`'s scrollbar area): for each
  UNRESOLVED region (from `parseConflictRegions`), draw a tick at vertical fraction
  `region.startLine / max(1, totalLines - 1)` of the scroll height.
- Ticks are `--accent`-tinted 2–3px bars. Clicking a tick scrolls the region into view:
  `view.dispatch({ effects: EditorView.scrollIntoView(view.state.doc.line(region.startLine + 1).from,
  { y: 'center' }) })`.
- The ruler recomputes on doc change (regions resolve → ticks vanish). No ticks when zero regions.

### 3.3 Save gate

**Stage resolved** stays enabled only when `hasUnresolvedMarkers(result)` is false (§2.3) — after all
regions are accepted/edited away, the button lights up. Keep the `mutating` disable.

### 3.4 Self-test extension

Extend `conflictSelfTest` to assert all three `applyResolution` choices on `MERGE_AUTH_TEXT`:
- `'ours'` → doc equals `MERGE_AUTH_OURS`-equivalent (ours body substituted), no markers.
- `'theirs'` → theirs body substituted, no markers.
- `'both'` → contains the ours body IMMEDIATELY followed by the theirs body (ours-then-theirs
  ordering, §0.4), no markers.
- A two-region synthetic fixture: resolving region 0 leaves region 1 intact and correctly indexed
  (re-parse after the rewrite finds exactly one remaining region).

---

## §4 P12d — Side-by-side MergeView + syntax highlighting + wiring

### 4.1 Side-by-side via `@codemirror/merge`

- A mode toggle in the editor header: **Unified** ⇄ **Side-by-side**. React state
  `mode: 'unified' | 'split'`.
- Split mode mounts a `MergeView` from `@codemirror/merge`: `a` = `file.ours` (read-only,
  `EditorState.readOnly.of(true)`, "Ours"); `b` = the SHARED editable result doc (seeded from the
  current `result` string, "Theirs/Result"). The `b` editor uses the same base extensions +
  updateListener as the unified editor so edits keep syncing to React state, and the same per-region
  widgets + overview ruler (§3) apply to `b`.
- **Toggle preserves the shared result doc (§0.6):** before switching modes, read the live doc from
  the currently-mounted view into `result` state; unmount it; mount the target mode seeded from
  `result`. Never lose in-progress edits. Because both modes render from and write to the one
  `result` string, the CM chunk-accept gutter (`@codemirror/merge` provides accept-chunk controls
  a→b) and the §3 region toolbar both mutate the same document consistently.
- The combination control (Accept Both) from §3 remains available in split mode (region widgets on
  the `b` editor).

### 4.2 Lazy syntax highlighting

- Resolve a language for `file.path` lazily via `@codemirror/language-data`: find the
  `LanguageDescription` by file extension (`LanguageDescription.matchFilename(languages, path)`),
  `await desc.load()`, then reconfigure the editor with the returned `Extension` via a
  `Compartment`. Guard against unmount (ignore the resolved extension if the view is gone). No
  highlighting when no language matches (plain text). Use `detectLanguage(file.path)` only for the
  header `lang-chip` LABEL, not for CM language selection (CM uses its own `language-data`).

### 4.3 RepoWorkspace wiring

- New handler next to `handleResolveConflict` (~952):
  ```ts
  async function handleResolveConflictText(path: string, content: string): Promise<void> {
    setMutating(true);
    try {
      await ipc.resolveConflictText(repoId, path, content);
      await refreshAll();          // existing full-refresh batch (drops the resolved path,
                                    // re-reads op state + conflicts; collapses the slot when the
                                    // path is no longer conflicted — same rule as handleResolveConflict)
      pushToast('success', `Staged resolution for ${path}`);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }
  ```
- Route text-kind conflict slots to `ConflictEditor`. The conflict body currently renders via
  `ConflictSlotView` → `ConflictMarkerView` (DiffOverlay §5). Change the DiffOverlay conflict-body
  branch so that when `slot.conflict` is a text-mergeable file
  (`kind === 'bothModified' || kind === 'bothAdded'`) and `!binary && !tooLarge && !missing`, it
  renders `<ConflictEditor file={slot.conflict} onResolve={onResolveConflictText} onCancel={onClose}
  mutating={mutating} />`; otherwise it renders the existing `ConflictMarkerView` placeholder path
  unchanged. Thread `onResolveConflictText` + `mutating` from RepoWorkspace → App → DiffOverlay props
  (extend `DiffOverlayProps` with `onResolveConflictText(path, content): Promise<void>` and
  `mutating: boolean`, both only used by the conflict branch). Pass
  `onResolveConflictText={(p, c) => handleResolveConflictText(p, c)}` and the existing `mutating`.
- The StatusPanel `ConflictRow` quick actions (ours/theirs/resolved) and `handleResolveConflict` are
  UNCHANGED — they remain the fallback for non-text kinds and a fast path for text kinds too. Clicking
  a conflict row still opens the overlay (`handleToggleConflictView`); for text kinds the overlay now
  hosts the editor instead of the read-only `<pre>`.

---

## §5 Mount decision — editor vs placeholder (DiffOverlay)

`ConflictMarkerView` STAYS as the fallback (do NOT delete it). The conflict body picks its renderer:

```
if file is text-mergeable (kind ∈ {bothModified, bothAdded} && !binary && !tooLarge && !missing):
    <ConflictEditor .../>
else:
    <ConflictMarkerView file={file} />   // binary/tooLarge/missing placeholders + read-only markers
```

This keeps every non-text kind on the proven P3c path and confines CodeMirror to the two text-merge
kinds (§0.5).

---

## §6 Consolidated acceptance checklist

Standing rules: scratch repos under `D:\Temp\bonsai-scratch`; `TMP`/`TEMP` = `D:\Temp`; NEVER run
`cargo test` and `clippy` concurrently (shared target-dir race); command tests target the
runtime-free `*_inner` fns; every wire type mirrored in `types.ts` + `tauri.ts` + `mock.ts`.

### Rust (`cargo test` / `cargo clippy -- -D warnings`)
- [ ] On a real `bothModified` merge fixture, `get_conflict().ours` == the stage-2 blob content and
      `.theirs` == the stage-3 blob content (byte-compare against `index.get_path(rel, 2|3)` blobs);
      `text` still carries the markers.
- [ ] Suppression: a `tooLarge` (> 1 MiB) and a `binary` (NUL) fixture return `ours == "" &&
      theirs == "" && text == ""`.
- [ ] `resolve_conflict_text` round-trip: after calling it with hand-merged content, the path is gone
      from `list_conflicts`, the index has NO conflict entry for the path (stage 0 only), and the
      worktree file bytes equal `content`.
- [ ] `resolve_conflict_text` on a non-conflicted path → `AppError::Git("... has no conflict")`.
- [ ] `resolve_conflict_text` with an escaping path (`../x`) → `AppError::InvalidName`.
- [ ] Leftover-marker content is accepted (no rejection) and stages at stage 0 (documents the trust
      model).
- [ ] `wire_shapes_are_camel_case` updated: `ConflictFile` JSON includes `"ours"`/`"theirs"`.
- [ ] `resolve_conflict_text_inner` NoRepo test (no open repo → `noRepo`).
- [ ] `conflict_cli.rs` oracle: where a `git` comparison applies, resolving via `resolve_conflict_text`
      with the same hand-merged bytes yields the same stage-0 blob oid the twin gets after
      `printf ... > f; git add f`.

### Frontend harness (`pnpm build` + `tsc` + `pnpm dev` with `VITE_MOCK_IPC=1`)
- [ ] `pnpm build` / `tsc` clean; `src/ipc/mock.ts` compiles with the new `resolveConflictText` + the
      extended `getConflict` fixture (`ours`/`theirs`).
- [ ] `?op=merge` → clicking the `src/auth.ts` (`bothModified`) conflict row opens `ConflictEditor`
      (not the `<pre>` marker view); `README.md` (`deletedByThem`) still opens the read-only
      placeholder + keeps its quick actions.
- [ ] `window.__bonsai.conflictSelfTest()` returns `N pass / 0 fail` (parse, applyResolution
      ours/theirs/both incl. ordering, hasUnresolvedMarkers, multi-region indexing).

### USER CHECKPOINT (native `pnpm tauri dev` — self-declare FORBIDDEN)
CodeMirror layout, scrollbar, and DOM measurement depend on `requestAnimationFrame`, which the
browser THROTTLES when `document.hidden === true` (the headless harness pane). Therefore ALL
visual/interactive CodeMirror verification is USER-only and MUST NOT be declared passed from the
harness:
- [ ] Editor renders with line numbers + syntax highlighting; conflict regions visibly tinted.
- [ ] Unified ⇄ side-by-side toggle works and PRESERVES in-progress edits both directions.
- [ ] Per-region Accept Ours / Theirs / Both rewrite that block correctly (Both = ours-then-theirs).
- [ ] Direct editing of the merged result works; Stage-resolved stays disabled until zero markers.
- [ ] Overview-ruler ticks render at each unresolved region and click-jump scrolls to it.
- [ ] Save stages the file and the conflict clears (row disappears / OpBanner count drops); resolving
      all conflicts then committing the merge works via the existing OpBanner path.

---

## §7 Flags / decisions for the orchestrator

1. **`ConflictMarkerView` retained as fallback (§5), not replaced.** `ConflictEditor` handles only
   text-mergeable kinds; `binary`/`tooLarge`/`missing` and all non-text kinds keep the read-only
   marker/placeholder view. Recommendation: KEEP the fallback (deleting it would strand those kinds).
2. **Drop the P3c "marker view is a plain `<pre>`, not DiffView" note?** P12 supersedes it for text
   kinds only. Recommendation: leave P3c's contract as historical record; note in P12 §5 that the
   `<pre>` path now applies only to non-text kinds. No code note to remove.
3. **Combination ordering = ours-then-theirs (§0.4).** Matches git marker order and CM `a→b` chunk
   semantics. Recommendation: keep; expose a reversed option only if a user asks (not in scope).
4. **`conflictSelfTest` registration** merges non-destructively onto `window.__bonsai` from
   `ConflictEditor`'s effect (§2.2), because `GraphCanvas` owns the object's lifecycle. Recommendation:
   this merge approach; alternative is a `DEV`-guarded module-load registration in `conflictRegions.ts`
   (simpler but always-on). Flagged so the orchestrator can pick.
5. **Tripled payload cap unchanged (§0).** `ConflictFile` now carries up to ~3×1 MiB for one
   on-demand file; the per-side suppression (all three `""` when `tooLarge`) already bounds it.
   Recommendation: no separate cap — a single user-initiated file is fine. Revisit only if a
   multi-file prefetch is ever added.
6. **No CRLF normalization on save (§1.2 step 7).** The editor's bytes are written verbatim, matching
   `git add`. Recommendation: keep (normalizing would surprise users with CRLF worktrees); Git's
   own `core.autocrlf` filters still apply through libgit2's `add_path`.
7. **CM theme.** P12b uses a minimal inline light/dark `EditorView.theme` rather than adding a theme
   dep (`@codemirror/theme-one-dark` etc.). Recommendation: inline theme keyed on the app's
   `data-theme`; add a theme package later only if styling is insufficient.
