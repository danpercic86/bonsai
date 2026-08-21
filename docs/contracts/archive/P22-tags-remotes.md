# P22 — Tags & remotes management

User request: two groups of operations that today are **absent** (tags are only *displayed* via
`list_refs`; remotes are only *fetch/pull/push*):

1. **Tags** — create (lightweight AND annotated), delete, push a tag to a remote.
2. **Remotes** — add, remove, rename, set-url (plus a new `list_remotes` so the section can
   enumerate configured remotes, not just remote-tracking branches).

New Rust module `crates/bonsai-core/src/git/tags.rs` + additions to
`crates/bonsai-core/src/git/remote.rs`; eight new commands; `Tag* ` / `RemoteInfo` wire types; the
IPC triple; a **tag row/pill context menu** (tags have none today), a **"Create tag here"** item on
the commit-row menu, and management affordances on the **Remotes** sidebar section.

Reference contracts (patterns reused verbatim): `docs/contracts/M6-remotes.md` (credential chain,
push refspec + rejection handling, `map_remote_err`, command/`_inner`/`spawn_blocking` template,
stateful-mock + `?remote=` triggers), `docs/contracts/P19-submodules.md` (new-module + IPC-triple +
sidebar-section + stateful-mock structural spine, "no new AppError variant" philosophy),
`docs/contracts/P9-stash-management.md` (context-menu + ConfirmDialog wiring).

Source files studied (exact patterns to mirror):
- `crates/bonsai-core/src/git/branches.rs` — `list_refs` (surfaces tags via `repo.tag_names(None)`
  **read fresh every call**, §confirmed below), `create_branch`/`delete_branch`/`create_branch_here`,
  `validate_branch_name`, `open_repo_at` (`NO_SEARCH`).
- `crates/bonsai-core/src/git/remote.rs` — `fetch_all`/`pull_ff`/`push_current`; the credential chain
  `next_cred_method`/`acquire_cred`/`CredAttempts`/`CRED_EXHAUSTED_MSG`/`map_remote_err`
  (all already `pub(crate)`); the push refspec + `push_update_reference` rejection pattern
  (`remote.rs:438-489`).
- `crates/bonsai-core/src/git/commit.rs` — `resolve_signature` (annotated tags need a tagger; error
  `ConfigMissing` naming each missing key, exactly like commit).
- `crates/bonsai-core/src/git/stash.rs` + `src-tauri/src/commands.rs` stash inners
  (`commands.rs:1488-1608`) — core + `#[tauri::command]` + `_inner` + `repo_path(state, repo_id)?` +
  `spawn_blocking` template.
- `src-tauri/src/lib.rs:15-87` — `generate_handler!` registration.
- `src/ipc/{types.ts,tauri.ts,mock.ts}` + `index.ts` — IPC triple + `MockRepoState`
  (`mock.ts:207-235`); mirror the submodule/stash method shapes.
- `src/components/Sidebar.tsx` — Remotes section (`Sidebar.tsx:478-508`), Tags section
  (`:510-534`, collapsed by default), `TagRow` (`:182-191`, no menu today), `RemoteRow`
  (`:152-180`), `SectionHeader` + `extra` header-action slot (`:70-95`, `:541-557` "Stash changes"
  button precedent), `SidebarProps` (`:29-68`).
- `src/components/RepoWorkspace.tsx` — `commitMenuItems` (`:1991-2029`), `buildContextItems`
  (`:2034-2061`, `r.kind === 'tag'` returns `[]` today, `:2043`), `branchMenuItems`,
  `handleSidebarContextMenu`, the shared `ContextMenu`/`setMenu` (`:229`, `:2700-2702`), the
  ConfirmDialog cluster (`:2581-2680`) and PromptDialog (`:2682-2698`), stash/delete handler
  shape (`:1133-1174`, `:1423-1487`), `refetchBranches`/`refetchGraph`/`refreshAll`.
- `src/components/PromptDialog.tsx` — **single-input** modal (name-only); a name+url or
  name+toggle+message form needs a new sibling dialog (§7.4, §OPEN-6).
- `crates/bonsai-core/src/error.rs` — existing variants; `NoRemote`/`InvalidName` reused, **no new
  variant** added (§OPEN-1).

---

## OPEN DECISIONS (recommended defaults chosen; implementation is NOT blocked)

1. **Duplicate-tag / duplicate-remote error variant.** → **Recommend NO new `AppError` variant**
   (mirrors P19). A duplicate tag (git2 `ErrorCode::Exists`, `force==false`) →
   `AppError::Git("tag '<name>' already exists")`; a duplicate remote →
   `AppError::Git("remote '<name>' already exists")`. Both are pre-validated by the frontend
   (tag names against `branches.tags`, remote names against the `remotes` list), so this is a
   race/edge backstop. *(Alternative: add `TagExists`/`RemoteExists` — rejected, not worth the wire
   surface for a list-sourced, pre-validated key.)*
2. **Tag target default.** → **Recommend REQUIRE an explicit `targetOid`** from the UI
   ("Create tag here" always supplies the commit oid). No implicit HEAD default — an empty/blank
   oid is `AppError::Git("cannot create tag: '<oid>' is not a valid commit id")`. *(Alternative:
   default to HEAD when omitted — rejected: the only entry point is a commit row, which always has
   an oid; an implicit default invites tagging the wrong commit.)*
3. **Delete-tag-on-remote (`push :refs/tags/x`).** → **Recommend OUT of scope v1.** `delete_tag`
   is **local only**. Pushing a deletion refspec is a separate, easily-mis-fired destructive remote
   op; defer to a later milestone. Documented in the `pushTag` JSDoc and the USER CHECKPOINT.
4. **`force` on create/push.** → **Recommend carry `force: bool` through core → command → IPC, but
   the v1 UI always sends `false`.** Keeps the core signatures faithful to git semantics and unit-
   testable, while the UI never force-overwrites a tag or force-pushes (consistent with M6 "never
   force"). *(Alternative: drop `force` from the IPC and hardcode `false` in the command — rejected:
   the task specifies `force` in the core signatures and a thin honest passthrough is cheaper than a
   divergent command surface.)*
