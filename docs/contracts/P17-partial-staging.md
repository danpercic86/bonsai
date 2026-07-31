# P17 — Interactive Diff: File/Diff Toggle + Partial Staging: Implementation Contract

Status: authoritative for P17. Implementer: senior-dev, three fresh-context passes
(P17a Rust, P17b IPC+mock, P17c frontend). Builds on `M4-diff.md` (diff data model
LineKind/DiffLine/Hunk/FileDiff, diff engine §2, IPC conventions §2.8/§3, mock policy §5),
`M3-commit.md` (path validation §2.1, stage/unstage semantics, mock statefulness),
`P3a-diff-overlay.md` (full-pane DiffOverlay, `DiffOverlayMeta.kind`, DiffSlot machinery).

**P17 deliberately expands past M4's locked "no hunk staging, read-only view."** The rest of
M4's scope (diff engine, size caps, wire shapes, accordion→overlay render path) is unchanged;
P17 adds granular staging in **working-directory diffs only** and a **File/Diff view toggle in
every diff view**.

Architecture invariants (unchanged, enforced in review):
- Rust owns ALL Git logic. The frontend sends a granularity-agnostic **line selection**; Rust
  does the byte-exact index edit. React only renders + collects the selection.
- IPC carries compact precomputed data. Commands = request/response; no new events, no channels.
  No `repo-changed` emit from the new mutations — the frontend refetches imperatively (mirrors
  `stage`/`unstage`, M3 §2.7).
- git2 0.20 is blocking → every command runs its core via `spawn_blocking` (`_inner` twin).
- The mock IPC (`src/ipc/mock.ts`, `VITE_MOCK_IPC=1`) is updated in the same pass as the IPC
  surface so the harness runs in a plain browser.

---

## 0. Headline decisions

1. **Blob reconstruction, not patch-text synthesis** (§2.4). git2 0.20 has no line-level apply
   primitive and the wire `DiffLine.content` is lossy (`from_utf8_lossy`, `\n`/`\r` stripped —
   M4 §2.4). The backend recomputes the exact diff for the direction, reads the **raw bytes** of
   both blobs, and splices line-slices by their `old_no`/`new_no` to build the new index blob,
   written with `Index::add_frombuffer`. Line slices keep their own terminator → CRLF and
   no-newline-at-EOF round-trip exactly.
2. **Direction encoded by the command**, not a parameter: `stage_partial` (index→ toward workdir)
   vs `unstage_partial` (index→ toward HEAD). Mirrors the existing separate `stage`/`unstage`.
   The backend is **granularity-agnostic**: whole-hunk / single-line / mouse-range all collapse
   to "which changed lines are in the selection set."
3. **Symmetric** (locked): `stage_partial` promotes selected unstaged/untracked changes into the
   index; `unstage_partial` demotes selected staged changes back out. A file can legitimately
   appear in **both** the staged and unstaged sections afterward (each side rebuilds from the
   current index, so partial stages compose).
4. **Granular staging is working-directory-only** (locked): kinds `unstaged`/`untracked` → stage,
   `staged` → unstage. Commit & compare diffs stay read-only (no gutter/hunk/selection controls).
   The **File/Diff toggle is available in every diff view** including commit/compare/conflict.
5. **File View = full-context diff** (§2.6): a `full_context: bool` param threads into
   `build_diff_options` → `context_lines(u32::MAX)`, producing one whole-file hunk. Selection
   coordinates are absolute line numbers, so File View and Diff View are interchangeable for
   staging. The 5000-line cap still applies (huge file in File View trips `too_large`).
6. **Reject what can't be spliced safely** (§2.5): binary / too_large / renamed → clear error
   (frontend falls back to whole-file stage). **Stale selection** (a coordinate absent from the
   freshly recomputed diff) → error `"selection is stale; refresh the diff"`. No new `AppError`
   variants — reuse `AppError::Other` (invalid/stale/unsupported) and `AppError::Git`.
7. **No mock reimplementation of git** (§4): exactly one fixture file (`src/main.rs`) gets a
   minimal three-way line-array model (head/index/workdir); its diffs and partial mutations are
   computed from it with the SAME reconstruction rule (on `string[]`, terminators irrelevant).
   Every other file stays static and rejects partial staging.

---

## 1. New / changed files

```
crates/bonsai-core/src/git/
  stage_partial.rs            # NEW: LineSelection + stage_partial/unstage_partial + apply_partial
                              #      + split_keep_terminator + assemble + reconstruction + unit tests
  diff.rs                     # LineKind: derive Deserialize; collect_file_diff -> pub(crate);
                              #   build_diff_options(paths, full_context); full_context param on the
                              #   three *_file_diff fns; update commit_diff/compare_head_diff call sites
  mod.rs                      # + pub mod stage_partial;
  tests/stage_partial_cli.rs  # NEW (crates/bonsai-core/tests/): CLI-oracle integration tests (§5)
src-tauri/src/
  commands.rs                 # + stage_partial, unstage_partial (+ _inner twins); full_context arg
                              #   on get_workdir_file_diff / get_commit_file_diff /
                              #   compare_with_head_file_diff (+ their _inner twins)
  lib.rs                      # register stage_partial, unstage_partial
src/
  ipc/types.ts                # + LineSelection; IpcApi.stagePartial/unstagePartial;
                              #   fullContext arg on the three file-diff getters
  ipc/tauri.ts                # + 2 invoke wrappers; thread fullContext into the three getters
  ipc/mock.ts                 # + stagePartial/unstagePartial; three-way model wiring; fullContext
  ipc/fixtures/diffs.ts       # + three-way model for src/main.rs + a shared reconstruct/lineDiff
  components/DiffView.tsx      # interactive props: viewMode, stageable, onStageLines, onStageHunk;
                              #   File View layout; gutter controls; hunk button; mouse selection
  components/DiffOverlay.tsx   # File/Diff segmented toggle; forward staging props to DiffSlotView
  components/RepoWorkspace.tsx # diffViewMode state; handleStageLines/handleStageHunk; fetcher wiring
  styles.css                  # gutter controls, hunk button, selected-line highlight, floating
                              #   action button, File/Diff segmented toggle
```

