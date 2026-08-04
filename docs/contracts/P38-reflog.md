# P38 — Reflog Viewer + Restore — Architect Contract

Status: DESIGN. Implementer builds to this file verbatim. Mirrors P23c (blame /
file-history) for the read path and reuses the shipped P20 reset + P11 create-branch
primitives for the two restore actions.

## 1. Overview & goal

A read-mostly **reflog viewer**: read the reflog for HEAD (default) or for a selected
branch, present entries (short oid, old→new oid, reflog message, committer + time,
entry index `<ref>@{N}`), and offer two recovery actions on an entry:
- **Create branch here** — new branch at the entry's `newOid` (reuses the shipped
  `create_branch_here` command).
- **Reset current branch to this** — moves HEAD to the entry's `newOid` (reuses the
  shipped `reset_branch` command, behind the existing reset ConfirmDialog gates).

This is the safety net for force-push (P37), interactive rebase (P23), amend (P20),
and reset: it exposes prior HEAD/branch positions so a user can recover.

## 2. Invariants (enforce in review)

- **`reflog.rs` is READ-ONLY.** It contains NO mutation code. The two restore actions
  dispatch the ALREADY-SHIPPED `reset_branch` / `create_branch_here` commands. No new
  mutation primitive, no new command for mutation.
- Rust owns all git logic; `reflog.rs` is runtime-free (`&Path` / `&str`, no Tauri
  types) → CLI-testable without the tauri `test` feature, like `blame.rs`.
- git2 is blocking → the command wraps `read_reflog` in `spawn_blocking`.
- IPC is compact request/response. **No new events, no new channels.** `read_reflog`
  does NOT emit `repo-changed` (pure read). The restore actions inherit the existing
  commands' `repo-changed` emission + `refreshAll()`.
- A missing reflog (never-updated ref) → **empty `Vec`, NOT an error**.
- Cap at `MAX_REFLOG_ENTRIES` (newest-first, take the first N).
- `mock.ts` stays compiling; the harness renders the view + both actions with fixtures.
- Scratch repos only under `D:\Temp\bonsai-scratch`; TMP/TEMP=`D:\Temp` for cargo;
  run `cargo test` and `clippy` sequentially.

## 3. git2 0.21.0 reflog API (verified against Cargo.lock git2 0.21.0)

- `Repository::reflog(&self, name: &str) -> Result<Reflog, Error>` — `name` is a full
  ref name: `"HEAD"` or `"refs/heads/<branch>"`. A ref that has never been updated
  returns `Err` with `ErrorCode::NotFound` → map to empty `Vec`.
- `Reflog::len(&self) -> usize`, `Reflog::iter(&self) -> ReflogIter`,
  `Reflog::get(&self, i: usize) -> Option<ReflogEntry>`.
- Reflog storage order: **index 0 is the NEWEST** entry. `<ref>@{N}` therefore maps
  directly to `Reflog::get(N)` / iteration index `N`.
- `ReflogEntry::id_old(&self) -> Oid`, `id_new(&self) -> Oid`,
  `committer(&self) -> Signature`, `message(&self) -> Option<&str>` (may be non-UTF8 →
  use `message_bytes()` if present in 0.21; if only `message()` is exposed, fall back
  to that. Prefer bytes + `String::from_utf8_lossy` to match blame.rs style — verify
  `ReflogEntry::message_bytes` exists in 0.21; if absent, `message().unwrap_or("")`).
- `Signature::name_bytes` / `email_bytes` / `when().seconds()` as in blame.rs.

## 4. Rust module — `crates/bonsai-core/src/git/reflog.rs`

Register in `crates/bonsai-core/src/git/mod.rs`: add `pub mod reflog;` in alphabetical
position **after `rebase_interactive;` and before `remote;`** (line 18/19).

### 4.1 Constants

