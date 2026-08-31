# P44 — Identity Profiles — Architect Contract

## Addendum (2026-08-05, P44d — supersedes §5.1/§5.2/§6.1/§6.2/§6.3/§7.1 where they conflict)

Review found an **edit-then-Apply staleness race**: resolving the profile from persisted
`settings.json` inside the command is wrong, because profile CRUD only reaches disk after App's
~300 ms `set_ui_settings` debounce. A user who edits a profile and immediately clicks Apply would
get the PREVIOUS persisted identity written to their repo (silent wrong `user.name`/`user.email`),
and a freshly-added profile would error "no such profile".

**Fix (locked):** the Apply command carries the identity FIELDS directly (App's in-memory
`profiles` state updates synchronously on edit; only persistence is debounced), so the backend never
re-reads settings for Apply. Concretely, superseding the sections below:

- **Command (§5.1):**
  `apply_identity_profile(app, state, repo_id: String, user_name: String, user_email: String, signing_key: Option<String>) -> ConfigView`.
  Resolve workdir via `repo_path` (`NoRepo` if unknown), then call
  `config::apply_identity_profile(&workdir, &user_name, &user_email, signing_key.as_deref())` in
  `spawn_blocking`. **No** `settings::load_from`, **no** profile lookup.
- **Error table (§5.2):** the `unknown profile_id → Other(other)` row is REMOVED (no id lookup).
  Remaining: `NoRepo` (unknown repo), `InvalidName`/`Git` (write failure). The
  `apply_identity_profile_unknown_repo_errors` command test still applies; there is NO
  unknown-profile command test.
- **IPC (§6.1/§6.2):**
  `applyIdentityProfile(repoId: string, userName: string, userEmail: string, signingKey: string | null): Promise<ConfigView>`;
  `tauri.ts` invokes with `{ repoId, userName, userEmail, signingKey }`.
- **Mock (§6.3):** `applyIdentityProfile(repoId, userName, userEmail, signingKey)` writes those
  fields straight into the mock Local config store (no profile lookup / no `other` throw).
- **Component (§7.1):** the Apply button passes the profile's CURRENT in-memory fields
  (`profile.userName`, `profile.userEmail`, `profile.signingKey`), not `profile.id`.

The core `config::apply_identity_profile` fn (§4) and its four unit tests (§4.1) are UNCHANGED — it
already took fields. Everything else (data model §3, `apply_patch`/CRUD persistence, match
indicator, no `repo-changed`) stands.

---

Status: DESIGN. Implementer builds to this file verbatim. Lightweight named identity profiles
(GitKraken-lite): global app-settings objects holding a git identity; one-click **Apply** writes
`user.name`/`user.email`/(optional) `user.signingkey` to the open repo's **Local** git config,
reusing the P40 config-write path. No per-repo storage. Data model rides the existing
`Settings`/`UiSettings`/`UiSettingsPatch` + debounced `set_ui_settings` plumbing; Apply is one new
command that reuses `config::set_config` + `config::read_config` (P40).

## 1. Overview & goal

- Users create named profiles ("Work", "Personal"), each = display label + `user.name` +
  `user.email` + OPTIONAL signing key.
- Profiles are **global app settings** persisted in `settings.json` (`profiles: Vec<IdentityProfile>`),
  edited entirely on the frontend and persisted via the existing debounced `set_ui_settings`
  (whole-array replace semantics — the patch carries the entire `profiles` array).
- One click **Apply to current repo** writes the profile's identity to the repo's **Local** config
  and returns a refreshed `ConfigView` (same shape as `get_config`).

## 2. Invariants (enforce in review)

- **Rust owns all git logic.** The which-keys / Local-level / skip-None-signing-key rules live in a
  runtime-free core helper (`config.rs`), reusing the existing validated `set_config` write path —
  the frontend never writes config directly.
