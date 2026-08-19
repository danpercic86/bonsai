# P73 — Submodule init/update: reconnect an orphaned `.git/modules` gitdir

Fixes two user-reported defects on the P19 submodule surface (real Azure DevOps superproject,
submodule `src/Hamilton.Voyager.Protocol/protocol`).

- **Bug 1 (UI-only)** — Init toasts "Initialized" while the badge stays "not initialized".
  `init_submodule` (`sm.init(false)`) is git-faithful: it writes `submodule.<name>.*` into
  `.git/config` and leaves the worktree empty, so `list_submodules` still classifies the row
  `uninitialized` (`git submodule status` prints `-`). **Decision (user, 2026-08-19): the UI's Init
  action invokes the existing `updateSubmodule` command** (`sm.update(init = true, …)` registers
  config before cloning). **No backend change; no `init_submodule` change.** Copy/labels/menu wiring
  are owned by `docs/contracts/P73-ui.md` (`ui-designer`). This contract does not specify UI.
- **Bug 2 (blocking, backend)** — `update_submodule` fails with
  `attempt to reinitialize '<super>/.git/modules/<path>'` when the submodule worktree is
  empty/gitlink-less but `.git/modules/<path>` is already a complete gitdir. This contract adds the
  reconnect ("salvage") path libgit2 lacks.

Prior contract: `docs/contracts/P19-submodules.md` (wire types, command surface, credential reuse,
error taxonomy — all unchanged by P73).

---

## 0. Confirmed diagnosis (do NOT re-derive; verified against vendored libgit2 1.9.6 in
`~/.cargo/registry/src/*/libgit2-sys-0.18.7+1.9.6/libgit2/src/libgit2/`)

1. `git_submodule_update` (`submodule.c:1408`) branches on ONE bit:
   `if (submodule_status & GIT_SUBMODULE_STATUS_WD_UNINITIALIZED)` (`submodule.c:1443`) → clone;
   else → open + checkout. Neither `git_submodule_update_strategy` nor `update_options.allow_fetch`
   is consulted on the clone branch (`allow_fetch` is read only in the else-branch,
   `submodule.c:1509`). **No `SubmoduleUpdateOptions` setting can steer around this.**
2. That bit is set at `submodule.c:2443-2445` (`__WD_SCANNED && !IN_WD`). `IN_WD` comes from
   `submodule_load_from_wd_lite` (`submodule.c:2222`) = `git_fs_path_contains(&path, ".git")` —
   purely "does `<workdir>/<path>/.git` exist". **Nothing about `.git/modules`.**
3. The clone branch sets `repository_cb = git_submodule_update_repo_init_cb`
   (`submodule.c:1379`) → `submodule_repo_create` (`submodule.c:1329`) with
   `MKPATH | NO_REINIT | NO_DOTGIT_DIR | RELATIVE_GITLINK`, repodir = `<gitdir>/modules/` joined
   (`git_str_joinpath`) with **`sm->path`** — path, not name. (Equal in the reported repo; §2.1
   handles the general case.)
4. `git_repository_init_ext` → `repository.c:2886-2891`: existing valid repo + `NO_REINIT` →
   `"attempt to reinitialize '%s'"`, `GIT_EEXISTS`.
5. Upstream `git submodule update` instead REUSES the module gitdir and rewrites the worktree
   gitlink (`connect_work_tree_and_git_dir`). libgit2 has no such path ⇒ Bonsai adds one.

Two subtleties that MUST be honoured:

- **`git2::Repository::set_workdir(abs, true)` is unusable as the primitive.**
  `git_repository_set_workdir` (`repository.c:3259`) early-returns 0 when `repo->workdir` already
  equals the prettified path (`repository.c:3271-3274`) — and the wedged module config already has
  the correct `core.worktree`, so it writes nothing and the submodule stays `WD_UNINITIALIZED`.
  When it DOES fire it calls `repo_write_gitlink(..., use_relative_path = false)`
  (`repository.c:3284`) → an ABSOLUTE `gitdir:`, unlike clone's `RELATIVE_GITLINK` and unlike
  upstream git. ⇒ **Write the gitlink ourselves, relative** (§2.4).
- **A plain SAFE checkout will NOT repopulate the emptied workdir.** After reattach, `sm.update`
  takes the else-branch → `git_checkout_tree(sub_repo, target, SAFE)`. The module index is on disk
  and matches the target, so files missing from the workdir hit `checkout_action_no_wd` case
  `GIT_DELTA_UNMODIFIED` → `CHECKOUT_ACTION_IF(RECREATE_MISSING, UPDATE_BLOB, NONE)`
  (`checkout.c:302-306`); `RECREATE_MISSING` is auto-added only under `FORCE` or when the index is
  not on disk (`checkout.c:2447-2455`). A naive fix therefore returns `Ok`, writes nothing, and
  since `wd_oid == HEAD == index_id` the badge flips to `upToDate` over an EMPTY directory — worse
  than the reported bug. ⇒ **`CheckoutBuilder::recreate_missing(true)` on the SALVAGE PATH ONLY**
  (§3).
- `Submodule::reload(force)` ignores `force` (`GIT_UNUSED(force)`, `submodule.c:1781`) but does
  clear `IN_WD | __WD_OID_VALID | __WD_FLAGS`; `git_submodule_update` also re-looks-up status from
  disk. ⇒ After reattach, **drop the handle and re-`find_submodule`** (cheap; keeps
  `index_id`/`head_id` coherent) AND **assert the wedge cleared** before calling `update` (§2.5
  step 9).

### INVARIANT preserved (assert in review)

`recreate_missing(true)` is **not** `force`. git2's `CheckoutBuilder` defaults to
`GIT_CHECKOUT_SAFE`, so the non-force invariant asserted by
`crates/bonsai-core/tests/submodule_cli_2.rs:113` (`update_refuses_to_clobber_dirty_submodule`)
still holds: a dirty tracked edit still makes update REFUSE. `recreate_missing` only permits
writing files that are *absent* from the worktree — it never overwrites existing content. And the
salvage path is entered only when the worktree is empty (§2.5 step 6), so `recreate_missing` can
never be in effect over user files.

---

## 1. Scope & non-goals

**In scope (backend):** a private reconnect/salvage path inside
`crates/bonsai-core/src/git/submodule.rs`, a rollback for a failed fresh clone in
`update_submodule`, and the `remove_cached_git_dir` commondir fix (§6).

