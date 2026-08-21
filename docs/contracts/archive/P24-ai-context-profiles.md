# P24 — AI-asset management: inventory + drift + per-model context profiles (A1 + A2)

The flagship of the repo-management roadmap (Theme A). Bonsai becomes the repo-aware GUI for the
**AI-asset layer**: it inventories the instruction files every agent reads (`CLAUDE.md`, `AGENTS.md`,
Cursor rules, Copilot instructions, Windsurf rules, `GEMINI.md`), reports **drift** between them,
and lets the user define **context profiles** (a rich CLAUDE.md for Opus, a terse one for a cheap
model, an OpenAI-flavored AGENTS.md) and **activate** one — writing its canonical content to the
mapped target file(s), gated behind a confirm + diff preview.

Rust (`crates/bonsai-core/src/assets/`) owns ALL logic; React only renders and confirms. The new
core is **Tauri-free, network-free, and needs NO `claude` CLI** — it is pure filesystem read/write +
hashing, unit-testable with a temp-dir fixture. AI is OPTIONAL and isolated to one clearly-marked
command (§6.8).

Sub-increments (see §10): **P24a** core assets module (taxonomy + inventory + drift + read) ·
**P24b** profiles store (CRUD + preview + activate write-path) · **P24c** IPC triple + stateful
mock · **P24d** UI (inventory/drift panel + profile manager + diff-preview-gated activation) ·
**P24e** (optional) AI-generate/translate helper.

---

## 0. Invariants held

- New logic lives in a NEW pure module tree `crates/bonsai-core/src/assets/` — Tauri-free,
  runtime-free, directly unit-testable (same rule as `git/merge.rs`, `git/tags.rs`). Registered via
  a single `pub mod assets;` line in `crates/bonsai-core/src/lib.rs`.
- Errors use the existing `crate::error::AppError` — **NO `error.rs` change, no new variant.** Every
  failure maps to an existing kind (§9). The TS `AppError` union in `types.ts` is unchanged.
- Every command carries `repoId` first (resolved via `repo_path(state, &repo_id)?`), wraps the
  blocking core in `spawn_blocking` (fs + hashing is blocking), and does **NOT** emit `repo-changed`
  — the frontend refetches imperatively after every mutation. The `notify` watcher will also fire on
  a profile write and is absorbed by the existing request-id guards; the inventory panel refreshes
  on that signal too (free live-drift updates).
- **No new crate dependency.** Content hashing reuses git2's object hasher:
  `git2::Oid::hash_object(git2::ObjectType::Blob, bytes)` → a stable 40-hex SHA-1 string. No `sha2`.
- serde is `rename_all = "camelCase"`; all TS wire types are camelCase.
- **Safety (activation writes files):** activation is gated behind an explicit UI confirm + a
  per-target diff preview (current-file-content vs profile-content), never overwrites blindly, writes
  atomically (temp + rename), creates parent dirs, and **validates every target path stays inside the
  repo workdir** (reuse the `validate_rel_path` discipline from `git/stage.rs`). Never touches files
  outside the workdir.

---

## OPEN DECISIONS (recommended default in brackets; contract proceeds on the default)

1. **Canonical source for drift.** [**Auto-pick, user-overridable.**] Drift is measured against one
   reference asset. Default reference = first existing managed single-file instruction in this
   priority: `CLAUDE.md` → `AGENTS.md` → `.github/copilot-instructions.md` → `GEMINI.md` →
   `.windsurfrules` → `.cursorrules`. If none exist, `DriftReport.canonicalId = null` and every entry
   is `comparable:false`. A future setting can pin the canonical; not wired in P24 (the frontend may
   pass an optional `canonical` override to `list_ai_assets`, §6.1).
2. **What counts as "in sync".** [**Normalized-content hash equality**, §4.] Two instruction docs are
   in sync iff their *normalized* content hashes match. Normalization (§4.2) folds away EOL/BOM/
   trailing-whitespace/edge-blank-line differences only — it does NOT reflow or lowercase. Files that
   carry tool-specific frontmatter (`.mdc`, `.instructions.md`) or are rules-*dirs* are **excluded
   from the sync comparison** (`comparable:false`) but still inventoried.
3. **Profile target content: inline vs sourceRef.** [**Inline `content` in v1.**] `profiles.json`
   stores each target's full text verbatim — self-contained, diffable, version-controllable. A
   `sourceRef` (point a target at another file/asset) is deferred; the struct leaves room (§5) but
   P24 implements inline only.
4. **Profile targets: single-file assets only.** [**Yes — single-file targets only in P24.**]
   Activating into a rules-*dir* (`.cursor/rules/`, `.windsurf/rules/`) is ambiguous (which member
   file? overwrite the whole dir?) and deferred to A3. `save_profile` rejects a target whose
   `assetId` is a rules-dir with `InvalidName`.