5. **How the Remotes section enumerates configured remotes.** There is **no `list_remotes` today** —
   the section renders only remote-tracking branches (`BranchesSnapshot.remote`, e.g. `origin/main`),
   so a freshly-added remote with no tracking refs is invisible and there is no per-remote row to
   right-click. → **Recommend ADD `list_remotes` and render configured-remote rows at the TOP of the
   Remotes section** (a `RemoteInfo` list from `list_remotes`), each row carrying the
   Rename/Edit-URL/Remove context menu; the existing remote-tracking-branch tree renders **below**,
   unchanged. This is the smallest change that (a) shows remotes even with zero tracking branches and
   (b) gives each remote a right-click target, without restructuring the tracking-branch tree.
   *(Alternative: nest tracking branches under their remote row — rejected for v1: a larger tree
   refactor with no functional gain.)*
6. **Multi-field dialogs.** `PromptDialog` is single-input. → **Recommend two small new sibling
   components** modeled on `PromptDialog`: `TagCreateDialog` (name + annotated toggle + optional
   message) and `RemoteEditDialog` (name + url, reused for Add). Rename and Edit-URL are single-field
   → **reuse `PromptDialog`** (Rename: `initialValue = <current name>`; Edit-URL:
   `initialValue = <current url>`). *(Alternative: overload `PromptDialog` with optional extra
   fields — rejected: muddies a clean, well-tested single-purpose component.)*
7. **"Push tag" remote selection.** → **Recommend: no chooser dialog.** 0 remotes → item omitted;
   1 remote → single item `Push tag to <remote>`; >1 remotes → one `Push tag to <remote>` item per
   remote, flattened into the context menu. Reuses the existing `ContextMenu`; no new modal.

All defaults are read-only-safe or standard-git-equivalent; the only worktree/network effects are
`push_tag` (network, credentialed) and none touch the superproject worktree.

---

## 1. Overview & invariants held

- **Rust owns all Git logic.** `tags.rs` + the `remote.rs` additions wrap every git2 call; React only
  renders `RemoteInfo`/`BranchesSnapshot.tags` and dispatches commands.
- **IPC carries compact precomputed data.** Tag create/delete return `()` and the frontend refetches
  `list_branches` (tags re-surface, §2.5); `list_remotes` returns a small `Vec<RemoteInfo>`. No raw
  libgit2 objects; no per-item round-trips. **Commands = request/response; no new events or channels.**
- **git2 is blocking → `spawn_blocking`.** Every command wraps its blocking core exactly like the
  stash/remote inners. `push_tag` is the **only** networked (credentialed) command; the four remote-
  mgmt ops and both tag-mutation ops are local-only (no network, no credentials).
- **Runtime-free cores.** `tags.rs` and the `remote.rs` additions take `&Path`/`&str`, no Tauri types
  → unit/CLI-testable without the Tauri "test" feature.
- **Credential reuse (locked M6 chain).** `push_tag` reuses `acquire_cred`/`CredAttempts`/
  `map_remote_err` (already `pub(crate)` in `remote.rs`); never prompts, never stores passwords;
  never force-pushes (refspec has no leading `+` in v1, §OPEN-4).
- **Mock-implementable.** Every command has a stateful `mock.ts` impl seeded so `VITE_MOCK_IPC=1`
  runs the whole feature in a plain browser (create tag adds to `branches.tags`; delete removes it;
  push tag is a no-op success; remotes list/add/remove/rename/set-url mutate a `remotes` list).
- **The perf-gated graph walk is untouched.** Tags/remotes management adds no `RefLabel` computation
  to the walk; the tag pill already comes from the existing walk. `list_refs` is unchanged.

---

## 2. New Rust module `crates/bonsai-core/src/git/tags.rs`

Register in `crates/bonsai-core/src/git/mod.rs`: add `pub mod tags;` in alphabetical position
(`stash` < `status` < `submodule` < `tags`) — i.e. **after** `pub mod submodule;`.

Open with the module-standard `open_repo_at(&Path)` helper (copy the `NO_SEARCH` helper used by
`branches.rs:57-63` / `remote.rs:192-198`).

### 2.0 Confirmation: `list_refs` re-reads tags every call

`branches::list_refs` calls `repo.tag_names(None)?` fresh on every invocation
(`branches.rs:158-164`). Therefore **no new `list_tags` command is needed**: after `create_tag` /
`delete_tag`, a `refetchBranches()` (→ `list_branches` → `list_refs`) re-surfaces the updated tag
list, exactly as the sidebar already consumes it (`BranchesSnapshot.tags`).

### 2.1 Function signatures

```rust
/// Blocking. Create a tag `name` pointing at commit `target_oid`.
/// - `message: Some(_)` → ANNOTATED tag (`Repository::tag`, needs a tagger from
///   `resolve_signature` — `ConfigMissing` if user.name/email unset, exactly like commit).
/// - `message: None`    → LIGHTWEIGHT tag (`Repository::tag_lightweight`).
/// `force` overwrites an existing tag of the same name; v1 UI always passes `false` (§OPEN-4).
/// Errors: invalid/blank name → `InvalidName`; bad/unknown `target_oid` → `Git`; duplicate
/// (Exists, !force) → `Git("tag '<name>' already exists")`; missing identity (annotated) →
/// `ConfigMissing`.
pub fn create_tag(
    workdir: &Path,
    name: &str,
    target_oid: &str,
    message: Option<String>,
    force: bool,
) -> Result<(), AppError>;

/// Blocking. Delete LOCAL tag `name` (`Repository::tag_delete`). Local-only — does NOT
/// contact any remote (§OPEN-3). Errors: not-found → `Git("tag '<name>' not found")`;
/// blank name → `InvalidName`.
pub fn delete_tag(workdir: &Path, name: &str) -> Result<(), AppError>;

/// Blocking. Push `refs/tags/<tag_name>` to `remote_name` over the M6 credential path.
/// Refspec `refs/tags/<n>:refs/tags/<n>` — NO leading '+' unless `force` (v1 UI always false).
/// Errors: remote not found → `NoRemote`; tag not found locally → `Git`; auth exhausted/auth code
/// → `AuthFailed`; Net/Http/Ssh → `NetworkError`; server rejection (`push_update_reference`
/// status / NotFastForward) → `PushRejected`; other → `Git`.
pub fn push_tag(
    workdir: &Path,
    remote_name: &str,
    tag_name: &str,
    force: bool,
) -> Result<(), AppError>;
```

