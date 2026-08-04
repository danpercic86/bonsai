# P40 — Git Config Editing — Architect Contract

Status: DESIGN. Implementer builds to this file verbatim. Read path mirrors the runtime-free
`&Path`/`&str` + `#[cfg(test)]` shape of `remote.rs` / `blame.rs`; command/IPC plumbing mirrors
P38 (reflog). Settings surface extends `SettingsPanel.tsx` with a new self-contained section file.

## 1. Overview & goal

Read and edit git config from inside Bonsai at the **repo (Local)** and **Global** levels:
- A curated form of the common identity/behaviour keys (typed inputs).
- An "Advanced" list of arbitrary `section.key = value` entries (add / edit / remove).
- Reads present the **effective** value plus which level set it; writes target the chosen level.

Also closes the long-standing "author/committer identity unset" gap: when `user.name`/`user.email`
are missing, committing errors with `ConfigMissing` (see `commit.rs::resolve_signature`). P40 lets
the user set identity in-app and makes that error actionable — the commit error banner gains a
"Set identity…" affordance that opens Settings → Git config → Identity.

## 2. Invariants (enforce in review)

- **Rust owns all config logic.** `config.rs` is runtime-free (`&Path`/`&str`, no Tauri types) →
  CLI-testable without the tauri `test` feature, like `remote.rs`.
- **Writes are validated server-side** (key shape + bool/enum values) — NEVER trust the client.
- git2 is blocking → the three commands wrap `config.rs` calls in `spawn_blocking`.
- IPC is compact request/response. **No new events, no new channels.** `set_config`/`unset_config`
  do NOT emit `repo-changed` (a config edit does not retroactively change tree/graph state the user
  expects a refresh for); the Settings section re-fetches `get_config` after each write. (Flag §11.)
- **Global writes touch the user's real `~/.gitconfig`.** In-process unit tests write **Local only**
  (repo-scoped, safe). Global-level verification lives in the CLI oracle subprocess with an
  isolated `GIT_CONFIG_GLOBAL`/`HOME` under `D:\Temp\bonsai-scratch`. Tests MUST NEVER write the
  developer's real global config. (Flagged prominently for tester — §9, §11.)
- **No new `AppError` variant.** Reuse `Git` (libgit2 failures) + `InvalidName` (bad key/value).
- System level is **read-only / out of scope** (may appear as the effective source of a value, but
  is never a write target).
- Multi-valued keys: v1 reads the **last/effective** value and edits **single-valued** keys only;
  multivar editing deferred (§11).
- `mock.ts` stays compiling; harness renders the section + identity linkage on fixtures.
- Scratch repos only under `D:\Temp\bonsai-scratch`; TMP/TEMP=`D:\Temp`; run `cargo test` and
  `clippy` sequentially.

## 3. git2 0.21.0 Config API (verified: Cargo.lock git2 0.21.0)

- `Repository::config(&self) -> Result<Config, Error>` — merged view (system+global+local).
  Use `.snapshot()?` for consistent reads.
- `Config::open_default() -> Result<Config, Error>` — global/system when no repo (not needed; all
  commands take a repo).
- `Config::open_level(&self, level: ConfigLevel) -> Result<Config, Error>` — single-level, writable
  view. Called on a repo config: `repo.config()?.open_level(ConfigLevel::Local)`.
- `Config::open(path: &Path) -> Result<Config, Error>` — single-file config (creates on write);
  fallback for a not-yet-existing global file.
- `Config::find_global() -> Result<PathBuf, Error>` — path of `~/.gitconfig`.
- Reads: `get_string(name)`, `get_bool(name)`, `get_entry(name) -> ConfigEntry`,
  `entries(glob: Option<&str>) -> Result<ConfigEntries, Error>` (iterate `ConfigEntry`).
- Writes: `set_str(&mut self, name, value)`, `set_bool(&mut self, name, bool)`,
  `remove(&mut self, name)`.
