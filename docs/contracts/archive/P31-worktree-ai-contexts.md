# P31 — Per-Worktree AI Contexts (contract)

Status: contract v1. Composes P24 (context profiles, `crates/bonsai-core/src/assets/`) with
P27 (worktrees, `crates/bonsai-core/src/git/worktree.rs`). Rust owns all logic; React renders.
No git mutations anywhere in this milestone — the only writes are instruction files inside a
worktree root plus the shared `.bonsai/profiles.json`.

Prior contracts: `docs/contracts/P24-ai-context-profiles.md`, `docs/contracts/P27-worktrees.md`.

---

## 1. Decisions (resolved defaults — flag to orchestrator if any should change)

| # | Decision | Choice | Rationale |
|---|---|---|---|
| D1 | Where the profile store lives | ALWAYS the MAIN worktree's `<main>/.bonsai/profiles.json`. Profiles are defined once per repo and shared by all worktrees. | Linked worktrees must see the same profile set; `.bonsai/` in a linked worktree would fork the store. |
| D2 | Store resolution rule | `resolve_store_root(workdir)`: open repo; if `repo.is_worktree()` → `commondir().parent()` (same derivation as `worktree.rs::main_workdir`), else `repo.workdir()`. If the dir is not a git repo (P24 today works on any folder), fall back to `workdir` itself. | Reuses the proven P27 commondir rule; keeps P24's no-repo behavior. |
| D3 | Worktree identity key in the store | The git worktree NAME (`Worktree::name()`), NOT the abs path. The main worktree uses the reserved key `"@main"`. | Names are stable across `git worktree move` and machine-to-machine path differences; the main row's basename is rename-fragile, and `@main` cannot collide (`@` is rejected by `sanitize_slug`, so no linked worktree can be named `@main`). |
| D4 | Schema evolution | `version: 1 → 2` by adding one `#[serde(default)]` map field. v1 files keep loading unchanged (empty map). Version `2` is stamped only on the next `persist()` — reads never rewrite the file. Legacy `activeProfile` is kept and mirrors the `"@main"` entry (written on every main-worktree activation) so P24-era UI/tests stay correct. | Zero-migration, byte-safe for untouched stores; no sidecar file to keep in sync. |
| D5 | Existing `activate_profile(workdir, name)` | Becomes a thin wrapper: it resolves the calling workdir's worktree identity and delegates to the per-worktree core, so a worktree opened as its own tab records its activation in the shared map automatically. Wire shape of `ProfileActivation` is unchanged (its `store` now carries the map). | One write path (safety), and the P27 tab flow gets P31 behavior for free. |
| D6 | Activation eligibility | Refuse when the target worktree is `!valid`, `prunable`, or `locked`. Locked = "pinned, do not touch" per P27 semantics; invalid/prunable = working dir missing/stale (nothing sane to write into). Preview is allowed only for eligible worktrees (it reads files from the working dir). | Mirrors `remove_worktree`'s refusal ladder; writing into a locked worktree violates the user's explicit pin. |
| D7 | Dirty-target guard | BLOCK (not warn) when any profile-target file is TRACKED and modified (index or worktree status ≠ CURRENT) in the target worktree → `AppError::Git("… has uncommitted changes to <path>; commit or stash first")`. UNTRACKED target files do NOT block: they are typically prior activations never committed; the preview diff still shows exactly what is replaced. Whole-worktree dirtiness elsewhere never blocks. | Overwriting uncommitted human edits is unrecoverable (no git safety net); untracked-but-previewed overwrites are consciously confirmed. |
| D8 | UI surface | (a) Worktrees sidebar menu gains "AI context…" → new `WorktreeContextDialog` (matrix: worktree × {active profile, drift chip, Activate…}); (b) `AiAssetsPanel` header gains a "Worktrees" button opening the same dialog; (c) activation reuses `ProfileActivateDialog` extended with an optional `worktreeName` prop. No new panel. | Stays inside the shipped P24d/P27c patterns; one dialog, two entry points, no per-tab opening needed. |
| D9 | P29 health drift rollup | DEFERRED. `StructureSection.driftedCount` keeps meaning the OPEN workdir. `list_worktree_contexts` already returns per-worktree `driftedCount`; wiring it into repo health is a follow-up line item, noted here so P29 is not silently wrong. | Keeps P31 scoped; the data source ships now. |
| D10 | Drift semantics per worktree | Per-worktree drift = P24 `scan_inventory(<that worktree's root>, None)` verbatim (canonical auto-picked per worktree). No cross-worktree canonical in v1. | Different worktrees intentionally diverge — cross-worktree drift is the feature, not a defect. |