No new wire *type* for tags — create/delete/push all return `()`; the sidebar reads
`BranchesSnapshot.tags` (§2.0). A `#[cfg(test)]` name-validation table is the only pure test.

### 2.2 `create_tag` internals

```rust
if name.trim().is_empty() || name.starts_with('-') {
    return Err(AppError::InvalidName(format!("invalid tag name: '{name}'")));
}
if !git2::Reference::is_valid_name(&format!("refs/tags/{name}")) {
    return Err(AppError::InvalidName(format!("invalid tag name: '{name}'")));
}
let repo = open_repo_at(workdir)?;
let oid = git2::Oid::from_str(target_oid)
    .map_err(|_| AppError::Git(format!("cannot create tag: '{target_oid}' is not a valid commit id")))?;
let target = repo.find_object(oid, None)
    .map_err(|_| AppError::Git(format!("cannot create tag: commit '{target_oid}' not found")))?;

let result = match &message {
    Some(msg) => {
        let sig = resolve_signature(&repo.config()?.snapshot()?)?;   // ConfigMissing if unset
        repo.tag(name, &target, &sig, msg, force)
    }
    None => repo.tag_lightweight(name, &target, force),
};
match result {
    Ok(_oid) => Ok(()),
    Err(e) if e.code() == git2::ErrorCode::Exists =>
        Err(AppError::Git(format!("tag '{name}' already exists"))),
    Err(e) => Err(e.into()),
}
```

Name-validation shape mirrors `branches::validate_branch_name` (`branches.rs:179-188`).

### 2.3 `delete_tag` internals

```rust
if name.trim().is_empty() {
    return Err(AppError::InvalidName("tag name is empty".to_string()));
}
let repo = open_repo_at(workdir)?;
match repo.tag_delete(name) {
    Ok(()) => Ok(()),
    Err(e) if e.code() == git2::ErrorCode::NotFound =>
        Err(AppError::Git(format!("tag '{name}' not found"))),
    Err(e) => Err(e.into()),
}
```

### 2.4 `push_tag` internals (credential reuse)

Mirror `push_current` (`remote.rs:438-489`) exactly, but with a fixed tag refspec and no
upstream/tracking logic:

```rust
let repo = open_repo_at(workdir)?;
// Confirm the local tag exists (clearer error than a server-side failure).
if repo.find_reference(&format!("refs/tags/{tag_name}")).is_err() {
    return Err(AppError::Git(format!("tag '{tag_name}' not found")));
}
let mut remote = match repo.find_remote(remote_name) {
    Ok(r) => r,
    Err(e) if e.code() == git2::ErrorCode::NotFound =>
        return Err(AppError::NoRemote(format!("remote '{remote_name}' not found"))),
    Err(e) => return Err(e.into()),
};
let config = repo.config()?;
let attempts = std::cell::RefCell::new(crate::git::remote::CredAttempts::default());
let rejected: std::cell::RefCell<Option<String>> = std::cell::RefCell::new(None);
{
    let mut cbs = git2::RemoteCallbacks::new();
    cbs.credentials(|url, user, allowed| crate::git::remote::acquire_cred(&config, &attempts, url, user, allowed));
    cbs.push_update_reference(|_ref, status| {
        if let Some(msg) = status { *rejected.borrow_mut() = Some(msg.to_string()); }
        Ok(())
    });
    let mut opts = git2::PushOptions::new();
    opts.remote_callbacks(cbs);
    let plus = if force { "+" } else { "" };                    // v1 UI: force == false
    let refspec = format!("{plus}refs/tags/{tag_name}:refs/tags/{tag_name}");
    remote.push(&[refspec.as_str()], Some(&mut opts))
        .map_err(|e| crate::git::remote::map_remote_err(e, remote_name))?;
}
if let Some(msg) = rejected.into_inner() {
    return Err(AppError::PushRejected(format!(
        "push rejected by remote: {msg}. Bonsai v1 never force-pushes — fetch/pull first."
    )));
}
Ok(())
```

Because `acquire_cred`, `CredAttempts`, `map_remote_err` are already `pub(crate)` in `remote.rs`
(exposed for P19), **no change to `remote.rs` is needed for `push_tag`**.

### 2.5 Error mapping (→ existing `AppError`, no new variant §OPEN-1)

| Situation | AppError kind |
|---|---|
| blank/invalid tag name | `invalidName` |
| bad/unknown target oid | `git` |
| duplicate tag (Exists, !force) | `git` ("tag '<name>' already exists") |
| annotated tag, identity unset | `configMissing` |
| tag not found (delete/push) | `git` |
| push: remote not found | `noRemote` |
| push: auth exhausted / auth code | `authFailed` (via `map_remote_err`) |
| push: Net/Http/Ssh | `networkError` (via `map_remote_err`) |
| push: server rejection / NotFastForward | `pushRejected` |
| any other libgit2 | `git` |

---

## 3. Remotes management — additions to `crates/bonsai-core/src/git/remote.rs`

All local-only (no network, no credentials). Use the existing `open_repo_at` in `remote.rs`.

### 3.1 Wire type

```rust
/// One configured remote (P22 §3). Wire: camelCase.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    /// Remote name, e.g. "origin".
    pub name: String,
    /// Fetch URL from config. None if unreadable/non-UTF-8. (Push-URL not surfaced in v1.)
    pub url: Option<String>,
}
```

### 3.2 Function signatures