**Explicit non-goals — nothing below changes:**
- No wire/type change: `SubmoduleStatus`, `SubmoduleInfo` (Rust + `src/ipc/types.ts`) untouched.
- No new IPC command, event, or channel. `update_submodule(repoId, name)` keeps its exact signature
  and error set (`noRepo | invalidName | authFailed | networkError | git`).
- No signature or behaviour change to `init_submodule`, `sync_submodule`, `add_submodule`,
  `deinit_submodule`, `remove_submodule`, `list_submodules`, `classify_status`, `submodule_info`.
- `src/ipc/mock/handlers/submodules.ts` needs **no** new handler. (Optional, §8: the mock may model
  a `wedged` fixture row purely to exercise the UI's Init→update path; it is not required by this
  contract and adds no new method.)
- `crates/bonsai-core/src/git/status.rs` still excludes submodules
  (`.exclude_submodules(true)`, P19 §7).
- No new `AppError` variant. Every new refusal is `AppError::Git` so the frontend `errorMessage`
  mapping keeps working unchanged.
- No shelling out to `git` (§10 REJECTED alternative).

**Invariants held:** Rust owns all Git logic; the IPC boundary is unchanged and compact;
`update_submodule` still runs under `spawn_blocking` from the existing command wrapper; the salvage
path performs **zero network I/O** (§3).

---

## 2. New private items in `crates/bonsai-core/src/git/submodule.rs`

All items are private (`fn` / `enum` / `struct` with no `pub`), placed after
`rollback_partial_add` and before `submodule_path`. Exact signatures:

```rust
/// Outcome of the P73 reconnect attempt. `NotApplicable` means "this is not the
/// wedged state — let libgit2 do its normal thing (fresh clone or plain
/// checkout)". `Reattached` means we rewrote the worktree gitlink so that
/// `git_submodule_update` will now take its open+checkout branch instead of its
/// NO_REINIT clone branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Salvage {
    NotApplicable,
    Reattached,
}

/// Locate an EXISTING orphaned module gitdir for this submodule, or `None`.
///
/// Uses `repo.commondir()` (NOT `repo.path()`): inside a linked worktree
/// `repo.path()` is `.git/worktrees/<wt>/`, whose `modules/` subdir does not
/// exist — the shared modules root lives under the commondir.
///
/// libgit2 keys the clone repodir on `sm->path` (`submodule.c:1329`) while
/// Bonsai's `remove_cached_git_dir` keys on `name`; the two diverge for a
/// renamed submodule, so BOTH candidates are probed (`name` first — it is git's
/// canonical key — then `path` when different). Each candidate is validated with
/// `validate_modules_name` / `validate_rel_path` and must canonicalize to a path
/// STRICTLY inside the canonicalized `<commondir>/modules` (same belt-and-braces
/// containment as `remove_cached_git_dir`). Returns the canonicalized dir.
fn module_gitdir(
    repo: &git2::Repository,
    name: &str,
    path: &str,
) -> Result<Option<std::path::PathBuf>, AppError>;

/// True when `abs` is absent, or is a directory holding no regular file, no
/// symlink and no `.git` entry at any depth (leftover EMPTY directories are
/// tolerated — see OPEN-4). A present `.git` entry means we are not in the
/// wedged state at all, so this returns false. Bounded: stops and returns false
/// after `EMPTY_SCAN_LIMIT` visited entries. Never follows symlinks.
fn workdir_is_empty(abs: &std::path::Path) -> bool;

/// Heuristic ownership check for the URL guard: equal after trimming ASCII
/// whitespace, stripping ONE trailing `.git` and any trailing `/` (in that
/// order, repeated until stable). NO percent-decoding (the reported URL contains
/// `%20`), no scheme/host canonicalization. Exact byte compare first; an
/// ASCII-case-insensitive match is also accepted (and logged) — see OPEN-3.
fn urls_equivalent(a: &str, b: &str) -> bool;

/// Reconnect `sub_workdir` to `module_dir`, mirroring upstream git's
/// `connect_work_tree_and_git_dir`:
///  1. write `<sub_workdir>/.git` = `gitdir: <p>\n`, where `<p>` is the
///     forward-slash relative path from `sub_workdir` to `module_dir` when both
///     canonicalize under `super_workdir`, else the absolute `module_dir`
///     (with any `\\?\` verbatim prefix stripped) — see OPEN-5;
///  2. set the module's `core.worktree` to the forward-slash relative path from
///     `module_dir` back to `sub_workdir` (same fallback rule);
///  3. set the module's `core.bare = false`.
/// NEVER touches the superproject repo, its config, its index or `.gitmodules`.
fn write_gitlink(
    sub_workdir: &std::path::Path,
    module_dir: &std::path::Path,
    super_workdir: &std::path::Path,
) -> Result<(), AppError>;

/// The P73 salvage: detect the wedged state (empty/gitlink-less worktree +
/// complete `.git/modules/<key>` gitdir) and reattach it so `Submodule::update`
/// reuses the existing gitdir instead of hitting libgit2's NO_REINIT clone.
/// Fail-CLOSED at every step: any doubt returns `NotApplicable` (libgit2 keeps
/// its current behaviour) or a refusal `AppError::Git` — it must NEVER silently
/// fall through into the clone branch after having modified anything.
fn reattach_module_gitdir(
    repo: &git2::Repository,
    sm: &git2::Submodule<'_>,
    name: &str,
) -> Result<Salvage, AppError>;

/// What existed on disk BEFORE a fresh-clone `update_submodule` attempt, so a
/// failure can be rolled back to exactly that state (§5). Cheap: three stat/config
/// probes, no I/O of consequence. Never errors — unknowable ⇒ the conservative
/// value (`true` = "existed", i.e. do not delete).
struct PreUpdateState {
    /// `<commondir>/modules/<key>` already existed (⇒ never delete it).
    module_dir_existed: bool,
    /// `<super_workdir>/<path>` already existed (⇒ never remove the dir itself).
    workdir_existed: bool,
    /// `submodule.<name>.url` was already present in the LOCAL config
    /// (⇒ never clear the registration).
    registered: bool,
}

fn snapshot_pre_update(repo: &git2::Repository, name: &str, path: &str) -> PreUpdateState;

/// Best-effort rollback of a failed FRESH-CLONE `update_submodule` (§5), modelled
/// on `rollback_partial_add`. Each step independent, its own failure ignored; the
/// caller returns the ORIGINAL error. Called ONLY when
/// `salvage == Salvage::NotApplicable`.
fn rollback_partial_update(
    repo: &git2::Repository,
    name: &str,
    path: &str,
    pre: &PreUpdateState,
);
```