```rust
/// Hard cap on reflog entries returned (newest-first). A deeper log is truncated
/// rather than streamed — streaming reflog over a channel is a later item.
pub const MAX_REFLOG_ENTRIES: usize = 2000;
```

### 4.2 Wire type

```rust
/// One reflog entry (contract §4.2). Serialize camelCase.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflogEntry {
    /// The N in `<ref>@{N}` (0 == newest / current tip).
    pub index: u32,
    /// 40-hex of the ref position BEFORE this entry.
    pub old_oid: String,
    /// 40-hex of the ref position AFTER this entry (restore actions target this).
    pub new_oid: String,
    /// Committer display name (lossy UTF-8).
    pub committer_name: String,
    /// Committer email (lossy UTF-8).
    pub committer_email: String,
    /// Committer time, seconds since the Unix epoch (UTC).
    pub committer_ts: i64,
    /// Reflog message, e.g. "commit: ...", "reset: moving to ...",
    /// "rebase (finish): ...", "commit (amend): ..." (lossy UTF-8; may be empty).
    pub message: String,
}
```

Note on `id_old == 0000…` (first entry of a freshly-created ref): keep the 40-zero
hex as-is; the frontend renders it as `(root)` / dims it. Do not special-case in Rust.

### 4.3 Function signature + internals pseudocode

```rust
/// Blocking. Reads the reflog for `ref_name` ("HEAD" or a plain local branch
/// name like "main"), newest-first, capped at `MAX_REFLOG_ENTRIES`.
///
/// `ref_name`:
///   - "HEAD"          -> reads `.git/logs/HEAD`.
///   - "<branch>"      -> reads `refs/heads/<branch>` (the fn prepends the prefix
///                        UNLESS `ref_name` already starts with "refs/").
/// A ref that has never been updated (no reflog on disk) yields Ok(empty vec).
/// Errors: NoRepo (open) | Git (unexpected libgit2 failure).
pub fn read_reflog(workdir: &Path, ref_name: &str) -> Result<Vec<ReflogEntry>, AppError>
```

Pseudocode:
```
repo = open_workdir_repo(workdir)                 // reuse stage::open_workdir_repo
full = if ref_name == "HEAD" || ref_name.starts_with("refs/") {
           ref_name.to_string()
       } else {
           format!("refs/heads/{ref_name}")
       }
reflog = match repo.reflog(&full) {
    Ok(r) => r,
    Err(e) if e.code() == NotFound => return Ok(Vec::new()),   // never-updated ref
    Err(e) => return Err(e.into()),
}
out = Vec::with_capacity(min(reflog.len(), MAX_REFLOG_ENTRIES))
for (i, entry) in reflog.iter().enumerate() {
    if out.len() >= MAX_REFLOG_ENTRIES { break }
    let c = entry.committer()
    out.push(ReflogEntry {
        index: i as u32,
        old_oid: entry.id_old().to_string(),
        new_oid: entry.id_new().to_string(),
        committer_name:  lossy(c.name_bytes()),
        committer_email: lossy(c.email_bytes()),
        committer_ts:    c.when().seconds(),
        message: entry.message_bytes().map(lossy).unwrap_or_default(),
    })
}
Ok(out)
```
Notes: do NOT validate `ref_name` as a path (it is a ref, not a filesystem path) —
`repo.reflog` rejects a bogus ref itself; a non-existent-but-valid ref → NotFound →
empty vec, which is the intended "never updated" behaviour.

### 4.4 Error table

| Condition | Variant | Wire `kind` |
|---|---|---|
| repo id not open / not a repo | `NoRepo` | `noRepo` |
| ref never updated (no reflog) | — (Ok empty) | — |
| unexpected libgit2 failure | `Git(String)` | `git` |

**No new `AppError` variant.** (Restore actions inherit their commands' error sets:
`reset_branch` → operationInProgress | git | noRepo; `create_branch_here` →
invalidName | branchExists | operationInProgress | git | noRepo.)