---

## 2. Rust — backend (P17a)

### 2.1 Wire type + LineKind change (`stage_partial.rs`, `diff.rs`)

```rust
// diff.rs — LineKind gains Deserialize (currently Serialize-only, M4 §2.1):
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineKind { Context, Add, Del }
```

```rust
// stage_partial.rs — the wire selection element. BOTH Serialize + Deserialize.
use crate::git::diff::LineKind;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineSelection {
    /// `Add` or `Del`. `Context` elements (if any leak from the UI) are IGNORED
    /// (context is always kept in both directions) and do NOT participate in the
    /// stale-selection check.
    pub kind: LineKind,
    /// OLD-file line number; the identity of a selected `Del` line.
    pub old_no: Option<u32>,
    /// NEW-file line number; the identity of a selected `Add` line.
    pub new_no: Option<u32>,
}
```

### 2.2 Public functions (`stage_partial.rs`)

```rust
use std::path::Path;
use crate::error::AppError;

#[derive(Clone, Copy)]
enum Direction { Stage, Unstage }

/// Stage the selected changed lines of ONE working-dir file (index moves toward
/// the workdir for the selected lines only). Empty selection -> Ok no-op.
pub fn stage_partial(
    workdir: &Path, path: &str, orig_path: Option<&str>, selection: &[LineSelection],
) -> Result<(), AppError>;

/// Unstage the selected changed lines of ONE staged file (index moves toward
/// HEAD for the selected lines only). Empty selection -> Ok no-op.
pub fn unstage_partial(
    workdir: &Path, path: &str, orig_path: Option<&str>, selection: &[LineSelection],
) -> Result<(), AppError>;

// Both delegate to:
fn apply_partial(
    workdir: &Path, path: &str, orig_path: Option<&str>,
    selection: &[LineSelection], dir: Direction,
) -> Result<(), AppError>;
```

### 2.3 `apply_partial` — control flow (normative)

```
apply_partial(workdir, path, orig_path, selection, dir):
  validate_rel_path(path)?                        # reuse stage.rs (M3 §2.1)
  if let Some(op) = orig_path { validate_rel_path(op)? }
  if selection.is_empty() { return Ok(()) }        # no-op before any repo work

  repo = open_workdir_repo(workdir)?               # reuse stage.rs (NO_SEARCH, reject bare)
  wd   = repo.workdir()  (else Git("repository has no workdir"))

  # Recompute the exact diff for this direction (DEFAULT 3-context is fine — the
  # reconstruction fills inter-hunk gaps from the blobs by line number; context
  # amount never changes add/del line numbers). Restricted to path (+orig_path).
  paths = pathspecs(path, orig_path)               # diff.rs helper (path + rename OLD side)
  opts  = build_diff_options(&paths, /*full_context=*/false)
  diff = match dir:
      Stage   -> repo.diff_index_to_workdir(None, Some(&mut opts))?     # old=index, new=workdir
      Unstage -> old = head_tree(&repo)?                                # None when HEAD unborn
                 repo.diff_tree_to_index(old.as_ref(), None, Some(&mut opts))?  # old=HEAD, new=index
  apply_find_similar(&mut diff)?                    # diff.rs (renames)

  fd = collect_file_diff(&diff)?                    # now pub(crate); Option<FileDiff>
       .ok_or_else(|| Other("selection is stale; refresh the diff"))?   # pathspec matched nothing

  # Guards (§2.5): frontend already avoids these, but defend.
  if fd.binary    { return Err(Other("partial staging is not supported for binary files")) }
  if fd.too_large { return Err(Other("partial staging is not supported for a too-large diff")) }
  if fd.orig_path.is_some() || fd.status == Renamed {
      return Err(Other("partial staging is not supported for renamed files")) }

  # Build selected sets + validate (stale check).
  sel_add: HashSet<u32> = selection where kind==Add  -> new_no (Some)
  sel_del: HashSet<u32> = selection where kind==Del  -> old_no (Some)
  valid_add = { l.new_no | l in all hunk lines, l.kind==Add }
  valid_del = { l.old_no | l in all hunk lines, l.kind==Del }
  if any x in sel_add not in valid_add  -> Err(Other("selection is stale; refresh the diff"))
  if any x in sel_del not in valid_del  -> Err(Other("selection is stale; refresh the diff"))

  # Raw bytes of both sides (NEVER DiffLine.content).
  match dir:
    Stage:
      old_bytes = index stage-0 blob bytes for `path`, or b"" if untracked (no entry)
      new_bytes = fs::read(wd.join(path)) , or b"" if the workdir file is missing (deleted)
    Unstage:
      old_bytes = HEAD blob bytes for `path`, or b"" if HEAD unborn or file added (absent in HEAD)
      new_bytes = index stage-0 blob bytes for `path`, or b"" if absent in index

  old_lines = split_keep_terminator(&old_bytes)
  new_lines = split_keep_terminator(&new_bytes)
  result    = reconstruct(dir, &fd.hunks, &old_lines, &new_lines, &sel_add, &sel_del)  # §2.4
  content   = assemble(&result)                                                        # §2.4

  # No-op: reconstructed index content == current index content -> return Ok, write nothing.
  cur_index_bytes = current index stage-0 blob bytes for `path`, or b"" if absent
  if content == cur_index_bytes { return Ok(()) }

  # Removal cases (§2.5): the file legitimately should not exist in the index.
  if content.is_empty() && should_remove(dir, &new_bytes, &old_bytes) {
      index.remove_path(Path::new(path))?; index.write()?; return Ok(())
  }

  entry = index.get_path(Path::new(path), 0)                # stage-0 template if present
          .unwrap_or_else(|| synthesize_entry(path, &wd))   # untracked: mode from symlink_metadata
  index.add_frombuffer(&entry, &content)?
  index.write()?
  Ok(())
```

