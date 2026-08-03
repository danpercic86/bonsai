# P28 — Discard Hunk: Implementation Contract

Status: authoritative for P28. Implementer: senior-dev (suggested two fresh-context passes:
P28a Rust core + command, P28b IPC/mock + frontend). Builds on `P17-partial-staging.md`
(LineSelection, blob-reconstruction machinery in `stage_partial.rs` §2.1–§2.4),
`P20-daily-essentials.md` §4 (whole-file discard semantics, tracked-only guard, ConfirmDialog
pattern), `M4-diff.md` (FileDiff/Hunk/DiffLine wire shapes).

**Feature:** a per-hunk "Discard hunk" button in the diff view that reverts that hunk in the
WORKTREE. Offered on **unstaged working-dir diffs only**. Destructive → always behind a
ConfirmDialog. The backend is **selection-based** (per-line capable, same `LineSelection[]`
wire type as P17) so a future per-line discard needs no backend change; the UI ships
hunk-button-only in P28.

Architecture invariants (unchanged, enforced in review):
- Rust owns ALL Git logic. Frontend sends a granularity-agnostic line selection; Rust does the
  byte-exact worktree edit. React only renders + confirms.
- Commands = request/response. **No new events, no channels, no `repo-changed` emit** — the
  frontend refetches imperatively (mirrors `discard_paths` / `stage_partial`).
- git2 is blocking → `_inner` twin + `spawn_blocking`.
- Mock IPC (`src/ipc/mock.ts`, `VITE_MOCK_IPC=1`) updated in the same pass as the IPC surface.

---

## 0. Headline decisions

1. **Same blob-reconstruction engine as P17, sides substituted.** Discarding selected changes
   means: take the WORKTREE content (NEW side) and undo the selected changes toward the INDEX
   (OLD side) — selected `Add` lines dropped, selected `Del` lines restored from the index
   blob. That is EXACTLY `reconstruct(Direction::Unstage, ...)` from `stage_partial.rs` with
   `old = index stage-0 blob bytes`, `new = worktree file bytes`, hunks from a freshly
   recomputed `diff_index_to_workdir`. **The result is written to the WORKTREE with
   `fs::write`; the index is NEVER touched.**
2. **Reuse, don't duplicate**: `stage_partial.rs` items `Direction`, `reconstruct`,
   `split_keep_terminator`, `assemble`, `nth`, `index_blob_bytes`, `stale` change from private
   to `pub(crate)`. No behavioral change to stage_partial.
3. **Tracked-only guard** (mirrors `discard.rs`): an untracked file has no index blob — a full
   discard would DELETE user content. Reject with the same error shape as `discard_paths`:
   `AppError::Git("cannot discard '<path>': not a tracked file")`.
4. **Guards as in P17 §2.5**: binary / too_large / renamed → `AppError::Other`. Stale
   selection (coordinate absent from the freshly recomputed diff, or pathspec matched
   nothing) → `AppError::Other("selection is stale; refresh the diff")`. Empty selection →
   `Ok(())` before any repo work. **No new `AppError` variants.**
5. **CRLF policy**: byte-exact for `core.autocrlf=false` (slices keep their terminators, as
   P17). For `core.autocrlf=true` repos the index blob stores LF while the worktree is CRLF;
   restored `Del` lines spliced from the LF index blob into a CRLF-majority worktree file get
   their terminator normalized to CRLF (§2.4). Never rewrite untouched worktree lines.
6. **No-op**: if the reconstructed bytes equal the current worktree bytes, return `Ok(())`
   without writing (preserves mtime, avoids a watcher storm).
7. **Worktree-deleted file**: an unstaged deletion shows as all-`Del` hunks; discarding those
   `Del` lines RECREATES the file (`fs::write` of the reconstructed bytes) — consistent with
   `discard_paths` recreating unstaged deletions.

---

## 1. New / changed files