### 4.5 `#[cfg(test)]` unit tests (in-module, mirror blame.rs style)

1. `reflog_entry_wire_shape_is_camel_case` — `serde_json::to_value` of a fixed
   `ReflogEntry` equals the exact camelCase JSON (`index`, `oldOid`, `newOid`,
   `committerName`, `committerEmail`, `committerTs`, `message`). Guards the TS wire type.
2. `read_reflog_head_after_commits` — scratch repo (`testutil::scratch_dir`, set
   user.name/email), stage+commit twice; `read_reflog(dir, "HEAD")` returns ≥2 entries,
   `entries[0].index == 0`, `entries[0].new_oid == HEAD oid`, newest-first
   (`entries[0].committer_ts >= entries[1].committer_ts`), and the newest message
   contains `"commit"`.
3. `read_reflog_missing_ref_is_empty` — init repo, `read_reflog(dir, "nonexistent")`
   returns `Ok(vec![])` (NOT an error).
4. `read_reflog_no_repo_errors` — call on an empty temp dir → `NoRepo`.
5. `read_reflog_branch_prefixing` — commit on `main`, `read_reflog(dir, "main")` returns
   the same tip as HEAD's newest `new_oid` (verifies the `refs/heads/` prefix path).

## 5. Commands + registration

### 5.1 `src-tauri/src/commands.rs`

Add `use bonsai_core::git::reflog::{self, ReflogEntry};` alongside the blame import
(line 14 region). Command + runtime-free inner, template-identical to `file_history`
(§4 pattern, `repo_path` helper, `spawn_blocking`, no `repo-changed`):

```rust
/// Reflog for `ref_name` ("HEAD" or a local branch name), newest-first, capped
/// at MAX_REFLOG_ENTRIES. A never-updated ref yields `[]` (not an error).
/// Read-only. Errors: `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn read_reflog(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    ref_name: String,
) -> Result<Vec<ReflogEntry>, AppError> {
    read_reflog_inner(state.inner(), &repo_id, ref_name).await
}