---

## 2. Module boundaries & file responsibilities

| File | Responsibility (P31 delta) |
|---|---|
| `crates/bonsai-core/src/assets/profiles.rs` | Schema v2 (`worktree_activations` map), `resolve_store_root`, per-worktree preview/activate cores, dirty-target guard, eligibility guard. All P24 functions keep their signatures. |
| `crates/bonsai-core/src/assets/worktree_context.rs` (NEW) | `list_worktree_contexts`: joins `git::worktree::list_worktrees` × store × per-worktree `scan_inventory` into the status matrix. Blocking, runtime-free, unit-testable. |
| `crates/bonsai-core/src/git/worktree.rs` | Make `main_workdir` + `canonical` `pub(crate)` (reused by assets). No behavior change. |
| `src-tauri/src/commands.rs` | 3 new commands (§5), each `spawn_blocking` + `_inner` pattern like `list_worktrees`. |
| `src/ipc/types.ts` | New wire types + 3 methods on the IPC interface. |
| `src/ipc/tauri.ts` | Thin invoke wrappers. |
| `src/ipc/mock.ts` | Stateful worktree × profile matrix (§6). |
| `src/components/WorktreeContextDialog.tsx` (NEW) | The matrix dialog (§7). |
| `src/components/ProfileActivateDialog.tsx` | Optional `worktreeName` prop → routes preview/activate to the worktree commands. |
| `RepoWorkspace.tsx` / Sidebar worktree menu / `AiAssetsPanel.tsx` | Menu item + header button opening the dialog. |

Rules unchanged from P24: validate-all-before-any-write, atomic same-dir temp+rename
(`atomic_write`), `resolve_single_file_target` + `validate_rel_path` containment (all writes are
`<worktree_root>/<static descriptor path>`), preview-first confirm-gated UI.

---

## 3. Store schema (v2) + migration

```jsonc
// <main>/.bonsai/profiles.json — version 2
{
  "version": 2,
  "profiles": [ { "name": "opus", "description": null, "model": "opus", "targets": [ { "assetId": "claude", "content": "…" } ] } ],
  "activeProfile": "opus",                       // LEGACY mirror of worktreeActivations["@main"]
  "worktreeActivations": {                        // NEW: worktree key -> profile name
    "@main": "opus",
    "feature-login": "haiku"
  }
}
```

Rust:

```rust
pub struct ProfileStore {
    pub version: u32,                                   // persist() now stamps 2
    #[serde(default)] pub profiles: Vec<ContextProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,                 // legacy mirror of "@main"
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub worktree_activations: std::collections::BTreeMap<String, String>, // NEW
}
```

Migration rules:
- Load: v1 files parse with an empty map (serde default). NO write on read (`list_profiles` stays
  read-only). If `active_profile` is Some and `"@main"` absent, `list_worktree_contexts` treats
  `"@main"` as that legacy value at READ time (in-memory only).
- Save: any `persist()` stamps `version = 2` and materializes the `"@main"` mirror
  (`active_profile == worktree_activations.get("@main")` invariant, both directions).
- `delete_profile(name)`: additionally removes every map entry whose value == `name`.
- Byte-safety test: a v1 fixture read via `list_profiles` leaves the file byte-identical.