- `ConfigEntry`: `name() -> Option<&str>`, `value() -> Option<&str>`,
  `name_bytes()`/`value_bytes()`, `level() -> ConfigLevel`, `has_value() -> bool`.
- `ConfigLevel` enum: `ProgramData | System | XDG | Global | Local | Worktree | App | Highest`.
- `git2::opts::set_search_path(ConfigLevel, &Path)` — process-global override of the global search
  path (used by the CLI oracle only if needed; unit tests prefer subprocess env — see §9).

## 4. Rust module — `crates/bonsai-core/src/git/config.rs`

Register in `crates/bonsai-core/src/git/mod.rs`: add `pub mod config;` in alphabetical position
**after `commit;` (line 10) and before `conflict;` (line 11)**.

### 4.1 Curated key table (constant)

```rust
/// Input widget kind for a curated key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ValueKind { Text, Bool, Enum }

/// One curated key definition (static). `enum_values` non-empty only for Enum.
struct CuratedKey {
    key: &'static str,
    kind: ValueKind,
    enum_values: &'static [&'static str],
}

/// The curated set (Decision 2). Order = display order under Identity then Behaviour.
const CURATED_KEYS: &[CuratedKey] = &[
    CuratedKey { key: "user.name",          kind: ValueKind::Text, enum_values: &[] },
    CuratedKey { key: "user.email",         kind: ValueKind::Text, enum_values: &[] },
    CuratedKey { key: "core.autocrlf",      kind: ValueKind::Enum, enum_values: &["true", "false", "input"] },
    CuratedKey { key: "init.defaultBranch", kind: ValueKind::Text, enum_values: &[] },
    CuratedKey { key: "pull.ff",            kind: ValueKind::Enum, enum_values: &["true", "false", "only"] },
    CuratedKey { key: "pull.rebase",        kind: ValueKind::Enum, enum_values: &["true", "false", "merges", "interactive"] },
];
```
`user.name`/`user.email` = the Identity sub-section (rendered first). No cross-validation between
`pull.ff` and `pull.rebase` in v1.

### 4.2 Wire types (serialize camelCase)

```rust
/// Write-target level requested by the client. System is NOT a valid target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigLevelArg { Local, Global }

/// The level a value's effective/target value actually lives at (read result).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigLevelName { Local, Global, System, Other }

/// A curated key with its effective value + the value set AT the target level.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CuratedEntry {
    /// e.g. "user.email".
    pub key: String,
    pub kind: ValueKind,
    /// Allowed values for Enum; empty otherwise.
    pub enum_values: Vec<String>,
    /// Effective value from the merged snapshot; None if unset at every level.
    pub effective_value: Option<String>,
    /// Which level the effective value came from; None if unset.
    pub effective_level: Option<ConfigLevelName>,
    /// Value set explicitly AT the target level; None if inherited/unset there.
    /// Drives the form: when None, the field shows the effective value as a
    /// placeholder and "inherited from <level>".
    pub target_value: Option<String>,
}

/// An arbitrary `section.key = value` entry read AT the target level (Advanced list).
/// Multivar keys collapse to the LAST value.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigEntry {
    /// Full dotted name, e.g. "alias.co".
    pub name: String,
    pub value: String,
    /// Always == the target level for Advanced entries.
    pub level: ConfigLevelName,
}

/// Response of `read_config` for one target level.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigView {
    pub target_level: ConfigLevelArg,
    /// Curated keys (fixed order = CURATED_KEYS).
    pub curated: Vec<CuratedEntry>,
    /// Arbitrary entries defined at the target level, EXCLUDING the curated keys
    /// (those are surfaced in `curated`). Sorted by name.
    pub advanced: Vec<ConfigEntry>,
}
```

### 4.3 Function signatures

