# P77 — Tag Sync Management (local ↔ remote tag reconciliation)

Status: LOCKED (architect contract). UI/visuals owned by `docs/contracts/P77-ui.md`.

## LOCKED DECISIONS (orchestrator + user, 2026-08-20) — these override any conflicting draft text below
- **Surface:** inline sidebar Tags list + context menu only. No new panel.
- **Actions:** full set — force-refresh stale local tag; push unpushed local tag; delete
  local-only tag; delete remote tag (net-new); force-move remote tag (reuse `push_tag` force).
  Destructive remote ops behind explicit confirm dialogs.
- **Remote truth:** live `ls-remote` per view. Compare against `origin`/first remote only for v1
  (name the remote in every label). 10s in-memory cache + manual/focus refresh.
- **Shipping status set:** `in-sync` · `local-only` · `stale` · `remote-only`.
  - **`deleted-on-remote` is FOLDED into `local-only` for v1** (D1) — a single ls-remote cannot
    distinguish "pushed then deleted upstream" from "never pushed"; git stores no per-tag upstream.
    Keep the `deleted-on-remote` enum variant **reserved** (never emitted in v1) so a future
    pushed-tags-set upgrade is additive. Do NOT build `.git/bonsai/pushed-tags.json` in v1.
  - **`remote-only` IS in scope** (Q3=yes): the report must supply upstream tags absent locally
    (name + remote peeled oid, local_oid=null) cheaply from the same ls-remote pass; the sidebar
    renders them as ghost rows with a "fetch this tag" action.
- **Annotated tags:** compare the PEELED committish on both sides (never the annotated-tag-object
  oid). `annotated` is a display flag only.

## 0. Problem & scope

A pushed tag can be *moved* on the remote (force-updated) while a machine that
fetched earlier keeps the OLD target — git never force-updates an existing local
tag on a normal fetch (`AutotagOption::Auto`, no `+`). Bonsai currently lists tags
as bare names (`BranchesSnapshot.tags: Vec<String>`) and cannot show or fix drift.

P77 adds a **live remote-truth tag reconciliation** feature: classify every tag
against a chosen remote using a one-shot `ls-remote`, surface a status badge per
tag in the existing sidebar Tags list, and provide the resolve actions (refresh /
push / delete-local / delete-remote / force-move-remote).

Invariants held: Rust owns ALL git logic + classification; React only renders
precomputed `TagSyncEntry` rows. IPC is request/response (commands). No events,
no channels — the result set is small (one row per tag). Heavy git2 calls run in
`spawn_blocking`. All new IPC is mockable in `src/ipc/mock.ts`.

## 1. Module boundaries & files

### Rust (bonsai-core)
- **NEW** `crates/bonsai-core/src/git/tag_sync.rs` — all P77 core logic: DTOs
  (`TagSyncEntry`, `TagSyncStatus`, `TagSyncReport`), `ls_remote_tags`,
  `list_tag_sync` (join + classify), `force_refresh_tag`, `delete_remote_tag`,
  default-remote resolver. Peer module to `git/tags.rs` (do NOT touch tags.rs).
  - Register `pub mod tag_sync;` in `crates/bonsai-core/src/git/mod.rs` (next to
    `pub mod tags;` at L64).
  - **File-split note (flag):** `tags.rs` is 231 lines and stays under the ~500
    soft limit; a peer `tag_sync.rs` avoids splitting it. If `tag_sync.rs` itself
    approaches ~500 lines during SI-2, split the network helpers (`ls_remote_tags`,
    force-refresh, delete-remote) into `git/tag_sync/net.rs` and keep classification
    in `mod.rs`. Do not pre-emptively convert `tags.rs` into a directory.
  - **Reuse (do not reimplement):** `remote::{acquire_cred, CredAttempts,
    map_remote_err}` (already `pub(crate)`), and the auth-eviction pattern
    `evict_fresh_on_auth_fail` used by `fetch_remote` (`git/remote.rs` L140).
    `open_repo_at` pattern is duplicated per-module (copy the `NO_SEARCH` helper).

### Rust (src-tauri, command layer)
- `src-tauri/src/commands/tags.rs` — append the three P77 commands + `_inner`
  cores (same `spawn_blocking` shape as the existing `push_tag`/`delete_tag`).
- Register the three commands in the `invoke_handler!` list (wherever
  `create_tag`/`delete_tag`/`push_tag` are registered).