```rust
/// Blocking. Enumerate configured remotes (name + fetch URL), sorted case-insensitively
/// by name. Empty repo / no remotes → Ok(vec![]) (NOT an error — unlike `fetch_all`).
pub fn list_remotes(workdir: &Path) -> Result<Vec<RemoteInfo>, AppError>;

/// Blocking. Add remote `name` → `url` (`Repository::remote`). Errors: invalid name
/// (`git2::Remote::is_valid_name`) → `InvalidName`; duplicate (Exists) →
/// `Git("remote '<name>' already exists")`.
pub fn add_remote(workdir: &Path, name: &str, url: &str) -> Result<(), AppError>;

/// Blocking. Remove remote `name` (`Repository::remote_delete` — also drops its
/// remote-tracking refs + config). Errors: not found → `NoRemote`.
pub fn remove_remote(workdir: &Path, name: &str) -> Result<(), AppError>;

/// Blocking. Rename remote `name` → `new_name` (`Repository::remote_rename` — moves
/// refs/remotes/<name>/* and rewrites config). The returned non-default-refspec "problem"
/// list is logged (eprintln) and ignored. Errors: not found → `NoRemote`; invalid new name →
/// `InvalidName`; new name exists → `Git("remote '<new_name>' already exists")`.
pub fn rename_remote(workdir: &Path, name: &str, new_name: &str) -> Result<(), AppError>;

/// Blocking. Set the FETCH url of remote `name` (`Repository::remote_set_url`, push=false).
/// Errors: not found → `NoRemote`; invalid url → `Git`.
pub fn set_remote_url(workdir: &Path, name: &str, url: &str) -> Result<(), AppError>;
```

### 3.3 `list_remotes` internals

```rust
let repo = open_repo_at(workdir)?;
let mut out = Vec::new();
for n in repo.remotes()?.iter() {
    let name = match n { Some(n) => n.to_string(), None => {
        eprintln!("bonsai: skipping remote with non-UTF-8 name"); continue; } };
    let url = repo.find_remote(&name).ok().and_then(|r| r.url().map(str::to_string));
    out.push(RemoteInfo { name, url });
}
out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.name.cmp(&b.name)));
Ok(out)
```

### 3.4 Mutating internals (error mapping)

- **add_remote**: validate `git2::Remote::is_valid_name(name)` → else `InvalidName`; then
  `repo.remote(name, url)`; `ErrorCode::Exists` → `Git("remote '<name>' already exists")`.
- **remove_remote**: `repo.remote_delete(name)`; `ErrorCode::NotFound` → `NoRemote(...)`.
- **rename_remote**: `Remote::is_valid_name(new_name)` → else `InvalidName`; then
  `repo.remote_rename(name, new_name)?` (returns `git2::string_array::StringArray` of problems —
  `if !problems.is_empty() { eprintln!(...) }`, do not fail); `NotFound` → `NoRemote`, `Exists` →
  `Git("remote '<new_name>' already exists")`.
- **set_remote_url**: `repo.remote_set_url(name, url)`; `NotFound` → `NoRemote`; invalid url is a
  git2 error → `Git`.

**No new `AppError` variant** — reuses `noRemote` | `invalidName` | `git`.

---

## 4. Commands (`src-tauri/src/commands.rs`) + registration

Add imports to the block (`commands.rs:1-35`):

```rust
use bonsai_core::git::tags;
use bonsai_core::git::remote::{list_remotes, add_remote, remove_remote, rename_remote, set_remote_url, RemoteInfo};
```

Follow the stash template exactly (`#[tauri::command] pub async fn` → runtime-free `_inner` →
`repo_path(state, repo_id)?` → `spawn_blocking` → join error `AppError::Other(format!("task join
error: {e}"))`). **None emit `repo-changed`** — the frontend refetches imperatively.

```rust
// --- Tags ---
// create_tag(repoId, name, targetOid, message?, force) -> ()
//   errors: noRepo | invalidName | configMissing | git
#[tauri::command] pub async fn create_tag(
    state, repo_id: String, name: String, target_oid: String,
    message: Option<String>, force: bool) -> Result<(), AppError>;
async fn create_tag_inner(...) { spawn_blocking(move || tags::create_tag(&path, &name, &target_oid, message, force)) }

// delete_tag(repoId, name) -> ()                 errors: noRepo | invalidName | git
#[tauri::command] pub async fn delete_tag(state, repo_id: String, name: String) -> Result<(), AppError>;
async fn delete_tag_inner(...) { spawn_blocking(move || tags::delete_tag(&path, &name)) }

// push_tag(repoId, remote, tagName, force) -> ()
//   errors: noRepo | noRemote | authFailed | networkError | pushRejected | git
#[tauri::command] pub async fn push_tag(
    state, repo_id: String, remote: String, tag_name: String, force: bool) -> Result<(), AppError>;
async fn push_tag_inner(...) { spawn_blocking(move || tags::push_tag(&path, &remote, &tag_name, force)) }

// --- Remotes ---
// list_remotes(repoId) -> Vec<RemoteInfo>        errors: noRepo | git
#[tauri::command] pub async fn list_remotes(state, repo_id: String) -> Result<Vec<RemoteInfo>, AppError>;
async fn list_remotes_inner(...) { spawn_blocking(move || list_remotes(&path)) }

// add_remote(repoId, name, url) -> ()            errors: noRepo | invalidName | git
#[tauri::command] pub async fn add_remote(state, repo_id: String, name: String, url: String) -> Result<(), AppError>;
async fn add_remote_inner(...) { spawn_blocking(move || add_remote(&path, &name, &url)) }

// remove_remote(repoId, name) -> ()              errors: noRepo | noRemote | git
#[tauri::command] pub async fn remove_remote(state, repo_id: String, name: String) -> Result<(), AppError>;
async fn remove_remote_inner(...) { spawn_blocking(move || remove_remote(&path, &name)) }

// rename_remote(repoId, name, newName) -> ()     errors: noRepo | noRemote | invalidName | git
#[tauri::command] pub async fn rename_remote(state, repo_id: String, name: String, new_name: String) -> Result<(), AppError>;
async fn rename_remote_inner(...) { spawn_blocking(move || rename_remote(&path, &name, &new_name)) }

// set_remote_url(repoId, name, url) -> ()        errors: noRepo | noRemote | git
#[tauri::command] pub async fn set_remote_url(state, repo_id: String, name: String, url: String) -> Result<(), AppError>;
async fn set_remote_url_inner(...) { spawn_blocking(move || set_remote_url(&path, &name, &url)) }
```