```
crates/bonsai-core/src/git/
  discard_partial.rs           # NEW: discard_partial + guards + unit tests
  stage_partial.rs             # Direction, reconstruct, split_keep_terminator, assemble,
                               #   nth, index_blob_bytes, stale -> pub(crate). No logic change.
  mod.rs                       # + pub mod discard_partial;
crates/bonsai-core/tests/
  discard_partial_cli.rs       # NEW: CLI-oracle integration tests (§5)
src-tauri/src/
  commands.rs                  # + discard_partial (+ _inner twin)
  lib.rs                       # register commands::discard_partial
src/
  ipc/types.ts                 # IpcApi.discardPartial
  ipc/tauri.ts                 # invoke wrapper
  ipc/mock.ts                  # discardPartial (src/main.rs three-way model only)
  components/DiffView.tsx      # onDiscardHunk prop + danger hunk-header button
  components/DiffOverlay.tsx   # forward onDiscardHunk (via DiffSlotView)
  components/RepoWorkspace.tsx # pendingHunkDiscard state + ConfirmDialog + handler
  styles.css                   # .diff-hunk-discard-btn
```

---

## 2. Rust — core (`crates/bonsai-core/src/git/discard_partial.rs`)

### 2.1 Public function

```rust
use std::path::Path;
use crate::error::AppError;
use crate::git::stage_partial::LineSelection;   // unchanged wire type (P17 §2.1)

/// Blocking. Discards the selected changed lines of ONE tracked working-dir
/// file: the WORKTREE moves toward the INDEX for the selected lines only.
/// Selected `Add` (new-side) lines are removed from the worktree file; selected
/// `Del` (old-side) lines are restored from the index blob. The index is never
/// modified. Destructive — the UI must confirm first.
/// Empty selection -> `Ok(())` no-op. Untracked path -> Err (tracked-only).
pub fn discard_partial(
    workdir: &Path,
    path: &str,
    orig_path: Option<&str>,
    selection: &[LineSelection],
) -> Result<(), AppError>;
```

### 2.2 Control flow (normative)

```
discard_partial(workdir, path, orig_path, selection):
  validate_rel_path(path)?                          # reuse stage.rs
  if let Some(op) = orig_path { validate_rel_path(op)? }
  if selection.is_empty() { return Ok(()) }          # no-op before any repo work

  repo = open_workdir_repo(workdir)?
  wd   = repo.workdir() (else Git("repository has no workdir"))
  index = repo.index()?

  # Tracked-only guard (mirrors discard.rs; BEFORE the diff — cheap and decisive).
  if index.get_path(path, 0).is_none() {
      return Err(Git("cannot discard '{path}': not a tracked file")) }

  # Freshly recompute the unstaged diff for this file (P17 §2.3 pattern;
  # DEFAULT 3-context — line numbering is context-independent).
  paths = pathspecs(path, orig_path)
  opts  = build_diff_options(&paths, /*full_context=*/false)
  diff  = repo.diff_index_to_workdir(None, Some(&mut opts))?   # old=index, new=worktree
  apply_find_similar(&mut diff)?
  fd = collect_file_diff(&diff)?.ok_or_else(stale)?            # matched nothing -> stale

  # Guards (same messages, "discard" wording):
  if fd.binary    -> Err(Other("partial discard is not supported for binary files"))
  if fd.too_large -> Err(Other("partial discard is not supported for a too-large diff"))
  if fd.orig_path.is_some() || fd.status == Renamed
                  -> Err(Other("partial discard is not supported for renamed files"))

  # Selected sets + stale validation: IDENTICAL to P17 §2.3 —
  # sel_add from kind==Add -> new_no; sel_del from kind==Del -> old_no;
  # each must be a subset of the coordinates present in fd.hunks, else stale().
  # Stray Context elements ignored.

  # Raw bytes of both sides (NEVER DiffLine.content). SIDE SUBSTITUTION:
  old_bytes = index_blob_bytes(&repo, &index, path)?           # index stage-0 blob
  new_bytes = fs::read(wd.join(path)) or b"" if NotFound       # worktree (b"" = deleted)

  old_lines = split_keep_terminator(&old_bytes)
  new_lines = split_keep_terminator(&new_bytes)

  # SIDE-SUBSTITUTED RECONSTRUCT (§2.3): Direction::Unstage semantics on the
  # (index, worktree) pair — base = NEW (worktree), selected changes undone
  # toward OLD (index).
  result  = reconstruct(Direction::Unstage, &fd.hunks, &old_lines, &new_lines,
                        &sel_add, &sel_del)?
  result  = normalize_terminators(result, &new_lines, &old_lines, autocrlf(&repo))  # §2.4
  content = assemble(&result)

  # No-op: nothing to write.
  if content == new_bytes { return Ok(()) }

  # Write the WORKTREE. Recreates a worktree-deleted file (new_bytes == b"").
  # The index is never touched — `git diff --cached` is invariant under this fn.
  fs::write(wd.join(path), &content)?
  Ok(())
```

