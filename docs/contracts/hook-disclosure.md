# Contract — first-time per-repo git-hook execution disclosure

`hooks_enabled` defaults **true** when `bonsai.runHooks` is unset (`hooks.rs:117-122`), so opening a
pre-existing on-disk repo and committing/merging/pushing silently runs arbitrary `.git/hooks/*` code.
A GUI user's mental model differs from a CLI user's. Add a **one-time, per-repo** disclosure that
fires the FIRST time Bonsai is about to run any hook in a repo.

## Decisions

- **Block-until-acknowledged** (recommended). The security intent is to inform BEFORE arbitrary code
  runs; disclose-and-proceed would run the hook before the user sees the notice. The operation does
  not proceed until the user confirms; declining cancels it.
- **Once per repo, not per op-type.** A single ack covers commit / amend / merge-commit / push.
- **Frontend gate, backend detection + persistence.** Unlike the novel-content gate (attacker-controlled
  content → needs backend write enforcement), this is a user-consent UX gate over the user's OWN repo;
  hooks do nothing git itself would not do. The backend supplies detection + a durable per-repo flag;
  the frontend owns the gate. No backend refusal of the commit is added.

## Detection (Rust) — `crates/bonsai-core/src/git/hooks.rs`

Extract the hook-dir resolution out of `plan_hook` (behaviour-preserving) so detection and execution
share ONE discovery path (module ethos A-D1 — discovery is git's job, single-sourced):

```rust
/// The on-disk path git would consult for `hook` (core.hooksPath if set & non-empty,
/// else <commondir>/hooks/<name>), or None if introspection fails. Does NOT check
/// existence. Used by both plan_hook and detection.
fn hook_file_path(workdir: &Path, hook: HookName) -> Option<PathBuf>;

/// True iff the repo has ≥1 hook Bonsai would actually run — for detection/disclosure
/// only. Precise (unlike plan_hook, which over-runs harmlessly under --ignore-missing):
///   present AND, on unix, executable (mode & 0o111 != 0); on windows, present is enough
///   (git is shebang-driven, no exec bit). Checks PreCommit, CommitMsg, PostCommit, PrePush.
pub fn repo_has_runnable_hooks(workdir: &Path) -> bool;
```

`plan_hook` is refactored to `hook_file_path(...).map_or(Run, |p| if p.is_file(){Run}else{Skip})`
plus the introspection-failure `Run` fallback (unchanged semantics: a wrong "run" is harmless).
`repo_has_runnable_hooks` adds the unix exec-bit check so we disclose only for repos that *really*
have runnable hooks. Blocking (git2 + fs) → callers use `spawn_blocking`.

## Persistence (Rust) — `src-tauri/src/settings.rs`

Mirror `repo_forge_overrides` (per-repo, keyed by canonical workdir path):

```rust
// on Settings (additive #[serde(default)]; pre-existing file loads []; NO version bump):
/// Repos whose git-hook disclosure the user has acknowledged (canonical workdir paths).
pub hooks_ack_repos: Vec<String>,
```
Helpers beside the forge-override ones:
```rust
pub fn hooks_ack_contains(s: &Settings, repo_path: &str) -> bool; // dedupe via commands::same_repo_path
pub fn set_hooks_ack(s: &mut Settings, repo_path: &str);          // idempotent push if absent
```
Persist through the existing atomic `settings::update(&file, |s| set_hooks_ack(s, &workdir_str))`.
Survives restart; per-repo.

## IPC surface

### Rust commands — `src-tauri/src/commands/merge.rs` (near the conflict/hook code) or a new `commands/hooks.rs`
```rust
#[tauri::command]
pub async fn get_repo_hooks_disclosure(
    app: tauri::AppHandle, state: tauri::State<'_, AppState>, repo_id: String,
) -> Result<RepoHooksDisclosure, AppError>;

#[tauri::command]
pub async fn ack_repo_hooks(
    app: tauri::AppHandle, state: tauri::State<'_, AppState>, repo_id: String,
) -> Result<(), AppError>;
```
Plus runtime-free `_inner(state, settings_file, repo_id)` cores (mirror `forge_set_repo_account`).

```rust
#[derive(serde::Serialize)] #[serde(rename_all = "camelCase")]
pub struct RepoHooksDisclosure {
    pub has_hooks: bool,      // repo_has_runnable_hooks(workdir) — under spawn_blocking
    pub acknowledged: bool,   // hooks_ack_contains(&settings, canonical_workdir)
}
```
`get_…` resolves `workdir = repo_path(state, repo_id)?`, computes both fields. `ack_…` calls
`settings::update`. Register both in `lib.rs`. Errors: `noRepo | git | other`.

### TypeScript — `src/ipc/types/ipc-api.ts` + `src/ipc/types/*`
```ts
export interface RepoHooksDisclosure { hasHooks: boolean; acknowledged: boolean }

/** Whether this repo has runnable git hooks and whether the user has been shown the
 *  one-time disclosure. Rejects noRepo | git. */
getRepoHooksDisclosure(repoId: string): Promise<RepoHooksDisclosure>;
/** Record that the user acknowledged this repo's hook disclosure (persisted, per-repo). */
ackRepoHooks(repoId: string): Promise<void>;
```
Wire in `src/ipc/tauri/merge.ts` (or a `tauri/hooks.ts`).

## Frontend wiring — single choke point, no double-prompt

All hook-bearing ops (commit / amend / merge-commit / push) already funnel through
`runWithHookGate` (`useHookGate.ts`, injected into `useCommitActions`, `useMergeActions`,
`useRemoteOps`). Put the disclosure at the TOP of that one gate.

### New hook `src/components/repoWorkspace/useHookDisclosure.ts`
Owns the disclosure dialog state + a per-session cache:
```ts
export interface HookDisclosure {
  pendingHookDisclosure: boolean;                 // drives the ConfirmDialog open state
  ensureHooksDisclosed(skipHooks: boolean): Promise<boolean>; // true ⇒ proceed
  onHookDiscloseConfirm(): void;
  onHookDiscloseCancel(): void;
}
export function useHookDisclosure(repoId: string): HookDisclosure;
```
`ensureHooksDisclosed(skipHooks)`:
1. `if (skipHooks) return true;` — skip-hooks runs no hook, so no disclosure.
2. `if (disclosedThisSession.current) return true;` — in-memory ref; also makes commit&push a single
   prompt (push's gate sees the cache set by the commit gate).
3. `const d = await ipc.getRepoHooksDisclosure(repoId);`
4. `if (!d.hasHooks || d.acknowledged) { disclosedThisSession.current = true; return true; }`
5. open the ConfirmDialog, await; **confirm** → `await ipc.ackRepoHooks(repoId)`,
   set cache, resolve `true`; **cancel/Esc** → resolve `false`.

### `useHookGate.ts`
Take one new dep `ensureHooksDisclosed: (skipHooks: boolean) => Promise<boolean>`. At the top of
`runWithHookGate`, before `attempt`:
```ts
if (!(await ensureHooksDisclosed(skipHooks))) throw COMMIT_HOOK_CANCELED;
```
`COMMIT_HOOK_CANCELED` is the existing sentinel every caller already treats as a silent cancel
(keeps the typed message, no error banner). Every existing call site is unchanged — the disclosure is
free at all four. Sequence for a hook repo: disclose → confirm → attempt runs hooks → (if a hook
rejects) the existing HookOutputDialog. Not a double-prompt (different purposes, different moments).

### Dialog — reuse `ConfirmDialog` (`src/components/ConfirmDialog.tsx`), rendered in `WorkspaceDialogs`
- `confirmVariant="primary"` (non-destructive). Initial focus stays on Cancel (deliberate: the user
  reads before proceeding), matching the security intent.
- Copy:
  - title: **"This repository defines git hooks"**
  - body: "Committing, merging, or pushing in this repository will run its git hooks
    (`.git/hooks`). This is standard git behavior, but the scripts can run arbitrary code on your
    machine. Bonsai shows this once per repository. To disable hooks, set `bonsai.runHooks=false`
    in this repo's git config, or use the **Skip hooks** toggle per operation."
  - confirmLabel: **"Run hooks"**

## Mock IPC (`VITE_MOCK_IPC=1`)

- `src/ipc/mock/repoState.ts`: add `hooksAcked: boolean` (default false) and derive `hasHooks` from a
  fixture flag / `?hooks=present` URL seam (default: fixtures have NO hooks → never disclosed).
- New mock handlers (in `merge.ts` or a `hooks.ts`):
  - `getRepoHooksDisclosure(repoId)` → `{ hasHooks, acknowledged: state.hooksAcked }`.
  - `ackRepoHooks(repoId)` → sets `state.hooksAcked = true`.
- `?hooks=present` drives the disclosed-once-then-quiet path in the browser harness.

## Acceptance criteria

- Repo with **no** runnable hooks → `getRepoHooksDisclosure.hasHooks === false` → never disclosed;
  no dialog on any op.
- Repo **with** hooks, first commit/merge/push → disclosed exactly once. Confirm → op proceeds AND
  `ackRepoHooks` persists.
- After ack, subsequent ops (same session and after restart) → `acknowledged === true` → no dialog.
- **Decline cancels** the operation (throws `COMMIT_HOOK_CANCELED`; nothing committed/pushed; typed
  message kept; no error banner).
- **Skip hooks** bypasses BOTH the disclosure and any hook-rejection dialog (no hook runs).
- commit&push discloses **once** (session cache), never twice.
- unix: a present-but-non-executable hook does NOT trigger disclosure (matches git skipping it).

## Test list

Rust:
- `repo_has_runnable_hooks_*`: none → false; executable pre-commit → true; (unix) non-exec present →
  false; `core.hooksPath` honored; PrePush-only repo → true.
- `hooks_ack_roundtrip`: `set_hooks_ack` idempotent; `hooks_ack_contains` dedupes via `same_repo_path`;
  survives `save_to`/`load_from`.
- command cores: `get_repo_hooks_disclosure_inner` reports both fields; `ack_repo_hooks_inner`
  persists; both `NoRepo` for unknown id.
- Additive-load: a pre-existing settings.json without `hooksAckRepos` loads `[]`.

Frontend (`useHookDisclosure` / `useHookGate` tests):
- `hasHooks:false` → `ensureHooksDisclosed` returns true, no dialog, no `ackRepoHooks`.
- `hasHooks:true, acknowledged:false` → dialog; confirm → true + `ackRepoHooks` called; cancel → false.
- `runWithHookGate` throws `COMMIT_HOOK_CANCELED` on decline; runs `attempt` on confirm.
- `skipHooks:true` → no `getRepoHooksDisclosure` call.
- second op in the same session → no second `getRepoHooksDisclosure` (cache).

## Flags for the orchestrator

- **Re-disclose if the hook set changes after ack** (e.g. a later pull adds a hook): v1 is
  once-per-repo per the task, so a newly-added hook after ack is NOT re-disclosed. Recommend deferring;
  flag if you want a content/hash-based re-trigger.
- **Where to host the commands:** recommend a small new `commands/hooks.rs` (SRP) over growing
  `merge.rs`; either is fine — pick one.
- **Detection timing:** lazy on first hook-bearing op (spec above) avoids slowing repo-open. If you
  prefer eager detection at open, stash `hasHooks` on the repo session — flagged, not recommended.
