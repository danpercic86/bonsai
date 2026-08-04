# P37 — Force-push with lease

Safe force-push (`git push --force-with-lease` equivalent) so a rewritten history
(interactive rebase P23, amend P20) can be published without clobbering a teammate's
work. A bare force-push is NEVER offered. The lease refuses when the remote branch
advanced past the oid the client last fetched. Confirm-gated in the UI **and**
guarded server-side (the lease check IS the guard).

---

## 0. State of the world (investigated — implementers rely on this)

- **Push authenticates through libgit2, not a `git push` subprocess.** `push_current`
  (`crates/bonsai-core/src/git/remote.rs:528`) calls `git2::Remote::push` with a
  `RemoteCallbacks` whose `credentials` closure is `acquire_cred(...)`. The recent
  credential rework (4e4b8f4, P35) lives *inside* that callback: `acquire_cred` →
  `cred_cache::resolve` → `credential_fill` (real `git credential fill`). So libgit2
  owns the transport; real-git only supplies the username/password.
- `tags.rs::push_tag` (`crates/bonsai-core/src/git/tags.rs:114`) already builds a
  `+`-prefixed force refspec through the *same* `acquire_cred` / `map_remote_err`
  machinery — the direct precedent for P37.
- `map_remote_err` (remote.rs:300) already maps `NotFastForward` → `PushRejected`,
  `Auth` → `AuthFailed`, Net/Http/Ssh → `NetworkError`.
- git2 = **0.21.0** (libgit2-sys 0.18.7+1.9.6), verified in `Cargo.lock`.
- `PushResult` enum (remote.rs:59) `{ UpToDate | Pushed{set_upstream} }` is reused as-is.
- `ConfirmDialog` (`src/components/ConfirmDialog.tsx`) supports `confirmVariant: 'danger'`.
- The current-HEAD **local branch has no context menu** (`workspaceMenus.ts:158`
  returns `[]` when `isHead`), so a "force-push the current branch" action must attach
  to the **toolbar Push control**, not a branch context menu.

---

## 1. Invariants held

- Rust owns all git logic; `force_push_with_lease` takes `&Path`, returns `AppError` —
  no Tauri types → CLI-testable without the `test` feature.
- Reuses the existing credential path verbatim (`acquire_cred` / `CredAttempts` /
  `cred_cache`) — no new auth code, no `git push` subprocess.
- IPC = one compact request/response command. **No new events, no channels.**
- git2 is blocking → the command wraps in `spawn_blocking`.
- Destructive op → UI confirm **and** server-side safety (the lease check).
- No new `AppError` variant (§Decision 3).

---

## 2. Design decisions (recommended defaults — implementation NOT blocked)

**D1 — Implementation approach: libgit2 `Remote::push` with a `+` refspec + a manual
ls-remote lease pre-check. RECOMMENDED.**
Approach (b), shelling out to `git push --force-with-lease`, would need an entirely
separate auth path and could not reuse the in-process credential cache — it throws
away the P35 rework. Approach (a) reuses `push_current`'s machinery wholesale and
matches the `push_tag` force precedent. So (a).
*Mechanics of the lease (libgit2 has no native force-with-lease):* before pushing,
`connect_auth(Direction::Push)` + `Remote::list()` (ls-remote) to read the remote's
**live** oid for `refs/heads/<remote_branch>`; compare it to the **expected** oid
(the remote-tracking ref we last fetched). Only if they match do we force-push.