Plus one module-level constant:

```rust
/// Upper bound on entries visited by `workdir_is_empty` before it gives up and
/// declares the dir non-empty (a huge tree is certainly not the wedged state).
const EMPTY_SCAN_LIMIT: usize = 4096;
```

### 2.1 `module_gitdir` pseudocode

```
fn module_gitdir(repo, name, path) -> Result<Option<PathBuf>, AppError>:
    root = repo.commondir().join("modules")
    canon_root = root.canonicalize()  or return Ok(None)   // no modules dir at all
    candidates = []
    if validate_modules_name(name).is_ok():      candidates.push(name)
    if path != name and validate_rel_path(path).is_ok()
       and validate_modules_name(path).is_ok():  candidates.push(path)   // name first (OPEN-1)
    for key in candidates:
        dir = root.join(key)
        canon = dir.canonicalize()  or continue            // absent → next candidate
        if !canon.starts_with(&canon_root) or canon == canon_root: continue   // containment
        if !canon.is_dir(): continue
        return Ok(Some(canon))
    Ok(None)
```

`validate_modules_name` / `validate_rel_path` failures are **not** errors here (this is a probe);
they simply drop the candidate. The hostile-name refusal happens earlier, in
`reattach_module_gitdir` step 1, which is what the traversal test asserts.

### 2.2 `workdir_is_empty` pseudocode

```
fn workdir_is_empty(abs) -> bool:
    md = symlink_metadata(abs) or return true          // absent ⇒ "empty"
    if !md.is_dir(): return false                      // a file/symlink at the path ⇒ not wedged
    stack = [abs]; visited = 0
    while let Some(d) = stack.pop():
        entries = read_dir(d) or return false          // unreadable ⇒ refuse (fail closed)
        for e in entries:
            visited += 1
            if visited > EMPTY_SCAN_LIMIT: return false
            if e.file_name() == ".git": return false   // already attached ⇒ not the wedged state
            t = e.symlink_metadata() or return false
            if t.is_dir(): stack.push(e.path())        // leftover empty dirs tolerated
            else: return false                         // any file or symlink ⇒ not empty
    true
```

### 2.3 `urls_equivalent` pseudocode

```
fn normalize(u) -> String:
    s = u.trim()
    loop:
        if s.ends_with('/'):    s = s[..len-1]; continue
        if s.ends_with(".git"): s = s[..len-4]; continue
        break
    s.to_string()

fn urls_equivalent(a, b) -> bool:
    na = normalize(a); nb = normalize(b)
    if na == nb: return true
    if na.eq_ignore_ascii_case(&nb):
        eprintln!("bonsai: submodule url match only case-insensitively: {na} vs {nb}")
        return true                                    // OPEN-3
    false
```

### 2.4 `write_gitlink` pseudocode

```
fn write_gitlink(sub_workdir, module_dir, super_workdir) -> Result<(), AppError>:
    // Both dirs exist by now (caller create_dir_all'd the worktree).
    c_sub  = sub_workdir.canonicalize()?
    c_mod  = module_dir.canonicalize()?                // already canonical from module_gitdir
    c_root = super_workdir.canonicalize()?

    // Relative when both live under the superproject root (the normal case);
    // absolute otherwise (OPEN-5). NOTE: on Windows `canonicalize` yields a
    // `\\?\` verbatim prefix — use the canonical forms ONLY for the containment
    // test and the hop computation; emit the ABSOLUTE fallback from the
    // un-canonicalized (already absolute) inputs, and strip any `\\?\` prefix.
    gitdir_value = if c_mod.starts_with(&c_root) && c_sub.starts_with(&c_root)
                   { rel_path(&c_sub, &c_mod) }        // e.g. "../../.git/modules/vendor/sub"
                   else { strip_verbatim(module_dir) } // e.g. "D:/other/.git/modules/vendor/sub"
    worktree_value = if (same condition) { rel_path(&c_mod, &c_sub) }  // e.g. "../../../vendor/sub"
                     else { strip_verbatim(sub_workdir) }

    write_atomic(sub_workdir.join(".git"), format!("gitdir: {gitdir_value}\n"))?

    let sub_repo = git2::Repository::open_ext(module_dir, NO_SEARCH, &[] as &[&OsStr])?
    let mut cfg = sub_repo.config()?
    cfg.set_str("core.worktree", &worktree_value)?
    cfg.set_bool("core.bare", false)?
    Ok(())
```

`rel_path(from_dir, to)` is a pure component-diff helper (both inputs canonicalized and sharing a
prefix): drop the common prefix, emit one `..` per remaining `from_dir` component, then the
remaining `to` components, joined with `/`. Empty result ⇒ `"."`. **Always forward slashes**
(git requires them in `core.worktree` / the gitlink on all platforms).

`write_atomic` = write to `<sub_workdir>/.git.bonsai-tmp` then `fs::rename` over `.git`; on rename
failure fall back to a direct write. Rationale: a torn `.git` file would leave the submodule in a
worse state than the wedge. Both `AppError::Io`-mapped through the existing `From` impl (or
`AppError::Git` with the path in the message if no `Io` variant exists — check `error.rs`; the
frontend mapping only distinguishes the tags listed in §7).

### 2.5 `reattach_module_gitdir` — the 10 ordered, fail-closed steps