Helper notes:
- **Reading blob bytes.** Index side: `index.get_path(path, 0)` → `IndexEntry`; `repo.find_blob(entry.id)?.content().to_vec()`. HEAD side: `head_tree.get_path(Path::new(path))` → `TreeEntry`; `repo.find_blob(entry.id())?.content().to_vec()` (missing entry → `b""`). Absent index/HEAD entry → `b""`.
- **`should_remove(dir, new_bytes, old_bytes)`**: `Stage` → `new_bytes.is_empty()` (workdir file deleted, all old lines selected). `Unstage` → `old_bytes.is_empty()` (HEAD lacks the file, i.e. a never-committed add being fully unstaged). Otherwise write the (possibly empty) blob.
- **`synthesize_entry(path, wd)`** (untracked stage only): build a `git2::IndexEntry` with `path = path.as_bytes().to_vec()`, `mode` from `wd.join(path).symlink_metadata()` (`0o120000` symlink, `0o100755` if the owner-exec bit is set on unix, else `0o100644`; on Windows default `0o100644`), and all other fields (`ctime/mtime/dev/ino/uid/gid/file_size/flags/flags_extended`) `0` and `id = Oid::zero()` — `add_frombuffer` recomputes `id`/`file_size` from the buffer.

### 2.4 Reconstruction + assembly (normative)

```
# Each returned slice keeps its trailing terminator; only the LAST slice of a file
# may lack one. b"" -> vec![]. This is what makes CRLF + no-EOF-newline exact.
split_keep_terminator(bytes: &[u8]) -> Vec<&[u8]>:
    out = []; start = 0
    for i in 0..bytes.len():
        if bytes[i] == b'\n' { out.push(&bytes[start..=i]); start = i+1 }
    if start < bytes.len() { out.push(&bytes[start..]) }   # trailing no-newline line
    return out

# 1-based line numbers. Fill inter-hunk gaps from the base side by line number.
reconstruct(dir, hunks, old_lines, new_lines, sel_add, sel_del) -> Vec<&[u8]>:
    result = []
    match dir:
      Stage:                                   # base = OLD (index); target = new index
        cursor = 1                              # next OLD line not yet emitted
        for h in hunks:
            while cursor < h.old_start { push old_lines[cursor-1]; cursor += 1 }
            for line in h.lines:
                match line.kind:
                  Context: push old_lines[line.old_no-1]; cursor = line.old_no + 1
                  Del: if sel_del.contains(line.old_no) { /* drop */ }
                       else { push old_lines[line.old_no-1] }
                       cursor = line.old_no + 1
                  Add: if sel_add.contains(line.new_no) { push new_lines[line.new_no-1] }
                       # unselected Add: skip (cursor unchanged — Add has no old line)
        while cursor <= old_lines.len() { push old_lines[cursor-1]; cursor += 1 }
      Unstage:                                 # base = NEW (index); target = new index
        cursor = 1                              # next NEW line not yet emitted
        for h in hunks:
            while cursor < h.new_start { push new_lines[cursor-1]; cursor += 1 }
            for line in h.lines:
                match line.kind:
                  Context: push new_lines[line.new_no-1]; cursor = line.new_no + 1
                  Add: if sel_add.contains(line.new_no) { /* drop from index */ }
                       else { push new_lines[line.new_no-1] }
                       cursor = line.new_no + 1
                  Del: if sel_del.contains(line.old_no) { push old_lines[line.old_no-1] }  # restore HEAD line
                       # unselected Del: skip (cursor unchanged — Del has no new line)
        while cursor <= new_lines.len() { push new_lines[cursor-1]; cursor += 1 }
    return result

# Concatenate. A slice that lacked a terminator (was EOF in its SOURCE file) but is
# now interior gets a single b'\n'. The final slice keeps its own terminator state
# (present -> file ends with newline; absent -> no final newline).
assemble(lines: &[&[u8]]) -> Vec<u8>:
    out = []
    for (i, s) in lines.iter().enumerate():
        out.extend_from_slice(s)
        let is_last = i == lines.len()-1
        if !is_last && s.last() != Some(&b'\n') { out.push(b'\n') }
    return out
```

Correctness intuition: **Stage** = OLD (index) with unselected changes reverted and selected
changes applied. **Unstage** = NEW (index) with selected changes undone (adds removed, dels
restored from HEAD) and unselected changes kept. Both are pure re-derivations of the current
index, so repeated partial calls compose.

### 2.5 Guards / edge cases (all covered by §2.3 flow — restated for review)

- **binary / too_large / renamed** → `AppError::Other(...)`; frontend never reaches this (it
  suppresses controls) but it is the defensive contract.