5. **Store location + gitignore.** [**`.bonsai/profiles.json` at repo root**, created lazily on first
   `save_profile`.] Recommend the user **commit** `.bonsai/profiles.json` (profiles are meant to be
   shared/version-controlled) — so the contract does NOT add it to `.gitignore` and does NOT modify
   `.gitignore`. The UI shows a one-line hint ("Profiles are stored in `.bonsai/profiles.json` —
   commit it to share with your team"). Documented recommendation only.
6. **Activation and the working tree.** [**No git awareness.**] Activation is a plain file write; it
   does NOT stage, commit, or check the working-tree state. The written files simply appear in the
   normal status panel as modified/untracked, which the user reviews and commits through Bonsai's
   existing flow. This keeps A2 orthogonal to the git layer.
7. **Detect-only assets.** [**Inventory only, `managed:false`.**] `.claude/` (skills/agents/commands),
   `.mcp.json`, `.github/instructions/*.instructions.md`, `.github/prompts/*.prompt.md` are DETECTED
   and listed (so the panel shows the full asset picture) but are NOT drift-compared and NOT valid
   profile targets in P24. They are managed in later milestones (A3/A4/A5).
8. **AI helper scope.** [**Optional P24e, one isolated command.**] `ai_generate_asset` translates one
   existing instruction file into another agent's flavor via `run_claude`. It is gated on
   `ai_enabled && ai_consented`, writes NOTHING (returns proposed text the user pastes/saves into a
   profile target), and nothing else in P24 depends on it. If P24e is dropped, A1+A2 still ship whole.

None of these block implementation; all defaults are safe.

---

## 1. Module boundaries & file responsibilities

New core module tree `crates/bonsai-core/src/assets/` (register `pub mod assets;` in `lib.rs`,
alphabetical after `ai`):

| File | Responsibility | Increment |
|------|----------------|-----------|
| `assets/mod.rs` | `pub mod` decls; re-exports; `AssetKind`; the public API surface (`scan_inventory`, `read_asset`) | P24a |
| `assets/taxonomy.rs` | the static descriptor table of known AI-asset files/dirs (§2) — pure data + a `descriptors() -> &[AssetDescriptor]` accessor | P24a |
| `assets/inventory.rs` | walk the workdir per descriptor → `Vec<AiAsset>`; raw + normalized hashing; content normalization (§4.2) | P24a |
| `assets/drift.rs` | `DriftReport` from the inventory + a chosen canonical (§4.3) | P24a |
| `assets/profiles.rs` | `.bonsai/profiles.json` load/save; `ContextProfile` CRUD; `preview_profile`; `activate_profile` (atomic write + path validation, §5) | P24b |
| `assets/generate.rs` (optional) | `ai_generate_asset` core — builds a prompt, calls `ai::run_claude`, returns proposed text | P24e |

Command layer: `src-tauri/src/commands.rs` gains one `#[tauri::command]` + runtime-free `_inner` per
command (§6); `src-tauri/src/lib.rs` registers each in `generate_handler!`.

Frontend: IPC triple (`src/ipc/{types.ts,tauri.ts,mock.ts}`) + new UI under `src/components/`
(`AiAssetsPanel.tsx`, `ProfileManager.tsx`, `ProfileActivateDialog.tsx`) surfaced from `App.tsx`
(reuse `DiffView` for the preview where practical, §8).

---

## 2. AI-asset taxonomy (the descriptor table)

`taxonomy.rs` defines a static ordered table. Each descriptor:

```rust
pub struct AssetDescriptor {
    /// Stable slug used as the wire `id` and as a `ProfileTarget.assetId`.
    pub id: &'static str,
    /// Tool/agent this serves (wire `agent`).
    pub agent: &'static str,
    /// Human label for the UI.
    pub label: &'static str,
    pub kind: AssetKind,
    /// Repo-relative location. For SingleFile/Config: the file path.
    /// For RulesDir: the directory path; `glob` selects members.
    pub path: &'static str,
    /// Member glob for RulesDir (e.g. "*.mdc"); ignored otherwise.
    pub glob: Option<&'static str>,
    /// true => A1 drift-compares it and it is a valid profile target.
    /// false => inventory-only (detect, don't manage) in P24.
    pub managed: bool,
    /// true => excluded from the sync comparison even when managed=false is not
    /// the reason (frontmatter-bearing or dir). See §4.3 `comparable`.
    pub frontmatter: bool,
}
```

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AssetKind { SingleFile, RulesDir, Config }
```

Concrete table (order = display order):

| id | agent | label | path | kind | glob | managed | frontmatter |
|----|-------|-------|------|------|------|---------|-------------|
| `claude` | Claude Code | `CLAUDE.md` | `CLAUDE.md` | SingleFile | — | ✔ | ✘ |
| `agents` | Codex/Cursor/Gemini/Zed | `AGENTS.md` | `AGENTS.md` | SingleFile | — | ✔ | ✘ |
| `copilot` | GitHub Copilot | `copilot-instructions.md` | `.github/copilot-instructions.md` | SingleFile | — | ✔ | ✘ |
| `gemini` | Gemini CLI | `GEMINI.md` | `GEMINI.md` | SingleFile | — | ✔ | ✘ |
| `windsurf` | Windsurf (legacy) | `.windsurfrules` | `.windsurfrules` | SingleFile | — | ✔ | ✘ |
| `cursorLegacy` | Cursor (legacy) | `.cursorrules` | `.cursorrules` | SingleFile | — | ✔ | ✘ |
| `cursorRules` | Cursor | `.cursor/rules/` | `.cursor/rules` | RulesDir | `*.mdc` | ✔ (inventory) | ✔ |
| `windsurfRules` | Windsurf | `.windsurf/rules/` | `.windsurf/rules` | RulesDir | `*.md` | ✔ (inventory) | ✔ |
| `copilotInstr` | GitHub Copilot | `.github/instructions/` | `.github/instructions` | RulesDir | `*.instructions.md` | ✘ | ✔ |
| `copilotPrompts` | GitHub Copilot | `.github/prompts/` | `.github/prompts` | RulesDir | `*.prompt.md` | ✘ | ✔ |
| `claudeDir` | Claude Code | `.claude/` (skills/agents/commands) | `.claude` | Config | — | ✘ | ✔ |
| `mcp` | MCP clients | `.mcp.json` | `.mcp.json` | Config | — | ✘ | ✘ |

Notes:
- **Drift-comparable** set = descriptors with `managed && !frontmatter && kind==SingleFile` →
  `claude, agents, copilot, gemini, windsurf, cursorLegacy`. Everything else is inventoried but
  `comparable:false`.
- Rules-dirs (`cursorRules`, `windsurfRules`) are managed=✔ only for **inventory display** (member
  file listing); they are NOT drift-compared and NOT profile targets in P24 (OPEN #4). The table's
  `managed` flag drives the UI's "managed vs detected" grouping; §4.3 uses the finer
  drift-comparable predicate above, not `managed` alone.
- Also detect `CLAUDE.local.md` as a variant row of `claude` if present? **No** in P24 — keep the
  table fixed; nested/local variants are a later refinement.

---

## 3. Rust data model — inventory

`assets/mod.rs` / `inventory.rs`:

```rust
/// One concrete file inside an asset (the single file for SingleFile/Config, or
/// each matched member for RulesDir). Paths are repo-relative, forward slashes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetFile {
    pub path: String,
    /// Byte length of the raw file.
    pub size: u64,
    /// git-blob SHA-1 (40 hex) of the RAW bytes.
    pub content_hash: String,
    /// git-blob SHA-1 (40 hex) of the NORMALIZED content (§4.2).
    pub normalized_hash: String,
    /// mtime, epoch seconds; None if unavailable.
    pub modified: Option<i64>,
}