Notes:
- There is NO removal branch: discarding can only move the worktree toward the index, and the
  tracked-only guard guarantees an index blob exists, so a "file should not exist" outcome is
  impossible. An all-`Add` untracked-like state cannot occur (untracked is rejected). A fully
  emptied result writes an empty file (matches the index containing an empty blob).
- `orig_path` is accepted on the wire for signature symmetry with `stage_partial` but any diff
  that actually reports a rename is rejected (guard above). The frontend passes the row's
  `origPath` verbatim.

### 2.3 Why `Direction::Unstage` is the correct reuse (normative reasoning)

P17's `reconstruct(Unstage, hunks, old, new, sel_add, sel_del)` computes: **NEW-side base**,
walking the hunks by `new_no`; selected `Add` lines are dropped, selected `Del` lines are
restored from `old_lines` by `old_no`, unselected changes are kept, inter-hunk gaps filled
from `new_lines`. With `old = index`, `new = worktree`, hunks = `diff_index_to_workdir`, that
is precisely "worktree with the selected changes reverted to the index" — the definition of a
partial discard. No fork of the algorithm; only the sides and the output sink differ.

Pseudocode of the reused path (for review reference; body lives in `stage_partial.rs`):

```
reconstruct(Unstage, hunks, old_lines=index, new_lines=worktree, sel_add, sel_del):
    result = []; cursor = 1                       # next WORKTREE line not yet emitted
    for h in hunks:
        while cursor < h.new_start: push worktree[cursor-1]; cursor += 1
        for line in h.lines:
            Context: push worktree[line.new_no-1]; cursor = line.new_no + 1
            Add:     if !sel_add.contains(line.new_no) { push worktree[line.new_no-1] }
                     cursor = line.new_no + 1      # selected Add: dropped from worktree
            Del:     if sel_del.contains(line.old_no) { push index[line.old_no-1] }
                     # restored index line; cursor unchanged (Del has no new line)
    while cursor <= worktree.len(): push worktree[cursor-1]; cursor += 1
    return result                                  # out-of-range anywhere -> stale()
```

### 2.4 Terminator normalization (`normalize_terminators`, discard-direction only)

Problem: with `core.autocrlf=true` the index blob is LF-normalized while the worktree file is
CRLF. A restored `Del` slice comes from the index blob (`...\n`); splicing it verbatim into a
CRLF worktree file would produce mixed line endings that git then reports as perpetually
modified.

```
autocrlf(repo) -> bool:            # repo.config().get_bool("core.autocrlf") == Ok(true)

normalize_terminators(result, worktree_lines, index_lines, autocrlf) -> Vec<Cow<[u8]>>:
    if !autocrlf: return result as-is              # byte-exact mode (P17 semantics)
    # Majority terminator of the CURRENT worktree file decides. Empty/deleted
    # worktree (recreate case): fall back to the index blob's own terminators
    # (write it verbatim; git will renormalize on next checkout anyway).
    crlf = count of worktree_lines ending b"\r\n"
    lf   = count of worktree_lines ending b"\n" but not b"\r\n"
    if worktree_lines.is_empty() || crlf <= lf: return result as-is
    # CRLF-majority: any slice ending in bare b"\n" (i.e. it came from the LF
    # index blob) is rewritten to end b"\r\n". Slices already CRLF or with no
    # terminator (EOF) are untouched.
    for s in result: if s ends b"\n" and not b"\r\n" -> replace terminator with b"\r\n"
```