```
fn reattach_module_gitdir(repo, sm, name) -> Result<Salvage, AppError>:

  // Resolve inputs once.
  path = sm.path().to_string_lossy().replace('\\', "/")
  super_wd = repo.workdir().ok_or(AppError::Git("repository has no working directory"))?

  // 1. Hostile-name / traversal guard BEFORE any filesystem decision.
  validate_modules_name(name)?                       // AppError::Git "...unsafe name"
  crate::git::stage::validate_rel_path(&path)?       // rejects absolute / `..` / backslash escape

  // 2. Only the WD_UNINITIALIZED state can be wedged.
  flags = repo.submodule_status(name, git2::SubmoduleIgnore::None)?
  if !flags.contains(git2::SubmoduleStatus::WD_UNINITIALIZED): return Ok(NotApplicable)

  // 3. No orphaned gitdir ⇒ this is a genuine first clone. Let libgit2 clone.
  module_dir = match module_gitdir(repo, name, &path)? { Some(d) => d, None => return Ok(NotApplicable) }

  // 4. The gitdir must be a REAL repo. NO_SEARCH so a bogus dir can never
  //    resolve upward to the superproject and make us rewrite the wrong thing.
  if git2::Repository::open_ext(&module_dir,
        git2::RepositoryOpenFlags::NO_SEARCH, &[] as &[&std::ffi::OsStr]).is_err():
      return Ok(NotApplicable)                       // incomplete/garbage dir → libgit2's problem
  let sub_repo = <the successfully opened repo>       // reused in step 7

  // 5. Workdir containment: the target must be strictly inside the superproject.
  sub_wd = super_wd.join(&path)
  parent = sub_wd.parent().ok_or(AppError::Git("submodule path has no parent"))?
  c_parent = parent.canonicalize().map_err(|e| AppError::Git(format!(
      "cannot resolve submodule parent directory '{}': {e}", parent.display())))?
  c_super  = super_wd.canonicalize().map_err(...)?
  if !c_parent.starts_with(&c_super):
      return Err(AppError::Git(format!(
          "refusing to reconnect submodule '{name}': '{}' is outside the repository", sub_wd.display())))

  // 6. The workdir must be empty/absent. NEVER clobber user files.
  if !workdir_is_empty(&sub_wd):
      return Err(AppError::Git(format!(
          "submodule '{name}' has files in '{}' but no .git link; refusing to reconnect its \
           existing git directory. Move or delete that directory, or run \
           `git submodule update --init -- {path}` manually.", sub_wd.display())))

  // 7. URL guard — prove the orphaned gitdir belongs to THIS submodule.
  //    Configured url: LOCAL config only (a GLOBAL `submodule.<name>.url` key must
  //    not be able to fake registration), falling back to .gitmodules via sm.url().
  local = repo.config().and_then(|c| c.open_level(git2::ConfigLevel::Local)).ok()
          .or_else(|| git2::Config::open(&repo.commondir().join("config")).ok())
  configured = local.and_then(|c| c.get_string(&format!("submodule.{name}.url")).ok())
               .or_else(|| sm.url().ok().flatten().map(str::to_string))
  configured = match configured { Some(u) if !u.trim().is_empty() => u,
      _ => return Err(AppError::Git(format!(
             "submodule '{name}' has no configured url; cannot verify the existing git \
              directory belongs to it — run Sync, then Update"))) }
  origin = sub_repo.find_remote("origin").ok().and_then(|r| r.url().map(str::to_string))
  origin = match origin { Some(u) => u, None => return Err(AppError::Git(format!(
             "the existing git directory for submodule '{name}' has no 'origin' remote; \
              refusing to reconnect it"))) }                                   // OPEN-2
  if !urls_equivalent(&configured, &origin):
      return Err(AppError::Git(format!(
          "refusing to reconnect submodule '{name}': its configured url '{configured}' does not \
           match the existing git directory's origin '{origin}'. Run Sync to update the url, or \
           remove '{}' if it is stale.", module_dir.display())))

  // 8. Reattach. Containment is already proven, so creating the dir is safe.
  if !sub_wd.exists(): std::fs::create_dir_all(&sub_wd).map_err(|e| AppError::Git(format!(
      "cannot create submodule directory '{}': {e}", sub_wd.display())))?
  write_gitlink(&sub_wd, &module_dir, super_wd)?

  // 9. Verify the wedge actually cleared, from a FRESH handle (Submodule::reload
  //    ignores `force`; a stale handle would lie).
  fresh = repo.find_submodule(name)?                 // cheap; keeps index_id/head_id coherent
  let _ = &fresh;                                    // handle dropped by the caller's re-open
  flags2 = repo.submodule_status(name, git2::SubmoduleIgnore::None)?
  if flags2.contains(git2::SubmoduleStatus::WD_UNINITIALIZED):
      return Err(AppError::Git(format!(
          "reattach did not take effect for submodule '{name}': git still reports it as \
           uninitialized after writing '{}'", sub_wd.join(".git").display())))

  // 10. Done.
  eprintln!("bonsai: reconnected submodule '{name}' to existing git dir {}", module_dir.display())
  Ok(Salvage::Reattached)
```

Steps 6, 7 and 9 are **refusals** (user-actionable, `AppError::Git`). Steps 2, 3 and 4 are
**not-applicable** (no state changed, libgit2 proceeds as today). Step 1 and 5 are **security
refusals**. Nothing is written before step 8, so a refusal always leaves the repo byte-identical.

---

## 3. Rewritten `update_submodule` (signature UNCHANGED)

```rust
pub fn update_submodule(workdir: &Path, name: &str) -> Result<(), AppError>
```

```
let repo = open_workdir_repo(workdir)?;
let sm   = open_submodule(&repo, name)?;              // blank → InvalidName, unknown → Git
let path = sm.path().to_string_lossy().replace('\\', "/");

// (a) Snapshot before we can create anything (used only on the clone path).
let pre = snapshot_pre_update(&repo, name, &path);

// (b) P73 salvage: reconnect an orphaned .git/modules gitdir, if that is the state.
//     Any refusal here propagates immediately — we do NOT fall through to libgit2's
//     clone branch (it would fail with "attempt to reinitialize" anyway).
let salvage = reattach_module_gitdir(&repo, &sm, name)?;

// (c) Re-open the submodule handle: reattach changed on-disk state and
//     Submodule::reload ignores `force`. Drop the old handle FIRST (borrowck).
drop(sm);
let mut sm = open_submodule(&repo, name)?;

// (d) EXISTING, UNCHANGED credential block (P19 §2.5 — verbatim, do not touch).
let attempts = RefCell::new(CredAttempts::default());
let mut callbacks = git2::RemoteCallbacks::new();
callbacks.credentials(|url, username_from_url, allowed| {
    acquire_cred(repo.workdir(), &attempts, url, username_from_url, allowed)
});
let mut fo = git2::FetchOptions::new();
fo.remote_callbacks(callbacks);
let mut opts = git2::SubmoduleUpdateOptions::new();
opts.fetch(fo);

// (e) On the salvage path ONLY: allow the checkout to recreate files that are
//     absent from the (empty) worktree. NOT `force` — dirty content is still
//     protected (see the INVARIANT in §0).
if salvage == Salvage::Reattached {
    let mut co = git2::build::CheckoutBuilder::new();
    co.recreate_missing(true);
    opts.checkout(co);
}

// (f) init=true → init-then-update in one call (P19 §OPEN-4). Same error mapping.
match sm.update(true, Some(&mut opts)).map_err(|e| map_remote_err(e, name)) {
    Ok(()) => Ok(()),
    Err(e) => {
        // Rollback ONLY the fresh-clone path. On the salvage path the module
        // gitdir pre-existed and MUST NOT be deleted; the rewritten gitlink is
        // left in place (it is correct, and it makes a retry work).
        if salvage == Salvage::NotApplicable {
            rollback_partial_update(&repo, name, &path, &pre);
        }
        Err(e)
    }
}
```