### TypeScript / React
- `src/ipc/types.ts` — add `TagSyncStatus`, `TagSyncEntry`, `TagSyncReport`, and
  three `IpcApi` method signatures (near the P22 tags block ~L2401-2416).
- `src/ipc/tauri.ts` — wire the three real `invoke` calls (near L770-785).
- **NEW** `src/ipc/mock/handlers/tagSync.ts` — mock handlers; register in
  `src/ipc/mock.ts` (spread into `mockIpc`).
- **NEW** `src/ipc/fixtures/tagSync.ts` — canned `TagSyncReport` exercising all
  five statuses (incl. one annotated stale/moved tag).
- `src/components/Sidebar.tsx` (TagRow ~L214) — render the status badge from the
  report; trigger `listTagSync` when the Tags section expands.
- `src/components/repoWorkspace/useTagRemoteActions.ts` — add
  `handleForceRefreshTag`, `handleDeleteRemoteTag`, force-move (pushTag force),
  and reuse existing `handleDeleteTag`/`handlePushTag`.
- `src/components/workspaceMenus.ts` (`tagMenuItems` ~L503) — status-conditional
  menu entries.
- Destructive-confirm dialogs reuse `src/components/dialogs/BranchTagDialogs.tsx`.

## 2. DTO shapes

### Rust (`git/tag_sync.rs`)
```rust
/// One tag's reconciliation state against a chosen remote (P77). Compact,
/// fully precomputed — the frontend renders it verbatim, no per-tag round-trips.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagSyncEntry {
    /// Short tag name (no `refs/tags/` prefix).
    pub name: String,
    pub status: TagSyncStatus,
    /// Peeled committish the LOCAL tag resolves to (40-hex). None => remote-only.
    pub local_oid: Option<String>,
    /// Peeled committish the REMOTE tag resolves to (40-hex). None => local-only.
    pub remote_oid: Option<String>,
    /// True if the tag is an annotated tag object on EITHER side.
    pub annotated: bool,
}

/// serde => kebab-case: "in-sync" | "local-only" | "stale" | "remote-only"
/// | "deleted-on-remote".
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TagSyncStatus {
    /// Both sides present, peeled committish equal.
    InSync,
    /// Present locally, absent on remote. In v1 this SUBSUMES deleted-on-remote
    /// (see §5 decision D1); `DeletedOnRemote` is not emitted unless a pushed-set
    /// is available.
    LocalOnly,
    /// Both sides present, peeled committish differ (the moved-tag case).
    Stale,
    /// Present on remote, absent locally.
    RemoteOnly,
    /// Reserved: present locally, previously pushed to this remote, now gone
    /// upstream. NOT produced in v1 (folded into LocalOnly) — kept in the enum
    /// so the UI can label it if D1 flips to the pushed-set option.
    DeletedOnRemote,
}

/// Result of one live ls-remote reconciliation pass.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagSyncReport {
    /// The remote actually queried (resolved default when caller passed None).
    pub remote: String,
    /// One row per tag in the local∪remote union, sorted case-insensitively.
    pub entries: Vec<TagSyncEntry>,
}
```

### TypeScript (`src/ipc/types.ts`)
```ts
export type TagSyncStatus =
  | 'in-sync'
  | 'local-only'
  | 'stale'
  | 'remote-only'
  | 'deleted-on-remote';

export interface TagSyncEntry {
  name: string;
  status: TagSyncStatus;
  localOid: string | null;
  remoteOid: string | null;
  annotated: boolean;
}

export interface TagSyncReport {
  remote: string;
  entries: TagSyncEntry[];
}
```

## 3. IPC surface

### 3.1 Commands (request/response only — no events, no channels)

| Command | Kind | Network | Destructive |
|---|---|---|---|
| `list_tag_sync` | read | YES (ls-remote) | no |
| `force_refresh_tag` | mutate local ref | YES (1-tag fetch) | overwrites 1 local ref |
| `delete_remote_tag` | mutate remote | YES (push delete) | YES — confirm |
| *(reuse)* `push_tag` `force=true` | mutate remote | YES | YES — confirm (force-move) |
| *(reuse)* `push_tag` `force=false` | mutate remote | YES | no (push unpushed) |
| *(reuse)* `delete_tag` | mutate local | no | local-only cleanup |