```rust
/// Blocking. Reads the merged config for effective values + the single-level
/// (target) config for `target_value`/advanced entries.
/// Errors: NoRepo (workdir not a repo) | Git.
pub fn read_config(workdir: &Path, level: ConfigLevelArg) -> Result<ConfigView, AppError>;

/// Blocking. Validates `key` shape + (for curated Enum/Bool keys) the value, then
/// writes `value` at `level`. Single-valued write (replaces any existing value).
/// Errors: NoRepo | InvalidName (bad key/value) | Git.
pub fn set_config(workdir: &Path, level: ConfigLevelArg, key: &str, value: &str) -> Result<(), AppError>;

/// Blocking. Removes `key` at `level`. Idempotent: a key not present at that level
/// yields Ok(()) (NotFound is swallowed). Errors: NoRepo | InvalidName | Git.
pub fn unset_config(workdir: &Path, level: ConfigLevelArg, key: &str) -> Result<(), AppError>;
```

### 4.4 Internals pseudocode

```
// --- open helpers ---
fn open_target(repo, level) -> Config:            // WRITABLE single-level view
  match level:
    Local  => repo.config()?.open_level(Local)
    Global => match repo.config()?.open_level(Global):
                Ok(c)  => c
                Err(_) => // global file may not exist yet
                    let p = Config::find_global().unwrap_or_else(default_global_path)
                    Config::open(&p)              // single-file, created on first write
// (Flag §11: verify open_level(Global) vs the find_global fallback at impl time.)

// --- read_config ---
repo   = open_workdir_repo(workdir)               // reuse stage::open_workdir_repo -> NoRepo
merged = repo.config()?.snapshot()?
target = open_target(repo, level)?.snapshot()?    // snapshot of the single level

curated = []
for ck in CURATED_KEYS:
    eff = merged.get_entry(ck.key).ok()           // effective (highest-priority) entry
    tgt = target.get_string(ck.key).ok()          // value AT the target level (or None)
    curated.push(CuratedEntry {
        key: ck.key, kind: ck.kind,
        enum_values: ck.enum_values.map(String),
        effective_value: eff.and_then(|e| e.value().map(String)),
        effective_level: eff.map(|e| map_level(e.level())),
        target_value: tgt,
    })

advanced = []
curated_set = CURATED_KEYS.keys()
for e in target.entries(None)?:                    // iterate target-level entries only
    name = lossy(e.name_bytes()); if name in curated_set { continue }
    // multivar: later entries overwrite earlier -> last value wins
    advanced.upsert(name, ConfigEntry { name, value: lossy(e.value_bytes()), level: map_level(level) })
advanced.sort_by(name); advanced = advanced.into_values()

Ok(ConfigView { target_level: level, curated, advanced })

// map_level(ConfigLevel) -> ConfigLevelName: Local->Local, Global/XDG->Global,
//   System/ProgramData->System, _ -> Other

// --- set_config ---
validate_key(key)?                                // §4.5
validate_curated_value(key, value)?               // §4.5 (Enum/Bool only)
repo = open_workdir_repo(workdir)
mut cfg = open_target(repo, level)?
if is_bool_curated(key): cfg.set_bool(key, parse_bool(value)?)   // core.autocrlf uses set_str (tri-state), not bool
else:                    cfg.set_str(key, value.trim())
Ok(())
// NOTE: core.autocrlf/pull.ff/pull.rebase are STRING enums (true/false/input/only/...),
// written with set_str, NOT set_bool. No curated key uses set_bool in v1; keep the
// helper but every current curated write is set_str.

// --- unset_config ---
validate_key(key)?
repo = open_workdir_repo(workdir)
mut cfg = open_target(repo, level)?
match cfg.remove(key):
    Ok(()) => Ok(())
    Err(e) if e.code() == NotFound => Ok(())      // idempotent
    Err(e) => Err(e.into())
```

### 4.5 Validation