- **Additive, non-breaking settings.** `profiles` is an additive `#[serde(default)]` field on
  `Settings` (empty `Vec`); a legacy `settings.json` without the key loads fine. No
  `SETTINGS_VERSION` bump (same bar as every prior additive field).
- **No new event, no new channel.** `apply_identity_profile` is request/response and does NOT emit
  `repo-changed` (mirrors `set_config`; identity does not change tree/graph state). Profile CRUD is
  pure `set_ui_settings`.
- **No new `AppError` variant.** Reuse `NoRepo` (unknown repo id), `Other` (unknown profile id),
  `Git`/`InvalidName` (config write failure, via `set_config`).
- git2 is blocking → the command wraps the core call in `spawn_blocking`.
- Apply writes **Local only**, **overwrites** existing values, and writes `user.signingkey` **only
  when Some and non-empty** — a None/empty signing key is left UNTOUCHED (never unset), to avoid
  surprising removals (§4, Decision 1).
- `mock.ts` stays compiling; the harness exercises full create/edit/delete + apply against the mock
  config store and persists `profiles` in mock settings.
- Frontend `SettingsProfilesSection.tsx` is its own small file (file-size discipline), composed by
  `SettingsPanel` between Git config and AI assistance.
- Scratch repos only under `D:\Temp\bonsai-scratch`; TMP/TEMP=`D:\Temp`; run `cargo test`/`clippy`
  sequentially.

## 3. Data model

### 3.1 Rust — `src-tauri/src/settings.rs`

```rust
/// One named identity profile (P44). Global app setting; applied to a repo's
/// Local git config on demand. `id` is a stable frontend-generated UUID.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityProfile {
    /// Stable id (frontend-generated `crypto.randomUUID()`); never reused.
    pub id: String,
    /// Display label, e.g. "Work". Empty/duplicate allowed but discouraged
    /// (frontend soft-validates non-empty).
    pub label: String,
    pub user_name: String,
    pub user_email: String,
    /// Optional `user.signingkey`. None/empty ⇒ not written on apply.
    pub signing_key: Option<String>,
}
```
Add to `Settings` (additive default = empty `Vec`):
```rust
/// P44: named identity profiles (global). Additive `#[serde(default)]`; a
/// legacy file without this key loads as an empty Vec.
pub profiles: Vec<IdentityProfile>,
```
`Settings::default()` gains `profiles: Vec::new()`. (`Settings` already derives
`#[serde(default)]` at the container level, so a missing key is covered; the explicit field default
keeps `..Default::default()` ergonomic.)

Note: `UiSettings`/`UiSettingsPatch` currently derive `Copy` — adding a `Vec` field removes `Copy`
from **both**. That is fine (they are already `Clone`); drop `Copy` from their `derive` lists and
from any `by-value` reuse. `apply_patch` takes `patch` by value already.

### 3.2 TypeScript — `src/ipc/types.ts`

```ts
/** One named identity profile (P44). `id` is a stable crypto.randomUUID(). */
export interface IdentityProfile {
  id: string;
  label: string;
  userName: string;
  userEmail: string;
  /** Optional user.signingkey; null/empty ⇒ not written on apply. */
  signingKey: string | null;
}
```
Add `profiles: IdentityProfile[]` to `UiSettings` and `profiles?: IdentityProfile[]` to
`UiSettingsPatch`. Rust `Option<String>` → TS `string | null` (existing convention).

### 3.3 id generation & validation

- **id:** frontend generates `crypto.randomUUID()` when a profile is added; stable thereafter;
  never regenerated on edit. Backend treats `id` as opaque.
- **Soft validation (frontend only, non-blocking — mirrors the P40 `user.email` "missing @" warn):**
  empty `label` → muted "Name this profile" hint; `userEmail` lacking `@` → muted warning. Both are
  advisory; save/apply still proceed. Empty/duplicate labels are allowed.
- Backend does NOT validate profile fields beyond what `set_config` enforces on apply.

## 4. Rust — `crates/bonsai-core/src/git/config.rs`