Register all eight in `src-tauri/src/lib.rs` `generate_handler!` (append after
`commands::init_repo`; add a trailing comma to that line):

```rust
        commands::init_repo,
        commands::create_tag,
        commands::delete_tag,
        commands::push_tag,
        commands::list_remotes,
        commands::add_remote,
        commands::remove_remote,
        commands::rename_remote,
        commands::set_remote_url
```

Command surface after P22 adds these eight. Events: `repo-changed` (unchanged). Channels: none.

---

## 5. IPC layer (TypeScript) — `src/ipc/{types.ts,tauri.ts,mock.ts}` + `index.ts`

### 5.1 `types.ts`

```ts
export interface RemoteInfo {
  name: string;
  url: string | null;   // fetch URL; null if unreadable
}
```

`IpcApi` additions (place near `deleteBranch`/`listStashes`; mirror the JSDoc style):

```ts
/** Create a tag at `targetOid`. `message` non-null ⇒ annotated (needs git identity),
 *  null ⇒ lightweight. `force` overwrites (v1 UI passes false). Rejects
 *  noRepo | invalidName | configMissing | git. */
createTag(repoId: string, name: string, targetOid: string, message: string | null, force: boolean): Promise<void>;
/** Delete a LOCAL tag (does not touch any remote). Rejects noRepo | invalidName | git. */
deleteTag(repoId: string, name: string): Promise<void>;
/** Push refs/tags/<tagName> to `remote`. `force` false in v1. Rejects
 *  noRepo | noRemote | authFailed | networkError | pushRejected | git. */
pushTag(repoId: string, remote: string, tagName: string, force: boolean): Promise<void>;

/** Configured remotes (name + fetch URL). Rejects noRepo | git. */
listRemotes(repoId: string): Promise<RemoteInfo[]>;
/** Add a remote. Rejects noRepo | invalidName | git. */
addRemote(repoId: string, name: string, url: string): Promise<void>;
/** Remove a remote (drops its tracking refs). Rejects noRepo | noRemote | git. */
removeRemote(repoId: string, name: string): Promise<void>;
/** Rename a remote. Rejects noRepo | noRemote | invalidName | git. */
renameRemote(repoId: string, name: string, newName: string): Promise<void>;
/** Set a remote's fetch URL. Rejects noRepo | noRemote | git. */
setRemoteUrl(repoId: string, name: string, url: string): Promise<void>;
```

No `AppError.kind` union change — all kinds already exist. Re-export `RemoteInfo` from
`src/ipc/index.ts` (add to the `export type { ... }` list).

### 5.2 `tauri.ts` (snake_case command, camelCase arg keys — Tauri auto-converts)

```ts
createTag: (repoId, name, targetOid, message, force) =>
  invoke<void>('create_tag', { repoId, name, targetOid, message, force }),
deleteTag: (repoId, name) => invoke<void>('delete_tag', { repoId, name }),
pushTag:   (repoId, remote, tagName, force) =>
  invoke<void>('push_tag', { repoId, remote, tagName, force }),
listRemotes:  (repoId) => invoke<RemoteInfo[]>('list_remotes', { repoId }),
addRemote:    (repoId, name, url) => invoke<void>('add_remote', { repoId, name, url }),
removeRemote: (repoId, name) => invoke<void>('remove_remote', { repoId, name }),
renameRemote: (repoId, name, newName) => invoke<void>('rename_remote', { repoId, name, newName }),
setRemoteUrl: (repoId, name, url) => invoke<void>('set_remote_url', { repoId, name, url }),
```

### 5.3 `mock.ts` (stateful)

- Import `RemoteInfo` (with `SubmoduleInfo`, `mock.ts` import block).
- Add `remotes: RemoteInfo[]` to `MockRepoState` (`mock.ts:207-235`, near `submodules`).
- Seed in `createRepoState`: default/detached/unborn all get
  `[{ name: 'origin', url: 'https://example.com/repo.git' }]` (so add/rename/remove/set-url and
  push-tag remote selection are all exercisable; keeps `listRemotes` non-empty).
- Command methods (mirror the submodule/stash method style; `delay(...)` optional):
  - `createTag(repoId, name, targetOid, message, force)` → if `!force && state.branches.tags`
    includes `name` → `throw { kind: 'git', message: \`tag '${name}' already exists\` }`; else insert
    `name` into `state.branches.tags` **sorted case-insensitively**; *(optional harness fidelity:
    also push a `{ name, kind: 'tag' }` `RefLabel` onto the graph node whose oid === `targetOid`, if
    present)*; resolve `void`.
  - `deleteTag(repoId, name)` → remove from `state.branches.tags` (and any matching graph pill);
    `void`.
  - `pushTag(repoId, remote, tagName, force)` → honor the `?remote=` triggers (authfail/network/
    rejected → throw the M6 canned errors); otherwise `delay` + resolve `void` (no state change).
  - `listRemotes(repoId)` → `structuredClone(state.remotes)`.
  - `addRemote(repoId, name, url)` → if name exists → `throw { kind: 'git', message: \`remote
    '${name}' already exists\` }`; else push `{ name, url }` (sorted); `void`.
  - `removeRemote(repoId, name)` → filter out by name (also drop `origin/*` from
    `state.branches.remote` when removing that remote, to mirror tracking-ref cleanup); `void`.
  - `renameRemote(repoId, name, newName)` → map the matching entry's `name`; rewrite
    `state.branches.remote` entries `\`${name}/…\`` → `\`${newName}/…\``; `void`.
  - `setRemoteUrl(repoId, name, url)` → set the matching entry's `url`; `void`.
- No new events/channels.

---

## 6. Frontend — Sidebar (`src/components/Sidebar.tsx`)

### 6.1 Tags section — give tag rows a context menu

- `SidebarProps` +`onTagContextMenu(name: string, clientX: number, clientY: number): void;`
- `TagRow` (`:182-191`) gains an `onContextMenu` prop and wires it exactly like `RemoteRow`
  (`e.preventDefault(); onTagContextMenu(name, e.clientX, e.clientY)`). Pass through in both the
  flat and tree render branches (`:525`, `:530`).