```
validate_key(key):
  trimmed = key.trim()
  reject empty -> InvalidName("config key must not be empty")
  // Shape: section.variable  OR  section.subsection(.sub...).variable
  // - first segment (section): [A-Za-z0-9-]+  (non-empty)
  // - last segment (variable): [A-Za-z][A-Za-z0-9-]*  (starts with a letter)
  // - any middle segment(s) (subsection): non-empty, no whitespace
  parts = trimmed.split('.')
  reject if parts.len() < 2                        -> InvalidName("key must be section.key")
  reject if section !~ ^[A-Za-z0-9-]+$             -> InvalidName("invalid section name")
  reject if variable !~ ^[A-Za-z][A-Za-z0-9-]*$    -> InvalidName("invalid key name")
  reject if any subsection is empty or has whitespace -> InvalidName("invalid subsection")
  Ok(())
  // Rationale: a friendly pre-check; git2 set_str also rejects truly-invalid names.

validate_curated_value(key, value):
  if key is a curated Enum key and value NOT in its enum_values:
      -> InvalidName("value for <key> must be one of: <list>")
  else Ok(())          // Text keys unconstrained; email format is a SOFT client warn, not blocked.
```
Email: **not** validated server-side (git does not enforce it). The frontend shows a soft warning
if `user.email` lacks an `@`, but still allows the save.

### 4.6 Error table

| Condition | Variant | Wire `kind` |
|---|---|---|
| workdir not a repo | `NoRepo` | `noRepo` |
| empty/malformed key, or bad Enum value | `InvalidName(String)` | `invalidName` |
| libgit2 failure (open/read/write) | `Git(String)` | `git` |
| unset of an absent key | — (Ok, idempotent) | — |

**No new `AppError` variant** (Decision 6 confirmed).

### 4.7 `#[cfg(test)]` unit tests (in-module, Local level only — never touch global)

Use a scratch repo helper (`testutil::scratch_dir` under `D:\Temp\bonsai-scratch`); set/read Local.
1. `config_view_wire_shape_is_camel_case` — `serde_json::to_value` of a fixed `ConfigView` equals
   exact camelCase JSON (`targetLevel`, `curated[].effectiveValue`/`effectiveLevel`/`targetValue`,
   `advanced[].name`/`value`/`level`, `kind`, `enumValues`). Guards the TS wire types.
2. `read_config_reports_curated_identity` — repo with `user.name`/`user.email` set locally →
   curated entries have `effectiveValue == Some`, `effectiveLevel == Local`, `targetValue == Some`.
3. `read_config_unset_key_is_none` — repo without `pull.ff` → its curated `effectiveValue == None`.
4. `set_config_writes_local_then_reads_back` — `set_config(dir, Local, "user.email", "a@b.co")`;
   `read_config` shows `targetValue == Some("a@b.co")`.
5. `set_config_rejects_bad_key` — `"nodot"`, `"user."`, `".email"`, `"user.1bad"` → `InvalidName`.
6. `set_config_rejects_bad_enum` — `set_config(dir, Local, "core.autocrlf", "maybe")` → `InvalidName`.
7. `set_config_accepts_enum_value` — `core.autocrlf = "input"` succeeds, reads back `"input"`.
8. `unset_config_removes_and_is_idempotent` — set then unset `user.name`; second unset also Ok;
   effective now None.
9. `advanced_excludes_curated_and_lists_arbitrary` — set `alias.co = "checkout"` + `user.name`;
   `advanced` contains `alias.co` but NOT `user.name`.
10. `read_config_no_repo_errors` — empty temp dir → `NoRepo`.

## 5. Commands + registration

### 5.1 `src-tauri/src/commands.rs`

Add `use bonsai_core::git::config::{self, ConfigLevelArg, ConfigView};`. Three commands + runtime-free
inners, template-identical to `read_reflog` (`repo_path` helper + `spawn_blocking`). None emit
`repo-changed`.