Add ONE function beside `set_config`/`read_config` (config module already registered in
`git/mod.rs`; no new module):

```rust
/// Blocking. Applies an identity to the repo's LOCAL git config: writes
/// `user.name`, `user.email`, and — only when `signing_key` is Some AND
/// non-empty (after trim) — `user.signingkey`. A None/empty signing key is
/// left UNTOUCHED (never unset). Overwrites existing Local values. Returns the
/// refreshed Local `ConfigView` (same shape as `read_config(_, Local)`).
/// Errors: `NoRepo` (workdir not a repo) | `InvalidName` | `Git`.
pub fn apply_identity_profile(
    workdir: &Path,
    user_name: &str,
    user_email: &str,
    signing_key: Option<&str>,
) -> Result<ConfigView, AppError>;
```

Pseudocode (reuses the validated §4 write path — no reinvented config logic):
```
set_config(workdir, ConfigLevelArg::Local, "user.name",  user_name)?
set_config(workdir, ConfigLevelArg::Local, "user.email", user_email)?
if let Some(k) = signing_key:
    if !k.trim().is_empty():
        set_config(workdir, ConfigLevelArg::Local, "user.signingkey", k)?
read_config(workdir, ConfigLevelArg::Local)
```
`user.signingkey` is NOT a curated key → it surfaces in the returned `ConfigView.advanced` list, not
`curated`. That is expected. (Optional micro-optimization — open the Local config once and write all
keys via a single `open_target` instead of 2–3 `set_config` opens — is allowed but not required;
correctness is identical. Flag §11.1.)

### 4.1 Unit tests (`#[cfg(test)]`, Local level only — never touch global)

Use `testutil::scratch_dir` under `D:\Temp\bonsai-scratch`; write/read Local.
1. `apply_identity_profile_writes_local_identity` — apply `("Ada","ada@x.io", None)` to a fresh
   scratch repo → `read_config(_, Local)` curated `user.name`/`user.email` `targetValue` equal the
   applied values; `user.signingkey` absent from `advanced`.
2. `apply_identity_profile_writes_signing_key_when_set` — apply `(_, _, Some("KEYID"))` →
   `advanced` contains `user.signingkey = KEYID`.
3. `apply_identity_profile_leaves_existing_signing_key_on_none` — pre-set Local
   `user.signingkey = OLD`; apply `(_, _, None)` → `user.signingkey` still `OLD` (not unset).
4. `apply_identity_profile_overwrites_existing_identity` — pre-set a different Local identity; apply
   → the new values win.

## 5. Command + registration

### 5.1 `src-tauri/src/commands.rs`

`apply_patch` gains one line (whole-array replace, like `pane_widths`):
```rust
if let Some(profiles) = patch.profiles {
    s.profiles = profiles;
}
```
`get_ui_settings` / `set_ui_settings` add `profiles: s.profiles.clone()` to the `UiSettings`
construction (no longer `Copy` — clone the Vec). `UiSettings`/`UiSettingsPatch` structs gain the
`profiles` field per §3.1/§3.2 wire mirror.

New command (resolve workdir + settings file BEFORE `spawn_blocking`, mirroring
`get_config_inner`; needs both `AppHandle` and `AppState`):
```rust
/// Apply identity profile `profile_id` (from persisted settings) to `repo_id`'s
/// LOCAL git config: writes user.name + user.email + (if set) user.signingkey,
/// returns the refreshed Local `ConfigView`. Does NOT emit `repo-changed`
/// (mirrors set_config). Errors: `noRepo` (unknown repo) | `other` (unknown
/// profile) | `invalidName` | `git` (write failure).
#[tauri::command]
pub async fn apply_identity_profile(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    profile_id: String,
) -> Result<ConfigView, AppError> {
    let workdir = repo_path(state.inner(), &repo_id)?; // NoRepo if unknown
    let file = settings::settings_file(&app)?;
    tauri::async_runtime::spawn_blocking(move || -> Result<ConfigView, AppError> {
        let s = settings::load_from(&file);
        let profile = s
            .profiles
            .iter()
            .find(|p| p.id == profile_id)
            .ok_or_else(|| AppError::Other(format!("no such profile: {profile_id}")))?;
        config::apply_identity_profile(
            &workdir,
            &profile.user_name,
            &profile.user_email,
            profile.signing_key.as_deref(),
        )
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
```