### 6.2 Remotes section — configured-remote rows + Add affordance (§OPEN-5)

- `SidebarProps` additions:
  ```ts
  remotes: RemoteInfo[];
  onRemoteContextMenu(name: string, clientX: number, clientY: number): void;
  onAddRemote(): void;
  ```
- Render **at the top of the Remotes section**, above the existing remote-tracking tree, a small
  list of `ConfiguredRemoteRow` (new sibling of `RemoteRow`): glyph `☁`, `name`, `title={url ?? ''}`,
  `onContextMenu` → `onRemoteContextMenu(name, …)`. The existing `data.remote` tracking-branch
  rendering stays unchanged below it.
- `SectionHeader` `extra` slot: an `sidebar-add` button (mirror the Stashes "⊟" button
  `:541-557`) labeled "Add remote" / `+`, `disabled={busy}`, `onClick={onAddRemote}`.
- Empty state: when `remotes.length === 0 && data.remote.length === 0` → the existing
  `<p className="branch-muted">No remotes</p>`.

---

## 7. Frontend — RepoWorkspace (`src/components/RepoWorkspace.tsx`)

### 7.1 State + refetch

- `const [remotes, setRemotes] = useState<RemoteInfo[]>([]);`
- `refetchRemotes()` → `ipc.listRemotes(repoId)` → `setRemotes`; add to `refreshAll` and the
  `repo-changed` / window-focus refresh batch (mirror `refetchSubmodules`, P19 §6.3).
- Pending-dialog state:
  ```ts
  const [pendingCreateTag, setPendingCreateTag]   = useState<{ oid: string } | null>(null);
  const [pendingDeleteTag, setPendingDeleteTag]   = useState<string | null>(null);
  const [pendingAddRemote, setPendingAddRemote]   = useState<boolean>(false);
  const [pendingRenameRemote, setPendingRenameRemote] = useState<{ name: string } | null>(null);
  const [pendingEditUrl, setPendingEditUrl]       = useState<{ name: string; url: string } | null>(null);
  const [pendingRemoveRemote, setPendingRemoveRemote] = useState<string | null>(null);
  ```

### 7.2 Menu builders

- **`commitMenuItems(oid)`** (`:1991-2029`): add, next to "Create branch here":
  ```ts
  { label: 'Create tag here', icon: <TagIcon />, disabled: gate,
    onSelect: () => setPendingCreateTag({ oid }) },
  ```
  (Add a small `TagIcon`, or reuse an existing glyph icon.)
- **`tagMenuItems(name)`** (new): built from `remotes`:
  ```ts
  function tagMenuItems(name: string): ContextMenuItem[] {
    const gate = mutating || opActive;
    const items: ContextMenuItem[] = [
      { label: 'Delete tag', icon: <DeleteIcon />, disabled: gate,
        onSelect: () => setPendingDeleteTag(name) },
      { label: 'Copy tag name', disabled: false, onSelect: () => void copyToClipboard(name) },
    ];
    // §OPEN-7: 0 remotes → no push item; 1 → single; >1 → one per remote.
    for (const r of remotes)
      items.push({ label: `Push tag to ${r.name}`, disabled: gate,
        onSelect: () => void handlePushTag(r.name, name) });
    return items;
  }
  ```
- **`remoteMenuItems(name)`** (new):
  ```ts
  function remoteMenuItems(name: string): ContextMenuItem[] {
    const gate = mutating || opActive;
    const url = remotes.find((r) => r.name === name)?.url ?? '';
    return [
      { label: 'Rename…',  disabled: gate, onSelect: () => setPendingRenameRemote({ name }) },
      { label: 'Edit URL…', disabled: gate, onSelect: () => setPendingEditUrl({ name, url }) },
      { label: 'Remove…',  icon: <DeleteIcon />, disabled: gate,
        onSelect: () => setPendingRemoveRemote(name) },
    ];
  }
  ```
- Handlers to open the shared menu:
  ```ts
  function handleTagContextMenu(name, x, y)    { setMenu({ x, y, items: tagMenuItems(name) }); }
  function handleRemoteContextMenu(name, x, y) { setMenu({ x, y, items: remoteMenuItems(name) }); }
  ```
- **`buildContextItems`** (`:2043`): change the tag branch — replace
  `if (r.kind === 'tag' || r.kind === 'head') return [];` with:
  ```ts
  if (r.kind === 'head') return [];
  if (r.kind === 'tag')  return tagMenuItems(r.name);
  ```
  so the **graph tag pill** opens the same menu as the sidebar tag row.

### 7.3 Operation handlers (mirror `handleDeleteBranch` / stash handlers — set flag, try, toast,
refetch, finally)

```ts
async function handleCreateTag(oid, name, message /* string|null */) {
  setMutating(true);
  try {
    await ipc.createTag(repoId, name, oid, message, /* force */ false);
    pushToast('success', `Created tag ${name}`);
    await Promise.all([refetchBranches(), refetchGraph()]); // tag re-surfaces (§2.0) + pill
  } catch (e) { pushToast('error', errorMessage(e)); }
  finally { setMutating(false); }
}
async function handleDeleteTag(name) { /* deleteTag → toast → refetchBranches + refetchGraph */ }
async function handlePushTag(remote, name) { /* pushTag(...,false) → toast `Pushed tag ${name} → ${remote}` */ }
async function handleAddRemote(name, url) { /* addRemote → refetchRemotes + refetchBranches + refetchGraph */ }
async function handleRemoveRemote(name)   { /* removeRemote → refetchRemotes + refetchBranches + refetchGraph */ }
async function handleRenameRemote(name, newName) { /* renameRemote → same refetch trio */ }
async function handleSetRemoteUrl(name, url)     { /* setRemoteUrl → refetchRemotes */ }
```

Rationale for refetch sets: remove/rename move remote-tracking refs (and thus graph pills), so those
three refetch `remotes + branches + graph`; set-url changes only the `RemoteInfo` list → `remotes`
only; create/delete tag refetch `branches` (tag list) + `graph` (pill).