```rust
/// Read the config view for `level` ("local" | "global") of `repo_id`: curated keys
/// (effective value + level + target-level value) + advanced entries at the target
/// level. Read-only. Errors: `git` | `noRepo`.
#[tauri::command]
pub async fn get_config(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    level: ConfigLevelArg,
) -> Result<ConfigView, AppError> {
    let workdir = repo_path(state.inner(), &repo_id)?;
    tauri::async_runtime::spawn_blocking(move || config::read_config(&workdir, level))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Write `value` to `key` at `level` of `repo_id`. Validated server-side (key shape,
/// enum value). Errors: `invalidName` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn set_config(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    level: ConfigLevelArg,
    key: String,
    value: String,
) -> Result<(), AppError> {
    let workdir = repo_path(state.inner(), &repo_id)?;
    tauri::async_runtime::spawn_blocking(move || config::set_config(&workdir, level, &key, &value))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Remove `key` at `level` of `repo_id` (idempotent). Errors: `invalidName` | `git` | `noRepo`.
#[tauri::command]
pub async fn unset_config(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    level: ConfigLevelArg,
    key: String,
) -> Result<(), AppError> {
    let workdir = repo_path(state.inner(), &repo_id)?;
    tauri::async_runtime::spawn_blocking(move || config::unset_config(&workdir, level, &key))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
```
Add a command test `config_commands_require_an_open_repo` (mirror the reflog one): each inner on an
unknown repo id → `NoRepo`.

### 5.2 `src-tauri/src/lib.rs`

Register `commands::get_config`, `commands::set_config`, `commands::unset_config` in
`generate_handler!` (near `commands::read_reflog`). No new event/channel registration.

## 6. IPC triple

### 6.1 `src/ipc/types.ts`

```ts
/** Write-target level (P40). System is never a write target. */
export type ConfigLevelArg = 'local' | 'global';
/** Where a value actually lives (read result). */
export type ConfigLevelName = 'local' | 'global' | 'system' | 'other';
export type ConfigValueKind = 'text' | 'bool' | 'enum';

/** A curated key with effective value + the value set at the target level (P40 §4.2). */
export interface CuratedConfigEntry {
  key: string;
  kind: ConfigValueKind;
  enumValues: string[];
  effectiveValue: string | null;
  effectiveLevel: ConfigLevelName | null;
  targetValue: string | null;
}

/** An arbitrary section.key entry at the target level. */
export interface ConfigEntry {
  name: string;
  value: string;
  level: ConfigLevelName;
}

/** Result of getConfig for one target level (P40 §4.2). */
export interface ConfigView {
  targetLevel: ConfigLevelArg;
  curated: CuratedConfigEntry[];
  advanced: ConfigEntry[];
}
```
Rust `Option<T>` → TS `T | null` (matches existing convention). Add to the `Ipc` interface:
```ts
/** Config view for `level` of `repoId`. Read-only. Rejects git | noRepo. */
getConfig(repoId: string, level: ConfigLevelArg): Promise<ConfigView>;
/** Write `value` to `key` at `level`. Validated server-side. Rejects invalidName | git | noRepo. */
setConfig(repoId: string, level: ConfigLevelArg, key: string, value: string): Promise<void>;
/** Remove `key` at `level` (idempotent). Rejects invalidName | git | noRepo. */
unsetConfig(repoId: string, level: ConfigLevelArg, key: string): Promise<void>;
```

### 6.2 `src/ipc/tauri.ts`

```ts
getConfig(repoId, level) { return invoke<ConfigView>('get_config', { repoId, level }); },
setConfig(repoId, level, key, value) { return invoke<void>('set_config', { repoId, level, key, value }); },
unsetConfig(repoId, level, key) { return invoke<void>('unset_config', { repoId, level, key }); },
```
(Import `ConfigView`, `ConfigLevelArg` in the type import block.)

### 6.3 `src/ipc/mock.ts`

