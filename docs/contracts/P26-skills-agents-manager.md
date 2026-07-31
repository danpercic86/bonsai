# P26 — Skills / Subagents / Slash-commands manager (Theme A, A3)

The direct follow-on to the P24 flagship. Bonsai already **inventories** the `.claude/` bundle as a
single detected row (`claudeDir`, managed:false). P26 turns the three agent-asset kinds inside it —
**skills**, **subagents**, **slash commands** — into a **managed surface**: parse + validate their
frontmatter, list them with validation badges, and provide form-based **CRUD** (create / edit /
delete) gated behind atomic writes + explicit delete confirmation.

Rust (`crates/bonsai-core/src/assets/`) owns ALL logic (parse / validate / read / write). React only
renders + confirms. The new core is a NEW pure module — Tauri-free, network-free, needs NO `claude`
CLI — unit-testable with a `tempfile::TempDir` fixture. It reuses P24's exact patterns: the IPC
"triple", the `#[tauri::command]` + `_inner` + `spawn_blocking` template, `crate::error::AppError`
(no new variant), `validate_rel_path` from `git/stage.rs`, and the atomic temp+rename write from
`assets/profiles.rs`.

Sub-increments (see §10): **P26a** core parse/validate/inventory + read commands + IPC (read-only) ·
**P26b** write path (create/edit/delete) + commands + IPC + stateful mock · **P26c** UI section +
editor · **P26d** (optional) richer templates + generic frontmatter-key editor.

---

## 0. Invariants held

- New logic lives in a NEW pure module `crates/bonsai-core/src/assets/bundle.rs` (register
  `pub mod bundle;` + re-exports in `assets/mod.rs`). **Module name is `bundle` (the ".claude/
  bundle"), NOT `agents`**, to avoid clashing with the "agent" *kind*. Tauri-free, runtime-free,
  directly unit-testable (same rule as `assets/inventory.rs`, `assets/profiles.rs`).
- Errors use existing `crate::error::AppError` — **NO `error.rs` change, no new variant** (§9). The
  TS `AppError` union is unchanged.
- Every command carries `repoId` first (resolved via `repo_path(state, &repo_id)?`), wraps the
  blocking core in `spawn_blocking` (fs is blocking), and emits **no events / no channels** — the
  frontend refetches imperatively after every mutation, and the existing `notify` watcher fires
  `repo-changed` on any `.claude/` write (the panel already refreshes on that signal, §8).
- **No new crate dependency** — frontmatter is parsed by a minimal hand-rolled splitter (§4),
  NOT `serde_yaml`. Justification in §4.
- serde `rename_all = "camelCase"`; field-less enums are plain unit enums → bare camelCase strings
  (P24 §6.2 rule). All TS wire types camelCase.
- **Safety (writes create/edit/delete files):** atomic temp+rename (reuse the `atomic_write` idiom
  from `profiles.rs`), parent-dir creation, path validation staying inside `.claude/` within the
  workdir (`validate_rel_path` on the computed rel path + a stricter `validate_asset_name` on the
  name, §4.4). Every delete requires an explicit UI confirm (§8).
- The P24 `claudeDir` taxonomy row is **left unchanged** (still `managed:false`, still a single
  "detected" row in the drift panel). P26's managed CRUD is a **separate surface** with its own
  inventory (`list_agent_assets`) — it does NOT feed drift or profile-target logic.

---

## 1. File formats (sourced — state precisely)

Sourced from the official Claude Code docs (fetched 2026-08, `code.claude.com/docs/en/`):
`sub-agents`, `slash-commands`, and `skills`. Skills follow the **Agent Skills** open standard
(`agentskills.io`); Claude Code extends it. Per the docs, "custom commands have been merged into
skills" — a `.claude/commands/<name>.md` and a `.claude/skills/<name>/SKILL.md` both create
`/<name>` and share the same frontmatter reference — but the two on-disk shapes remain distinct, so
P26 models them as **three kinds** that map to three locations.

### 1.1 Subagents — `.claude/agents/<name>.md`
- Markdown file with YAML frontmatter + a body that becomes the **system prompt**.
- **Required:** `name` (unique id, lowercase letters + hyphens, may not contain `:`),
  `description` (when Claude should delegate).
- **Optional (known):** `tools` (comma-separated string), `model`
  (`sonnet|opus|haiku|fable|<full-id>|inherit`, default `inherit`).
- **Optional (many more, must be PRESERVED but not surfaced as first-class inputs in v1):**
  `disallowedTools`, `permissionMode`, `maxTurns`, `skills`, `mcpServers`, `hooks`, `memory`,
  `background`, `effort`, `isolation`, `color`, `initialPrompt`.

### 1.2 Skills — `.claude/skills/<name>/SKILL.md`
- The skill owns its **directory**; `SKILL.md` (required file) holds frontmatter + a markdown body
  (the instructions). Supporting files (scripts/templates/references) may sit beside it.
- **All frontmatter fields optional**; `description` is *recommended* (if omitted, the first body
  paragraph is used). The command name comes from the **directory name**, not `name`.
- **Optional (known):** `name` (display label; defaults to dir name), `description`,
  `argument-hint`, `allowed-tools` (space/comma string or YAML list), `model`,
  `disable-model-invocation` (bool).
- **Optional (more, PRESERVED):** `when_to_use`, `arguments`, `user-invocable`, `disallowed-tools`,
  `effort`, `context`, `agent`, `background`.

### 1.3 Slash commands — `.claude/commands/<name>.md`
- Markdown file with **optional** YAML frontmatter + a body (the prompt template; supports
  `$ARGUMENTS`, `$1`…, `!`-bash, `@`-file refs — treated as opaque body text in v1).
- **Optional (known):** `description`, `argument-hint`, `allowed-tools`, `model`,
  `disable-model-invocation`. No required fields.

**Naming (all three, v1):** the on-disk id is the file stem (`agents`/`commands`) or the directory
name (`skills`). Docs specify lowercase-and-hyphens for agents; P26 enforces only *filesystem-safe*
names hard (§4.4) and surfaces "not lowercase-hyphen" as a **warning**, so existing valid-on-disk
assets never fail to load.

**Default note:** where a field's serialization is ambiguous (e.g. `tools`/`allowed-tools` may be a
comma string OR a YAML list), v1 treats **every frontmatter value as an opaque single-line scalar
string** (§4) — the mainstream inline form. Multi-line/sequence frontmatter is detected and the
asset is flagged read-only (§4.3), never silently rewritten.