Key hygiene: map keys are validated on write — `"@main"` or a name for which
`repo.find_worktree(key)` succeeds. Stale keys (worktree since removed) are tolerated on read,
skipped by the matrix, and garbage-collected on the next `persist()`.

---

## 4. Rust core API (all blocking; command layer wraps in `spawn_blocking`)

```rust
// profiles.rs
/// D2. Best-effort: non-repo dirs return `workdir` unchanged.
pub fn resolve_store_root(workdir: &Path) -> PathBuf;

/// D3. "@main" for the main worktree, else the linked worktree name.
/// Err(Git) if `workdir` matches no worktree of the repo.
pub fn worktree_key_for(workdir: &Path) -> Result<String, AppError>;

/// Preview `profile` against WORKTREE `worktree_key`'s files. Store read from the
/// shared root. Enforces D6 eligibility BEFORE reading (locked/invalid/prunable → Git).
/// Writes nothing. Reuses ProfilePreviewEntry unchanged (paths worktree-relative).
pub fn preview_profile_for_worktree(
    workdir: &Path,          // ANY worktree of the repo (the open tab's workdir)
    worktree_key: &str,      // "@main" | linked worktree name
    name: &str,              // profile name
) -> Result<Vec<ProfilePreviewEntry>, AppError>;

/// The ONE write path. Order: resolve store root → find profile → resolve target
/// worktree root (main_workdir / find_worktree(...).path()) → D6 eligibility →
/// validate ALL targets (SingleFile + validate_rel_path) → D7 dirty-target guard
/// over ALL targets → write each (atomic, parent dirs created) → update
/// worktree_activations[key] (+ legacy mirror when key == "@main") → persist.
/// Returns ProfileActivation (unchanged shape; `store` is the v2 store).
pub fn activate_profile_for_worktree(
    workdir: &Path,
    worktree_key: &str,
    name: &str,
) -> Result<ProfileActivation, AppError>;

// Existing fns — signatures unchanged, behavior redirected:
//   list_profiles / save_profile / delete_profile → operate on resolve_store_root(workdir).
//   preview_profile(workdir, name)  → preview_profile_for_worktree(workdir, worktree_key_for(workdir)?, name)
//     (fallback: non-repo dir keeps pure-P24 behavior on `workdir` directly).
//   activate_profile(workdir, name) → activate_profile_for_worktree(likewise).       // D5
```

```rust
// assets/worktree_context.rs (NEW)
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeContextStatus {
    pub worktree_key: String,          // "@main" | linked name (store key + command arg)
    pub name: String,                  // display name (main basename / linked name)
    pub abs_path: String,              // forward slashes
    pub branch: Option<String>,
    pub is_main: bool,
    pub is_current: bool,
    pub locked: bool,
    pub prunable: bool,
    pub valid: bool,
    pub active_profile: Option<String>,// from worktree_activations (v1 legacy folded in)
    pub drifted_count: u32,            // D10: entries comparable && exists && !in_sync
    pub missing_count: u32,            // comparable descriptors with exists == false
    pub activatable: bool,             // D6: valid && !prunable && !locked
    pub blocked_reason: Option<String>,// human string when !activatable
}

/// list_worktrees × shared store × per-worktree scan_inventory (skips scan and
/// reports 0/0 when !activatable — its dir may not exist). One call, no
/// per-worktree round-trips.
pub fn list_worktree_contexts(workdir: &Path) -> Result<Vec<WorktreeContextStatus>, AppError>;
```

Dirty-target guard (D7), internal:

```rust
/// For each mapped target rel-path: open the worktree's repo, take its git
/// status for that path; TRACKED && status != CURRENT → Err(Git(...)).
/// Untracked (WT_NEW) and clean/missing pass. Checked for ALL targets before
/// ANY write. Uses status_file / a pathspec-limited StatusOptions — never a
/// full-repo scan per target.
fn ensure_targets_clean(wt_root: &Path, rels: &[&str]) -> Result<(), AppError>;
```