Implementation detail: because rewriting requires owned bytes, `discard_partial` may collect
the reconstruct output into `Vec<Cow<'a, [u8]>>` (or copy to `Vec<Vec<u8>>`) before `assemble`;
add an `assemble` overload or a tiny local `assemble_cow` — do NOT change `assemble`'s
existing signature used by stage_partial.

### 2.5 `stage_partial.rs` visibility changes (exhaustive; nothing else changes)

```rust
pub(crate) enum Direction { Stage, Unstage }        // was private
pub(crate) fn stale() -> AppError;
pub(crate) fn index_blob_bytes(&Repository, &Index, &Path) -> Result<Vec<u8>, AppError>;
pub(crate) fn split_keep_terminator(&[u8]) -> Vec<&[u8]>;
pub(crate) fn nth<'a>(&[&'a [u8]], Option<u32>) -> Result<&'a [u8], AppError>;
pub(crate) fn reconstruct<'a>(Direction, &[Hunk], &[&'a [u8]], &[&'a [u8]],
                              &HashSet<u32>, &HashSet<u32>) -> Result<Vec<&'a [u8]>, AppError>;
pub(crate) fn assemble(&[&[u8]]) -> Vec<u8>;
```

---

## 3. Command layer (`src-tauri/src/commands.rs`, `lib.rs`)

Same `_inner` + `spawn_blocking` pattern as `stage_partial`:

```rust
#[tauri::command]
pub async fn discard_partial(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    orig_path: Option<String>,
    selection: Vec<LineSelection>,
) -> Result<(), AppError>;
// _inner: repo_path(state, repo_id)? -> spawn_blocking(move ||
//   bonsai_core::git::discard_partial::discard_partial(
//       &workdir, &path, orig_path.as_deref(), &selection))
//   join error -> AppError::Other. NoRepo when repo_id unknown (existing helper).
```

Register `commands::discard_partial` in `generate_handler!` (`lib.rs`). No new events, no new
`AppError` variants, no capability changes. Command surface after P28 gains exactly one command.

---

## 4. IPC — TypeScript

`src/ipc/types.ts` (`LineSelection` already exists from P17):

```ts
/** Discard the selected changed lines of one tracked working-dir file: the
 *  WORKTREE moves toward the INDEX; the index is never modified. DESTRUCTIVE —
 *  callers must confirm first. Empty selection is a no-op. Rejects AppError
 *  ('noRepo' | 'git'[untracked] | 'other'[stale/unsupported/invalid path]). */
discardPartial(repoId: string, path: string, origPath: string | null,
               selection: LineSelection[]): Promise<void>;
```

`src/ipc/tauri.ts`:

```ts
discardPartial: (repoId, path, origPath, selection) =>
  invoke<void>('discard_partial', { repoId, path, origPath, selection }),
```

### 4.1 Mock (`src/ipc/mock.ts` — mirror `stagePartial`, ~line 1827)

- `requireRepo`; `await delay(150)`; no `repo-changed` emit.
- Only the live three-way model file is supported: if `path !== 'src/main.rs'` → throw
  `{ kind: 'other', message: 'mock: partial discard is only modeled for src/main.rs' }`.
- Build `selAdd`/`selDel`; recompute `hunks = lineDiff(state.mainRs.index,
  state.mainRs.workdir, ...).hunks`; then
  `state.mainRs.workdir = reconstructLines('unstage', hunks, index, workdir, selAdd, selDel)`
  — same side substitution as the backend (old=index, new=workdir), reusing the existing
  `reconstructLines` from `fixtures/diffs.ts` unchanged.
- No `getStatus` change needed: it already derives the unstaged row from
  `workdir !== index`, so a full-hunk discard that equalizes them clears the row naturally.

---

## 5. Frontend

### 5.1 `DiffView.tsx`