Put the fixture in `src/ipc/fixtures/config.ts` (single-responsibility). Model a per-level store:
```ts
// Two flat maps keyed by dotted name -> value, one per level. Global seeds identity
// so the harness default is a WORKING identity; a `?fixture=noconfig` query drops
// identity so the commit-error / Set-identity flow is demoable.
export interface MockConfigStore { local: Record<string, string>; global: Record<string, string>; }
export function makeMockConfigStore(): MockConfigStore { ... }   // global: user.name/email, init.defaultBranch=main; local: {}
```
Mock methods build a `ConfigView` from the store (effective = local overrides global; system omitted;
curated from the same CURATED_KEYS list mirrored in TS; advanced = target-level non-curated keys):
```ts
async getConfig(repoId, level) { await delay(80); requireRepo(repoId); return buildConfigView(store, level); }
async setConfig(repoId, level, key, value) { await delay(80); requireRepo(repoId);
  validateKeyOrThrow(key); validateEnumOrThrow(key, value); store[level][key] = value.trim(); }
async unsetConfig(repoId, level, key) { await delay(80); requireRepo(repoId);
  validateKeyOrThrow(key); delete store[level][key]; }
```
`validateKeyOrThrow`/`validateEnumOrThrow` throw an `AppError`-shaped `{ kind:'invalidName', message }`
mirroring §4.5 so the harness exercises client + server-shaped errors identically. The existing
`?fixture=noconfig` commit path (mock.ts ~2038/3385) stays; after `setConfig('local'|'global',
'user.name'/'user.email', ...)` a subsequent mock `commit()` must succeed — wire the mock commit
identity check to read the config store (Flag §11: make the mock commit consult the store so the
end-to-end identity-gap demo works in the browser).

## 7. Frontend

### 7.1 New section component — `src/components/SettingsGitConfigSection.tsx`

Self-contained container (own IPC + local form state), so `SettingsPanel` stays lean (file-size
discipline). SettingsPanel renders `<SettingsGitConfigSection repoId={repoPathOrId} initialFocus={...} />`
inside a new `<section className="settings-section">`. Renders nothing (or a "no repo open" note) when
`repoId` is null.

```ts
export interface SettingsGitConfigSectionProps {
  /** Open repo id (== path). Null → render a disabled "Open a repository to edit its config" note. */
  repoId: string | null;
  /** When 'identity', scroll/expand the Identity sub-section on mount (commit-error linkage). */
  initialFocus?: 'identity' | null;
}
```
Behaviour:
- **Level toggle** — segmented control `Local | Global` (default `Local` when a repo is open). On
  mount + on level change → `ipc.getConfig(repoId, level)` into local state; skeleton while loading.
- **Identity sub-section (top)** — `user.name`, `user.email` text inputs seeded from
  `curated.targetValue ?? ''`. Below each: if `targetValue == null && effectiveValue != null`, show
  a muted `inherited from <effectiveLevel>: <effectiveValue>`. Soft email warning when the entered
  email lacks `@` (non-blocking). Save on blur/Enter → `ipc.setConfig(repoId, level, key, value)`;
  empty value + had a `targetValue` → `ipc.unsetConfig`; refetch after each write. Per-field inline
  error on `invalidName`.
- **Behaviour keys** — the remaining curated keys: `core.autocrlf`/`pull.ff`/`pull.rebase` as
  `<select>` from `enumValues` (plus an "(inherit / unset)" option that maps to `unsetConfig`),
  `init.defaultBranch` as text. Same effective/inherited hinting + save-on-change semantics.
- **Advanced list** — table of `advanced` entries (`name` | `value` | remove). Each value is
  editable (save → `setConfig`); a remove button → `unsetConfig` (confirm not required — trivial).
  A footer "Add entry" row: `section.key` + value inputs → `setConfig`; client pre-validates key
  shape (mirror §4.5) before calling, and surfaces server `invalidName` inline.
- All writes disable the touched control while in flight; on success refetch `getConfig` for the
  current level so effective/inherited hints update.

### 7.2 SettingsPanel wiring — `src/components/SettingsPanel.tsx`

- Add props `repoPath: string | null` is ALREADY present (MCP section uses it) — reuse it as the
  config `repoId`. Add optional `configInitialFocus?: 'identity' | null`.
- Render the new `<SettingsGitConfigSection repoId={repoPath} initialFocus={configInitialFocus} />`
  as a new section (recommend directly under "Appearance", above "AI assistance").

### 7.3 Identity-gap linkage (commit error → Settings)