Command test `apply_identity_profile_unknown_repo_errors` (mirror the P40 reflog/config command
test): unknown repo id → `NoRepo`.

### 5.2 Error table

| Condition | Variant | Wire `kind` |
|---|---|---|
| unknown `repo_id` | `NoRepo` | `noRepo` |
| unknown `profile_id` | `Other(String)` | `other` |
| config write failure (bad value / libgit2) | `InvalidName` / `Git` | `invalidName` / `git` |

### 5.3 `src-tauri/src/lib.rs`

Register `commands::apply_identity_profile` in `generate_handler!` (near `commands::set_config`).
No new event/channel registration.

## 6. IPC triple

### 6.1 `src/ipc/types.ts`

Add `IdentityProfile` (§3.2) and to the `Ipc` interface:
```ts
/** Apply profile `profileId` to `repoId`'s Local git config; returns the
 *  refreshed Local ConfigView. Rejects noRepo | other | invalidName | git. */
applyIdentityProfile(repoId: string, profileId: string): Promise<ConfigView>;
```

### 6.2 `src/ipc/tauri.ts`

```ts
applyIdentityProfile(repoId, profileId) {
  return invoke<ConfigView>('apply_identity_profile', { repoId, profileId });
},
```

### 6.3 `src/ipc/mock.ts` + `src/ipc/fixtures/config.ts`

- `DEFAULT_UI_SETTINGS` gains `profiles` — seed two demo profiles (fixed string ids so the harness
  shows a populated list and apply is exercisable):
  ```ts
  profiles: [
    { id: 'mock-work', label: 'Work',
      userName: 'Mock Fixture User', userEmail: 'work@bonsai.dev', signingKey: null },
    { id: 'mock-personal', label: 'Personal',
      userName: 'Mock Personal', userEmail: 'me@personal.dev', signingKey: 'ABC123' },
  ],
  ```
- `readUiSettings` parses `profiles`: `Array.isArray(parsed.profiles) ? parsed.profiles :
  DEFAULT_UI_SETTINGS.profiles` (degrade to default, mirroring the other fields).
- `setUiSettings` adds `profiles: patch.profiles ?? current.profiles` to the `next` object.
- New mock method (writes the mock Local config store so a subsequent `getConfig`/mock `commit`
  reflects the applied identity — full round-trip in the harness):
  ```ts
  async applyIdentityProfile(repoId, profileId): Promise<ConfigView> {
    await delay(120);
    const state = requireRepo(repoId);
    const profile = readUiSettings().profiles.find((p) => p.id === profileId);
    if (!profile) {
      const err: AppError = { kind: 'other', message: `no such profile: ${profileId}` };
      throw err;
    }
    state.config.local['user.name'] = profile.userName.trim();
    state.config.local['user.email'] = profile.userEmail.trim();
    if (profile.signingKey && profile.signingKey.trim() !== '') {
      state.config.local['user.signingkey'] = profile.signingKey.trim();
    }
    return buildConfigView(state.config, 'local');
  }
  ```
  (No new fixture helper needed — reuse `buildConfigView` from `fixtures/config.ts`.)

## 7. Frontend

### 7.1 New section — `src/components/SettingsProfilesSection.tsx`

Self-contained (own Apply IPC + match-indicator fetch). CRUD is lifted to the parent via
`onProfilesChange` (parent persists via the existing debounced `onChange({ profiles })`).