```ts
export interface DiffViewProps {
  // ...existing P17 props...
  /** Discard every add/del line of hunk `hunkIndex` in the WORKTREE. Rendered
   *  only when provided AND stageable === 'stage' (unstaged diffs). */
  onDiscardHunk?(hunkIndex: number): void;
}
```

- Render a second, **danger-styled** button on each `.diff-hunk-header`, after the existing
  "Stage hunk" button: label "Discard hunk", class `.diff-hunk-discard-btn`, onClick
  `onDiscardHunk(hunkIndex)`. Shown only when `onDiscardHunk` is set AND `stageable ===
  'stage'` (this already excludes staged/commit/compare/conflict, binary, tooLarge, renamed via
  `deriveStageable`). File View: same rule as the existing hunk button (hidden with headers).
- `DiffSlotViewProps` gains optional `onDiscardHunk` and forwards to `<DiffView>`;
  `DiffOverlay` forwards it into the `DiffSlotView` branch only.

### 5.2 `RepoWorkspace.tsx`

- State: `const [pendingHunkDiscard, setPendingHunkDiscard] = useState<{ path: string;
  origPath: string | null; hunkIndex: number } | null>(null)` — copies the existing
  `pendingDiscard` pattern (~line 1073) and its ConfirmDialog mount (~line 3236).
- `onDiscardHunk(hunkIndex)` is passed to `<DiffOverlay>` ONLY when
  `overlayMeta.kind === 'unstaged'` and `deriveStageable(...) === 'stage'` (i.e. never for
  untracked — tracked-only; never for staged/commit/compare/binary/tooLarge/renamed).
  The handler just sets `pendingHunkDiscard` from `overlayMeta` + `hunkIndex`.
- ConfirmDialog (danger variant, same component as `pendingDiscard`): title "Discard hunk?",
  body naming the file and warning the change cannot be undone; Confirm →
  `handleConfirmHunkDiscard()`:
  1. Guarded by `mutating` (like `handleDiscard`).
  2. Build the selection from `slot.diff.hunks[hunkIndex]` exactly like `handleStageHunk`
     (~line 1140): every `add`/`del` line → `{ kind, oldNo, newNo }`; empty → skip.
  3. `await ipc.discardPartial(repoId, path, origPath, selection)`.
  4. `await refetchStatus()` (which re-fetches the open slot per the P17 §9.9 mechanism);
     clear `pendingHunkDiscard`. Errors → `reportStatusError(errorMessage(e))`.

### 5.3 CSS (`styles.css`)

`.diff-hunk-discard-btn` — sibling of `.diff-hunk-stage-btn` (same size/placement, right-
aligned group), danger colors: idle `--text-3`, hover `--danger` (or the token used by the
existing discard button); dark/light both.

---

## 6. Testing

**HARD RULES (memory):** scratch/temp on **D:** via `scratch_dir()`
(`crates/bonsai-core/src/testutil.rs`), `TMP`/`TEMP=D:\Temp`; never run `cargo test` and
`cargo clippy` concurrently. Fixture repos pin `core.autocrlf` explicitly per scenario,
`init.defaultBranch=main`, repo-local `user.name`/`user.email`.

### 6.1 Unit tests (in `discard_partial.rs`)

1. `empty_selection_noop` — `Ok(())` on a nonexistent repo path (no repo work).
2. `invalid_path_rejected` — `""`, `../escape`, `/abs`, `a\\b` and bad `orig_path`.
3. `untracked_rejected` — untracked file → `AppError::Git("...not a tracked file")`;
   worktree file byte-identical after.
4. `binary_rejected`, `too_large_rejected` (6000-line change), `renamed_rejected`
   (`git mv` + edit) → `AppError::Other`.
5. `stale_selection` — selection coordinate absent from the fresh diff → `Other("selection is
   stale; refresh the diff")`; also pathspec-matches-nothing (clean file).
6. `one_of_three_hunks` — file edited in 3 places; discard the middle hunk → worktree has
   hunks 1 & 3 still applied, hunk 2 reverted to index content; index blob oid unchanged.