> ⚠ **FLAG for orchestrator (known limitation, not blocking):** libgit2's push refspec
> API cannot send the expected-old-oid to the server, so this is a **client-side**
> compare-and-swap: a tiny TOCTOU window exists between our ls-remote and our push.
> This is strictly safer than a bare force-push (it catches the common "teammate
> pushed" case) but is not the atomic server-side CAS that `git --force-with-lease`
> gets on protocol-v2 servers. Document in the confirm-dialog/help text. Acceptable
> for v1; revisit if libgit2 gains CAS refspecs.

**D2 — Lease baseline oid: the backend derives it from the remote-tracking ref
`refs/remotes/<remote>/<branch>`; the frontend passes NO oid. RECOMMENDED.**
This mirrors `git push --force-with-lease` with no value (leases against the tracking
ref). Keeps IPC compact and stops the frontend from shipping a stale oid. If **no
remote-tracking ref exists** (branch never fetched) there is no baseline → refuse with
a fetch-first `PushRejected` (matches git's "no valid ref to lease against").

**D3 — Error surface: reuse `PushRejected`; NO new variant. RECOMMENDED.**
A lease failure and an ordinary non-ff rejection resolve the same way (fetch first),
and the frontend force-push handler always appends a "Fetch and retry" hint on any
`pushRejected`, so a discriminator buys nothing. The distinct message text carries the
"remote moved" meaning.

**D4 — UI placement: a split-button dropdown (caret) beside the toolbar Push button;
item "Force-push with lease…". RECOMMENDED** (the current-HEAD branch has no context
menu; force-push targets the current branch). Enabled only when
`canPullPush && headBranch?.upstream != null`. Danger-variant confirm dialog naming
branch + remote and warning it rewrites published history.

> ⚠ **FLAG (minor):** if the user prefers a different affordance (right-click the Push
> button, or an entry in a branch menu for non-HEAD branches), that is a pure P37b
> swap — say so and P37b adjusts. Default is the caret dropdown.

**D5 — Mock: a dedicated `remoteTrigger === 'leasefail'` query flag drives the refuse
path; otherwise force-push succeeds and moves the remote-tracking tip. RECOMMENDED**
(deterministic + browser-harness verifiable via `?remote=leasefail`).

---

## 3. Rust — `crates/bonsai-core/src/git/remote.rs`

### 3.1 New public function

```rust
/// Blocking. Force-push the current branch to its configured upstream WITH A
/// LEASE: refuse if the remote branch moved past the oid we last fetched
/// (someone else pushed), otherwise force-update it. For republishing a
/// rewritten history (amend / interactive rebase). NEVER a bare force.
///
/// Requires a configured upstream (unlike `push_current`, which can create
/// origin/<branch>). Lease baseline = the remote-tracking ref
/// `refs/remotes/<remote>/<branch>` (git's default --force-with-lease). Live
/// remote oid read via connect_auth(Push)+list() (ls-remote) BEFORE any push.
///
/// Errors: unborn/detached/no-name -> `Git`; no upstream -> `NoUpstream`;
/// no remote-tracking baseline -> `PushRejected` (fetch first); remote moved
/// (live != baseline, or remote ref deleted) -> `PushRejected` (lease failed);
/// remote missing -> `NoRemote`; connect/list/push git2 errors -> via
/// `map_remote_err` (`AuthFailed` / `NetworkError` / `PushRejected` / `Git`);
/// server-side ref rejection -> `PushRejected`.
pub fn force_push_with_lease(workdir: &Path) -> Result<PushResult, AppError>;
```

Returns `PushResult::UpToDate { remote, branch }` when the baseline already equals the
local tip (nothing to force), else `PushResult::Pushed { remote, branch, set_upstream:
false }` (upstream is required, never newly set here).

### 3.2 Lease message constants (module-private)

```rust
// Remote advanced (or its ref was deleted) since our last fetch.
const LEASE_MOVED: &str = // format!-ed with remote/branch:
  "force-push refused: '{remote}/{branch}' has moved on the remote since you last \
   fetched — someone may have pushed. Fetch and review before force-pushing again.";
// No remote-tracking ref to lease against.
const LEASE_NO_BASELINE: &str =
  "cannot force-push with lease: no remote-tracking ref for '{remote}/{branch}'. \
   Fetch first so Bonsai knows the remote's current tip.";
```

### 3.3 Algorithm (pseudocode)

```
force_push_with_lease(workdir):
    repo = open_repo_at(workdir)
    head = read_head_info(repo)
    if head.unborn:   Err(Git "cannot force-push: repository has no commits yet")
    if head.detached: Err(Git "cannot force-push: HEAD is detached")
    name = head.branch_name  or Err(Git "cannot force-push: HEAD has no branch name")
    refname = "refs/heads/{name}"

    branch = repo.find_branch(name, Local)
    local_tip = branch.get().target() or Err(Git "branch '{name}' has no target commit")

    # Upstream is REQUIRED (force-with-lease republishes an existing upstream).
    match branch.upstream():
        Ok(_) -> {}
        Err(NotFound) -> Err(NoUpstream "cannot force-push: '{name}' has no upstream; use a normal push")
        Err(e) -> Err(e.into())
    remote_name = repo.branch_upstream_remote(refname)   # utf8, else Git
    remote_ref  = config.get_string("branch.{name}.merge")   # already "refs/heads/<x>"
    remote_branch = remote_ref.strip_prefix("refs/heads/").unwrap_or(remote_ref)

    # --- lease baseline: the remote-tracking ref we last fetched ---
    tracking = "refs/remotes/{remote_name}/{remote_branch}"
    expected = repo.find_reference(tracking).ok().and_then(|r| r.target())
    if expected is None:
        return Err(PushRejected(LEASE_NO_BASELINE))

    if expected == Some(local_tip):
        return Ok(PushResult::UpToDate { remote: remote_name, branch: name })   # nothing to force

    remote = repo.find_remote(remote_name)   # NotFound -> NoRemote

    attempts = RefCell(CredAttempts::default())   # shared across connect + push

    # --- (1) lease check: read the LIVE remote oid via ls-remote ---
    cb1 = RemoteCallbacks; cb1.credentials(|u,ufu,a| acquire_cred(repo.workdir(), &attempts, u, ufu, a))
    remote.connect_auth(Direction::Push, Some(cb1), None)
        .map_err(|e| map_remote_err(e, remote_name))?
    actual = remote.list()?               # &[RemoteHead], valid while connected
                   .iter().find(|h| h.name() == remote_ref).map(|h| h.oid())
    lease_ok = actual == expected         # Some(x)==Some(x); absent remote ref => None != Some => refuse
    if !lease_ok:
        let _ = remote.disconnect()
        return Err(PushRejected(LEASE_MOVED))

    # --- (2) lease holds: force-push over the same connection ---
    rejected: RefCell<Option<String>> = None
    cb2 = RemoteCallbacks
    cb2.credentials(|u,ufu,a| acquire_cred(repo.workdir(), &attempts, u, ufu, a))
    cb2.push_update_reference(|_ref, status| { if let Some(s)=status { *rejected.borrow_mut()=Some(s.into()) } Ok(()) })
    opts = PushOptions; opts.remote_callbacks(cb2)
    refspec = "+{refname}:{remote_ref}"    # leading '+' = force
    remote.push(&[refspec], Some(&mut opts)).map_err(|e| map_remote_err(e, remote_name))?
    let _ = remote.disconnect()

    if let Some(msg) = rejected.into_inner():
        return Err(PushRejected("push rejected by remote: {msg}"))

    return Ok(PushResult::Pushed { remote: remote_name, branch: name, set_upstream: false })
```

git2 0.21 APIs relied on: `Remote::connect_auth(&mut self, Direction, Option<RemoteCallbacks>,
Option<ProxyOptions>)`, `Remote::list(&self) -> Result<&[RemoteHead]>`,
`RemoteHead::name()`/`oid()`, `Remote::disconnect(&mut self)`, `Remote::push(&[&str],
Option<&mut PushOptions>)`, refspec `+` force prefix. The shared `RefCell<CredAttempts>`
is borrowed immutably by both closures (same pattern as elsewhere in this file).

### 3.4 Error mapping table

| Condition | AppError |
|---|---|
| unborn / detached / no branch name / no target | `Git` |
| upstream not configured (`NotFound`) | `NoUpstream` |
| no remote-tracking baseline ref | `PushRejected` (`LEASE_NO_BASELINE`) |
| baseline == local tip | `Ok(UpToDate)` (not an error) |
| remote name not found | `NoRemote` |
| live remote oid != baseline (moved), or remote ref deleted | `PushRejected` (`LEASE_MOVED`) |
| `connect_auth` / `list` / `push` git2 error | `map_remote_err` → `AuthFailed`/`NetworkError`/`PushRejected`/`Git` |
| `push_update_reference` status set | `PushRejected` |

### 3.5 Unit tests (in-module, offline)

- `PushResult` wire shape already covered (remote.rs:778) — no change.
- Add: the two lease-message helpers interpolate `{remote}`/`{branch}` (string assertion).

---

## 4. Command + registration

### 4.1 `src-tauri/src/commands.rs`

```rust
use bonsai_core::git::remote::{ ... , force_push_with_lease};   // add to the existing import

/// Force-push the current branch to its upstream WITH A LEASE (P37). Refuses if
/// the remote moved since the last fetch. Errors: `noUpstream` | `noRemote` |
/// `authFailed` | `networkError` | `pushRejected` | `git` | `noRepo`.
#[tauri::command]
pub async fn force_push(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<PushResult, AppError> {
    force_push_inner(state.inner(), &repo_id).await
}

async fn force_push_inner(state: &AppState, repo_id: &str) -> Result<PushResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || force_push_with_lease(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
```
Add a `force_push_inner` no-repo test mirroring the `push_inner` one at commands.rs:3696.

### 4.2 `src-tauri/src/lib.rs`

Add `commands::force_push,` to `generate_handler!` immediately after `commands::push,`
(lib.rs:70).

---

## 5. IPC triple

### 5.1 `src/ipc/types.ts`

Add to the `BonsaiIpc` interface, next to `push`:

```ts
/** Force-push the current branch to its upstream WITH A LEASE (P37). Refuses
 *  (pushRejected) if the remote moved since the last fetch. Rejects
 *  noUpstream | noRemote | authFailed | networkError | pushRejected | git | noRepo. */
forcePush(repoId: string): Promise<PushResult>;
```
`PushResult` (types.ts:635) is reused unchanged. No new error kind.

### 5.2 `src/ipc/tauri.ts`

```ts
forcePush(repoId: string): Promise<PushResult> {
  return invoke<PushResult>('force_push', { repoId });
}
```

### 5.3 `src/ipc/mock.ts` (stateful)

```ts
async forcePush(repoId: string): Promise<PushResult> {
  await delay(400);
  const state = requireRepo(repoId);
  if (state.remoteTrigger === 'authfail') throwAuthFailed();
  if (state.remoteTrigger === 'network') throwNetworkError();
  if (state.remoteTrigger === 'leasefail') {           // NEW trigger
    const err: AppError = {
      kind: 'pushRejected',
      message:
        "force-push refused: 'origin/" + state.headBranch + "' has moved on the remote " +
        'since you last fetched — someone may have pushed. Fetch and review before ' +
        'force-pushing again.',
    };
    throw err;
  }
  const branch = state.branches.local.find((b) => b.name === state.headBranch);
  if (branch === undefined || branch.upstream === null) {
    const err: AppError = { kind: 'noUpstream', message: 'cannot force-push: no upstream' };
    throw err;
  }
  // Lease held: force-update the remote-tracking tip to the local tip.
  branch.ahead = 0;
  branch.behind = 0;
  const rt = state.branches.remote.find((r) => r.name === branch.upstream);
  if (rt !== undefined) rt.tip = branch.tip;
  return { kind: 'pushed', remote: 'origin', branch: branch.name, setUpstream: false };
}
```
No change to `RepoState` needed — `remoteTrigger` (mock.ts:393) already carries the flag
via the `?remote=` query. Keep `mock.ts` compiling (`forcePush` satisfies the widened
`BonsaiIpc`).

---

## 6. Frontend (P37b)

### 6.1 `src/components/WorkspaceToolbar.tsx`
- Add props `canForcePush: boolean` and `onForcePush(): void`.
- Render a split-button: the existing Push button plus a caret button (`▾`,
  `className="toolbar-btn toolbar-caret"`, `disabled={refreshing || mutating ||
  !canForcePush}`, `aria-label="More push actions"`). Clicking the caret opens a tiny
  menu (reuse the existing `ContextMenu` component or a local `<ul>`), single item:
  **"Force-push with lease…"** → `onForcePush()`. Disabled tooltip when `!canForcePush`:
  "Force-push needs a branch with an upstream."

### 6.2 `src/components/RepoWorkspace.tsx`
- Derive `const canForcePush = canPullPush && (headBranch?.upstream != null);`
- State: `const [pendingForcePush, setPendingForcePush] = useState(false);`
- `function handleForcePush() { setPendingForcePush(true); }`
- ```ts
  async function doForcePush() {
    setPendingForcePush(false);
    beginRemoteOp('push');
    try {
      const res = await ipc.forcePush(repoId);
      if (res.kind === 'upToDate') pushToast('info', 'Already up to date');
      else pushToast('success', `Force-pushed ${res.branch} → ${res.remote}/${res.branch}`);
      await Promise.all([refetchBranches(), refetchGraph()]);
    } catch (e) {
      // Any pushRejected from a force-push resolves the same way: fetch first.
      const hint = isAppError(e) && e.kind === 'pushRejected' ? ' — fetch and retry' : '';
      pushToast('error', errorMessage(e) + hint);
    } finally {
      endRemoteOp();
    }
  }
  ```
- Thread `canForcePush` + `onForcePush={handleForcePush}` into `<WorkspaceToolbar>`.

### 6.3 `src/components/WorkspaceDialogs.tsx`
Add a `ConfirmDialog` (danger variant), wired to `pendingForcePush`:
- title: **"Force-push with lease?"**
- body: “This rewrites the published history of <span className="mono">{headBranch.name}</span>
  on <span className="mono">origin</span>. Bonsai first checks the remote hasn’t moved
  since your last fetch and refuses if someone else pushed. Continue?”
  (Interpolate the actual remote name from `headBranch.upstream`’s prefix when available.)
- confirmLabel: **"Force-push"**, `confirmVariant="danger"`, `busy={remoteOp === 'push'}`.
- `onConfirm={() => void doForcePush()}`, `onCancel={() => setPendingForcePush(false)}`.

No other component changes. Keep every touched file under the 500-line soft limit; the
toolbar caret menu goes in its own small helper if `WorkspaceToolbar.tsx` would bloat.

---

## 7. Sub-increments

- **P37a — backend + command + IPC + mock + CLI oracle.**
  `remote.rs::force_push_with_lease`, module unit test, `commands.rs` (`force_push` +
  `_inner` + no-repo test), `lib.rs` registration, `types.ts`/`tauri.ts`/`mock.ts`
  (incl. `leasefail` handling), and `tests/force_push_cli.rs`. One fresh-context pass.
- **P37b — UI.** `WorkspaceToolbar` split-button, `RepoWorkspace` state/handlers,
  `WorkspaceDialogs` confirm dialog. One fresh-context pass.

---

## 8. CLI-oracle test plan — `crates/bonsai-core/tests/force_push_cli.rs`

Cross-check `force_push_with_lease` against a **local bare origin** driven by the real
`git` CLI. All scratch repos under `D:\Temp\bonsai-scratch` (Windows: set
`TMP`/`TEMP=D:\Temp`). Local `file://`/path remotes need no credentials → hermetic (no
GCM). Guard every test with `have_git()`; build history with git2 or a handful of CLI
commits (not thousands). Reset `credential.helper` to empty on each repo for belt-and-braces.

Fixture helper: `init_origin_and_clone()` → bare `origin.git`, a `work` clone with an
initial commit on `main` pushed to origin (so `refs/remotes/origin/main` exists).

- **A. Lease refuses when the remote moved.** From a *second* clone push a new commit to
  `origin/main` (origin tip = Y). In `work` (remote-tracking still = X, no fetch), rewrite
  `main` to Z (amend/reset). `force_push_with_lease(work)` ⇒ `Err(PushRejected)` whose
  message contains "moved"/"fetch". Assert origin `main` is UNCHANGED (still Y) via
  `git --git-dir=origin.git rev-parse main`.
- **B. Lease succeeds and the ref moves.** No third-party push (origin tip = X =
  remote-tracking). In `work`, amend `main` to Z. `force_push_with_lease` ⇒
  `Ok(Pushed { set_upstream: false })`. Assert origin `main` == Z.
- **C. Up-to-date.** Baseline == local tip ⇒ `Ok(UpToDate)`; origin `main` unchanged; no
  mutation.
- **D. No upstream.** A local branch with no upstream ⇒ `Err(NoUpstream)`.
- **E. No baseline.** Delete `refs/remotes/origin/main` (simulate never-fetched) then
  rewrite + force-push ⇒ `Err(PushRejected)` containing "Fetch first"; origin unchanged.

---

## 9. Acceptance criteria

- `force_push_with_lease` refuses (PushRejected, no remote mutation) when the remote
  advanced since the last fetch, and succeeds (moving the remote ref) when it did not —
  proven by the CLI oracle (A/B), plus up-to-date/no-upstream/no-baseline (C/D/E).
- Reuses the existing credential path; no `git push` subprocess; no new `AppError`
  variant; no new event/channel.
- `force_push` command registered and `spawn_blocking`-wrapped; no-repo test passes.
- `mock.ts` `forcePush` models success (ref/ahead update) and `?remote=leasefail`
  refusal; `tsc`/`build` clean.
- UI: Push split-button exposes "Force-push with lease…", enabled only with an upstream;
  danger confirm names branch+remote and warns about rewriting published history; a
  lease failure toasts with a "fetch and retry" hint.

### AI gate (orchestrator verifies)
- `cargo test -p bonsai-core force_push_cli` + module tests green; `cargo clippy` clean
  (run sequentially, never concurrent with test — target-dir race).
- `tsc --noEmit` + `pnpm build` clean.
- Browser harness (`VITE_MOCK_IPC=1`): Push caret → Force-push confirm → success toast
  and refreshed ahead/behind; then `?remote=leasefail` → error toast with fetch hint.
  Batch the checks; one screenshot for final proof.

### USER CHECKPOINT (native window; user confirms)
- Against a real remote (e.g. GitHub) after an amend / interactive rebase: force-push
  publishes the rewrite; when a teammate has pushed, the lease refuses with a readable
  message; confirm-dialog wording is clear; the fetch-and-retry hint is actionable.