```ts
export interface SettingsProfilesSectionProps {
  /** Open repo id (== workdir path). Null → Apply disabled + "open a repo" note. */
  repoId: string | null;
  /** Current profiles (from UiSettings). */
  profiles: IdentityProfile[];
  /** Persist the WHOLE next list (replace semantics). Parent maps to
   *  `onChange({ profiles: next })`; debounced persistence lives upstream. */
  onProfilesChange(next: IdentityProfile[]): void;
}
```
Behaviour:
- **List** — each profile row is inline-editable: `label`, `userName`, `userEmail`, `signingKey`
  text inputs. Edits build a new array (replace the edited element) → `onProfilesChange(next)`.
  Empty `signingKey` input maps to `null`.
- **Add** — appends `{ id: crypto.randomUUID(), label: '', userName: '', userEmail: '',
  signingKey: null }` → `onProfilesChange`.
- **Delete** — removes by `id` → `onProfilesChange`. No confirm (app-settings only, not a git
  mutation; mirrors the P40 advanced-list remove).
- **Soft validation** — muted, non-blocking: empty `label`; `userEmail` without `@` (mirrors the
  P40 email warn). Never blocks save or apply.
- **Apply to current repo** — per-profile button. Disabled with a muted "Open a repository to apply
  a profile" note when `repoId == null` (mirror the P40 `SettingsGitConfigSection` no-repo pattern).
  On click → `ipc.applyIdentityProfile(repoId, profile.id)`; button shows in-flight; on success,
  re-run the match-indicator fetch (below) and show a transient "Applied" confirmation; on error,
  inline error from the `AppError.message`.
- **Active/match indicator (read-only, best-effort)** — on mount and after each successful apply,
  if `repoId != null`, `ipc.getConfig(repoId, 'local')`; read the Local `curated` `targetValue` for
  `user.name`/`user.email`. A profile is "Active on this repo" when both its `userName` and
  `userEmail` equal those Local target values (trimmed, exact compare). Show an "Active" badge on the
  matching profile (at most one). Fetch failure is swallowed (no error surface, no badge).

### 7.2 `SettingsPanel.tsx` wiring

- Add prop `profiles: IdentityProfile[]` to `SettingsPanelProps`; destructure it.
- Render between the Git config section (`<SettingsGitConfigSection …/>`, ~line 409) and the "AI
  assistance" `<section>` (~line 412):
  ```tsx
  <SettingsProfilesSection
    repoId={repoPath}
    profiles={profiles}
    onProfilesChange={(next) => onChange({ profiles: next })}
  />
  ```
  (`onChange(patch: UiSettingsPatch)` and `repoPath` already exist on the panel.)

### 7.3 `App.tsx` wiring

App already owns `UiSettings` and threads `onChange`→`setUiSettings` (debounced) into
`SettingsPanel`. Pass the new prop: `profiles={settings.profiles}` (source of truth = the app's
`UiSettings` state, updated from the `set_ui_settings` round-trip result like every other field).

## 8. Sub-increments

**P44a — settings model + command + IPC + mock** (Rust + IPC, no UI):
- `settings.rs`: `IdentityProfile` + `Settings.profiles` (§3.1) + settings round-trip/back-compat
  unit tests (mirror `onboarding_seen_roundtrip` + the "old file without X loads default" test).
- `commands.rs`: `UiSettings`/`UiSettingsPatch` `profiles` fields (drop `Copy`), `apply_patch` line,
  `get`/`set_ui_settings` clone, `apply_identity_profile` command + command test (§5).
- `config.rs`: `apply_identity_profile` core fn + unit tests (§4.1). `lib.rs` registration.
- IPC: `types.ts` (§6.1), `tauri.ts` (§6.2), `mock.ts` + seeded profiles + method (§6.3).
- Gate: `cargo test -p bonsai-core config` + settings/command tests green; `clippy` clean; `tsc`
  clean; `mock.ts` compiles.