- **Stale selection** → `AppError::Other("selection is stale; refresh the diff")` for both the
  pathspec-matched-nothing case and any selection coordinate absent from the recomputed diff.
- **Empty selection** → `Ok(())` (before opening the repo).
- **Deleted file, all old lines selected (Stage)** → `index.remove_path`.
- **Fully unstaging a never-committed add (Unstage, HEAD lacks file, all adds selected)** →
  `index.remove_path`.
- **Untracked partial (Stage)** → `old_bytes = b""`; synthesized `IndexEntry`.
- **Unborn HEAD (Unstage)** → `head_tree` = `None` → `old_bytes = b""` (all index lines are Add).
- **Compose-on-partially-staged** → base is always the CURRENT index, so it composes naturally.
- **Result == current index content** → `Ok(())`, no `index.write()`.

### 2.6 Full-context diffs (`diff.rs`)

```rust
// Signature change (thread full_context through; caps unchanged):
pub(crate) fn build_diff_options(paths: &[&str], full_context: bool) -> git2::DiffOptions;
//   full_context == true  -> opts.context_lines(u32::MAX)   (one whole-file hunk)
//   full_context == false -> opts.context_lines(3)          (M4 default)

// Add `full_context: bool` as the LAST param of these three (M4 §2.2):
pub fn workdir_file_diff(workdir, path, orig_path, staged, full_context) -> Result<FileDiff, _>;
pub fn commit_file_diff(workdir, oid, path, orig_path, full_context) -> Result<FileDiff, _>;
pub fn compare_head_file_diff(workdir, to_oid, path, orig_path, full_context)
    -> Result<FileDiff, _>;
```

- `collect_file_diff` becomes `pub(crate)` (unchanged body).
- Update the two header call sites (`commit_diff`, `compare_head_diff`) to
  `build_diff_options(&[], false)`. `apply_partial` calls `build_diff_options(&paths, false)`.
- The 5000-line cap (`MAX_FILE_DIFF_LINES`) is enforced by `collect_file_diff` regardless of
  context, so a huge file in File View still returns `too_large: true`.

### 2.7 Commands + registration (`commands.rs`, `lib.rs`)

Same `_inner` + `spawn_blocking` pattern as `stage` / `get_workdir_file_diff` (§ commands.rs).

```rust
#[tauri::command]
pub async fn stage_partial(state: tauri::State<'_, AppState>, repo_id: String,
    path: String, orig_path: Option<String>, selection: Vec<LineSelection>) -> Result<(), AppError>;

#[tauri::command]
pub async fn unstage_partial(state: tauri::State<'_, AppState>, repo_id: String,
    path: String, orig_path: Option<String>, selection: Vec<LineSelection>) -> Result<(), AppError>;
```

`_inner` twins: `repo_path(state, repo_id)?` → `spawn_blocking(move || stage_partial(&workdir,
&path, orig_path.as_deref(), &selection))`, join error → `AppError::Other`.

Add `full_context: bool` to the three existing file-diff commands + their `_inner` twins:
`get_workdir_file_diff(.., staged, full_context)`, `get_commit_file_diff(.., orig_path,
full_context)`, `compare_with_head_file_diff(.., orig_path, full_context)`; forward into the
core fns. Import `stage_partial`, `unstage_partial`, `LineSelection` from
`bonsai_core::git::stage_partial`.

`lib.rs`: register `commands::stage_partial, commands::unstage_partial` in `generate_handler!`.
No `repo-changed` emit. No new `AppError` variants. No capability changes.

Command surface after P17 gains exactly two commands (`stage_partial`, `unstage_partial`); the
three file-diff commands gain one argument each.

---

## 3. IPC layer (P17b — TypeScript)

`src/ipc/types.ts`:

```ts
export interface LineSelection {
  kind: LineKind;             // 'add' | 'del' (context dropped before sending)
  oldNo: number | null;
  newNo: number | null;
}
```

`IpcApi` gains + changes (fullContext is the LAST arg, matching the Rust param order):

```ts
/** Stage only the selected changed lines of one working-dir file (index moves
 *  toward the workdir). Empty selection is a no-op. Rejects AppError
 *  ('noRepo' | 'git' | 'other'[stale/unsupported/invalid path]). */
stagePartial(repoId: string, path: string, origPath: string | null,
             selection: LineSelection[]): Promise<void>;
/** Unstage only the selected changed lines of one staged file (index moves
 *  toward HEAD). Empty selection is a no-op. Same rejections. */
unstagePartial(repoId: string, path: string, origPath: string | null,
               selection: LineSelection[]): Promise<void>;

// fullContext added to the three getters (true -> one whole-file hunk / File View):
getWorkdirFileDiff(repoId, path, origPath, staged, fullContext: boolean): Promise<FileDiff>;
getCommitFileDiff(repoId, oid, path, origPath, fullContext: boolean): Promise<FileDiff>;
compareWithHeadFileDiff(repoId, oid, path, origPath, fullContext: boolean): Promise<FileDiff>;
```

`src/ipc/tauri.ts` (camelCase args auto-map to snake_case Rust params):