Error kinds reused: `InvalidName` (bad profile name / non-SingleFile target / bad key),
`Git` (eligibility, dirty target, worktree not found, "profile not found" stays `Other`
matching P24), `Io`, `Other`. No new `AppError` variants.

---

## 5. Tauri commands (request/response only — no new events, no channels; the
existing `repo-changed` fires naturally if the watcher sees instruction-file writes in the
OPEN worktree; other worktrees are refreshed by imperative refetch after activation)

```rust
#[tauri::command] pub async fn list_worktree_contexts(state, repo_id: String)
    -> Result<Vec<WorktreeContextStatus>, AppError>;
#[tauri::command] pub async fn preview_worktree_profile(state, repo_id: String,
    worktree_key: String, name: String) -> Result<Vec<ProfilePreviewEntry>, AppError>;
#[tauri::command] pub async fn activate_worktree_profile(state, repo_id: String,
    worktree_key: String, name: String) -> Result<ProfileActivation, AppError>;
```

Each: `repo_path(state, repo_id)` → `spawn_blocking(core fn)`, `_inner` split, registered in
`generate_handler!`. Existing P24/P27 commands unchanged on the wire.

TypeScript (`src/ipc/types.ts`):

```ts
export interface WorktreeContextStatus {
  worktreeKey: string; name: string; absPath: string; branch: string | null;
  isMain: boolean; isCurrent: boolean; locked: boolean; prunable: boolean; valid: boolean;
  activeProfile: string | null; driftedCount: number; missingCount: number;
  activatable: boolean; blockedReason: string | null;
}
// ProfileStore gains: worktreeActivations?: Record<string, string>;
// IPC interface adds:
listWorktreeContexts(repoId: string): Promise<WorktreeContextStatus[]>;
previewWorktreeProfile(repoId: string, worktreeKey: string, name: string): Promise<ProfilePreviewEntry[]>;
activateWorktreeProfile(repoId: string, worktreeKey: string, name: string): Promise<ProfileActivation>;
```

---

## 6. Mock IPC (`src/ipc/mock.ts`, must compile under `VITE_MOCK_IPC=1`)

State additions per mock repo:
- `worktreeFiles: Map<worktreeKey, Map<relPath, string>>` — instruction-file content per
  worktree (seed: `@main` reuses the existing P24 mock file map; seed 2 linked worktrees from
  the P27 fixtures, one with drifted CLAUDE.md, one missing AGENTS.md; one locked worktree).
- `store.worktreeActivations` seeded (`@main: "opus"`, one linked: `"haiku"`).

Behavior:
- `listWorktreeContexts` derives `driftedCount`/`missingCount` from `worktreeFiles` the same way
  the existing mock recomputes drift per `listAiAssets`; locked/prunable rows report
  `activatable:false` + `blockedReason`.
- `previewWorktreeProfile` diffs profile targets vs that worktree's file map; throws the D6
  error object for the locked fixture.
- `activateWorktreeProfile` mutates THAT worktree's file map + `worktreeActivations`, so a
  re-list flips its drift chips (harness-verifiable), leaves other worktrees untouched.
- Legacy `activateProfile(repoId, name)` in the mock now also writes
  `worktreeActivations` for the tab's own worktree key (mirrors D5).

---

## 7. Frontend

- `WorktreeContextDialog.tsx`: table of `WorktreeContextStatus` rows — name/branch, active-profile
  cell (current value + "Activate…" opening `ProfileActivateDialog` with a profile picker),
  drift chip (`driftedCount`/`missingCount`, same chip styling as AiAssetsPanel), lock/stale
  badge with `blockedReason` tooltip; Activate disabled when `!activatable`. Refetches the
  matrix after every activation.