async fn read_reflog_inner(
    state: &AppState,
    repo_id: &str,
    ref_name: String,
) -> Result<Vec<ReflogEntry>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || reflog::read_reflog(&workdir, &ref_name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
```

Add a command test mirroring `blame_commands_require_an_open_repo`:
`read_reflog_requires_an_open_repo` → `read_reflog_inner(&state, "nope", "HEAD")`
→ `NoRepo`.

### 5.2 `src-tauri/src/lib.rs`

Register `commands::read_reflog` in `generate_handler!` (near `commands::file_history`,
line ~101). No new event/channel registration.

## 6. IPC triple

### 6.1 `src/ipc/types.ts`

Add the wire type near `FileHistoryEntry`:
```ts
/** One reflog entry (P38 §4.2). `index` is the N in `<ref>@{N}` (0 == newest).
 *  `oldOid`/`newOid` are full 40-hex; the UI shortens. */
export interface ReflogEntry {
  index: number;
  oldOid: string;
  newOid: string;
  committerName: string;
  committerEmail: string;
  committerTs: number;
  message: string;
}
```
Add the method to the `Ipc` interface (near `fileHistory`, line ~1263):
```ts
/** Reflog for `refName` ("HEAD" or a local branch name), newest-first, capped.
 *  A never-updated ref yields `[]` (not an error). Read-only. Rejects git | noRepo. */
readReflog(repoId: string, refName: string): Promise<ReflogEntry[]>;
```

### 6.2 `src/ipc/tauri.ts`

```ts
readReflog(repoId: string, refName: string): Promise<ReflogEntry[]> {
  return invoke<ReflogEntry[]>('read_reflog', { repoId, refName });
},
```
(Import `ReflogEntry` in the type import block.)

### 6.3 `src/ipc/mock.ts`

Import `ReflogEntry`. Add a seeded HEAD reflog fixture module — put the large fixture in
`src/ipc/fixtures/reflog.ts` (single-responsibility; keep mock.ts lean), exporting
`MOCK_HEAD_REFLOG: ReflogEntry[]` and `MOCK_BRANCH_REFLOGS: Record<string, ReflogEntry[]>`.
Seed a believable HEAD reflog newest-first covering the P37/P23/P20 story, e.g.:

```
index 0  message "reset: moving to HEAD~1"          new=<c3> old=<c4>
index 1  message "commit (amend): tidy message"     new=<c4> old=<c3b>
index 2  message "rebase (finish): returning to ..."new=<c3b> old=<c3>
index 3  message "commit: add feature"              new=<c3> old=<c2>
index 4  message "pull: Fast-forward"               new=<c2> old=<c1>
index 5  message "commit (initial): base"           new=<c1> old=0000…
```
Use oids that overlap the existing MOCK graph fixture where possible so "Create branch
here" / reveal land on real nodes; committerTs decreasing with index.

Mock method (stateful-read, mirrors `fileHistory`):
```ts
async readReflog(repoId: string, refName: string): Promise<ReflogEntry[]> {
  await delay(120);
  requireRepo(repoId);
  if (refName === 'HEAD') return structuredClone(MOCK_HEAD_REFLOG);
  const branch = MOCK_BRANCH_REFLOGS[refName];
  return branch ? structuredClone(branch) : [];   // never-updated ref → []
}
```
The two restore actions reuse the existing `createBranchHere` / `resetBranch` mocks —
no new mutating mock. (Their mock side effects already update the mock graph/refs.)

## 7. Frontend

### 7.1 New overlay — `src/components/ReflogView.tsx`

A sibling read-only overlay to `FileHistoryView.tsx` (same `diff-overlay` layout,
skeleton loading, error/empty placeholders, Esc-close via a `reflogReqId` stale-guard).
Presentational only; container owns fetch + actions.

```ts
export interface ReflogViewProps {
  /** "HEAD" or a branch name — drives the header label ("HEAD" / "branch: main"). */
  refName: string;
  entries: ReflogEntry[];
  loading: boolean;
  error: string | null;
  /** True while a restore action is in flight → disable per-row action buttons. */
  busy: boolean;
  onClose(): void;
  /** Reveal (select + scroll) the entry's newOid in the graph, if present. */
  onRevealCommit(oid: string): void;
  /** Arm "Create branch here" for this entry's newOid (opens the shared PromptDialog). */
  onCreateBranch(newOid: string): void;
  /** Arm "Reset current branch to this" for newOid (opens the shared reset ConfirmDialog). */
  onReset(newOid: string): void;
}
```
Row layout (per entry, mono where oid): `<ref>@{index}` badge · short `newOid` · message ·
`committerName` · `relativeDate(committerTs, now)` · a compact action affordance
(kebab/two buttons) offering **Create branch here** and **Reset current branch to this…**.
Render the old→new transition as `short(oldOid) → short(newOid)` (dim a 40-zero oldOid as
`(root)`). Row click → `onRevealCommit(newOid)`.

Action gating in the view: disable both action buttons when `busy`. The container further
suppresses **Reset** when HEAD is detached/unborn (see §7.3) — pass already-filtered
callbacks or a `canReset` flag (recommend: container passes `onReset: undefined` when reset
is not allowed and the view hides the item when the prop is undefined).

### 7.2 Container wiring — `src/components/RepoWorkspace.tsx`

Add, mirroring the blame/file-history overlay wiring (§2253–2319 region):
- State: `const [reflog, setReflog] = useState<{ refName: string; entries: ReflogEntry[]; loading: boolean; error: string | null } | null>(null);`
  plus `reflogReqId = useRef(0)`, `reflogOpenRef` (for the Esc layer), and a `reflogBusy`
  flag (or reuse `mutating`) to disable row actions during a restore.
- `openReflog(refName: string)` — cross-invalidate the sibling overlays (bump
  blameReqId/historyReqId, `setBlame(null)`/`setHistory(null)`), set loading, then:
  ```
  const reqId = ++reflogReqId.current;
  try { const entries = await ipc.readReflog(repoId, refName);
        if (reflogReqId.current !== reqId) return;
        setReflog({ refName, entries, loading:false, error:null }); }
  catch (e) { if (reflogReqId.current !== reqId) return;
        setReflog({ refName, entries:[], loading:false, error: errorMessage(e) }); }
  ```
- `closeReflog()` — bumps `reflogReqId`, `setReflog(null)` (drops any in-flight fetch),
  like `closeBlame`/`closeHistory`. Add it to the Esc layering effect BEFORE the
  diff/commit layers (adjacent to the blame/history close branches, §2562).
- Restore action handlers — **reuse the existing handlers, no new IPC**:
  - Create branch: call the existing `setPendingCreateBranch({ oid: newOid })` (arms the
    shared PromptDialog → existing `createBranchHere` flow at §1471). On success the
    existing flow toasts + refreshes; leave the reflog overlay open (re-fetch is optional).
  - Reset: call the existing `setPendingReset({ oid: newOid, mode })` (arms the shared reset
    ConfirmDialog → existing `handleResetBranch` at §1158). **Reuse the existing confirm
    gate verbatim** — for hard reset the existing destructive ConfirmDialog wording applies.
    Recommend surfacing all three modes (soft/mixed/hard) in the row action menu exactly as
    `resetMenuItems` does, so behaviour matches the graph context menu.
  - After a successful reset the reflog list is stale (HEAD moved) — recommend the
    container calls `openReflog(reflog.refName)` again after `refreshAll()` so the new
    `reset: moving to …` entry appears. Flag: OPTIONAL polish; MVP may just leave it.

### 7.3 Entry points

- **HEAD reflog:** a toolbar / repo-menu item **"View HEAD reflog"** → `openReflog('HEAD')`.
  Always available on a born repo. (Wire into the existing repo/overflow menu; if none is
  convenient, add a small button in the graph pane header next to existing controls.)
- **Branch reflog:** add a **"View reflog"** item to `branchMenuItems` in
  `src/components/workspaceMenus.ts` (local branches only — remote-tracking refs have
  reflogs too but are out of v1 scope; recommend local-only). `onSelect: () =>
  openReflog(name)`. Add an `onViewReflog(name: string)` dep to `WorkspaceMenuDeps` +
  `createWorkspaceMenus` and pass `openReflog` from RepoWorkspace. Use `BranchIcon` (or a
  new `HistoryIcon` in `menuIcons.tsx`).
- **Reset item availability inside the view:** the container computes `canReset = head &&
  !head.unborn && !head.detached` (same predicate as `resetMenuItems`). When false, pass
  `onReset: undefined` so the view hides the Reset action (Create-branch stays available).
  A reset whose target equals current HEAD oid is a no-op — recommend the view still shows
  it; the backend/refresh handle it harmlessly (parity with graph menu which suppresses
  `targetOid === head.oid`; container MAY suppress identically).

## 8. Sub-increments

**P38a — core read + IPC + oracle test** (Rust + IPC only, no UI):
- `crates/bonsai-core/src/git/reflog.rs` (§4) + `pub mod reflog;` in `git/mod.rs`.
- `read_reflog` command + inner + registration (§5).
- IPC triple: `ReflogEntry` type + `readReflog` on `Ipc` (types.ts), tauri.ts binding,
  `src/ipc/fixtures/reflog.ts` fixture + mock.ts method (§6).
- Unit tests (§4.5) + command test (§5.1) + CLI oracle `reflog_cli.rs` (§9).
- Gate: `cargo test -p bonsai-core reflog`, command tests green, `tsc` clean, oracle green.

**P38b — UI overlay + entry points + restore actions** (frontend only):
- `src/components/ReflogView.tsx` (§7.1).
- RepoWorkspace container wiring + Esc layer + entry points (§7.2/§7.3).
- `workspaceMenus.ts` "View reflog" branch item + dep threading; toolbar "View HEAD reflog".
- Restore actions reuse `setPendingCreateBranch` / `setPendingReset` (NO new IPC).
- Gate: browser harness (`VITE_MOCK_IPC=1`) shows the seeded HEAD reflog, both actions
  fire the existing confirm/prompt dialogs; `tsc`/build clean.

## 9. CLI-oracle test plan — `crates/bonsai-core/tests/reflog_cli.rs`

Mirror the existing `*_cli.rs` oracle style (runtime-free core vs the real `git` CLI).
Build a scratch repo under `D:\Temp\bonsai-scratch` (helper) that exercises the real
reflog-writing operations, then cross-check.

Fixture ops (via `git` CLI or git2, each writes a reflog entry):
1. `commit` base, 2. `commit` A, 3. `commit` B, 4. `git reset --soft HEAD~1`,
5. `git commit --amend` (message change), 6. a rebase that moves HEAD.

Oracle:
- Run `git reflog show HEAD --format=%H %gd %gs` (or `git log -g --format=...`) →
  parse into (index, new_oid, message).
- Call `read_reflog(dir, "HEAD")`.
- Assert: same length (up to `MAX_REFLOG_ENTRIES`); for each index, `new_oid` matches the
  CLI's `%H`; `index` matches the `@{N}` from `%gd`; `message` matches `%gs` (trim). Assert
  newest-first (index 0 == current HEAD oid).
- Second case: `read_reflog(dir, "main")` new_oid[0] equals `git rev-parse main`.
- Third: `read_reflog(dir, "does-not-exist")` == `[]`.

Windows/env: set TMP/TEMP=`D:\Temp`; run this test NOT concurrently with clippy.

## 10. AI gate vs USER CHECKPOINT

**AI gate (orchestrator verifies alone):**
- `cargo test -p bonsai-core reflog` + `reflog_cli` oracle green (entries match `git reflog`).
- Command test `read_reflog_requires_an_open_repo` green; `clippy` clean.
- `tsc` + frontend build clean; `mock.ts` compiles.
- Browser harness screenshot: HEAD reflog overlay renders the seeded entries with
  `@{N}` badges, old→new oids, messages; "Create branch here" opens the PromptDialog and
  "Reset current branch to this…" opens the reset ConfirmDialog (existing dialogs).
- Console: no errors on open/close/reveal; Esc closes the overlay first among the read
  overlays.

**USER CHECKPOINT (native Tauri, human perception):**
- On a real repo that has done commit/amend/rebase/reset/force-push, "View HEAD reflog"
  lists the true history; branch "View reflog" works.
- "Create branch here" actually creates the branch at the entry's oid; "Reset current
  branch to this" (with confirm) actually moves HEAD and the working tree per mode; the
  reflog gains the new entry after refresh.
- Confirm the hard-reset ConfirmDialog wording is present and blocking for the destructive
  path.

## 11. Flagged ambiguities (non-blocking; recommended defaults chosen)

1. **Remote-tracking reflogs** — out of v1 scope; branch entry point is local-only.
   Recommend keeping it that way; revisit if requested.
2. **Reset modes inside the view** — recommend exposing soft/mixed/hard (parity with
   `resetMenuItems`). Minimum acceptable: hard-only behind the existing ConfirmDialog.
3. **Auto-refetch reflog after a restore** — recommend re-fetching so the new entry
   appears; acceptable to defer as polish.
4. **HEAD reflog toolbar placement** — exact host (repo overflow menu vs graph-pane header
   button) left to senior-dev's read of current toolbar layout; either is fine.
5. **`ReflogEntry::message_bytes` availability in git2 0.21** — if the bytes accessor is
   absent, use `message().unwrap_or("")`; verify at implementation time.