### 3.2 Rust command signatures (`src-tauri/src/commands/tags.rs`)
```rust
/// Live tag reconciliation against `remote` (None => default: "origin" else the
/// first configured remote). One ls-remote round-trip. Rejects
/// noRepo | noRemote | authFailed | networkError | git.
#[tauri::command]
pub async fn list_tag_sync(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    remote: Option<String>,
) -> Result<TagSyncReport, AppError>;

pub(crate) async fn list_tag_sync_inner(
    state: &AppState, repo_id: &str, remote: Option<String>,
) -> Result<TagSyncReport, AppError>;

/// Force-update ONE local tag from `remote` (refspec `+refs/tags/<n>:refs/tags/<n>`,
/// AutotagOption::None). Rejects noRepo | invalidName | noRemote | authFailed |
/// networkError | git. Does NOT emit repo-changed (frontend refetches).
#[tauri::command]
pub async fn force_refresh_tag(
    state: tauri::State<'_, AppState>,
    repo_id: String, remote: String, tag_name: String,
) -> Result<(), AppError>;

pub(crate) async fn force_refresh_tag_inner(
    state: &AppState, repo_id: &str, remote: String, tag_name: String,
) -> Result<(), AppError>;

/// Delete a tag ON the remote (refspec `:refs/tags/<n>`). NET-NEW. Destructive —
/// caller MUST confirm in UI first. Rejects noRepo | invalidName | noRemote |
/// authFailed | networkError | pushRejected | git.
#[tauri::command]
pub async fn delete_remote_tag(
    state: tauri::State<'_, AppState>,
    repo_id: String, remote: String, tag_name: String,
) -> Result<(), AppError>;

pub(crate) async fn delete_remote_tag_inner(
    state: &AppState, repo_id: &str, remote: String, tag_name: String,
) -> Result<(), AppError>;
```
Each `_inner` mirrors the existing `push_tag_inner`: `repo_path(state, repo_id)?`
→ `spawn_blocking(move || tag_sync::<fn>(&path, ...))` → map join error to
`AppError::Other`.

### 3.3 Core signatures (`crates/bonsai-core/src/git/tag_sync.rs`)
```rust
pub fn list_tag_sync(workdir: &Path, remote: Option<&str>)
    -> Result<TagSyncReport, AppError>;
pub fn force_refresh_tag(workdir: &Path, remote: &str, tag_name: &str)
    -> Result<(), AppError>;
pub fn delete_remote_tag(workdir: &Path, remote: &str, tag_name: &str)
    -> Result<(), AppError>;

/// Live ls-remote of only refs/tags/*. Returns (full_ref_name, oid) pairs,
/// including peeled "refs/tags/X^{}" entries. Auth/network mapped via
/// map_remote_err; fresh creds evicted on auth failure.
fn ls_remote_tags(repo: &git2::Repository, remote: &str)
    -> Result<Vec<(String, git2::Oid)>, AppError>;

/// "origin" if configured, else the first entry of repo.remotes(); NoRemote if
/// none configured.
fn resolve_default_remote(repo: &git2::Repository, remote: Option<&str>)
    -> Result<String, AppError>;
```

### 3.4 TypeScript `IpcApi` (`src/ipc/types.ts`)
```ts
/** Live tag reconciliation vs `remote` (null => default remote). One ls-remote
 *  round-trip; best-effort — callers must render the plain tags list even when
 *  this rejects. Rejects noRepo | noRemote | authFailed | networkError | git. */
listTagSync(repoId: string, remote: string | null): Promise<TagSyncReport>;
/** Force-update one local tag from `remote`. Rejects noRepo | invalidName |
 *  noRemote | authFailed | networkError | git. */
forceRefreshTag(repoId: string, remote: string, tagName: string): Promise<void>;
/** Delete a tag on `remote` (destructive — confirm first). Rejects noRepo |
 *  invalidName | noRemote | authFailed | networkError | pushRejected | git. */
deleteRemoteTag(repoId: string, remote: string, tagName: string): Promise<void>;
```
Force-move-remote and push-unpushed both reuse the existing
`pushTag(repoId, remote, tagName, force)` (force=true / false respectively).
Delete-local reuses existing `deleteTag(repoId, name)`.

### 3.5 Frontend call discipline
- `listTagSync` is a SEPARATE, best-effort call. The sidebar Tags list renders
  from `BranchesSnapshot.tags` (unchanged) and is NEVER blocked on it. The report
  only *augments* rows with a badge. A rejected `listTagSync` => show tags with no
  badge + a small "sync unavailable" affordance (visuals: P77-ui.md).
- Trigger: on Tags-section expand (one round-trip per open) and on the manual
  refresh button / window-focus rescan (architecture invariant — the watcher path).