---

## 2. Module boundaries & file responsibilities

| File | Responsibility | Increment |
|------|----------------|-----------|
| `assets/bundle.rs` | Everything: `AgentAssetKind`, `FrontmatterField`, `AgentAsset`, `AgentAssetInput`, `AgentAssetInventory`, `Validation`, `AssetIssue`; the per-kind spec table; `parse_frontmatter`/`serialize_asset`; `scan_agent_assets`, `read_agent_asset`, `save_agent_asset`, `delete_agent_asset`; `validate_asset_name` | P26a (read) + P26b (write) |
| `assets/mod.rs` | add `pub mod bundle;` + re-exports (alphabetical, after `ai`/before `drift`) | P26a |

Command layer: `src-tauri/src/commands.rs` gains one `#[tauri::command]` + runtime-free `_inner` per
command (§5); `src-tauri/src/lib.rs` registers each in `generate_handler!` (append after
`ai_generate_asset`).

Frontend: IPC triple (`src/ipc/{types.ts,tauri.ts,mock.ts}`); UI extends `AiAssetsPanel.tsx` with a
new "Agent assets" section + a new `AgentAssetEditor.tsx` (form) reusing `ConfirmDialog`, the toast
context, and the ProfileManager form idioms.

---

## 3. Rust data model (`bundle.rs`, serde camelCase)