- `ProfileActivateDialog` gains `worktreeName?: string` (the `worktreeKey`): when set, preview =
  `previewWorktreeProfile`, confirm = `activateWorktreeProfile`; header shows the target
  worktree. The SAFETY GATE is unchanged and non-negotiable: per-target diff preview is shown
  BEFORE any write, and the write happens only on explicit confirm; dirty-target/eligibility
  errors surface as the dialog's error state, nothing written.
- Entry points: Worktrees sidebar menu item "AI context…" (added to `worktreeMenuItems`) and an
  "Worktrees" button in the `AiAssetsPanel`/`ProfileManager` header — both open the same dialog.

---

## 8. Sub-increments

- **P31a — core:** schema v2 + mirror invariant + GC, `resolve_store_root`, `worktree_key_for`,
  `preview/activate_profile_for_worktree`, D5 redirection of the legacy fns,
  `list_worktree_contexts`, `ensure_targets_clean`; full unit + fixture tests (below).
- **P31b — IPC:** 3 commands + `_inner`s, `types.ts`, `tauri.ts`, stateful mock (§6); `tsc` green.
- **P31c — UI:** `WorktreeContextDialog`, `ProfileActivateDialog` extension, menu/header entry
  points. Merge into P31b's pass if the reviewer deems P31b small.

---

## 9. Acceptance criteria

**AI gate** (tests in bonsai-core + fs oracles on scratch repos under `D:\Temp\bonsai-scratch`):
1. Migration: v1 `profiles.json` fixture loads (empty map, legacy `activeProfile` honored as
   `@main`); a pure read leaves the file BYTE-identical; first save stamps `version: 2` +
   materialized mirror; delete_profile clears matching map entries.
2. Resolution: from a real linked worktree (git2-created), `list_profiles`/`save_profile`
   read/write the MAIN worktree's `.bonsai/profiles.json`; no `.bonsai/` appears in the linked
   worktree.
3. Activation: `activate_profile_for_worktree(main_wd, "feature-x", "p")` writes byte-exact
   files inside the LINKED worktree root only — main worktree files untouched (byte-compare),
   no `.bonsai-tmp` remnants; `worktreeActivations["feature-x"]` persisted; second run all
   `unchanged`.
4. D7: tracked+modified target file in the target worktree → `Git` error, ZERO files written
   (all-targets-checked-first proven with a 2-target profile where target #2 is dirty);
   untracked target file does NOT block.
5. D6: locked worktree → refuse preview AND activate; prunable/invalid row → refuse; matrix
   row reports `activatable:false` + reason.
6. `list_worktree_contexts` matrix correct on a fixture with main + 2 linked (one drifted, one
   missing a doc): counts, `activeProfile`, key mapping (`@main`).
7. Path containment: existing `validate_rel_path` escape tests still pass; every written path
   asserted to be under the target worktree root.
8. `cargo test` then `cargo clippy` (SEQUENTIAL — never concurrent, target-dir race), `tsc`
   + `pnpm build` green; mock.ts compiles.
9. Browser harness (`pnpm dev` + `VITE_MOCK_IPC=1`): dialog renders the matrix; activating a
   profile onto a linked worktree walks the preview-diff gate and flips that worktree's drift
   chip only; locked row's Activate disabled.

**USER CHECKPOINT** (native `pnpm tauri dev`): on a real repo with two linked worktrees,
activate different profiles per worktree from the dialog; verify with the CLI that each
worktree's CLAUDE.md/AGENTS.md holds its own profile's content and `git -C <wt> status` shows
only the expected files; drift chips per worktree match; locked worktree is refused.

**Env constraints:** all scratch repos under `D:\Temp\bonsai-scratch`; set `TMP`/`TEMP` to
`D:\Temp` in every test-running shell; run cargo test/clippy sequentially; never touch real
repos with destructive git commands (this milestone performs none regardless).

---

## 10. Flagged for orchestrator

- D4 (in-store map, no sidecar) and D7 (block tracked-dirty, allow untracked) are judgment
  calls — confirm or override before P31a.
- D9 defers the P29 health rollup; add a backlog line so it is not lost.