### 7.4 Dialogs

- **`TagCreateDialog`** (new component, §OPEN-6): fields = name (text), "Annotated" checkbox,
  message (textarea, shown/enabled only when annotated). `validate`: name non-empty & not
  leading-`-` & not already in `branches.tags`; if annotated, message non-empty. On submit →
  `handleCreateTag(pendingCreateTag.oid, name, annotated ? message : null)`. Wire
  `open={pendingCreateTag !== null}`, `onCancel={() => setPendingCreateTag(null)}`.
- **`RemoteEditDialog`** (new component, §OPEN-6): fields = name (text) + url (text); used for
  **Add remote** (`open={pendingAddRemote}`, both empty; submit → `handleAddRemote`). Validate name
  via a client regex mirroring git remote-name rules (non-empty, no whitespace/control) + not
  already in `remotes`; url non-empty.
- **Rename** → reuse `PromptDialog` (`open={pendingRenameRemote !== null}`, label "New remote name",
  `initialValue={pendingRenameRemote?.name}`, validate: non-empty, not already in `remotes`;
  submit → `handleRenameRemote(pendingRenameRemote.name, value.trim())`).
- **Edit URL** → reuse `PromptDialog` (`open={pendingEditUrl !== null}`, label "Fetch URL",
  `initialValue={pendingEditUrl?.url}`, validate non-empty; submit →
  `handleSetRemoteUrl(pendingEditUrl.name, value.trim())`).