```ts
stagePartial: (repoId, path, origPath, selection) =>
  invoke<void>('stage_partial', { repoId, path, origPath, selection }),
unstagePartial: (repoId, path, origPath, selection) =>
  invoke<void>('unstage_partial', { repoId, path, origPath, selection }),
getWorkdirFileDiff: (repoId, path, origPath, staged, fullContext) =>
  invoke<FileDiff>('get_workdir_file_diff', { repoId, path, origPath, staged, fullContext }),
getCommitFileDiff: (repoId, oid, path, origPath, fullContext) =>
  invoke<FileDiff>('get_commit_file_diff', { repoId, oid, path, origPath, fullContext }),
compareWithHeadFileDiff: (repoId, oid, path, origPath, fullContext) =>
  invoke<FileDiff>('compare_with_head_file_diff', { repoId, oid, path, origPath, fullContext }),
```

**All existing callers of the three getters must pass the new `fullContext` argument** —
`RepoWorkspace.refetchStatus`, `handleToggleWorkdirDiff`, the selection→commit-file fetchers,
`DiffBrowser`, and any P15 AI paths. In P17b (before P17c wires the toggle) pass `false`.

---

## 4. Mock IPC (P17b — `src/ipc/mock.ts` + `src/ipc/fixtures/diffs.ts`)

Policy (M4 §5 extended): every existing static per-path fixture stays. **Exactly one file,
`src/main.rs`, becomes a live three-way model** so partial stage/unstage visibly moves lines and
the file shows in BOTH sections. Everything else is static and rejects partial staging.

### 4.1 Three-way model + shared reconstruction (`fixtures/diffs.ts`)

```ts
export interface ThreeWay { head: string[]; index: string[]; workdir: string[]; }

/** Seed for src/main.rs: index != head (a staged change) AND workdir != index (an
 *  unstaged change) so the file appears in both sections from first paint. */
export function initialMainRs(): ThreeWay; // returns fresh arrays (callers own the copy)

/** Minimal LCS line diff -> hunks. `fullContext` true -> one whole-file hunk
 *  (context = all); false -> 3 context lines around changes (multiple hunks). */
export function lineDiff(oldLines: string[], newLines: string[], path: string,
                         status: FileStatus, fullContext: boolean): FileDiff;

/** SAME rule as the Rust reconstruction (§2.4) on string[] (terminators irrelevant).
 *  Returns the new `index` array given the recomputed hunks + selected sets. */
export function reconstructLines(dir: 'stage' | 'unstage', hunks: Hunk[],
    oldLines: string[], newLines: string[],
    selAdd: Set<number>, selDel: Set<number>): string[];
```

`lineDiff` may use any small correct LCS (the fixture arrays are ~15 lines). It builds
`DiffLine`s with correct `oldNo`/`newNo`/`kind`; `fullContext` only changes how much context is
emitted, never the add/del numbering.

### 4.2 Wiring in `mock.ts`

- `MockRepoState` gains `mainRs: ThreeWay` (seeded `initialMainRs()` in `createRepoState`).
- **`INITIAL_STATUS`**: remove the static `src/main.rs` entry from `staged` (its presence is now
  model-derived). Keep every other entry.
- **`getStatus`**: start from the static snapshot, then append model-derived `src/main.rs` rows:
  push `{ path:'src/main.rs', origPath:null, status:'modified' }` to `staged` when
  `mainRs.index` ≠ `mainRs.head`, and to `unstaged` when `mainRs.workdir` ≠ `mainRs.index`.
  Re-sort both sections by path (existing `sortByPath`).
- **`getWorkdirFileDiff`**: for `src/main.rs`, return `lineDiff(head, index, ..., fullContext)`
  when `staged`, else `lineDiff(index, workdir, ..., fullContext)`. For every other path, keep
  the existing static return but honor `fullContext` by collapsing the static fixture to a single
  whole-file hunk when `true` (a tiny helper `asFullContext(fileDiff)` that concatenates hunks +
  synthesizes the intervening context is acceptable; if a static fixture has gaps it cannot fill,
  return it unchanged — the harness only exercises File View interactivity on `src/main.rs`).
- **`getCommitFileDiff` / `compareWithHeadFileDiff`**: accept + ignore `fullContext` OR apply the
  same `asFullContext` collapse; commit/compare are read-only so exactness is not required.
- **`stagePartial(repoId, path, origPath, selection)`**:
  - `requireRepo`; if `path !== 'src/main.rs'` → throw `{ kind:'other', message:'mock: partial
    staging is only modeled for src/main.rs' }` (mirrors the backend rejecting non-model files).
  - Build `selAdd`/`selDel` from `selection`; recompute `hunks = lineDiff(index, workdir).hunks`;
    `state.mainRs.index = reconstructLines('stage', hunks, index, workdir, selAdd, selDel)`.
- **`unstagePartial`**: same guard; recompute `hunks = lineDiff(head, index).hunks`;
  `state.mainRs.index = reconstructLines('unstage', hunks, head, index, selAdd, selDel)`.
- Both `await delay(150)` first (like `stage`/`unstage`), return `void`, and DO NOT emit
  `repo-changed` (the frontend refetches imperatively).

Result: staging a line moves it head→index (appears/stays in staged; leaves unstaged when the
last unstaged change is staged); unstaging moves it index→head. The file naturally shows in both
sections while both diffs are non-empty. `MAIN_RS` static fixture in `WORKDIR_DIFFS` is removed
(superseded by the model); `mockWorkdirDiff`'s `src/main.rs` branch is replaced by the model path.

---

## 5. Frontend (P17c)

### 5.1 `DiffView.tsx` — interactive props (stays presentational; App owns mutations)