- After `forceRefreshTag`/`deleteRemoteTag`/`pushTag`/`deleteTag` succeed, the
  frontend re-runs `getBranches` + `listTagSync`. No `repo-changed` emission
  (consistent with the existing tag commands).
- Remote selection when multiple remotes exist: the UI passes the chosen remote;
  default resolution lives in Rust (`resolve_default_remote`). `TagSyncReport.remote`
  echoes which remote was queried so the UI can label the badge context.

## 4. Algorithm — join & classification (pseudocode)

Crux: an annotated tag's `refs/tags/X` points at a **tag object**, while ls-remote
also advertises the **peeled committish** as `refs/tags/X^{}`. Comparing a local
peeled commit against a remote tag-object oid would FALSELY report `Stale`.
**Rule: compare the peeled committish on both sides.**

```
fn list_tag_sync(workdir, remote_opt):
    repo = open(workdir, NO_SEARCH)
    remote_name = resolve_default_remote(repo, remote_opt)?   # NoRemote if none

    # --- LOCAL side: name -> (peeled_oid, annotated) ---
    local = {}
    for ref in repo.references_glob("refs/tags/*"):
        name = strip_prefix(ref.name(), "refs/tags/")
        # peel(Any) follows an annotated tag object down to its target
        # (commit, or tree/blob for exotic tags) and stops there.
        peeled = ref.peel(ObjectType::Any)?.id()
        is_annotated = ref.peel(ObjectType::Tag).is_ok()   # target is a tag object
        local[name] = (peeled, is_annotated)

    # --- REMOTE side: live ls-remote (may error -> propagate; UI degrades) ---
    remote = {}                # name -> peeled committish oid
    remote_annotated = set()
    for (full, oid) in ls_remote_tags(repo, remote_name)?:
        if full ends_with "^{}":
            base = full[len("refs/tags/") .. -3]   # strip prefix and "^{}"
            remote[base] = oid                      # ^{} is authoritative -> OVERWRITE
            remote_annotated.add(base)
        else:
            name = strip_prefix(full, "refs/tags/")
            remote.entry(name).or_insert(oid)       # lightweight commit oid, OR the
                                                    # tag-object oid later overwritten by ^{}
    # ls-remote emits "X" before "X^{}", so or_insert-then-overwrite yields the
    # committish for annotated tags and the commit oid for lightweight tags.

    # --- JOIN over the union, sorted case-insensitively ---
    entries = []
    for name in sort_ci(union(local.keys, remote.keys)):
        l = local.get(name)          # Option<(oid, annotated)>
        r = remote.get(name)         # Option<oid>
        annotated = l?.annotated OR (name in remote_annotated)
        status = match (l, r):
            (Some((lo,_)), Some(ro)) => if lo == ro { InSync } else { Stale }
            (Some(_),      None)     => LocalOnly     # D1: deleted-on-remote folded in
            (None,         Some(_))  => RemoteOnly
            (None,         None)     => unreachable
        entries.push(TagSyncEntry {
            name, status,
            local_oid:  l.map(|(o,_)| hex(o)),
            remote_oid: r.map(hex),
            annotated,
        })
    return TagSyncReport { remote: remote_name, entries }
```

Notes:
- Same-name lightweight-local vs annotated-remote (or vice-versa) that peel to the
  same commit ⇒ `InSync`, `annotated=true`. This is correct (same committish); the
  annotated flag reflects "annotated somewhere". Documented, not a bug.
- Tag peeling to a non-commit (tree/blob): compare the peeled object oids anyway —
  classification still works; no special-casing.

## 5. Error / edge cases

- **No remote configured:** `resolve_default_remote` => `NoRemote`. Frontend shows
  tags with no badges + "no remote to compare against".
- **Offline / auth failure:** `ls_remote_tags` maps via `map_remote_err`
  (`NetworkError` / `AuthFailed`) and evicts fresh creds on auth fail. Command
  rejects; frontend degrades (plain tags list, no badges). MUST NOT crash the
  sidebar.
- **Lightweight vs annotated:** handled by the peel rule (§4). Never emit a false
  `Stale` for annotated tags — this is the primary regression test.
- **Tag peels to a non-commit:** compare peeled oids as-is (§4).
- **Multiple remotes:** UI passes the chosen remote; `resolve_default_remote`
  falls back to "origin" then first; `TagSyncReport.remote` echoes the choice.
- **`force_refresh_tag` on a tag absent upstream:** the `+refs/tags/<n>:...` fetch
  updates nothing (or errors from server) → surfaced as `git`/`networkError`; the
  UI should generally offer refresh only for `stale` rows.