Thread a callback so the commit-error banner can jump to the identity editor:
- `App.tsx`: add state `const [configFocus, setConfigFocus] = useState<'identity' | null>(null);`
  and a handler `openIdentitySettings = () => { setConfigFocus('identity'); setSettingsOpen(true); };`
  Pass `configInitialFocus={configFocus}` to `<SettingsPanel>`; clear `configFocus` to null when the
  panel closes (`onClose`). Pass `onOpenIdentitySettings={openIdentitySettings}` down to the workspace.
- `RepoWorkspace.tsx` → `CommitBox.tsx`: add optional prop `onOpenIdentitySettings?: () => void`.
  In the commit-error banner (`CommitBox.tsx` ~line 199, the `configMissing` branch), render a
  "Set identity…" button beside the message that calls `onOpenIdentitySettings`.
- Recommend YES (cheap, high value) — see Decision 4. If threading `onOpenIdentitySettings` through
  the workspace proves noisy, the minimum acceptable fallback is: the message links users to open
  Settings manually (no new callback). Prefer the button.

## 8. Sub-increments

**P40a — core + commands + IPC + mock + oracle** (Rust + IPC only, no UI):
- `crates/bonsai-core/src/git/config.rs` (§4) + `pub mod config;` in `git/mod.rs`.
- `get_config`/`set_config`/`unset_config` commands + inners + registration (§5).
- IPC triple: types (§6.1), tauri.ts bindings (§6.2), `src/ipc/fixtures/config.ts` + mock methods
  (§6.3), including wiring mock `commit()` identity to the store.
- Unit tests (§4.7) + command test (§5.1) + CLI oracle `config_cli.rs` (§9).
- Gate: `cargo test -p bonsai-core config` + `config_cli` oracle green; command test green; `tsc`
  clean; `clippy` clean.

**P40b — Settings UI + identity-gap linkage** (frontend only):
- `src/components/SettingsGitConfigSection.tsx` (§7.1); SettingsPanel section (§7.2).
- Identity-gap linkage: App state/handler + CommitBox "Set identity…" button (§7.3).
- Gate: browser harness (`VITE_MOCK_IPC=1`) — Git config section renders Local identity/behaviour +
  advanced list; level toggle switches to Global; editing user.email + saving updates effective hint;
  `?fixture=noconfig` → commit error banner "Set identity…" opens Settings focused on Identity, and
  after setting identity a commit succeeds. `tsc`/build clean; `mock.ts` compiles.

## 9. CLI-oracle test plan — `crates/bonsai-core/tests/config_cli.rs`

Mirror the `*_cli.rs` oracle style (runtime-free core vs real `git`). **Global-level isolation is
mandatory** — the subprocess `git` and the in-process libgit2 read must BOTH point at a scratch
global file, never the developer's `~/.gitconfig`:
- Create a scratch dir under `D:\Temp\bonsai-scratch`; a `global.gitconfig` file inside it.
- For every `git` CLI subprocess AND before any Global-level `config.rs` call in this test, set env:
  `GIT_CONFIG_GLOBAL=<scratch>/global.gitconfig`, and to be safe `HOME`/`USERPROFILE`=<scratch>.
  For libgit2 in-process, also call `git2::opts::set_search_path(ConfigLevel::Global, <scratch>)`
  (process-global) — do this ONCE at test start; run this test file NOT concurrently with others
  that touch global config, and NOT concurrently with clippy.
- **Assert the scratch file is the only global file written** — after the test, the real global
  config is untouched (do not read/modify it).

Local-level oracle:
1. `git init` scratch repo; `git config --local user.name X` / `user.email x@y.z`.
2. `read_config(dir, Local)` → curated `user.name`/`user.email` effective+target match; effectiveLevel==Local.
3. `set_config(dir, Local, "alias.co", "checkout")`; oracle `git config --local --get alias.co` == "checkout";
   and it appears in `advanced`.