```ts
export interface DiffViewProps {
  diff: FileDiff;
  /** 'diff' (hunks, today) or 'file' (one continuous full-context listing). Default 'diff'. */
  viewMode?: 'diff' | 'file';
  /** null = read-only (commit/compare/conflict, or binary/tooLarge/renamed). Otherwise the
   *  direction a granular action performs. */
  stageable?: null | 'stage' | 'unstage';
  /** Stage/unstage exactly these changed lines (context already dropped). */
  onStageLines?(selection: LineSelection[]): void;
  /** Stage/unstage every add/del line of hunk `hunkIndex` (Diff View header button). */
  onStageHunk?(hunkIndex: number): void;
}
```

- **File View** (`viewMode === 'file'`): render the single full-context hunk WITHOUT the `@@`
  header as one continuous listing; add/del lines keep their tint. When `hunks.length > 1`
  (defensive — File View should be one hunk) render them concatenated with no headers.
- **Per-line gutter control**: when `stageable != null`, each add/del line row shows a hover
  `+` (stage) / `−` (unstage) control in the marker gutter that calls
  `onStageLines([{ kind: line.kind, oldNo: line.oldNo, newNo: line.newNo }])`. Context lines get
  no control. The gutter stays `user-select: none` (mouse-range selection uses line rows, below).
- **Hunk header control** (Diff View only): a "Stage hunk" / "Unstage hunk" button on each
  `.diff-hunk-header` → `onStageHunk(hunkIndex)`.
- **Mouse-range selection → floating button**: track a selected contiguous line range via
  pointerdown/enter on `.diff-line` rows (each row carries `data-hunk` + `data-line` indices).
  While a range covering ≥1 changed line is active, render a floating "Stage N lines" /
  "Unstage N lines" button (N = count of add/del lines in range) near the selection; clicking
  builds the selection (Context dropped) and calls `onStageLines`. Escape or click-away clears
  the range. `N` counts changed lines only.
- **Read-only** (`stageable == null`): none of the above render — pure M4 behavior. Binary /
  tooLarge / "No changes" short-circuits are unchanged.

`DiffSlotViewProps` gains the same optional `viewMode` / `stageable` / `onStageLines` /
`onStageHunk` and forwards them to `<DiffView>`.

### 5.2 `DiffOverlay.tsx` — File/Diff toggle + forwarding

- Add a **segmented toggle** (`File` | `Diff`) to `.diff-overlay-header` (before the `×`),
  available for ALL kinds. New props: `viewMode: 'diff' | 'file'`, `onSetViewMode(m): void`, plus
  `stageable`, `onStageLines`, `onStageHunk` forwarded to `DiffSlotView`. The conflict/aiProposal
  branch (`ConflictSlotView`) ignores the staging props (still shows the toggle for symmetry, but
  the conflict editor is unaffected — pass `stageable` only into the `DiffSlotView` branch).
- `DiffOverlay` computes nothing about staging — RepoWorkspace passes `stageable` in (§5.3).

### 5.3 `RepoWorkspace.tsx` — state + handlers + wiring

- New state `const [diffViewMode, setDiffViewMode] = useState<'diff' | 'file'>('diff')`.
  Toggling re-fetches the OPEN slot with the new `fullContext`:
  ```
  onSetViewMode(m): setDiffViewMode(m); if diffSlot is a file slot -> re-run its fetcher with
    fullContext = (m === 'file'), reusing the same key (fetchDiffSlot preserves stale content).
  ```
- **`stageable` derivation** (passed to DiffOverlay), from `overlayMeta.kind` + the loaded diff:
  ```
  deriveStageable(meta, slot):
    if meta.kind == 'unstaged' || meta.kind == 'untracked' -> base = 'stage'
    else if meta.kind == 'staged' -> base = 'unstage'
    else return null                       # commit | compare | conflict | aiProposal
    d = slot.diff
    if d == null || d.binary || d.tooLarge || d.status == 'renamed' return null  # fall back to whole-file
    return base
  ```
- **`handleStageLines(selection)`** and **`handleStageHunk(hunkIndex)`** (guarded by `mutating`,
  like `handleStage`): resolve the open slot's `path`/`origPath` (from `overlayMeta`) and the
  direction (`stageable`); call `ipc.stagePartial`/`ipc.unstagePartial` accordingly; on success
  `await refetchStatus()` AND re-fetch the open slot (content changed; the file may now be in
  both sections — reuse the existing `fetchDiffSlot` with the slot's current key + fetcher at the
  current `diffViewMode`). `handleStageHunk` builds the selection from `slot.diff.hunks[hunkIndex]`
  (every add/del line). Errors → `reportStatusError(errorMessage(e))`. Empty selection → skip.
  - Note the request-id interplay: `refetchStatus` already re-fetches a matching mode-A slot
    (RepoWorkspace §refetchStatus). To avoid a double fetch, either (a) let `refetchStatus` do
    the slot refetch (it looks the entry up by path in the new snapshot) and only additionally
    refetch when the entry is gone from that section, or (b) bump `fileDiffReqId` once and fetch
    explicitly. Recommendation: rely on `refetchStatus`'s existing slot-refetch; no extra fetch
    needed because the staged/unstaged entry for `src/main.rs`-style files persists. Document
    whichever you pick in a comment.
- Thread `diffViewMode`, `onSetViewMode`, `deriveStageable(...)`, `handleStageLines`,
  `handleStageHunk` into `<DiffOverlay>` at the existing mount (§ RepoWorkspace render ~line 1879).
- **All existing `getWorkdirFileDiff`/`getCommitFileDiff`/`compareWithHeadFileDiff` call sites**
  pass `fullContext = diffViewMode === 'file'` for the primary overlay fetchers; secondary
  surfaces (DiffBrowser, AI targets) pass `false` unless they add their own toggle (out of scope).