- **`delete_remote_tag` rejected by server** (protected ref, etc.): captured via
  `push_update_reference` status → `PushRejected` (same pattern as `push_tag`).
- **Name validation:** `force_refresh_tag`/`delete_remote_tag` run the same
  `validate_tag_name` as create/delete (leading `-`, bad ref chars) → `InvalidName`.

### Decisions to confirm with the user
- **D1 (deleted-on-remote vs local-only):** a never-pushed local tag and a
  pushed-then-deleted-upstream tag are indistinguishable from a single ls-remote —
  git tracks no per-tag upstream. **Recommendation: fold both into `LocalOnly` for
  v1** (honest, zero extra state). Alternative (deferred follow-up): persist a
  per-remote "tags Bonsai has pushed" set in repo-local state
  (`.git/bonsai/pushed-tags.json`), updated on every successful `push_tag`, then
  emit `DeletedOnRemote` = in-set ∧ absent-upstream. Only ever covers tags Bonsai
  itself pushed. `TagSyncStatus::DeletedOnRemote` is kept in the enum so flipping
  D1 later is additive. **CONFIRM: ship v1 folded (recommended) or build the
  pushed-set now?**
- **D2 (auto-fetch on expand):** `listTagSync` fires a network round-trip every
  time the Tags section is expanded. Recommendation: fire on expand + manual
  refresh + focus rescan, with a short in-memory cache (e.g. skip re-query within
  ~10s) to avoid hammering on rapid toggles. **CONFIRM the cache window** (UI
  timing detail also relevant to P77-ui.md).

## 6. Single-tag force-refresh & delete-remote mechanics

**Force-refresh (git2, one-off fetch):**
```rust
let mut remote = repo.find_remote(remote)?;          // NotFound -> NoRemote
let attempts = RefCell::new(CredAttempts::default());
let mut cbs = git2::RemoteCallbacks::new();
cbs.credentials(|url, u, allowed| acquire_cred(repo.workdir(), &attempts, url, u, allowed));
let mut opts = git2::FetchOptions::new();
opts.remote_callbacks(cbs);
opts.download_tags(git2::AutotagOption::None);        // fetch ONLY the named tag
let refspec = format!("+refs/tags/{tag}:refs/tags/{tag}");   // leading '+' = force
remote.fetch(&[refspec.as_str()], Some(&mut opts), None)
    .map_err(|e| evict_fresh_on_auth_fail(&repo, &attempts, map_remote_err(e, remote_name)))?;
```

**Delete-remote (git2 push, empty source):**
```rust
let mut remote = repo.find_remote(remote)?;
let attempts = RefCell::new(CredAttempts::default());
let rejected: RefCell<Option<String>> = RefCell::new(None);
let mut cbs = git2::RemoteCallbacks::new();
cbs.credentials(|url,u,allowed| acquire_cred(repo.workdir(), &attempts, url, u, allowed));
cbs.push_update_reference(|_r, status| { if let Some(m)=status { *rejected.borrow_mut()=Some(m.into()); } Ok(()) });
let mut opts = git2::PushOptions::new();
opts.remote_callbacks(cbs);
let refspec = format!(":refs/tags/{tag}");            // empty src = delete remote ref
remote.push(&[refspec.as_str()], Some(&mut opts)).map_err(|e| map_remote_err(e, remote_name))?;
if let Some(msg) = rejected.into_inner() { return Err(AppError::PushRejected(...)); }
```

**ls-remote (git2 connect + list):**
```rust
let mut remote = repo.find_remote(remote)?;
let attempts = RefCell::new(CredAttempts::default());
let mut cbs = git2::RemoteCallbacks::new();
cbs.credentials(|url,u,allowed| acquire_cred(repo.workdir(), &attempts, url, u, allowed));
remote.connect_auth(git2::Direction::Fetch, Some(cbs), None)
    .map_err(|e| evict_fresh_on_auth_fail(&repo, &attempts, map_remote_err(e, remote_name)))?;
let out: Vec<(String, git2::Oid)> = remote.list()?          // &[RemoteHead]
    .iter()
    .filter(|h| h.name().starts_with("refs/tags/"))
    .map(|h| (h.name().to_string(), h.oid()))
    .collect();
remote.disconnect().ok();
```

## 7. Sub-increments (each a single fresh-context senior-dev pass, ordered)