7. `deleted_file_recreated` — delete the worktree file; discard all its `Del` lines → file
   recreated with the index bytes.
8. `noop_result` — selection whose reconstruction equals current worktree bytes → `Ok(())`,
   file mtime/bytes untouched.
9. `index_never_touched` — assert the index blob oid (and `index.write` not needed: compare
   on-disk `.git/index` mtime or `git2` entry id) identical before/after every mutating test.

### 6.2 CLI-oracle integration tests (`crates/bonsai-core/tests/discard_partial_cli.rs`)

For each scenario: build the fixture, snapshot `git diff --cached` output, call
`discard_partial`, then:

1. **Index invariant** — `git diff --cached` output is byte-identical before/after (the
   command never touches the index).
2. **Remainder equivalence** — on a sibling clone of the same fixture, apply the equivalent
   hunk with `git apply --reverse <hunk.patch>` (patch text generated from `git diff` limited
   to the discarded hunk); the resulting worktree file bytes must equal ours.
3. **CRLF byte cases** — `autocrlf=false` CRLF file: every `\r\n` preserved byte-exact after a
   partial discard; `autocrlf=true` CRLF worktree + LF index blob: restored `Del` lines carry
   `\r\n` (no mixed endings; `git status` reports only the surviving hunks). No-newline-at-EOF
   discard round-trips the terminator state exactly (byte-level assertions on the file).
4. **All-hunks cross-check** — discarding every hunk of a modified file leaves the worktree
   byte-identical to a sibling clone after `git checkout -- <path>`; `git status --porcelain`
   shows the row gone.

### 6.3 Frontend smoke (harness `VITE_MOCK_IPC=1`, `pnpm dev`)

1. Open `src/main.rs` unstaged diff → each hunk header shows "Stage hunk" + a danger
   "Discard hunk" button (screenshot).
2. Click "Discard hunk" → ConfirmDialog appears; Cancel changes nothing.
3. Confirm → the hunk disappears from the unstaged diff (mock workdir mutated); discarding the
   last hunk removes the file from the unstaged section entirely.
4. Staged diff, commit/compare overlays, binary/tooLarge/renamed rows show NO discard button.
5. Console clean; no `@tauri-apps/*` module executed.

---

## 7. Acceptance criteria

**AI gate (orchestrator):**
- `cargo test` + `cargo clippy -- -D warnings` green (sequential); all §6.1/§6.2 scenarios
  pass, including the index-invariant check on every mutating test and byte-level CRLF /
  no-newline assertions.
- `pnpm build` green after the IPC + frontend passes.
- Browser-harness evidence for §6.3 (screenshots: button, ConfirmDialog, hunk removed).

**USER CHECKPOINT (native `pnpm tauri dev` — never self-declared):** on a scratch repo with a
3-hunk modification — discard the middle hunk via the button + dialog; verify in a terminal
that `git diff` shows only hunks 1 & 3 remaining and `git diff --cached` is unchanged.

---

## 8. Ambiguities resolved here (flag to orchestrator if disagreed)

1. **Reuse `Direction::Unstage` with substituted sides** instead of a new algorithm or a new
   `Direction::Discard` variant — the math is identical (§2.3); only inputs/output sink differ.
   Recommendation: no new variant; the doc comment in `discard_partial.rs` states the mapping.
2. **Tracked-only** (locked, mirrors `discard.rs`): untracked files rejected — a discard would
   delete user content the index cannot restore.
3. **CRLF normalization only when `autocrlf=true` AND the worktree file is CRLF-majority**
   (§2.4); pure byte-splicing otherwise. Recommendation over always-normalizing (would corrupt
   intentional mixed-ending files in `autocrlf=false` repos).
4. **Backend selection-based, UI hunk-only** — per-line discard is a UI-only follow-up.
5. **No `repo-changed` emit; imperative refetch** — consistent with every other mutation. The
   watcher will also fire on the `fs::write`, which the existing debounce absorbs.
6. **Mock models only `src/main.rs`** (P17 §9.8 policy); all other paths throw
   `{ kind: 'other' }`.