4. `set_config(dir, Local, "core.autocrlf", "input")`; `git config --local --get core.autocrlf` == "input".
5. `unset_config(dir, Local, "alias.co")`; `git config --local --get alias.co` exits non-zero (unset).
6. Bad key/enum → `InvalidName`, and the CLI oracle confirms nothing was written.

Global-level oracle (isolated env):
7. `set_config(dir, Global, "user.name", "Global Person")`; `git config --global --get user.name`
   (with `GIT_CONFIG_GLOBAL` set) == "Global Person"; scratch file contains it.
8. `read_config(dir, Local)` with a Local override of `user.name` → effectiveLevel==Local, but
   `read_config(dir, Global).curated[user.name].targetValue` == "Global Person" (per-level isolation).

## 10. AI gate vs USER CHECKPOINT

**AI gate (orchestrator verifies alone):**
- `cargo test -p bonsai-core config` + `config_cli` oracle green (reads/writes match `git config`);
  command test green; `clippy` clean; real global config provably untouched by tests.
- `tsc` + frontend build clean; `mock.ts` compiles.
- Browser harness: Git config section renders curated Identity + Behaviour + Advanced on fixtures;
  Local/Global toggle re-fetches; editing + save updates the effective/inherited hint; add/remove
  advanced entry works; `?fixture=noconfig` shows the commit "Set identity…" affordance which opens
  Settings focused on Identity, and a post-set commit succeeds.
- Console: no errors on open/edit/save/toggle; invalid key/enum shows an inline `invalidName` error.

**USER CHECKPOINT (native Tauri, human perception):**
- On a real repo: reading Local + Global shows the true effective values and their source level.
- Setting `user.name`/`user.email` at Local then Global writes the real files; `git config --list`
  from a terminal confirms; a previously-blocked commit now succeeds and the "Set identity…" path
  from the commit error works end-to-end.
- Editing `core.autocrlf`/`pull.ff`/`pull.rebase` and an arbitrary advanced entry persists and is
  visible via `git config`. **Confirm a Global edit changed the user's real `~/.gitconfig`
  intentionally** (this is the one place tests could not cover).

## 11. Flagged ambiguities (non-blocking; recommended defaults chosen)

1. **Effective vs per-level presentation** (Decision 1) — RECOMMEND: form fields bind to the
   TARGET-level value; when a key is inherited (target None, effective Some) show a muted "inherited
   from <level>: <value>" hint under the field. Default target = Local when a repo is open, Global
   otherwise (but v1 always has a repo, so Local is the default). No standalone no-repo global editor
   in v1 (Decision 5).
2. **Curated set** (Decision 2) — the six keys in §4.1. Open to adding `commit.gpgsign`/`core.editor`
   later; not in v1.
3. **Multivar keys** (Decision 3) — v1 reads last/effective, edits single-valued only. Editing a
   real multivar (e.g. multiple `remote.*.fetch`) via the Advanced list would collapse it — RECOMMEND
   leaving such keys read-only-ish (the collapse is acceptable for v1); revisit if requested.
4. **`repo-changed` on write** — RECOMMEND no emission; the section refetches itself. Note
   `core.autocrlf` affects future checkouts, not the current tree, so no forced status refresh.
5. **`open_level(ConfigLevel::Global)` when `~/.gitconfig` is absent** — RECOMMEND the
   `find_global()` + `Config::open(path)` fallback in `open_target`; verify at impl time whether
   libgit2 0.21 auto-creates on `open_level(Global)` (if it does, drop the fallback).
6. **AppError** (Decision 6) — CONFIRMED: no new variant; `Git` + `InvalidName` only.
7. **Mock commit identity coupling** — to demo the identity-gap fix in the browser, the mock
   `commit()` must consult the mock config store rather than a static `?fixture=noconfig` flag.
   RECOMMEND wiring it; if too invasive, minimum is that `?fixture=noconfig` clears after a
   successful identity set.
8. **Set-identity affordance** (Decision 4) — RECOMMEND the "Set identity…" button in the commit
   error banner (threaded callback). Fallback: static hint pointing to Settings.