- **Delete tag** → `ConfirmDialog` (`open={pendingDeleteTag !== null}`, title "Delete tag",
  confirm "Delete tag"; note: "Deletes the local tag only; a tag already pushed to a remote is not
  removed there."). Mirror the delete-branch ConfirmDialog (`:2581-2597`).
- **Remove remote** → `ConfirmDialog` (`open={pendingRemoveRemote !== null}`, title
  "Remove remote", confirm "Remove remote"; note: "Removes the remote and its remote-tracking
  branches from this repo. The server is not affected.").

### 7.5 Sidebar wiring

Pass into `<Sidebar>`: `remotes`, `onTagContextMenu={handleTagContextMenu}`,
`onRemoteContextMenu={handleRemoteContextMenu}`, `onAddRemote={() => setPendingAddRemote(true)}`.

---

## 8. Testing (AI gate)

**Env (tester):** `TMP`/`TEMP` → `D:\Temp`; scratch repos under `D:\Temp\bonsai-scratch`; run
`cargo test` and `clippy` **sequentially** (MEMORY: target-dir race); forward slashes in Bash-tool
paths; local `file://` / bare remotes only (autonomous); `require_git!` skip guard when `git` absent.

### 8.1 `crates/bonsai-core/tests/tags_cli.rs` (CLI oracle, mirrors `remote_cli.rs`)

Fixture: `scratch_dir()` repo, deterministic identity (`user.name`/`user.email`), two commits.

1. **Lightweight parity**: `create_tag(name, <oidC1>, None, false)` → `git cat-file -t <name>` is
   `commit` (points straight at the commit); `git rev-parse refs/tags/<name>` == C1; equals
   `git tag <name> <oidC1>` on a twin.
2. **Annotated parity**: `create_tag(name, <oidC1>, Some("msg"), false)` → `git cat-file -t <name>`
   is `tag`; `git tag -a <name> -m msg <oidC1>` on a twin yields an equivalent tag object (type
   `tag`, target == C1, message == "msg\n" after git's newline normalization — compare
   `git for-each-ref --format='%(*objectname) %(contents:subject)'`).
3. **Annotated needs identity**: unset `user.email` in an isolated config → `create_tag(..,
   Some(..),..)` → `Err(ConfigMissing)` naming `user.email`; lightweight still succeeds.
4. **Duplicate**: second `create_tag(name, .., None, false)` → `Err(Git)` "already exists";
   with `force=true` → Ok and the tag moves to the new target.
5. **Bad target / bad name**: unknown 40-hex oid → `Err(Git)`; `""`/`"-x"` name → `Err(InvalidName)`.
6. **Delete parity**: `delete_tag(name)` → `git tag -l <name>` empty; matches `git tag -d <name>`;
   deleting a missing tag → `Err(Git)`.
7. **Push to bare remote**: `git init --bare origin.git`; add as `file://` remote on the scratch
   repo; create a tag; `push_tag("origin", <tag>, false)` → the tag ref appears in the bare repo
   (`git --git-dir origin.git tag -l` lists it / `show-ref` has `refs/tags/<tag>`); equals
   `git push origin <tag>` on a twin. Pushing a non-existent local tag → `Err(Git)`.
8. **`list_refs` re-surfaces**: after create then delete, `branches::list_refs(..).tags` contains
   then omits the tag (proves §2.0 — no separate list needed).

### 8.2 `crates/bonsai-core/tests/remote_mgmt_cli.rs`

Scratch repo + a `file://` bare path or two for URL/rename checks.

1. **add**: `add_remote("backup", <url>)` → `git remote -v` lists `backup` with the url; equals
   `git remote add`. Duplicate → `Err(Git)`; invalid name (`"bad name"`) → `Err(InvalidName)`.
2. **list**: `list_remotes` returns all configured remotes sorted (name+url) — cross-check against
   `git remote` / `git remote get-url <n>`; empty repo → `Ok(vec![])` (NOT an error).
3. **rename**: seed `origin` + a tracking ref; `rename_remote("origin","upstream")` →
   `git remote` shows `upstream`, and `git show-ref` has `refs/remotes/upstream/*` (moved); matches
   `git remote rename`. Missing → `Err(NoRemote)`; target exists → `Err(Git)`.
4. **set-url**: `set_remote_url("origin", <url2>)` → `git remote get-url origin` == `<url2>`;
   matches `git remote set-url`. Missing → `Err(NoRemote)`.
5. **remove**: `remove_remote("origin")` → `git remote` no longer lists it and its
   `refs/remotes/origin/*` are gone; matches `git remote remove`. Missing → `Err(NoRemote)`.

### 8.3 Unit tests (`#[cfg(test)]`)

- `tags.rs`: name-validation table (blank / leading `-` / valid / control chars); `RemoteInfo`
  wire-shape `serde_json` assertion (`{ "name": .., "url": .. }` camelCase, `url: null`).
- `remote.rs`: `list_remotes` sort order (case-insensitive, tie-break) — small pure-ish test.

### 8.4 Command-level tests (`commands.rs`)

Each new `_inner` with no repo open → `AppError::NoRepo` (extend the existing `MISSING_ID` pattern,
`commands.rs:2236+`).

### 8.5 Browser-harness (orchestrator-verifiable)

- `pnpm build` + `tsc` clean; no `@tauri-apps/*` module executed; no console errors.
- Right-click a commit row → menu shows **"Create tag here"** next to "Create branch here" →
  `TagCreateDialog` opens; create a lightweight tag → sidebar Tags section gains the tag; create an
  annotated tag with a message → also appears; duplicate name is blocked by dialog validation.
- Right-click a **tag row** (and a tag **pill** in the graph) → menu with Delete tag / Copy /
  "Push tag to origin"; Delete → ConfirmDialog → tag disappears; Push tag → success toast (mock
  no-op); `?remote=authfail` → Push tag shows the authFailed error toast.
- Remotes section shows the configured `origin` row + an **Add remote** (+) header button →
  `RemoteEditDialog`; add `backup` → new row appears; right-click a remote → Rename / Edit URL /
  Remove; Rename via PromptDialog updates the row; Edit URL updates `title`; Remove via ConfirmDialog
  drops the row (and any `origin/*` tracking rows).

### 8.6 USER CHECKPOINT (native `pnpm tauri dev`, real repo/remote)

- Create lightweight + annotated tags on real commits; verify with `git tag` / `git cat-file -t`.
  Delete a tag; verify with `git tag -l`.
- Add / rename / set-url / remove a real remote; verify each against `git remote -v`.
- **Push a tag to a REAL network remote** with the credential helper/agent (no password prompt);
  confirm the tag appears on the server. Confirm a bogus remote URL yields the `networkError`
  toast, not a hang. (Delete-on-remote is intentionally NOT offered — §OPEN-3.)

---

## 9. Sub-increments (each a single fresh-context senior-dev pass)

### P22a — Tags backend + tests
- New `crates/bonsai-core/src/git/tags.rs` (§2): `create_tag`/`delete_tag`/`push_tag` with exact
  git2 calls + credential reuse; `git/mod.rs` `pub mod tags;`.
- `commands.rs`: three `#[tauri::command]` + `_inner` pairs (§4); `lib.rs` registration (3).
- Unit tests (§8.3 tags, §8.4 for the 3 commands) + `crates/bonsai-core/tests/tags_cli.rs` (§8.1).
- **Acceptance**: `cargo check`/`clippy`/`test` clean; tag CLI-oracle parity (lightweight/annotated/
  delete/push-to-bare) passes; no frontend change needed to compile.

### P22b — Remotes backend + tests
- `crates/bonsai-core/src/git/remote.rs` additions (§3): `RemoteInfo` + `list_remotes` /
  `add_remote` / `remove_remote` / `rename_remote` / `set_remote_url`.
- `commands.rs`: five `#[tauri::command]` + `_inner` pairs (§4); `lib.rs` registration (5).
- Unit tests (§8.3 remote, §8.4 for the 5 commands) + `crates/bonsai-core/tests/remote_mgmt_cli.rs`
  (§8.2).
- **Acceptance**: `cargo check`/`clippy`/`test` clean; remote-mgmt CLI-oracle parity passes.

### P22c — IPC triple + frontend (tags + remotes)
- `types.ts` (`RemoteInfo` + 8 `IpcApi` methods), `index.ts` re-export; `tauri.ts` (8 wrappers);
  `mock.ts` (`remotes` state + seed + 8 command methods, `?remote=` reuse for pushTag) (§5).
- `Sidebar.tsx`: tag-row menu prop + `TagRow` onContextMenu; Remotes section configured-remote rows
  + Add header button + props (§6).
- `RepoWorkspace.tsx`: `remotes` state + `refetchRemotes`; `tagMenuItems`/`remoteMenuItems`;
  "Create tag here" in `commitMenuItems`; `buildContextItems` tag branch; 7 handlers; 6 dialogs;
  Sidebar wiring (§7).
- New components `TagCreateDialog.tsx`, `RemoteEditDialog.tsx` (§7.4).
- **Acceptance**: `pnpm build`/`tsc` clean; harness §8.5 passes (create/delete/push tag; add/rename/
  edit-url/remove remote), screenshots of the tag menu, remote rows, and both new dialogs.

---

## 10. File touch list

- **New**: `crates/bonsai-core/src/git/tags.rs`; `crates/bonsai-core/tests/tags_cli.rs`;
  `crates/bonsai-core/tests/remote_mgmt_cli.rs`; `src/components/TagCreateDialog.tsx`;
  `src/components/RemoteEditDialog.tsx`.
- `crates/bonsai-core/src/git/mod.rs` (`pub mod tags;`).
- `crates/bonsai-core/src/git/remote.rs` (`RemoteInfo` + 5 fns + unit test).
- `src-tauri/src/commands.rs` (imports + 8 command/_inner pairs + noRepo tests),
  `src-tauri/src/lib.rs` (register 8).
- `src/ipc/types.ts` (`RemoteInfo` + 8 `IpcApi` methods), `src/ipc/index.ts` (re-export),
  `src/ipc/tauri.ts` (8 wrappers), `src/ipc/mock.ts` (`remotes` state + seed + 8 methods).
- `src/components/Sidebar.tsx` (tag menu prop; Remotes rows + Add button + props),
  `src/components/RepoWorkspace.tsx` (state, refetch, menus, handlers, dialogs, wiring).
- `src/styles.css` (reuse existing `sidebar-add` / `branch-row` / dialog classes; add only if a new
  glyph/badge class is unavoidable).
- **No new `AppError` variant; no new events/channels; `notify` watcher, `list_refs`, and the graph
  walk unchanged.**
```