**Decision — unified `AgentAsset` with a `kind` discriminator + generic ordered frontmatter, NOT
per-kind structs.** Justification: the three kinds differ only in (a) location/naming and (b) which
frontmatter keys are *known/required*; a generic ordered `frontmatter` list handles all three
losslessly (preserves the dozen-plus evolving optional keys we can't hardcode), keeps the IPC
surface to ONE read/save/delete signature, and moves per-kind rules into a small static table
(mirroring `taxonomy.rs`). Three near-identical structs would triple the surface for no gain.

```rust
/// Which `.claude/` agent-asset kind. Wire: bare camelCase string
/// (`"skill"|"agent"|"command"`) — field-less enum, NOT tagged. Used both as a
/// serialized field AND as a command argument, so it needs Deserialize too.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentAssetKind {
    Skill,   // .claude/skills/<name>/SKILL.md
    Agent,   // .claude/agents/<name>.md
    Command, // .claude/commands/<name>.md
}

/// One frontmatter entry, preserving insertion order and unknown keys. Value is
/// the verbatim opaque scalar text after `key:` (§4). Serialize + Deserialize so
/// the editor can round-trip it back on save.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontmatterField {
    pub key: String,
    pub value: String,
}

/// Severity of a validation finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IssueSeverity { Error, Warning }

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIssue {
    pub severity: IssueSeverity,
    pub message: String,
}

/// Validation verdict for one asset. `valid == issues have NO Error-severity`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Validation {
    pub valid: bool,
    pub issues: Vec<AssetIssue>,
}

/// One parsed agent asset (read/inventory result). Serialize only — `validation`
/// is server-computed and never sent back on save (that uses `AgentAssetInput`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAsset {
    pub kind: AgentAssetKind,
    /// Directory name (skill) or file stem (agent/command).
    pub name: String,
    /// Repo-relative file path, forward slashes (e.g. `.claude/agents/foo.md`).
    pub path: String,
    pub exists: bool,
    /// Parsed flat frontmatter, in file order, unknown keys preserved (§4).
    pub frontmatter: Vec<FrontmatterField>,
    /// Everything after the closing `---` fence (verbatim); whole file if no
    /// fence.
    pub body: String,
    pub validation: Validation,
}

/// Full managed inventory of the three kinds, returned in one round-trip. Flat
/// list (UI groups by `kind`), sorted by (kind order skill<agent<command, name).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAssetInventory {
    pub assets: Vec<AgentAsset>,
}

/// The write payload for `save_agent_asset`. No `path`/`exists`/`validation` —
/// those are derived/computed by the backend. Deserialize only.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAssetInput {
    pub kind: AgentAssetKind,
    pub name: String,
    pub frontmatter: Vec<FrontmatterField>,
    pub body: String,
}
```

Blocking core functions:

```rust
/// Blocking. Scan `.claude/{skills,agents,commands}` under `workdir`, parse +
/// validate each, sorted (kind, name). Never touches anything outside `workdir`.
/// A missing `.claude/` (or any sub-dir) yields an empty group, not an error.
pub fn scan_agent_assets(workdir: &Path) -> Result<AgentAssetInventory, AppError>;

/// Blocking. Read + parse + validate one asset by (kind, name). A missing file
/// yields `exists:false` with empty frontmatter/body and validation `valid:false`
/// (issue "file does not exist"). `name` is validated (§4.4) first.
pub fn read_agent_asset(workdir: &Path, kind: AgentAssetKind, name: &str)
    -> Result<AgentAsset, AppError>;

/// Blocking. Create or overwrite the asset described by `input`. Validates the
/// name (§4.4) → InvalidName; validates the computed rel path stays in-workdir;
/// creates parent dirs; serializes (§4) + atomic temp+rename. Returns the FRESH
/// full inventory (frontend re-selects the saved asset by kind+name). Missing
/// recommended fields do NOT fail the write — they surface as warnings in the
/// returned inventory.
pub fn save_agent_asset(workdir: &Path, input: AgentAssetInput)
    -> Result<AgentAssetInventory, AppError>;

/// Blocking. Delete one asset. Skill → remove the whole `<name>/` directory
/// recursively (§8/OPEN-2); agent/command → remove the single `.md`. A missing
/// target is a no-op Ok. Returns the fresh inventory. `name` validated first.
pub fn delete_agent_asset(workdir: &Path, kind: AgentAssetKind, name: &str)
    -> Result<AgentAssetInventory, AppError>;
```

### 3.1 Per-kind spec table (static, private to `bundle.rs`)

Drives path computation, scanning, and known/required-field validation:

| kind | dir | file | required keys | known-optional keys |
|------|-----|------|---------------|---------------------|
| Skill | `.claude/skills/<name>/` | `SKILL.md` | — | `name`,`description`,`argument-hint`,`allowed-tools`,`model`,`disable-model-invocation` |
| Agent | `.claude/agents/` | `<name>.md` | `name`,`description` | `tools`,`model` |
| Command | `.claude/commands/` | `<name>.md` | — | `description`,`argument-hint`,`allowed-tools`,`model`,`disable-model-invocation` |

Path helpers: `rel_path(kind, name)` → `".claude/skills/<name>/SKILL.md"` | `".claude/agents/<name>.md"`
| `".claude/commands/<name>.md"`.

---

## 4. Frontmatter parse / serialize spec (LOCKED)

**Decision — minimal hand-rolled parser, NO `serde_yaml`.** Justification: the crate deliberately
avoids extra deps (P24 avoided `sha2`); the three formats' real-world frontmatter is overwhelmingly
flat `key: scalar` lines; and a generic ordered `Vec<FrontmatterField>` preserves unknown/future
keys losslessly (critical given ~12 evolving subagent keys) without pulling arbitrary-YAML fidelity
into v1. Complex YAML (sequences / nested maps / block scalars) is *detected and preserved
read-only* (§4.3), never mis-parsed.

### 4.1 Parse (`parse_frontmatter(raw: &str) -> (Vec<FrontmatterField>, String, bool)`)
Returns `(fields, body, complex)`.
1. Strip a leading UTF-8 BOM. Normalize the fence check on `\n` (accept `\r\n`).
2. If the first line is **exactly** `---`, scan forward for the next line that is exactly `---`.
   Everything between = the frontmatter block; everything **after that closing line's newline** =
   `body` (verbatim, unmodified). If no closing `---` exists → treat as NO frontmatter (fields
   empty, body = whole file, `complex=false`).
3. If the first line is not `---` → fields empty, body = whole file (common for commands).
4. Parse the block line-by-line:
   - blank line or a line starting with `#` (comment) → **skipped** (dropped on re-serialize;
     documented loss).
   - a line matching `^([A-Za-z0-9_.-]+):(?: (.*))?$` → push `FrontmatterField { key, value }`
     where `value` = the text after `": "` (or empty if the line is just `key:`), **trimmed of a
     single trailing `\r`** only, otherwise verbatim.
   - **any other non-blank line** (e.g. `- item`, ` nested: x`, `key: |`) → set `complex = true`
     and skip the line. `complex` propagates to validation (§4.3).
5. Duplicate keys are preserved as-is (both kept, in order).

### 4.2 Serialize (`serialize_asset(input) -> String`)
1. If `input.frontmatter` is empty → the file is just `body` (§4.2.3 body rule). No fence.
2. Else emit:
   ```
   ---\n
   {for each field}: `{key}: {value}\n`  (or `{key}:\n` when value is empty)
   ---\n
   {body}
   ```
   Values are written **verbatim** (opaque scalars, no auto-quoting) — the editor is responsible for
   keeping a value single-line (the UI uses single-line `<input>`s for frontmatter).
3. **Body rule:** write `body` verbatim, then ensure the file ends with exactly one trailing `\n`
   (append if missing; do not otherwise touch interior whitespace). This is the ONLY normalization.

**Round-trip guarantee:** for any asset with only flat `key: scalar` frontmatter and no
comments/blank lines inside the fence, `parse → serialize → parse` is a fixed point (fields identical
incl. unknown keys; body identical modulo a single trailing newline). Complex or commented
frontmatter does NOT round-trip and is guarded by §4.3.

### 4.3 Complex-frontmatter guard
If `parse_frontmatter` returns `complex = true`, the asset's `validation` gets an **Error** issue
`"frontmatter uses multi-line YAML this editor can't safely round-trip — edit the file directly"`,
so `valid == false`. The inventory still lists the asset (with its best-effort flat fields + body);
the editor opens it **read-only** and disables Save (§8), preventing silent loss of sequence/nested
frontmatter.

### 4.4 Name validation (`validate_asset_name(name) -> Result<(), AppError>`)
Reject (→ `AppError::InvalidName("invalid asset name: '<name>'")`) when `name`:
is blank/whitespace · equals `.` or `..` · starts with `-` · contains any of `/ \ :` · contains a
control char · contains any char outside `[A-Za-z0-9._-]`. (This charset makes a path-separator or
`..`-component impossible.) As belt-and-suspenders, `save`/`delete` also run
`validate_rel_path(rel_path(kind, name))` (rejects absolute/`..`/backslash) before any fs touch.
"Not lowercase-hyphen" is NOT rejected here — it is a **warning** issue in `Validation` (§4.5).

### 4.5 Validation rules (`validate(kind, name, fields, complex) -> Validation`)
- `complex == true` → Error (§4.3).
- Required key missing (per §3.1 table; check `fields` for the key with a non-empty value):
  Error `"<kind> requires frontmatter field '<key>'"` (agents: `name`, `description`).
- Warning if `name` charset is not `^[a-z0-9][a-z0-9-]*$`: `"name should be lowercase letters,
  digits, and hyphens"`.
- Agent/Skill: Warning if a `name` frontmatter field is present and differs from the on-disk
  `name` (the id): `"frontmatter name '<x>' differs from the file name '<name>'"`.
- Command/Skill: Warning if `body` is empty/whitespace: `"body is empty — nothing will run"`.
- `valid = !issues.any(|i| i.severity == Error)`.

---

## 5. Command surface (`commands.rs` + `lib.rs`)

Each command: `pub async fn NAME(state, repo_id, …) -> Result<T, AppError>` + runtime-free
`NAME_inner` that resolves `repo_path(state, &repo_id)?` and runs the core under
`spawn_blocking(move || bundle::…)` (identical template to `list_ai_assets` / `save_profile`).
Register all four in `lib.rs` `generate_handler!`. **No events, no channels.**

| Command (snake) | IPC method (camel) | Args | Returns | Error kinds |
|---|---|---|---|---|
| `list_agent_assets` | `listAgentAssets` | `repoId` | `AgentAssetInventory` | `io \| noRepo` |
| `read_agent_asset` | `readAgentAsset` | `repoId, kind, name` | `AgentAsset` | `invalidName \| io \| noRepo` |
| `save_agent_asset` | `saveAgentAsset` | `repoId, asset` | `AgentAssetInventory` | `invalidName \| other \| io \| noRepo` |
| `delete_agent_asset` | `deleteAgentAsset` | `repoId, kind, name` | `AgentAssetInventory` | `invalidName \| io \| noRepo` |

`kind` crosses the wire as the bare string `"skill"|"agent"|"command"`
(`invoke('read_agent_asset', { repoId, kind, name })`). `asset` for save is an `AgentAssetInput`
(`{ kind, name, frontmatter, body }`).

### 5.1 TypeScript wire types (`src/ipc/types.ts`)

```ts
export type AgentAssetKind = 'skill' | 'agent' | 'command';
export type IssueSeverity = 'error' | 'warning';

export interface FrontmatterField { key: string; value: string; }
export interface AssetIssue { severity: IssueSeverity; message: string; }
export interface Validation { valid: boolean; issues: AssetIssue[]; }

export interface AgentAsset {
  kind: AgentAssetKind;
  name: string;
  path: string;
  exists: boolean;
  frontmatter: FrontmatterField[];
  body: string;
  validation: Validation;
}

export interface AgentAssetInventory { assets: AgentAsset[]; }

// Save payload (no path/exists/validation — backend derives/computes those).
export interface AgentAssetInput {
  kind: AgentAssetKind;
  name: string;
  frontmatter: FrontmatterField[];
  body: string;
}
```

### 5.2 `IpcApi` additions + `tauri.ts`

```ts
// IpcApi:
listAgentAssets(repoId: string): Promise<AgentAssetInventory>;
readAgentAsset(repoId: string, kind: AgentAssetKind, name: string): Promise<AgentAsset>;
saveAgentAsset(repoId: string, asset: AgentAssetInput): Promise<AgentAssetInventory>;
deleteAgentAsset(repoId: string, kind: AgentAssetKind, name: string): Promise<AgentAssetInventory>;
```

`tauri.ts` — one thin `invoke` each, camelCase arg keys:
`invoke('list_agent_assets', { repoId })`, `invoke('read_agent_asset', { repoId, kind, name })`,
`invoke('save_agent_asset', { repoId, asset })`, `invoke('delete_agent_asset', { repoId, kind, name })`.

---

## 6. Mock IPC (`src/ipc/mock.ts`) — keep the browser harness implementable

Add a per-repo, STATEFUL agent-asset slice to `MockRepoState`, reusing `requireRepo`/`delay` and the
existing camelCase throw idiom (`const err: AppError = { kind, message }; throw err;`).

- `MockRepoState.agentAssets: AgentAsset[]` — seed with three assets so every kind renders:
  - skill `code-review` (frontmatter `name`,`description`; a body) — `valid:true`.
  - agent `test-runner` (frontmatter `name`,`description`,`tools`,`model`) — `valid:true`.
  - command `changelog` (frontmatter `description`,`argument-hint`; body with `$ARGUMENTS`) —
    `valid:true`.
  Plus one **invalid** seed to exercise badges: agent `broken` missing `description` →
  `validation.valid:false` with an Error issue.
- `listAgentAssets(repoId)`: return `structuredClone(state.agentAssets)` sorted (kind, name).
- `readAgentAsset(repoId, kind, name)`: validate name (throw `invalidName` on separators/`..`);
  find by (kind, name); if absent return an `exists:false` shell with a `valid:false` "does not
  exist" issue.
- `saveAgentAsset(repoId, asset)`: validate name (throw `invalidName`); recompute a mock
  `validation` (mirror §4.5 in a small helper: required-key + lowercase-hyphen + name-mismatch +
  empty-body); build the mapped `path`; upsert by (kind, name) into `state.agentAssets`; return the
  sorted clone.
- `deleteAgentAsset(repoId, kind, name)`: validate name; remove matching (kind, name); return the
  sorted clone. (Skill vs file removal is a no-op distinction in the mock.)

Because save/delete mutate `state.agentAssets`, the harness screenshots the full
create→edit→validate→delete loop with no backend.

---

## 7. Frontend behavior (P26c)

Extend `AiAssetsPanel.tsx` and add `AgentAssetEditor.tsx`. The panel already fetches on
open/repo-change and refreshes on `repo-changed`; add a parallel `listAgentAssets` fetch to the
existing `refresh` (a second `Promise.all` member) and store it in new state.

### 7.1 "Agent assets" section (in `AiAssetsPanel.tsx`, below the profiles section)
- One `settings-section` with three groups — **Skills**, **Subagents**, **Slash commands** —
  filtered from `inventory.assets` by `kind`.
- Each row: name + path (`mono`) + a validation chip: green `valid` / amber `N issue(s)` /
  `missing`; clicking a row opens the editor for that asset. Amber rows tooltip the first issue.
- A **New** button per group opens the editor in create mode seeded from that kind's template (§7.3).
- Reuse `asset-list` / `asset-row` / `asset-chip` classes already in the panel's CSS.

### 7.2 `AgentAssetEditor.tsx` (form)
- Props: `{ repoId, kind, name | null (create), onSaved(inventory), onClose }`. On open (edit mode)
  calls `readAgentAsset`; create mode starts from the template.
- Renders: a **name** input (disabled in edit mode — rename = create-new in v1, mirroring the
  ProfileManager upsert note); the kind's **known frontmatter fields** as labelled single-line
  inputs (§3.1 table) bound to the matching `FrontmatterField` (create/remove the entry as the input
  gains/loses a value); a **body** `<textarea>` (rows≈14; for agents labelled "System prompt", for
  commands "Prompt template"); a validation banner listing current issues.
- If the loaded asset has the **complex-frontmatter** Error (§4.3): show the frontmatter + body
  **read-only** with a notice, and disable Save.
- Save → build `AgentAssetInput` (drop empty-value known fields; keep any preserved unknown fields
  as-is) → `saveAgentAsset` → on success toast `Saved <kind> '<name>'`, call `onSaved(inventory)`,
  close. `invalidName` surfaces inline; other errors toast (mirror ProfileManager §8.4).
- **P26d (optional):** a generic "Advanced" disclosure listing every non-known frontmatter key as
  editable key/value rows + an "Add field" control, so unknown/preserved keys are visible and
  editable (until then they are preserved silently).

### 7.3 Templates (frontend constants; create mode)
- **skill** → `---\nname: <name>\ndescription: \n---\n\n# <name>\n\n`
- **agent** → `---\nname: <name>\ndescription: \ntools: \nmodel: inherit\n---\n\nYou are …\n`
- **command** → `---\ndescription: \nargument-hint: \n---\n\nUse $ARGUMENTS to …\n`

### 7.4 Delete
- Delete button (row action or in the editor) opens `ConfirmDialog`. For a **skill**, the dialog text
  warns it removes the **whole `.claude/skills/<name>/` directory (including supporting files)**; for
  agent/command it names the single file. Confirm → `deleteAgentAsset` → toast + `onSaved(inventory)`.

---

## 8. Safety / write gating (recap)

- `save`/`delete` validate the name (§4.4) + `validate_rel_path` on the computed path before any fs
  op; all paths are static-prefixed under `.claude/`, so nothing outside the workdir is reachable.
- Writes are atomic temp+rename with parent-dir creation (reuse the `profiles.rs` `atomic_write`
  helper — extract it to a shared `assets` helper if convenient, or duplicate; no behavior change).
- **Delete of a skill removes the whole `<name>/` directory (`remove_dir_all`)** — see OPEN #2.
- Every delete is confirm-gated in the UI (§7.4).

---

## 9. Error mapping (no `error.rs` change)

| Situation | Variant | TS kind |
|---|---|---|
| Blank / unsafe name (separators, `..`, control, bad charset) | `InvalidName` | `invalidName` |
| Filesystem read/write/rename/remove failure | `Io` | `io` |
| (save) serialize/other non-fatal logic error | `Other` | `other` |
| Unknown `repoId` | `NoRepo` | `noRepo` |

`validate_rel_path` returns `AppError::Other("invalid path: …")` on its defensive path (mapped to
`other`); the primary name check returns `InvalidName`.

---

## 10. Sub-increment breakdown (each = one fresh-context `senior-dev` pass)

- **P26a — core parse/validate/inventory + read commands + IPC (READ-ONLY).**
  - Rust: `assets/bundle.rs` types (§3) + spec table (§3.1) + `parse_frontmatter`/`serialize_asset`
    (§4) + `validate_asset_name`/`validate` + `scan_agent_assets`/`read_agent_asset`; `pub mod
    bundle;` + re-exports in `mod.rs`.
  - Commands: `list_agent_assets`, `read_agent_asset` (+ `lib.rs`).
  - IPC triple: types + two methods in `types.ts`/`tauri.ts`/`mock.ts` (read-only mock + seed).
  - Tests: §11 rows 1–6.
  - Acceptance: `scan_agent_assets` over a temp `.claude/` yields the exact three-kind set with
    parsed frontmatter/body + validation verdicts; `serialize∘parse` round-trips flat frontmatter
    incl. unknown keys; harness lists the section from the mock seed.
- **P26b — write path: create/edit/delete + commands + IPC + stateful mock.**
  - Rust: `save_agent_asset`, `delete_agent_asset` (atomic write, parent dirs, skill-dir recursive
    delete, all validation).
  - Commands: `save_agent_asset`, `delete_agent_asset` (+ `lib.rs`).
  - IPC triple: `AgentAssetInput` + two methods; stateful mock mutation.
  - Tests: §11 rows 7–11.
  - Acceptance: create writes the mapped file (temp+rename, parent dirs) and round-trips on re-read;
    edit preserves unknown keys; skill delete removes the directory; agent/command delete removes the
    file; bad names → `invalidName`.
- **P26c — UI section + editor.** `AiAssetsPanel.tsx` "Agent assets" section (three groups +
  validation chips + New buttons) + `AgentAssetEditor.tsx` (known-field inputs + body textarea +
  templates + read-only complex-frontmatter guard) + `ConfirmDialog` delete + toasts + `repo-changed`
  refresh. Acceptance: browser-harness screenshots of the three lists, the editor (create + edit),
  a validation badge on the invalid seed, and the skill delete-confirm.
- **P26d — (optional) richer templates + generic frontmatter-key editor.** The "Advanced" unknown-key
  key/value rows + "Add field" (§7.2), plus per-kind starter-template refinement. Foldable into
  P26c if small. Acceptance: preserved unknown keys are visible/editable and survive a round-trip in
  the harness.

Commit each approved sub-increment as `wip(P26a): …` etc. (orchestrator owns commits).

---

## 11. Tests (AI gate)

`#[cfg(test)]` in `bundle.rs`, `tempfile::TempDir` scratch workdir (no git repo needed);
`TMP`/`TEMP=D:\Temp`; run `cargo test` and `clippy` **sequentially**.

1. **Empty** — no `.claude/` → `scan_agent_assets` returns `assets: []`.
2. **Scan all kinds** — drop `.claude/skills/code-review/SKILL.md`, `.claude/agents/test-runner.md`,
   `.claude/commands/changelog.md`; assert the three assets, their `kind`, `name`, `path`, parsed
   frontmatter keys/values, body, and sort order (skill<agent<command, then name). A skill dir
   WITHOUT `SKILL.md` is skipped; a stray `.txt` in `commands/` is ignored.
3. **Parse** — fenced frontmatter with known + unknown keys → `frontmatter` preserves order and the
   unknown keys; body = text after the fence verbatim. A no-fence command file → empty frontmatter,
   body = whole file. A file whose fence has no closing `---` → no frontmatter, body = whole file.
4. **Round-trip** — `parse → serialize_asset → parse` is a fixed point for flat frontmatter (unknown
   keys preserved); an empty-frontmatter input serializes to body-only (no fence); body gains exactly
   one trailing newline.
5. **Validation** — agent missing `description` → `valid:false` Error; skill missing `description`
   → `valid:true` (recommended, not required) with a Warning if applicable; command with no
   frontmatter → `valid:true`; `name: Foo_Bar` → lowercase-hyphen Warning; a frontmatter with a
   `- item` sequence line → `complex` Error `valid:false`.
6. **Wire shapes** — `serde_json::to_value` on `AgentAssetInventory` + `AgentAsset` asserts camelCase
   keys and that `AgentAssetKind`/`IssueSeverity` serialize to bare strings (`"skill"`, `"warning"`).
7. **Save create** — a new skill writes `.claude/skills/x/SKILL.md` with fenced content; parent dirs
   created; `read_agent_asset` round-trips it; no `.bonsai-tmp` remnant beside the file.
8. **Save edit preserves unknown keys** — pre-drop an agent with a `color: blue` unknown key; load,
   edit `description`, save via a rebuilt `AgentAssetInput` that carries `color` through → the
   re-read file still has `color: blue`.
9. **Save validation** — names `""`, `"a/b"`, `"a\\b"`, `".."`, `"a:b"`, `"-x"` → `InvalidName`;
   a save with a missing required field still WRITES and the returned inventory flags it `valid:false`.
10. **Delete** — skill delete removes the whole `<name>/` dir (with a supporting `helper.py` beside
    SKILL.md gone too); agent/command delete removes the `.md`; deleting an absent asset is a no-op
    Ok; the returned inventory reflects removal.
11. **Path-escape defense** — `validate_asset_name("../x")` / `"a/b"` reject; and
    `validate_rel_path(rel_path(Agent, "..").?)` is never reachable because the name check fires first
    (assert both guards).

### 11.2 Frontend AI gate
`pnpm build` + `tsc` clean; browser harness (`VITE_MOCK_IPC=1`, port 1420) renders: the three
grouped lists with validation chips (incl. the invalid `broken` agent amber badge), the editor in
create mode (template prefilled) and edit mode, a save updating the list, and the skill
delete-confirm dialog.

---

## 12. OPEN DECISIONS (recommended default in brackets; contract proceeds on the default)

1. **Module name.** [**`assets/bundle.rs`**] — avoids clashing with the "agent" kind; covers all
   three. (`assets/agents.rs` was the alt.)
2. **Skill delete semantics.** [**Remove the whole `.claude/skills/<name>/` directory
   recursively.**] Justification: a skill *is* its directory (SKILL.md + supporting
   scripts/templates/references per the docs); pruning only SKILL.md orphans a broken bundle. The UI
   confirm names the directory; delete is always confirm-gated. (Alt: remove SKILL.md + prune-if-empty
   — rejected as it strands supporting files.)
3. **Frontmatter parser.** [**Hand-rolled flat `key: scalar` parser, generic ordered
   `Vec<FrontmatterField>`, no `serde_yaml`.**] Complex YAML detected → read-only guard (§4.3). Values
   are opaque single-line scalars. (Alt: add `serde_yaml` for full fidelity — rejected for v1;
   revisit only if users hit the complex-frontmatter guard often.)
4. **Round-trip preservation.** [**Preserve unknown keys; drop comments + blank lines inside the
   fence; ensure one trailing body newline.**] Flat frontmatter round-trips exactly; comment loss is
   documented and acceptable for v1.
5. **Recursion / namespacing.** [**Direct children only** — `.claude/agents/*.md`,
   `.claude/commands/*.md`, `.claude/skills/*/SKILL.md`.] Nested agents/commands, subfolder
   namespacing, and plugin skills are DEFERRED (§13). Note only.
6. **Mutation returns.** [**save/delete return the full `AgentAssetInventory`**] (mirrors P24
   save_profile returning the store) so the panel updates race-free in one round-trip; the editor
   re-selects the saved asset by (kind, name).

None block implementation; all defaults are safe.

---

## 13. Explicitly DEFERRED (state clearly)

- **Invocation / preview / execution** of a skill or command (no running `$ARGUMENTS`, no `/command`
  preview, nothing that shells out to `claude`).
- **The MCP config manager (A4)** — `.mcp.json` stays inventory-only in the P24 drift panel.
- **Copilot `.github/prompts` / `.github/instructions` management** — stays inventory-only
  (unchanged P24 taxonomy rows).
- **Nested / plugin skills, subfolder namespacing, `~/.claude` (user-scope) assets** — v1 is
  project-scope `.claude/` direct-children only.
- **Multi-line / sequence / nested YAML frontmatter editing** — detected and shown read-only (§4.3),
  not editable in v1.
- **Rename as a first-class op** — v1 rename = save-new + delete-old (name input disabled in edit
  mode, mirroring ProfileManager).

---

## 14. Acceptance criteria — AI gate vs USER CHECKPOINT

**AI gate (orchestrator-verifiable; no network, no native window, no `claude` CLI):**
- `cargo check` + `clippy` clean on `bonsai-core`; `pnpm build` + `tsc` clean.
- All §11.1 fs-oracle unit tests green (temp-dir fixtures).
- Browser-harness screenshots per §11.2: the three grouped lists + validation chips, the editor
  (create + edit), a save reflecting in the list, and the skill delete-confirm.

**USER CHECKPOINT (native `pnpm tauri dev`, real repo):**
- Open a repo with real `.claude/` skills/agents/commands; confirm the lists + validation badges
  match reality (cross-check a known-good and a deliberately-broken file).
- Create a new subagent via the form; confirm `.claude/agents/<name>.md` appears on disk with the
  expected frontmatter + body, and Claude Code picks it up.
- Edit a skill's description; confirm the file changes on disk and **unknown/preserved frontmatter
  keys survive** the round-trip.
- Delete a skill; confirm the confirm dialog appears and the whole `.claude/skills/<name>/` directory
  is removed; Cancel writes nothing.
- Confirm a file with multi-line YAML frontmatter opens read-only and Save is disabled (no silent
  loss).