/// One detected AI-asset target (a taxonomy descriptor resolved against the repo).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAsset {
    pub id: String,       // descriptor id
    pub agent: String,
    pub label: String,
    pub kind: AssetKind,
    pub path: String,     // file or dir, repo-relative
    pub managed: bool,
    pub exists: bool,
    /// SingleFile/Config: 0 or 1 entry. RulesDir: 0..N matched members (sorted by path).
    pub files: Vec<AssetFile>,
}

/// Full inventory + drift, returned by `list_ai_assets` in one round-trip.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAssetInventory {
    pub assets: Vec<AiAsset>,
    pub drift: DriftReport,
}
```

```rust
/// Blocking. Scan `workdir` for every taxonomy descriptor, hashing existing files.
/// Never touches anything outside `workdir`. `canonical`: optional override for
/// the drift reference asset id (OPEN #1); `None` => auto-pick.
pub fn scan_inventory(workdir: &Path, canonical: Option<&str>)
    -> Result<AiAssetInventory, AppError>;

/// Blocking. Read one asset FILE's raw content (a specific repo-relative path,
/// validated inside `workdir`). Used by the editor/preview. Returns the content
/// or `exists:false`. `path` must match a file under a known descriptor (defensive:
/// path is validated, but any in-workdir path is allowed to read).
pub fn read_asset(workdir: &Path, path: &str) -> Result<AssetContent, AppError>;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetContent {
    pub path: String,
    pub exists: bool,
    /// None when `exists == false` or the file is not valid UTF-8 (lossy-decoded
    /// otherwise). Raw content, unnormalized.
    pub content: Option<String>,
}
```

---

## 4. Drift model + normalization

### 4.1 Data

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftReport {
    /// The reference asset id, or None if no comparable single-file exists.
    pub canonical_id: Option<String>,
    /// Normalized hash of the canonical, or None.
    pub canonical_hash: Option<String>,
    /// One entry per drift-comparable descriptor (the §2 set), in table order.
    pub entries: Vec<DriftEntry>,
    /// Convenience: true iff every EXISTING comparable asset is in sync with the
    /// canonical (or there is 0/1 existing comparable asset). Drives the panel badge.
    pub in_sync: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftEntry {
    pub asset_id: String,
    pub exists: bool,
    /// false => not compared (missing, or not in the drift-comparable set).
    pub comparable: bool,
    /// Normalized hash when comparable && exists, else None.
    pub normalized_hash: Option<String>,
    /// true iff comparable && exists && normalized_hash == canonical_hash.
    /// The canonical entry itself is in_sync:true.
    pub in_sync: bool,
}
```

### 4.2 Normalization rules (LOCKED — state them exactly)

Given raw bytes of a comparable single-file asset, `normalize(raw) -> String`:
1. Lossy-decode UTF-8 (`String::from_utf8_lossy`).
2. Strip a leading UTF-8 BOM (`\u{FEFF}`) if present.
3. Normalize line endings: `replace("\r\n", "\n").replace('\r', "\n")`.
4. Right-trim trailing whitespace on **each** line (spaces/tabs).
5. Trim leading and trailing blank lines from the whole document.
6. Ensure exactly one trailing `\n` (append if the trimmed body is non-empty; empty stays empty).