### Property: the salvage path performs ZERO network I/O

On the reattached (else-)branch, `git_submodule_update` looks up the target oid in the module's own
ODB (`git_object_lookup`). The pinned commit is already local in the orphaned gitdir, so the lookup
succeeds and `update_options.allow_fetch` is never consulted (`submodule.c:1509`) — no remote is
contacted and `acquire_cred` is never invoked. **This is why P73 fixes the reported Azure DevOps
case even with no cached credentials and even with the remote unreachable** (see acceptance
criterion §9.3, the offline test).

---

## 4. `snapshot_pre_update` pseudocode

```
fn snapshot_pre_update(repo, name, path) -> PreUpdateState:
    module_dir_existed = module_gitdir(repo, name, path).ok().flatten().is_some()
    workdir_existed    = repo.workdir().map(|wd| wd.join(path).exists()).unwrap_or(true)
    registered = repo.config().and_then(|c| c.open_level(ConfigLevel::Local)).ok()
                 .and_then(|c| c.get_string(&format!("submodule.{name}.url")).ok())
                 .is_some()
    PreUpdateState { module_dir_existed, workdir_existed, registered }
```

Never returns `Err`; unknowable ⇒ the conservative value (`true`, "already existed", so rollback
leaves it alone).

---

## 5. `rollback_partial_update` semantics

Modelled on `rollback_partial_add` (`submodule.rs:275-296`): every step independent, every failure
ignored, the ORIGINAL error is what the caller returns. **Clone path only** — never called when
`salvage == Reattached`.

```
fn rollback_partial_update(repo, name, path, pre):
    // 1. Module gitdir: remove ONLY if the clone created it.
    if !pre.module_dir_existed && validate_modules_name(name).is_ok():
        remove_cached_git_dir(repo, name)              // commondir-based after §6
        if path != name && validate_modules_name(path).is_ok():
            remove_cached_git_dir(repo, path)          // libgit2 keys the clone on `path`

    // 2. Worktree: if we created the dir, remove it; if it pre-existed (empty),
    //    remove only its CONTENTS so we restore the pre-state exactly.
    if let Some(wd) = repo.workdir():
        if validate_rel_path(path).is_ok():
            let target = wd.join(path);
            if !pre.workdir_existed { let _ = std::fs::remove_dir_all(&target); }
            else { for e in read_dir(target) { let _ = remove_dir_all_or_file(e.path()); } }

    // 3. Registration: clear ONLY if `update(init = true)` created it.
    if !pre.registered:
        if let Ok(mut cfg) = repo.config():
            let _ = cfg.remove(&format!("submodule.{name}.url"));
            let _ = cfg.remove(&format!("submodule.{name}.update"));
            let _ = cfg.remove(&format!("submodule.{name}.active"));
```

**Never** touches `.gitmodules`, the superproject index, HEAD, or any path outside
`<workdir>/<path>` and `<commondir>/modules/<key>`. Step 2's "contents only" branch keeps
`git submodule status` reporting the same `-` row as before the failed attempt, so the UI does not
appear to lose the submodule.

`remove_dir_all_or_file(p)` = `remove_dir_all` when `symlink_metadata` says dir, else `remove_file`
(never follows symlinks).

---

## 6. Adjacent bug fixed in the same milestone

`remove_cached_git_dir` (`submodule.rs:391-392`) builds its root from
`repo.path().join("modules")`. Inside a **linked worktree** `repo.path()` is
`<commondir>/worktrees/<wt>/`, so the root is `.git/worktrees/<wt>/modules` — which does not exist,
`canonicalize` fails, and the cleanup silently no-ops (stale gitdir left behind, and a later
re-add hits exactly the P73 wedge).

**Change (one line):**

```rust
let root = repo.commondir().join("modules");
```

Everything else in the function stays byte-identical (name validation by the callers, canonicalized
strict-containment check, best-effort semantics). This also corrects `remove_submodule`
(`submodule.rs:364`) and `rollback_partial_add` (`submodule.rs:294`), both of which call it. In a
non-worktree repo `commondir() == path()`, so behaviour there is unchanged (assert this in a test,
§8.2).

---

## 7. Error taxonomy

All refusals are `AppError::Git` — **no new variant**, so `src/ipc/types.ts` `errorMessage`
mapping and the existing toasts keep working with zero frontend change.

| # | Failure mode | Where | `AppError` | Message shape | Kind |
|---|---|---|---|---|---|
| 1 | hostile submodule name (`..`, absolute, empty component) | reattach step 1 | `Git` | `refusing to touch .git/modules for submodule '<name>': unsafe name` (existing `validate_modules_name` text) | security refusal |
| 2 | submodule path escapes the repo | reattach step 1 | (from `validate_rel_path`, today `Other`) | existing text | security refusal |
| 3 | superproject has no workdir | reattach prologue | `Git` | `repository has no working directory` | internal |
| 4 | submodule parent dir unresolvable | reattach step 5 | `Git` | `cannot resolve submodule parent directory '<p>': <io>` | internal |
| 5 | submodule dir resolves outside the repo | reattach step 5 | `Git` | `refusing to reconnect submodule '<name>': '<p>' is outside the repository` | security refusal |
| 6 | worktree has files but no `.git` | reattach step 6 | `Git` | `submodule '<name>' has files in '<p>' but no .git link; refusing to reconnect its existing git directory. Move or delete that directory, or run \`git submodule update --init -- <path>\` manually.` | refusal (actionable) |
| 7 | no configured url anywhere | reattach step 7 | `Git` | `submodule '<name>' has no configured url; cannot verify the existing git directory belongs to it — run Sync, then Update` | refusal (actionable) |
| 8 | orphaned gitdir has no `origin` remote | reattach step 7 | `Git` | `the existing git directory for submodule '<name>' has no 'origin' remote; refusing to reconnect it` | refusal (actionable) |
| 9 | url mismatch | reattach step 7 | `Git` | `refusing to reconnect submodule '<name>': its configured url '<a>' does not match the existing git directory's origin '<b>'. Run Sync to update the url, or remove '<dir>' if it is stale.` | refusal (actionable) |
| 10 | cannot create the submodule dir | reattach step 8 | `Git` | `cannot create submodule directory '<p>': <io>` | internal |
| 11 | gitlink write / `core.worktree` write failed | `write_gitlink` | `Git` (or `Io` if `error.rs` has it) | `cannot write submodule gitlink '<p>': <io>` | internal |
| 12 | reattach had no effect | reattach step 9 | `Git` | `reattach did not take effect for submodule '<name>': git still reports it as uninitialized after writing '<p>'` | internal (bug signal) |
| 13 | fetch auth exhausted (clone path) | `sm.update` | `AuthFailed` (via `map_remote_err`) | unchanged | existing |
| 14 | transport failure (clone path) | `sm.update` | `NetworkError` (via `map_remote_err`) | unchanged | existing |
| 15 | any other libgit2 | `sm.update` | `Git` (via `map_remote_err`) | unchanged | existing |