**P44b — Settings UI** (frontend):
- `SettingsProfilesSection.tsx` (§7.1); `SettingsPanel`/`App` wiring (§7.2/§7.3).
- Gate: browser harness (`VITE_MOCK_IPC=1`) — profiles list renders seeded profiles; add/edit/delete
  update + persist; Apply on an open repo updates the mock Local config (verify via the Git config
  section or a subsequent `getConfig`) and lights the "Active" badge; Apply disabled with the note
  when no repo. `tsc`/build clean.

## 9. Acceptance criteria

**AI gate (orchestrator verifies alone):**
- `cargo test -p bonsai-core config` (incl. the four `apply_identity_profile_*` tests) + the
  settings round-trip/back-compat tests + `apply_identity_profile_unknown_repo_errors` command test
  green; `clippy` clean; real global config provably untouched (all tests write Local).
- `tsc` + frontend build clean; `mock.ts` compiles.
- Harness: create a profile, edit its fields, delete a profile; Apply a profile to the open repo →
  the Git config section / a `getConfig` shows the Local `user.email` changed to the profile's; the
  "Active" badge appears on the applied profile; Apply is disabled with the no-repo note when no repo
  is open. Console clean across create/edit/delete/apply.

**USER CHECKPOINT (native Tauri, human perception):**
- On a real repo: create a profile, click Apply, then in a terminal
  `git config --local user.email` / `user.name` (and `user.signingkey` if set) show the profile's
  values.
- Applying a profile with no signing key does NOT remove a pre-existing `user.signingkey`.
- Restart the app → the created/edited profiles persist (loaded from `settings.json`).

## 10. Files touched (summary)

- `src-tauri/src/settings.rs` — `IdentityProfile` + `Settings.profiles` (+ tests).
- `src-tauri/src/commands.rs` — `UiSettings`/`UiSettingsPatch` field, `apply_patch`,
  `get`/`set_ui_settings`, `apply_identity_profile` command (+ test).
- `crates/bonsai-core/src/git/config.rs` — `apply_identity_profile` core fn (+ tests).
- `src-tauri/src/lib.rs` — command registration.
- `src/ipc/types.ts`, `src/ipc/tauri.ts`, `src/ipc/mock.ts` — IPC triple + seeded mock profiles.
- `src/components/SettingsProfilesSection.tsx` (NEW), `src/components/SettingsPanel.tsx`,
  `src/App.tsx` — UI + wiring.

## 11. Flagged ambiguities (non-blocking; recommended defaults chosen)

1. **Write path** (Decision) — RECOMMEND reusing `set_config` (Local) for each key inside the core
   `apply_identity_profile` (validated, minimal code). The single-`open_target` batch write is an
   allowed optimization; not required. **Chosen: reuse `set_config`.**
2. **Signing key on None** (Decision) — RECOMMEND leave an existing `user.signingkey` UNTOUCHED when
   the profile has none (never auto-unset), to avoid surprising removals. **Chosen: leave untouched.**
   (If a user wants to clear it, they use the P40 Advanced config editor.)
3. **Unknown-profile error variant** — RECOMMEND `AppError::Other` (wire `other`); alternative is
   `InvalidName`. **Chosen: `Other`.** Flag for orchestrator if a dedicated variant is preferred.
4. **Match indicator scope** — the "Active" badge compares against the repo's **Local** `user.name`
   + `user.email` (per locked scope), NOT the effective/inherited values. A profile whose identity
   is only inherited from global will not show "Active". **Chosen: Local target compare.**
5. **Cross-section live refresh** — after Apply, an already-open `SettingsGitConfigSection` will not
   auto-refresh (each section fetches its own config). RECOMMEND accepting this (the config section
   refetches on its next level-toggle/reopen); a shared refresh signal is out of scope. Flag if the
   orchestrator wants live coupling.
6. **`repo-changed` on apply** — RECOMMEND no emission (mirrors `set_config`; identity does not
   change tree/graph). **Chosen: no emission.**