No lowercasing, no reflow, no internal blank-line collapsing, no markdown parsing. The
`normalized_hash` = `git2::Oid::hash_object(Blob, normalize(raw).as_bytes())`.

### 4.3 Drift algorithm

```
comparable_ids = descriptors where managed && !frontmatter && kind==SingleFile   // §2 set
existing_comparable = [ id in comparable_ids if asset(id).exists ]

canonical_id =
    if override given and override in existing_comparable: override
    else first id in PRIORITY [claude, agents, copilot, gemini, windsurf, cursorLegacy]
         that is in existing_comparable
    else None
canonical_hash = normalized_hash(canonical_id)     // None if canonical_id is None

for id in comparable_ids:                            // table order
    entry.assetId   = id
    entry.exists    = asset(id).exists
    entry.comparable= true                            // it is in the comparable set
    entry.normalizedHash = if exists { asset(id).files[0].normalized_hash } else None
    entry.inSync    = exists && canonical_hash.is_some()
                       && entry.normalizedHash == canonical_hash

report.inSync = every existing_comparable entry has inSync==true   // (0 or 1 existing => true)
```

Assets outside the comparable set do NOT appear in `DriftReport.entries` (they are in
`inventory.assets` with full file listing). The panel renders drift only for the comparable set and
lists the rest as "detected".

---

## 5. Profile model + store

### 5.1 Data (`profiles.rs`)

```rust
/// One profile target: which taxonomy asset to write, and the verbatim content.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileTarget {
    /// A descriptor id; MUST be a SingleFile descriptor (OPEN #4). Rules-dir /
    /// Config ids are rejected by `save_profile` with InvalidName.
    pub asset_id: String,
    /// Verbatim content written to the mapped file on activation.
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextProfile {
    /// Unique key within the store; also the display name. Validated (§5.3).
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Informational label (e.g. "opus", "haiku", "gpt-5"); not enforced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub targets: Vec<ProfileTarget>,
}

/// The on-disk store (`.bonsai/profiles.json`) AND the wire shape of list/save/delete.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileStore {
    /// Schema version; current = 1. Forward-compatible: unknown-higher versions
    /// still load (serde ignores unknown fields) but the UI may warn.
    pub version: u32,
    #[serde(default)]
    pub profiles: Vec<ContextProfile>,
    /// Name of the last activated profile, or None. Informational (activation does
    /// not "lock" anything — it just wrote files).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,
}
```

Store path: `<workdir>/.bonsai/profiles.json`. Load: missing file / missing `.bonsai/` dir →
`ProfileStore { version:1, profiles:[], active_profile:None }` (NOT an error — lazy). Malformed JSON
→ `AppError::Other("profiles.json is corrupt: …")`. Save: create `.bonsai/` if absent, write
pretty-printed JSON atomically (temp + rename), always stamp `version:1`.

### 5.2 CRUD + preview + activate