- **SI-1 — Core classification + ls-remote (Rust).**
  Files: `crates/bonsai-core/src/git/tag_sync.rs` (new), `git/mod.rs` (register).
  Content: DTOs (§2), `resolve_default_remote`, `ls_remote_tags`, `list_tag_sync`
  (§4). Unit tests against a scratch **bare-repo remote** (see §8) covering
  in-sync / stale-moved / local-only / remote-only + the annotated-not-false-stale
  case + offline error mapping.
  **Done:** `cargo test -p bonsai-core` green; clippy clean; classification table
  passes incl. annotated.

- **SI-2 — Resolve ops (Rust core).**
  Files: `crates/bonsai-core/src/git/tag_sync.rs`.
  Content: `force_refresh_tag`, `delete_remote_tag` (§6), name validation reuse.
  **Done:** CLI-oracle tests (a bare remote) prove a moved tag is corrected by
  refresh and a remote tag is removed by delete; error variants covered. Watch the
  ~500-line limit (split to `tag_sync/net.rs` if needed — §1).

- **SI-3 — Command layer (Rust/Tauri).**
  Files: `src-tauri/src/commands/tags.rs` (+ `_inner`), invoke_handler registration.
  Content: three commands (§3.2) wrapping SI-1/SI-2 in `spawn_blocking`.
  **Done:** `cargo build` + `cargo check` green; commands registered; existing tag
  command tests unaffected.

- **SI-4 — IPC types + wiring + mock (TS).**
  Files: `src/ipc/types.ts` (DTOs + 3 IpcApi sigs), `src/ipc/tauri.ts` (invokes),
  `src/ipc/mock/handlers/tagSync.ts` (new), `src/ipc/fixtures/tagSync.ts` (new),
  `src/ipc/mock.ts` (register). Fixture exercises all 5 statuses incl. an annotated
  stale row; mock `forceRefreshTag`/`deleteRemoteTag` mutate the in-memory report.
  **Done:** `tsc` green; `pnpm dev` (`VITE_MOCK_IPC=1`) serves the report;
  `listTagSync` reject-path (simulated offline) returns an error the UI can catch.

- **SI-5 — Sidebar render + actions (React).**
  Files: `src/components/Sidebar.tsx` (TagRow badge + expand trigger),
  `src/components/repoWorkspace/useTagRemoteActions.ts` (new handlers),
  `src/components/workspaceMenus.ts` (`tagMenuItems`), destructive-confirm via
  `src/components/dialogs/BranchTagDialogs.tsx`. Visuals per `docs/contracts/P77-ui.md`.
  **Done:** badges render from the mock report; per-status menu actions call IPC;
  remote-delete & force-move gated behind explicit confirm; plain tags list still
  renders when `listTagSync` rejects (graceful degrade).

## 8. Acceptance criteria

### AI-gate (orchestrator-verifiable)
- `cargo test -p bonsai-core` classification suite green, using a scratch repo with
  a **bare-repo remote** (build fixtures with git2, not shell loops — CLAUDE.md):
  - annotated tag with identical committish on both sides ⇒ `in-sync`, NOT `stale`
    (the core regression);
  - remote tag force-moved to a different commit ⇒ `stale`, with distinct
    `localOid` / `remoteOid`;
  - local-only (unpushed) tag ⇒ `local-only`;
  - remote-only tag ⇒ `remote-only`;
  - lightweight-vs-annotated same-commit ⇒ `in-sync`, `annotated=true`.
- Resolve ops (SI-2) proven against the bare remote: `force_refresh_tag` corrects a
  stale local tag (post-condition `in-sync`); `delete_remote_tag` removes the
  remote ref (post-condition `local-only`).
- Graceful offline: with an unreachable/nonexistent remote URL, `list_tag_sync`
  returns `NetworkError`/`AuthFailed` (mapped), never panics.
- Browser harness (`VITE_MOCK_IPC=1`): Tags section shows status badges from the
  fixture; a mocked `listTagSync` rejection still renders the tags list.
- `tsc` + `cargo clippy -D warnings` clean.

### USER CHECKPOINT (native — orchestrator must NOT self-pass)
- Against a REAL remote: expand Tags → live ls-remote returns correct statuses.
- Reproduce the origin bug: move `v1.1.0` on the remote from a second machine,
  fetch normally, confirm Bonsai shows `stale`, then **force-refresh** fixes it.
- Push an unpushed tag; delete a local-only leftover; delete a remote tag and
  force-move a remote tag — each destructive op prompts explicit confirmation and
  the credential chain works (no prompts stored).