Messages 6–9 name the concrete path/url so the user can act without opening a terminal. Keep them
single-sentence-first so the toast reads well truncated.

---

## 8. Test surface to extend (names + what each must PROVE; tester writes them)

### 8.1 `crates/bonsai-core/tests/submodule_cli_2.rs`

Reuse the existing helpers `build_sub()`, `build_super_with_sub(url)`, `cli_status_char(dir)`,
`only(dir)`, `require_git!()`, `SUB_PATH` (note: name == path for a CLI-added submodule).
Add a shared local helper:

```rust
/// Wedge the submodule: delete the gitlink and every workdir FILE while KEEPING
/// `.git/modules/<SUB_PATH>` intact, plus a sentinel file inside it so a later
/// success proves REUSE rather than a re-clone. Returns the sentinel path.
fn wedge(super_dir: &Path) -> PathBuf;
```

| Test | Must prove |
|---|---|
| `update_reconnects_orphaned_module_gitdir` | after `wedge`, `update_submodule` returns `Ok`; the sentinel inside `.git/modules/<path>` still exists (⇒ reused, not re-cloned); `<sub>/.git` is a FILE whose content is `gitdir: ` + a **relative** path (starts with `..`, no backslashes, no `\\?\`); every tracked file is back on disk with the pinned content; `only()` reports `UpToDate` with `wt_oid == index_oid`; `cli_status_char == ' '` |
| `reconnect_works_offline` | same wedge, but the upstream `file://` source directory is DELETED first ⇒ update still returns `Ok` and repopulates (proves zero network I/O on the salvage path) |
| `reconnect_refuses_non_empty_workdir` | wedge, then drop a stray `keepme.txt` in the submodule workdir ⇒ `Err(AppError::Git(m))` with `m.contains("no .git link")`; `keepme.txt` is untouched; the row is still `Uninitialized` |
| `reconnect_refuses_url_mismatch` | wedge, then rewrite the module's `remote.origin.url` (or the local `submodule.<name>.url`) to a different path ⇒ `Err(AppError::Git(m))` containing both urls; nothing written (no `<sub>/.git`) |
| `reconnect_tolerates_url_cosmetic_difference` | wedge, then set the configured url to `<same>.git/` ⇒ `Ok` (proves `urls_equivalent` normalization) |
| `update_refuses_to_clobber_dirty_submodule` (**existing, must still pass unmodified**) | the `recreate_missing` addition did not weaken the SAFE-checkout invariant |
| `failed_fresh_clone_rolls_back` | on a fresh superproject clone (submodule registered, never cloned) with a dead url ⇒ `update_submodule` errs, and afterwards: no `.git/modules/<key>`, the workdir dir is absent-or-empty exactly as before, `submodule.<name>.url` is absent from local config ⇒ a retry with a good url succeeds (mirrors `add_submodule_rolls_back_on_clone_failure`) |
| `reconnect_after_deinit_reinitializes` | `deinit_submodule` (which keeps `.git/modules`) followed by `update_submodule` succeeds — the real-world path that produced the bug |

### 8.2 In-file `#[cfg(test)] mod tests` in `crates/bonsai-core/src/git/submodule.rs`

| Test | Must prove |
|---|---|
| `urls_equivalent_table` | `.git`/trailing-`/` insensitivity (both orders, repeated), case-insensitive accept, and clear NON-matches (different host, different path, `%20` vs literal space are NOT equal) |
| `workdir_is_empty_table` | absent ⇒ true; empty dir ⇒ true; dir of empty dirs ⇒ true; dir containing a file ⇒ false; dir containing `.git` (file or dir) ⇒ false; a plain file at the path ⇒ false |
| `rel_path_table` | pure component-diff cases incl. sibling/nested/identical, forward slashes only, no `\\?\` leakage |
| `reattach_rejects_hostile_name_before_touching_disk` | `reattach_module_gitdir` with `../../escape` ⇒ `Err(Git("...unsafe name"))` and no file created (traversal guard, mirrors `remove_submodule_rejects_hostile_name_before_running_git`) |
| `module_gitdir_prefers_name_over_path` | with both `<modules>/<name>` and `<modules>/<path>` present (name != path), the returned dir is the `name` one (OPEN-1) |
| `module_gitdir_rejects_escaping_key` | a `..`-bearing name/path yields `None`, never a path outside `<commondir>/modules` |
| `remove_cached_git_dir_uses_commondir` | in a plain repo `commondir() == path()` (behaviour unchanged); in a linked worktree (`git worktree add`, or `Repository::open` on the worktree dir) the resolved root is `<commondir>/modules`, i.e. the dir is actually deleted (§6) |
| `salvage_is_not_applicable_for_healthy_and_virgin_submodules` | an already-checked-out submodule and a registered-but-never-cloned one both return `Salvage::NotApplicable` with zero disk writes |

### 8.3 `src-tauri/src/commands/tests_config_worktree_submodule.rs`

Extend `submodule_add_lifecycle_over_file_url` (`:226`) with a **wedge-and-repair leg** inserted
between the idempotent `update` (`:260`) and the `deinit` (`:262`): wedge the submodule (delete the
gitlink + workdir files, keep `.git/modules`), assert `list_submodules_inner` reports
`uninitialized`, then `block_on(update_submodule_inner(&state, &id, name.clone()))` succeeds and
the row is `upToDate` with the file content restored. This proves the fix through the real Tauri
command wrapper (`spawn_blocking` + `repo_path`), not just the core function.

### 8.4 Frontend

No new test is required by this contract (no wire change). Bug 1's UI change is covered by the
P73 UI contract; if the mock gains a `wedged` fixture row it must be a pure fixture addition to
`src/ipc/fixtures/` + the existing `src/ipc/mock/handlers/submodules.ts` `updateSubmodule`
handler flipping it to `upToDate` — **no new IPC method**, so `VITE_MOCK_IPC=1` keeps running the
whole feature in a plain browser.

---

## 9. Acceptance criteria (mechanically verifiable)

**Fixture recipe (used by 9.2–9.6).** With the `git` CLI + `build_sub` / `build_super_with_sub`
over a `file://` remote:
1. `let (sub, url, _v1, v2) = build_sub(); let dir = build_super_with_sub(&url);` — the submodule is
   cloned and checked out at the pinned `v2`.
2. Write a sentinel: `<super>/.git/modules/<SUB_PATH>/bonsai-sentinel` containing `"keep me"`.
3. Wedge: `remove_file(<super>/<SUB_PATH>/.git)` then delete every remaining entry under
   `<super>/<SUB_PATH>` (leave the directory itself present and empty).
4. Assert the wedge is real: `only(super).status == Uninitialized`, `cli_status_char(super) == '-'`,
   and (pre-fix) `update_submodule` errs with `attempt to reinitialize`.

| # | Criterion |
|---|---|
| 1 | `cargo check`, `cargo clippy -D warnings` clean; `tsc` / `pnpm build` clean (no frontend change required by this contract). |
| 2 | **Reconnect works.** On the wedged fixture, `update_submodule(super, SUB_PATH)` returns `Ok`; `<super>/<SUB_PATH>/lib.txt` contains the `v2` content; `only()` is `UpToDate` with `wt_oid == index_oid == v2`; `cli_status_char == ' '`; `git -C <super>/<SUB_PATH> rev-parse HEAD == v2`. |
| 3 | **Reuse, not re-clone.** `<super>/.git/modules/<SUB_PATH>/bonsai-sentinel` still exists with its original content after criterion 2. |
| 4 | **Offline reconnect.** Repeat the fixture, delete the upstream source dir (`sub`) so the `file://` url is dead, then wedge and `update_submodule` ⇒ still `Ok`, still repopulated (proves no network I/O and no credential use on the salvage path). |
| 5 | **Relative gitlink.** After criterion 2, `<super>/<SUB_PATH>/.git` is a regular FILE matching `^gitdir: \.\.[^\n]*\n$` — relative, forward slashes only, no `\\?\`, no absolute drive letter; and `git -C <super>/<SUB_PATH> rev-parse --git-dir` resolves inside `<super>/.git/modules`. |
| 6 | **Refusal A — non-empty workdir.** Wedge, then create `<super>/<SUB_PATH>/keepme.txt` ⇒ `update_submodule` returns `Err(AppError::Git)` whose message names the path and mentions `no .git link`; `keepme.txt` is byte-identical afterwards; no `<SUB_PATH>/.git` was created; the row is still `uninitialized`. |
| 7 | **Refusal B — url mismatch.** Wedge, then `git -C <super>/.git/modules/<SUB_PATH> remote set-url origin <other-file-url>` ⇒ `Err(AppError::Git)` quoting BOTH urls; no `<SUB_PATH>/.git` created; the module gitdir untouched. |
| 8 | **Rollback on a failed fresh clone.** A registered-but-never-cloned submodule with a dead url: after `update_submodule` errs, `<super>/.git/modules/<key>` does not exist, the workdir path is absent-or-empty, `git -C <super> config --local --get submodule.<name>.url` is absent, and a retry against a good url succeeds. |
| 9 | **Traversal guard.** `reattach_module_gitdir` with name `../../escape` errs with `unsafe name` and creates/deletes nothing outside the repo; the existing `remove_submodule_rejects_hostile_name_before_running_git` test still passes. |
| 10 | **Non-force invariant intact.** `crates/bonsai-core/tests/submodule_cli_2.rs::update_refuses_to_clobber_dirty_submodule` still passes **unmodified**. |
| 11 | **Commondir fix.** In a linked worktree (`git worktree add`), `remove_submodule` / `rollback_partial_update` actually delete `<commondir>/modules/<key>` (previously a silent no-op); in a plain repo behaviour is unchanged. |
| 12 | **No surface drift.** `git diff` shows no change to `SubmoduleInfo`/`SubmoduleStatus`, `src/ipc/types.ts`, `src/ipc/tauri.ts`, the `generate_handler!` list, or `status.rs`; the command count is unchanged. |
| 13 | Full suites green: workspace `cargo test`, `vitest`, and the e2e suite at the counts recorded in `TODO.md` (no regressions). |
| 14 | **USER CHECKPOINT (native).** On the real superproject `D:\Repos\ham-digi-backend`: the wedged submodule row's Update (and the new Init = init+checkout) completes with no "attempt to reinitialize", the badge becomes "up to date", the files are on disk, and `git submodule status` prints a leading space. No credential prompt appears. |

---

## 10. REJECTED alternative (recorded, with the acceptable future use)

**Rejected: shell out `git submodule update --init -- <path>` via the existing `&dyn GitRunner`.**
- `GitRunner::run` collapses every failure into `AppError::Git(<stderr tail>)`, so we lose the
  `authFailed` / `networkError` mapping that `map_remote_err` gives the frontend today.
- `GIT_TERMINAL_PROMPT=0` turns a stale Azure token into an opaque non-zero exit code with no
  actionable message.
- Every update test would need `require_git!()` gating, so update loses its git-less coverage.
- It buys nothing: the wedged case needs **no network at all** (§3), which is precisely the case the
  in-process fix handles best.

**Acceptable future use:** an explicit, user-triggered "Repair with git CLI" action behind the same
`&dyn GitRunner` seam, with a pure argv builder alongside `deinit_args` / `rm_args`:

```rust
/// Pure argv for `git submodule update --init -- <path>`. `path` is ALWAYS the
/// final token, after `--`, exactly like `deinit_args`.
fn update_init_args(path: &str) -> Vec<String> {
    vec!["submodule".into(), "update".into(), "--init".into(), "--".into(), path.into()]
}
```

Not implemented in P73.

---

## 11. OPEN items (each has a DEFAULT — implementation is never blocked)

1. **`name` vs `path` key under `.git/modules`.** libgit2's clone keys the repodir on `sm->path`
   (`submodule.c:1329`); Bonsai's `remove_cached_git_dir` keys on `name`. They diverge for a renamed
   submodule. → **DEFAULT: probe both, `name` first** (git's canonical key), `path` second; if both
   exist, **`name` wins** and the `path` one is left alone (never deleted outside rollback).
   *Alternative: prefer `path` to match libgit2's writer — rejected: git's own reader uses `name`,
   and a `path`-keyed dir only appears for repos libgit2 itself created.*
2. **Orphaned gitdir with no `origin` remote.** → **DEFAULT: REFUSE** (taxonomy #8) — we cannot
   prove ownership, and reattaching the wrong gitdir would look like data corruption.
   *Alternative: accept when the gitdir's HEAD commit equals `sm.index_id()` — strictly safer proof
   but a rarer state; add only if a user hits the refusal.*
3. **`urls_equivalent` case sensitivity.** → **DEFAULT: two-tier** — exact byte match after
   normalization (trim, strip trailing `/` and one trailing `.git`, repeated) ⇒ accept;
   ASCII-case-insensitive match ⇒ accept + `eprintln!` note; else refuse. No percent-decoding
   (`%20` stays `%20`), no scheme/host canonicalization. Rationale: the URL guard is an *ownership
   heuristic*, not a security boundary — the security boundary is the name/containment guard in
   steps 1 and 5 — so being lenient here only risks reattaching a gitdir the user themselves
   configured, while being strict risks blocking the real repair on a `HTTPS://`/host-case
   difference.
4. **What counts as an "empty" workdir.** The reported wedge may have left empty subdirectories
   behind. → **DEFAULT: recursive emptiness** — no regular file, no symlink, no `.git` entry at any
   depth, bounded by `EMPTY_SCAN_LIMIT = 4096` visited entries (over the limit ⇒ treat as
   non-empty). *Alternative (simpler, stricter): `read_dir().next().is_none()` — rejected because it
   would make the fix silently not fire on exactly the real-world repo we are fixing.*
5. **Gitlink when the module dir and the submodule worktree do NOT share the superproject root**
   (only reachable via an exotic `commondir`, e.g. a `.git` file pointing elsewhere). → **DEFAULT:
   write an ABSOLUTE `gitdir:`/`core.worktree`**, built from the un-canonicalized (already absolute)
   inputs with any Windows `\\?\` verbatim prefix stripped. *Alternative: refuse — rejected as
   needlessly restrictive; git accepts absolute gitlinks.*
6. **`AppError` variant for I/O failures in `write_gitlink`.** → **DEFAULT: whatever `error.rs`
   already exposes for I/O; if there is no `Io` variant, use `AppError::Git` with the path in the
   message.** No new variant either way (§1).
7. **Retry semantics after a salvage-path checkout failure.** → **DEFAULT: no rollback** — leave the
   freshly written gitlink in place (it is correct, and a retry then works) and never delete a
   pre-existing module gitdir. Only the `NotApplicable` (fresh-clone) path rolls back (§5).
8. **Telemetry / user-visible signal that a gitdir was reused.** → **DEFAULT: `eprintln!` only**
   (step 10). No wire change, no extra toast — the existing "Updated `<name>`" toast is accurate.
9. **Whether `list_submodules` should surface "wedged" as a distinct badge state.** → **DEFAULT:
   NO** — it stays `uninitialized` (git agrees, `-`), and Update now repairs it. Adding a state
   would be a wire change, which §1 forbids.

---

## 12. File touch list

- `crates/bonsai-core/src/git/submodule.rs` — `Salvage`, `module_gitdir`, `workdir_is_empty`,
  `urls_equivalent`, `write_gitlink` (+ its `rel_path` / `strip_verbatim` / `write_atomic` private
  helpers), `reattach_module_gitdir`, `PreUpdateState`, `snapshot_pre_update`,
  `rollback_partial_update`, `EMPTY_SCAN_LIMIT`; rewritten `update_submodule` body; one-line
  `remove_cached_git_dir` commondir fix; new `#[cfg(test)]` cases (§8.2).
  **Watch the ~500-line limit:** this file is already ~680 lines. Split in the SAME increment —
  move the P73 reconnect machinery into a new sibling module
  `crates/bonsai-core/src/git/submodule_reconnect.rs` (declared `mod submodule_reconnect;` from
  `submodule.rs`, items `pub(super)`), keeping `update_submodule` and the P19 surface in
  `submodule.rs`. The `#[cfg(test)]` unit tests for the new helpers move with them. Also consider
  moving the existing `#[cfg(test)] mod tests` block to `submodule_tests.rs` (`#[cfg(test)] mod
  submodule_tests;`) if the file still exceeds the limit; update
  `scripts/file-size-baseline.json` if it tracks these paths.
- `crates/bonsai-core/tests/submodule_cli_2.rs` — `wedge()` helper + the §8.1 tests.
- `src-tauri/src/commands/tests_config_worktree_submodule.rs` — wedge-and-repair leg in
  `submodule_add_lifecycle_over_file_url` (§8.3).
- **NOT touched:** `src-tauri/src/commands/*` (non-test), `src-tauri/src/lib.rs`,
  `crates/bonsai-core/src/git/status.rs`, `crates/bonsai-core/src/error.rs`, `src/ipc/types.ts`,
  `src/ipc/tauri.ts`, `src/ipc/mock/handlers/submodules.ts` (unless §8.4's optional fixture is
  taken). UI changes for Bug 1 live in `docs/contracts/P73-ui.md`.

---

## 13. Sub-increments (each one fresh-context senior-dev pass)

- **P73a** — core reconnect: the new module/helpers + `reattach_module_gitdir` + the
  `update_submodule` salvage wiring (`recreate_missing` on the salvage path only) + the
  `remove_cached_git_dir` commondir fix + the §8.2 unit tests + the file split (§12).
  *Acceptance: criteria 1, 2, 3, 4, 5, 9, 10, 11, 12.*
- **P73b** — `PreUpdateState` / `snapshot_pre_update` / `rollback_partial_update` + the clone-path
  wiring. *Acceptance: criterion 8.*
- **P73c** — UI: Init = init + checkout (per `docs/contracts/P73-ui.md`).
- **P73d** — tests: §8.1 + §8.3. *Acceptance: criteria 6, 7, 13.*