```rust
/// Blocking. Load the store (lazy default if absent).
pub fn list_profiles(workdir: &Path) -> Result<ProfileStore, AppError>;

/// Blocking. Insert or replace the profile keyed by `profile.name`, then persist.
/// Validates name (§5.3) and every target (`assetId` is a known SingleFile
/// descriptor; else InvalidName). Returns the updated store.
pub fn save_profile(workdir: &Path, profile: ContextProfile) -> Result<ProfileStore, AppError>;

/// Blocking. Remove the profile named `name` (no-op if absent), clear
/// `active_profile` if it pointed there, persist. Returns the updated store.
pub fn delete_profile(workdir: &Path, name: &str) -> Result<ProfileStore, AppError>;

/// Blocking. Compute, WITHOUT WRITING, the per-target before/after for the named
/// profile's activation. `current` = existing mapped-file content (None if absent).
pub fn preview_profile(workdir: &Path, name: &str)
    -> Result<Vec<ProfilePreviewEntry>, AppError>;

/// Blocking. WRITE each target's content to its mapped file (atomic temp+rename,
/// parent dirs created), set `active_profile = name`, persist the store. Returns
/// a per-target summary. This is the ONLY write-to-instruction-files path; the UI
/// gates it behind confirm + the §5.2 preview (§8.3).
pub fn activate_profile(workdir: &Path, name: &str)
    -> Result<ProfileActivation, AppError>;
```

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePreviewEntry {
    pub asset_id: String,
    pub path: String,          // resolved repo-relative mapped file
    pub current: Option<String>,
    pub proposed: String,
    /// true iff `current` differs from `proposed` (byte-exact; a missing file differs).
    pub changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum TargetWriteAction { Created, Written, Unchanged }

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetWriteResult {
    pub asset_id: String,
    pub path: String,
    pub action: TargetWriteAction,   // created (new file) | written (overwrote) | unchanged
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileActivation {
    pub profile: String,
    pub results: Vec<TargetWriteResult>,
    /// The store after `active_profile` was updated (frontend refreshes from this).
    pub store: ProfileStore,
}
```

`activate_profile` flow (all validation before any write):
1. Load store; find `name` → not found → `AppError::Other("profile '<name>' not found")`.
2. For each target: resolve `assetId` → descriptor; not a SingleFile → `InvalidName`; take
   `descriptor.path` as the mapped repo-relative path; `validate_rel_path(path)?` (reuse the
   `stage.rs` rule — rejects absolute / `..` / backslash; belt-and-suspenders since descriptor paths
   are static and safe).
3. For each target (now known-safe): read current bytes (if any); if equal to `content` →
   `Unchanged` (skip write); else create parent dirs, write `content` to `<path>.bonsai-tmp`, then
   `rename` over `<path>` → `Created` (file was absent) or `Written`.
4. Set `store.active_profile = Some(name)`, persist store (atomic).
5. Return `ProfileActivation`. On any I/O error mid-loop, return `AppError::Io(...)` — targets
   already written are left as-is (documented; atomic per-file, not transactional across files).

### 5.3 Validation

`validate_profile_name`: reject blank / leading `-` / path separators (`/`, `\`) / control chars →
`AppError::InvalidName("invalid profile name: '<name>'")`. Names are also the JSON store key, so
uniqueness is enforced by replace-on-save (same-name save overwrites).

---

## 6. Command surface (`commands.rs` + `lib.rs`)

Each command: `pub async fn NAME(state, repo_id, …) -> Result<T, AppError>` + runtime-free
`NAME_inner` that resolves `repo_path(state, &repo_id)?` and runs the core under
`spawn_blocking(move || assets::…)` (identical template to `stage` / `create_stash`). Register all in
`lib.rs` `generate_handler!`. **No events, no channels** (the watcher already emits `repo-changed` on
the profile write; the panel listens to that existing signal).

| Command (snake) | IPC method (camel) | Args | Returns | Error kinds |
|---|---|---|---|---|
| `list_ai_assets` | `listAiAssets` | `repoId, canonical?` | `AiAssetInventory` | `io \| noRepo` |
| `read_ai_asset` | `readAiAsset` | `repoId, path` | `AssetContent` | `other \| io \| noRepo` |
| `list_profiles` | `listProfiles` | `repoId` | `ProfileStore` | `other \| io \| noRepo` |
| `save_profile` | `saveProfile` | `repoId, profile` | `ProfileStore` | `invalidName \| other \| io \| noRepo` |
| `delete_profile` | `deleteProfile` | `repoId, name` | `ProfileStore` | `other \| io \| noRepo` |
| `preview_profile` | `previewProfile` | `repoId, name` | `ProfilePreviewEntry[]` | `other \| io \| noRepo` |
| `activate_profile` | `activateProfile` | `repoId, name` | `ProfileActivation` | `invalidName \| other \| io \| noRepo` |
| `ai_generate_asset` (P24e) | `aiGenerateAsset` | `repoId, sourceAssetId, targetAgent, guidance?` | `AiGeneratedAsset` | `aiUnavailable \| aiFailed \| other \| io \| noRepo` |

### 6.1 `list_ai_assets` — optional canonical override

`canonical: Option<String>` (a descriptor id). Tauri passes it as an optional arg
(`invoke('list_ai_assets', { repoId, canonical })`, `canonical` may be `undefined`). Forwarded to
`scan_inventory(workdir, canonical.as_deref())`.

### 6.8 `ai_generate_asset` (optional, P24e)

Isolated AI helper. Enforces the consent gate **before** `repo_path`, exactly like
`generate_commit_message_inner` (`commands.rs:1313`): resolve `settings_file(&app)`, load settings,
`if !(s.ai_enabled && s.ai_consented) { return Err(AppError::AiUnavailable("AI features are
disabled — enable them in Settings")) }`. Then read the `sourceAssetId` file, build a prompt
("Rewrite this AI-agent instruction file for <targetAgent>; keep the guidance identical, adapt
tone/format to that tool's conventions; output only the file body"), call
`assets::generate::generate_asset(&workdir, source_content, target_agent, guidance, RunOpts::default())`
under `spawn_blocking` → `ai::run_claude`. **Writes nothing** — returns the proposed text; the user
reviews it and saves it into a profile target or a file via the existing edit path.

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiGeneratedAsset {
    pub target_agent: String,
    /// Proposed content (fence-stripped by run_claude). NOT written anywhere.
    pub content: String,
}
```

### 6.2 TypeScript wire types (`src/ipc/types.ts`)

```ts
export type AssetKind = 'singleFile' | 'rulesDir' | 'config';

export interface AssetFile {
  path: string;
  size: number;            // u64 on the wire; safe as a JS number here
  contentHash: string;
  normalizedHash: string;
  modified: number | null; // epoch seconds
}

export interface AiAsset {
  id: string;
  agent: string;
  label: string;
  kind: AssetKind;
  path: string;
  managed: boolean;
  exists: boolean;
  files: AssetFile[];
}

export interface DriftEntry {
  assetId: string;
  exists: boolean;
  comparable: boolean;
  normalizedHash: string | null;
  inSync: boolean;
}

export interface DriftReport {
  canonicalId: string | null;
  canonicalHash: string | null;
  entries: DriftEntry[];
  inSync: boolean;
}

export interface AiAssetInventory {
  assets: AiAsset[];
  drift: DriftReport;
}

export interface AssetContent {
  path: string;
  exists: boolean;
  content: string | null;
}

export interface ProfileTarget {
  assetId: string;
  content: string;
}

export interface ContextProfile {
  name: string;
  description?: string | null;
  model?: string | null;
  targets: ProfileTarget[];
}

export interface ProfileStore {
  version: number;
  profiles: ContextProfile[];
  activeProfile?: string | null;
}

export interface ProfilePreviewEntry {
  assetId: string;
  path: string;
  current: string | null;
  proposed: string;
  changed: boolean;
}

export type TargetWriteAction = 'created' | 'written' | 'unchanged';

export interface TargetWriteResult {
  assetId: string;
  path: string;
  action: TargetWriteAction;
}

export interface ProfileActivation {
  profile: string;
  results: TargetWriteResult[];
  store: ProfileStore;
}

// P24e (optional):
export interface AiGeneratedAsset {
  targetAgent: string;
  content: string;
}
```

`TargetWriteResult.action` on the Rust side is the internally-tagged `TargetWriteAction` enum
serialized as `{ "action": "written" }`; because the enum carries no data it flattens to a bare
string field on `TargetWriteResult` when written as `pub action: TargetWriteAction`. To keep the TS
shape `action: 'created' | 'written' | 'unchanged'` simple, implement `TargetWriteAction` as a
**field-less serde enum with `rename_all="camelCase"` (NO `tag`)** so it serializes to the bare
string `"written"`. (Correction to §5.2: drop the `#[serde(tag = "action")]` line — a plain unit enum
gives the string form the TS union expects.)

### 6.3 `IpcApi` additions + `tauri.ts`

```ts
// IpcApi:
listAiAssets(repoId: string, canonical?: string): Promise<AiAssetInventory>;
readAiAsset(repoId: string, path: string): Promise<AssetContent>;
listProfiles(repoId: string): Promise<ProfileStore>;
saveProfile(repoId: string, profile: ContextProfile): Promise<ProfileStore>;
deleteProfile(repoId: string, name: string): Promise<ProfileStore>;
previewProfile(repoId: string, name: string): Promise<ProfilePreviewEntry[]>;
activateProfile(repoId: string, name: string): Promise<ProfileActivation>;
aiGenerateAsset(repoId: string, sourceAssetId: string, targetAgent: string,
                guidance?: string): Promise<AiGeneratedAsset>; // P24e
```

`tauri.ts` — one thin `invoke` each, camelCase arg keys, e.g.
`invoke('list_ai_assets', { repoId, canonical })`, `invoke('save_profile', { repoId, profile })`,
`invoke('activate_profile', { repoId, name })`.

---

## 7. Mock IPC (`src/ipc/mock.ts`) — keep the browser harness implementable

Add a per-repo, STATEFUL profiles + assets slice to `MockRepoState` and implement all methods on
`mockIpc` reusing `requireRepo`, `delay`, and the existing fixture helpers.

Fixture seed (module-level constants in `mock.ts` or a new `src/ipc/fixtures/aiAssets.ts`):
- `mockInventory`: an `AiAssetInventory` where `CLAUDE.md`, `AGENTS.md`, `.github/copilot-instructions.md`
  exist; make `AGENTS.md` **drifted** (different `normalizedHash`) and `copilot` **in sync** with the
  canonical `claude`, so the panel shows both states. Include one detected rules-dir
  (`.cursor/rules` with 2 members) and `.mcp.json` as `managed:false`.
- `mockProfiles`: a `ProfileStore` with two profiles — `"opus-rich"` (targets: `claude`, `agents`)
  and `"cheap-terse"` (target: `claude`) — `activeProfile: null`.

Behaviors:
- `listAiAssets(repoId, canonical?)`: return `state.inventory`; if `canonical` given and comparable,
  recompute `drift.canonicalId/Hash/inSync` client-side from the members' `normalizedHash` (small
  helper mirroring §4.3) so the override is demonstrable.
- `readAiAsset(repoId, path)`: return the fixture content for known paths (a canned markdown string),
  else `{ path, exists:false, content:null }`.
- `listProfiles`: return `state.profiles`.
- `saveProfile(repoId, profile)`: validate name (throw `{kind:'invalidName'}` on blank/separators);
  reject a target whose `assetId` is not a single-file descriptor (`invalidName`); upsert by name;
  return the updated store.
- `deleteProfile(repoId, name)`: remove; clear `activeProfile` if it matched; return store.
- `previewProfile(repoId, name)`: for each target, `current` = the fixture content of the mapped
  file (or null), `proposed` = the target content, `changed` = inequality.
- `activateProfile(repoId, name)`: compute `results` (created/written/unchanged vs the fixture
  current), **mutate the in-memory fixture** so the mapped files now hold the profile content and the
  drift recomputes (visual fidelity: after activating `opus-rich`, `AGENTS.md` becomes in-sync), set
  `state.profiles.activeProfile = name`, return `{ profile, results, store }`.
- `aiGenerateAsset(...)` (P24e): return a canned `{ targetAgent, content: "# <agent> instructions…" }`
  after a `delay`; gate on a mock `aiEnabled` flag if present, else always succeed.

Because activation mutates the mock inventory, the harness can screenshot the full drift→activate→
in-sync loop with no backend.

---

## 8. Frontend behavior (P24d)

New top-level surface reachable from `App.tsx` (e.g. an "AI Assets" tab/panel beside the existing
repo workspace). Three regions:

### 8.1 Inventory + drift panel (`AiAssetsPanel.tsx`)
- Header badge from `inventory.drift.inSync`: green "In sync" / amber "N file(s) drifted".
- **Managed instruction files** group: one row per drift-comparable asset — label, path, exists?,
  and a sync chip (`canonical` / `in sync` / `drifted` / `missing`). Clicking a drifted row opens a
  read-only **DiffView** (reuse the existing component) of canonical-vs-this (fetch both via
  `readAiAsset`, diff client-side or via the existing diff plumbing — a simple two-pane text compare
  is acceptable for v1).
- **Detected (not managed)** group: rules-dirs (with member count) + `.mcp.json` + `.claude/`, listed
  read-only with a "managed in a later release" note.
- Manual **Refresh** button (calls `listAiAssets`) plus auto-refresh on the `repo-changed` event.

### 8.2 Profile manager (`ProfileManager.tsx`)
- List profiles from `listProfiles`; show `activeProfile` with an "active" chip.
- Create/edit form: name, optional description, optional model label, and a target editor (add a
  target → pick a single-file `assetId` from a dropdown of managed single-file descriptors → a
  textarea for content; "Load from current file" button prefills via `readAiAsset`). Save →
  `saveProfile`. Delete (confirm) → `deleteProfile`.
- "Store hint" line (OPEN #5): *"Profiles live in `.bonsai/profiles.json` — commit it to share."*

### 8.3 Activation — confirm + diff preview (`ProfileActivateDialog.tsx`) — SAFETY GATE
- "Activate" on a profile opens a modal that first calls `previewProfile(repoId, name)` and renders,
  per target, a **DiffView** of `current` (left) vs `proposed` (right), flagging `changed` targets
  and new-file (`current:null`) targets.
- The primary **Activate & write files** button is enabled only after the preview loads; it calls
  `activateProfile`. Cancel writes nothing. This satisfies the "explicit confirm + diff preview,
  never overwrite blindly" requirement.
- On success → refresh inventory + profiles from `ProfileActivation.store`; toast
  `Activated '<name>' — wrote N file(s)` (`success`); if all `unchanged`, toast `info` "No changes —
  files already match the profile".

### 8.4 Toasts / errors
- `saveProfile`/`activateProfile` `invalidName` → `error` with the backend message (shown inline in
  the form for save).
- `io`/`other` → `error` toast with the message.
- P24e `aiGenerateAsset` `aiUnavailable` → `info` toast pointing to Settings; `aiFailed` → `error`.

---

## 9. Error mapping (no `error.rs` change)

| Situation | Variant | TS kind |
|---|---|---|
| Blank/separator profile or target-asset invalid | `InvalidName` | `invalidName` |
| Profile not found / corrupt profiles.json / read-path not under a descriptor | `Other` | `other` |
| Filesystem read/write/rename failure | `Io` | `io` |
| AI disabled (P24e) or CLI missing | `AiUnavailable` | `aiUnavailable` |
| AI call failed/timed out (P24e) | `AiFailed` | `aiFailed` |
| Unknown `repoId` | `NoRepo` | `noRepo` |

`validate_rel_path` currently returns `AppError::Other("invalid path: …")` → `other`; reused as-is.

---

## 10. Sub-increment breakdown (each = one fresh-context `senior-dev` pass)

- **P24a — core assets module: taxonomy + inventory + drift + read.**
  - Rust: `assets/{mod.rs,taxonomy.rs,inventory.rs,drift.rs}`; `pub mod assets;` in `lib.rs`;
    `AssetKind`, `AssetFile`, `AiAsset`, `AiAssetInventory`, `AssetContent`, `DriftReport`,
    `DriftEntry`; `scan_inventory`, `read_asset`, `normalize`, git-blob hashing.
  - Commands: `list_ai_assets`, `read_ai_asset` (+ `lib.rs`).
  - IPC triple: inventory/drift/content types + two methods in `types.ts`/`tauri.ts`/`mock.ts`
    (with `mockInventory` fixture).
  - Tests: §11 fs-oracle rows 1–4.
  - Acceptance: `scan_inventory` over a temp dir with a known file drop yields the exact asset set,
    exists flags, member listing, and drift verdict; harness renders the inventory panel from mock.
- **P24b — profiles store: CRUD + preview + activate (write path).**
  - Rust: `assets/profiles.rs` — `ProfileStore`/`ContextProfile`/`ProfileTarget`/preview/activation
    types; `list/save/delete/preview/activate_profile`; atomic write; `validate_profile_name`;
    path validation on targets.
  - Commands: five profile commands (+ `lib.rs`).
  - IPC triple: profile types + five methods; `mockProfiles` fixture + stateful mock mutation.
  - Tests: §11 rows 5–9.
  - Acceptance: round-trip save→list→activate writes the mapped files (temp+rename) and updates
    `activeProfile`; preview writes nothing; invalid name/target rejected.
- **P24c — IPC wiring polish + mock completeness.** (May merge into P24a/b if small.) Ensure the
  full triple compiles, `mock.ts` implements every method, `tsc`/`pnpm build` clean.
- **P24d — UI surface.** `AiAssetsPanel`, `ProfileManager`, `ProfileActivateDialog`; `App.tsx`
  wiring; `repo-changed` refresh; DiffView reuse; toasts. Acceptance: browser-harness screenshots of
  the drift panel, profile editor, and the confirm+diff activation dialog.
- **P24e — (optional) AI generate/translate helper.** `assets/generate.rs`; `ai_generate_asset`
  command (consent-gated); IPC triple; a "Translate for <agent>" button in the profile target editor.
  Acceptance: CLI-stub test (reuse the `claude_stub.cmd` harness) returns proposed text; disabled AI
  yields `aiUnavailable`.

Commit each approved sub-increment as `wip(P24a): …` etc. (orchestrator owns commits).

---

## 11. Tests (AI gate)

### 11.1 Rust unit tests (`#[cfg(test)]` in each module)
Use `tempfile::TempDir` (dev-dep already present) for a scratch workdir; no git repo needed (the core
is fs-only). `TMP`/`TEMP=D:\Temp` per the user mandate; run `cargo test` and `clippy` **sequentially**.

1. **Empty repo** — no AI files → every asset `exists:false`, `files:[]`; `drift.canonicalId:None`,
   `drift.inSync:true`.
2. **Inventory + hashing** — drop `CLAUDE.md`, `AGENTS.md`, two `.cursor/rules/*.mdc`; assert the
   asset set, `exists`, member count/sorting, and that `content_hash` == `git hash-object` of the raw
   bytes (compute via `git2::Oid::hash_object` in the test as the oracle).
3. **Normalization** — a `normalize` table: CRLF vs LF, BOM, trailing spaces, edge blank lines, and a
   final-newline-missing file all collapse to the same `normalized_hash`; a genuine content change
   does not. Assert `normalize("x\r\n\r\n")` etc. byte-exactly.
4. **Drift** — CLAUDE.md and AGENTS.md with identical *content but different EOL* → `inSync:true`;
   change one word in AGENTS.md → that entry `inSync:false`, `report.inSync:false`; canonical
   auto-picks `claude`; override to `agents` flips the reference; no comparable file → `canonicalId:None`.
5. **Wire shapes** — `serde_json::to_value` on `AiAssetInventory`, `ProfileStore`, `ProfileActivation`
   asserts camelCase keys and that `AssetKind`/`TargetWriteAction` serialize to bare strings
   (`"singleFile"`, `"written"`).
6. **Store lazy default + persist** — `list_profiles` on a bare temp dir returns the empty default
   (no file created); `save_profile` creates `.bonsai/profiles.json` and a re-load round-trips it;
   corrupt JSON → `Other`.
7. **Save validation** — blank/separator name → `InvalidName`; a target with a rules-dir/config
   `assetId` → `InvalidName`.
8. **Preview writes nothing** — `preview_profile` returns correct `current/proposed/changed` and the
   mapped files are byte-identical before and after the call (assert mtimes/content unchanged).
9. **Activate** — `activate_profile` creates a missing target (`Created`), overwrites a differing one
   (`Written`), skips an identical one (`Unchanged`); the files hold byte-exact `content` afterward;
   `active_profile` is set; a second identical activation reports all `Unchanged`. Assert the written
   file never contains a `.bonsai-tmp` remnant (rename cleaned up). Path-escape defense: a hand-built
   profile whose target maps outside the workdir cannot occur via the static table, but assert
   `validate_rel_path` rejects `..`/absolute defensively.

### 11.2 Frontend AI gate
`pnpm build` + `tsc` clean; browser harness (`VITE_MOCK_IPC=1`, port 1420) renders: the drift panel
(in-sync + drifted chips), a drifted-row DiffView, the profile manager (create/edit/delete), and the
activation confirm dialog with the per-target current-vs-proposed DiffView; activating in the mock
flips the drifted asset to in-sync (screenshot the before/after).

---

## 12. Acceptance criteria — AI gate vs USER CHECKPOINT

**AI gate (orchestrator-verifiable, no network, no native window, no `claude` CLI):**
- `cargo check` + `clippy` clean on `bonsai-core`; `pnpm build` + `tsc` clean.
- All §11.1 fs-oracle unit tests green (temp-dir fixtures; `git hash-object` oracle for hashing).
- Browser-harness screenshots per §11.2: inventory/drift panel, profile editor, and the
  confirm+diff-preview activation dialog, plus the mock drift→activate→in-sync transition.
- (P24e only) CLI-stub test green; AI-disabled path returns `aiUnavailable`.

**USER CHECKPOINT (native `pnpm tauri dev`, real repo):**
- Open a repo that has several instruction files; confirm the inventory lists them correctly and the
  drift badge/chips match reality (cross-check by eye and with a real edit that introduces drift).
- Create a profile with two targets, hit Activate, confirm the diff-preview dialog shows accurate
  current-vs-proposed for each target, and that confirming writes the real files (verify on disk /
  in the status panel) while Cancel writes nothing.
- Confirm `.bonsai/profiles.json` is created on first save and is a sensible, commit-able JSON file.
- Confirm activation never overwrites without the confirm dialog, and that re-activating an
  already-applied profile reports "no changes".
- (P24e only) With the real `claude` CLI + consent enabled, "Translate for <agent>" returns a sane
  proposed instruction file; with AI disabled the action is blocked with a clear message.
```