### 5.4 CSS (`src/styles.css`, near the M4 diff block ~lines 1600-1705)

New rules (reuse existing tokens; dark/light both):
- `.diff-gutter-btn` — hover-revealed `+`/`−` control in the marker column (`--accent` on hover,
  `--text-3` idle; `user-select: none`; keyboard-focusable).
- `.diff-hunk-stage-btn` — small button on `.diff-hunk-header`, right-aligned.
- `.diff-line-selected` — background `color-mix(in srgb, var(--accent) 14%, transparent)` for
  rows in the active mouse range (composes over the add/del tint).
- `.diff-stage-float` — the floating action button: `position: absolute; z-index: 7` (above the
  overlay body), `--accent` fill, small shadow.
- `.diff-view-toggle` — segmented File/Diff control in `.diff-overlay-header` (two buttons,
  active = `--bg-2` + `--text-1`, inactive = `--text-3`); reuse existing segmented-control tokens
  if present, else 1px `--border` + 4px radius.

---

## 6. Testing (contract for tester — P17a is independently landable behind the CLI oracle)

**HARD RULES (memory):** all scratch/temp on **D:** via the existing `scratch_dir()`
(`crates/bonsai-core/src/testutil.rs`) with `TMP`/`TEMP=D:\Temp`; the Bash tool (Git Bash) uses
forward-slash `D:/Temp`. **Never** run `cargo test` and `cargo clippy` concurrently
(target-dir race). Pin every fixture repo: `core.autocrlf=false`, `init.defaultBranch=main`,
repo-local `user.name`/`user.email` so git2 and the CLI see identical bytes.

### 6.1 Oracle strategy (load-bearing)

For each scratch scenario: build the FileDiff, pick a selection, call `stage_partial`
(or `unstage_partial`), then prove equivalence to git's own partial apply on BOTH sides:

- **Staged side**: the equivalent minimal patch applied with `git apply --cached` must produce
  the SAME index tree. Compare `git write-tree` (ours, after `stage_partial`) to
  `git write-tree` after `git apply --cached <minimal.patch>` on a sibling clone; OR compare
  `git diff --cached` (parsed to structures, M4 §6.1 parser) between the two.
- **Unstaged remainder**: after `stage_partial`, `git diff` (workdir vs index, ours) must equal
  the remainder the oracle leaves. Symmetrically for `unstage_partial` (`git diff --cached` after
  ≡ `git restore --staged`/reverse `git apply --cached`).
- **Byte-exact blob assertions**: also assert the reconstructed index blob bytes directly
  (`repo.find_blob` on the staged entry) for CRLF / no-newline scenarios — the parsed-structure
  oracle can hide terminator differences, so keep at least the CRLF + no-newline cases byte-level.

### 6.2 Scenarios (unit in `stage_partial.rs` + integration in `tests/stage_partial_cli.rs`)

1. `one_hunk_of_many` — 40-line file edited in 3 places; stage the middle hunk only → index has
   only that hunk applied; other two remain unstaged.
2. `single_add` — stage exactly one added line.
3. `del_only` — stage exactly one deleted line (removes it from the index).
4. `mixed_add_del_each_side` — a modification (del+add pair): stage only the add; then only the
   del; verify each is independent.
5. `range_across_two_hunks` — a selection spanning changed lines in two adjacent hunks.
6. `no_newline_stage` and `no_newline_unstage` — file with no final newline; stage/unstage a
   change touching the last line → terminator state exact (byte-level).
7. `crlf` — CRLF file (`autocrlf=false`); partial stage keeps `\r\n` on every line (byte-level;
   no phantom `^M`).
8. `untracked_partial` (stage part of an untracked file → index gains a partial blob, workdir
   keeps the rest) and `untracked_full` (stage all → whole file added).
9. `deleted_partial` (stage some del lines of a workdir-deleted file → index keeps the unselected
   lines) and `deleted_full` (stage all del lines → `index.remove_path`; `git status` shows
   staged deletion).
10. `compose_on_partial` — stage half, then stage the rest → final index == whole-file stage.
11. `symmetric_unstage` — fully stage, then `unstage_partial` a subset → index reverts exactly
    those lines toward HEAD; the rest stay staged.
12. `unborn_head_unstage` — unborn HEAD, staged file, unstage some added lines → index keeps the
    unselected lines; unstage all → `index.remove_path`.
13. `noop_result_equals_index` — a selection whose reconstruction equals the current index →
    `Ok(())`, no index change, blob oid unchanged.
14. Rejections: `binary` (NUL-byte blob) → Err; `too_large` (6000-line change) → Err;
    `renamed` (`git mv` + edit, `orig_path` set) → Err; `stale` (selection coordinate not in the
    recomputed diff) → Err `"selection is stale..."`; `empty_selection` → `Ok(())` no-op;
    invalid/`../escape` path → `AppError::Other("invalid path...")` (reused validator).
15. Command-level (`commands.rs`): `stage_partial_inner`/`unstage_partial_inner` return `NoRepo`
    when nothing is open (mirrors the M3/M4 `_inner` NoRepo tests).
16. Full-context regression: `workdir_file_diff(.., full_context=true)` returns exactly one hunk
    covering the whole file; the same file with `false` returns the M4 3-context hunks; a 6000-
    line file with `true` still returns `too_large: true`.

### 6.3 Frontend smoke (harness `VITE_MOCK_IPC=1`, `pnpm dev`)

1. Open `src/main.rs` (unstaged) → toggle File ↔ Diff (whole file vs hunks); layout differs.
2. Hover a changed line → gutter `+` stages that line; the file now shows in BOTH staged &
   unstaged sections (screenshot).
3. In the staged diff, a gutter `−` unstages a line back (symmetry).
4. "Stage hunk" stages a whole hunk (Diff View).
5. Mouse-select a range spanning ≥2 changed lines → floating "Stage N lines" button → stages the
   selection; Escape clears an un-actioned range.
6. Commit-selected + compare overlays show the File/Diff toggle but NO gutter/hunk/floating
   controls.
7. Console clean; no `@tauri-apps/*` module executed in the mock harness.

---

## 7. Sub-increment split for senior-dev

- **P17a — Rust core + commands + oracle tests.** `diff.rs` (LineKind Deserialize,
  `collect_file_diff` pub(crate), `build_diff_options(paths, full_context)` + full_context on the
  three `*_file_diff` fns + call-site updates), `stage_partial.rs` (§2.1–§2.4 + guards),
  `mod.rs`, commands + registration (§2.7), tests §6.1–§6.2.
  Gate: `cargo test` green + `cargo clippy -- -D warnings` clean (run SEQUENTIALLY); scratch on
  D: only. Independently landable before any UI exists.
- **P17b — IPC + mock.** `types.ts`/`tauri.ts` (§3), `mock.ts` + `fixtures/diffs.ts` three-way
  model (§4), update ALL existing three-getter call sites to pass `fullContext` (`false` for now).
  Gate: `pnpm build` (tsc + vite) green; harness `getStatus`/`getWorkdirFileDiff` still render;
  `stagePartial`/`unstagePartial` move `src/main.rs` lines between sections (console check).
- **P17c — Interactive frontend.** `DiffView` (§5.1), `DiffOverlay` toggle + forwarding (§5.2),
  `RepoWorkspace` state/handlers/wiring (§5.3), CSS (§5.4).
  Gate: `pnpm build` green; §6.3 smoke passes in the harness (screenshots).

Each pass is one senior-dev + reviewer round + a `wip(P17x): ...` commit; tester after review.

---

## 8. Acceptance criteria

**AI gate (orchestrator):**
- `cargo test` + `cargo clippy -- -D warnings` green (sequential), all §6.2 scenarios pass with
  the `git apply --cached` oracle proving partial stage/unstage on BOTH the staged side and the
  unstaged remainder; CRLF + no-newline cases pass byte-level.
- `pnpm build` green after P17b and P17c.
- Browser-harness screenshots (§6.3): File/Diff toggle; gutter-stage a line (file in both
  sections); Stage hunk; mouse-range floating button; commit/compare toggle-only (no staging).

**USER CHECKPOINT (native `pnpm tauri dev` — NEVER self-declared; self-declaring is FORBIDDEN):**
on a scratch repo — stage a hunk, a single line, and a mouse selection; verify with a terminal
`git diff --cached` / `git diff` that exactly the chosen lines moved and the remainder stayed
unstaged; confirm symmetric `unstage_partial` reverts exactly the chosen lines.

---

## 9. Ambiguities resolved here (flag to orchestrator if disagreed)

1. **Blob reconstruction over unified-patch synthesis** — git2 0.20 exposes no line apply; the
   lossy wire content forbids patch text; splicing raw blob slices by line number is the only
   byte-exact route. (Locked by the plan.)
2. **Direction via command name**, not a `direction` arg — mirrors `stage`/`unstage`, keeps each
   command's contract single-purpose.
3. **`apply_partial` recomputes with DEFAULT 3-context**, not full context — add/del numbering is
   context-independent, so a File-View selection maps perfectly; gap-filling from the blobs
   covers the untouched regions. Cheaper than `u32::MAX` context.
4. **`full_context` is the last positional param** on `build_diff_options` and the three getters
   (matches the Rust→camelCase invoke mapping); recommendation over an options struct to keep the
   change additive and grep-able.
5. **No new `AppError` variants** — `Other` carries stale/unsupported/invalid-path messages;
   `Git` carries libgit2 errors. The frontend gates controls so users rarely hit `Other`; the
   messages are for defense + the CLI-oracle tests.
6. **Renamed files rejected** for partial staging (splicing across a rename pair is out of scope);
   the File/Diff toggle still works on them (read-through). Frontend `deriveStageable` returns
   `null` for renamed/binary/tooLarge → whole-file stage remains available.
7. **Assembly rule**: interior slice lacking a terminator gets a single `\n`; the final slice
   keeps its own terminator state. This preserves CRLF (interior CRLF slices already end in `\n`)
   and no-EOF-newline exactly, and fixes the rare relocated-EOF-line case. Recommendation over
   "always re-join with `\n`" (which would corrupt CRLF).
8. **Mock: exactly one live file** (`src/main.rs`) via a three-way line model + a shared
   `reconstructLines` mirroring §2.4 on `string[]`; every other file static and partial-reject.
   Recommendation over modeling all files (needless mock git engine) — the harness only needs one
   file to demo both directions + the both-sections state.
9. **`refetchStatus` already refetches a matching mode-A slot** — the partial handlers rely on
   that rather than issuing a second explicit slot fetch (documented in a comment); avoids a
   double round-trip. If a file leaves its section entirely the existing collapse fires.
10. **Selection carries `{kind, oldNo, newNo}`** (not raw content) — granularity-agnostic; the UI
    drops Context lines before sending; the backend ignores any stray Context element and
    validates every Add/Del coordinate against the freshly recomputed diff (stale detection).
