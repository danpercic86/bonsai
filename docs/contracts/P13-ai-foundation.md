# P13 — Local-AI foundation (Claude Code CLI) + AI merge-conflict resolution

Status: authoritative for this milestone. Scope: a reusable "run the local `claude` CLI" layer
in Rust (subscription auth, no API key), and its first consumer — AI-assisted merge-conflict
resolution wired into the existing paused-merge/conflict machine. Plan of record:
`~/.claude/plans/analyze-the-possibility-to-logical-yeti.md` (approved).

Builds on / reuses:
- `docs/contracts/P3c-merge-conflicts.md` — `conflict.rs` stage reads + write/`add_path` mechanics,
  the paused-merge `RepoOpState::Merge` flow, `commit_merge` finalization (UNCHANGED here).
- `docs/contracts/P12-conflict-editor.md` — the `resolve_conflict_text` command + backend fn and
  the `ConflictEditor` component (the AI apply step and the proposal review reuse BOTH; see §7, §8).
- `docs/contracts/P2-followups.md` / `P11-followup.md` — `settings.rs` additive-field pattern,
  `UiSettings`/`UiSettingsPatch`/`apply_patch`, `get_ui_settings`/`set_ui_settings`, and the
  existing `SettingsPanel.tsx` page (the AI section slots in there).

Invariants (non-negotiable): Rust owns ALL Git logic AND ALL subprocess logic; React only renders.
IPC carries compact precomputed data; commands = req/resp; blocking work (git2 **and now
`std::process`**) runs under `spawn_blocking` via the established `*_inner` runtime-free pattern in
`commands.rs`. `src/ipc/mock.ts` is updated with EVERY IpcApi change and MUST work with **no
`claude` installed** (browser harness). Sending repo content to the CLI is an outward action →
one-time consent + an enable toggle, enforced in the **backend** (§9 decision 6).

Backend-spawn design (LOCKED, from the plan): spawn `claude` via `std::process::Command` inside
`spawn_blocking`. NO `tauri-plugin-shell`, NO new Tauri capability, ZERO new crates for the MVP.
`csp: null` in `tauri.conf.json` is unchanged.

> **BLOCKER — milestone numbering collision (flag to orchestrator, §9.7).** `P11` and `P12` are
> already spent in this codebase (`docs/contracts/P11-followup.md`, `P11g-revision.md`,
> `P12-conflict-editor.md`; `settings.rs`/`commands.rs` carry `(P11)`/`(P12)` comments for
> auto-fetch, graph knobs, and the conflict editor — all already shipped). This contract keeps the
> requested filename `P13-ai-foundation.md`, but I recommend the milestone TAG in new code comments
> be **`P13-ai`** to avoid colliding with the shipped P11/P12 comment labels. Decision needed.

---

## 1. Scope split (sub-increments)

| # | Increment | Content | Read |
|---|-----------|---------|------|
| 1 | **P13a** | `src-tauri/src/ai/mod.rs`: `RunOpts`, `AiResult`, `AiAvailability`, `run_claude`, `check_availability`; `error.rs` two variants; module unit tests via a stub `claude` (`BONSAI_CLAUDE_BIN`). | §2, §3 |
| 2 | **P13b** | `settings.rs` 3 additive fields + `AiAutonomy` enum; `commands.rs` `UiSettings`/`UiSettingsPatch`/`apply_patch`/`get_ui_settings`/`set_ui_settings` extension + unit tests. | §4 |
| 3 | **P13c** | `src-tauri/src/git/ai_resolve.rs`: `AiResolveProposal`, `ai_resolve_conflict` (proposal only, writes NOTHING). Two commands (`check_ai_availability`, `ai_resolve_conflict`) + `lib.rs`/`git/mod.rs` registration. **Apply reuses the existing `resolve_conflict_text` command — no new apply command (§9.5).** Test `tests/ai_resolve_cli.rs`. | §5, §6 |
| 4 | **P13d** | IPC mirror: `src/ipc/types.ts`, `src/ipc/tauri.ts`, `src/ipc/index.ts`, stateful `mock.ts` (canned proposal; `?ai=off` toggles availability). | §7 |
| 5 | **P13e** | Frontend: `SettingsPanel.tsx` AI section (enable + autonomy + availability + consent dialog), `StatusPanel.tsx` "Resolve with AI" conflict-row action, `RepoWorkspace.tsx` `handleAiResolveConflict` autonomy branch (proposal review reuses `ConflictEditor`). | §8 |

Each is a self-contained fresh-context senior-dev pass (this file + the exact source paths). Tester
runs after P13c lands (§10 unit tests + `ai_resolve_cli.rs`).

---

## 2. `src-tauri/src/ai/mod.rs` — the reusable subprocess layer (P13a)

Pure Rust, no Tauri types, no git2. Register `pub mod ai;` in `lib.rs`.

```rust
//! Drives the locally-installed `claude` CLI (Claude Code) as a pure text
//! transform on the user's subscription session (no API key). Blocking;
//! all callers invoke under spawn_blocking.

use std::path::Path;
use std::time::Duration;
use crate::error::AppError;

/// Default resolution model. `sonnet` = strong code-merge quality at ~1/5 the
/// cost/latency of `opus`; far better than `haiku` for conflict reasoning
/// (§9.2). Configurable per call via `RunOpts.model`.
pub const DEFAULT_MODEL: &str = "sonnet";
/// Wall-clock cap for one resolution call (§9.4).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(90);
/// Short cap for the `--version` availability probe.
pub const AVAILABILITY_TIMEOUT: Duration = Duration::from_secs(10);
/// Test/override hook: when set, this binary path is spawned instead of PATH
/// `claude` (points at the stub script in tests).
pub const CLAUDE_BIN_ENV: &str = "BONSAI_CLAUDE_BIN";

/// Knobs for one `run_claude` call. `Default` = subscription resolver defaults.
#[derive(Debug, Clone)]
pub struct RunOpts {
    /// `--model <alias>`; `None` => `DEFAULT_MODEL`. Aliases: sonnet|haiku|opus.
    pub model: Option<String>,
    /// Killed and mapped to `AiFailed("timed out …")` past this deadline.
    pub timeout: Duration,
    /// Appended via `--append-system-prompt`. Sets role + output contract.
    pub system_prompt: Option<String>,
    /// Reserved: `--json-schema <schema>` for structured output. `None` in v1
    /// (§9.1 locks reading `result` prose instead). Wired but unused so a later
    /// feature can opt in without changing the signature.
    pub json_schema: Option<String>,
}

impl Default for RunOpts {
    fn default() -> Self {
        RunOpts { model: None, timeout: DEFAULT_TIMEOUT, system_prompt: None, json_schema: None }
    }
}

/// A successful CLI text transform. `text` is the model's `result` field with a
/// single leading/trailing ``` fence stripped defensively (§3.3).
#[derive(Debug, Clone)]
pub struct AiResult {
    pub text: String,
    pub cost_usd: Option<f64>,
    pub session_id: Option<String>,
}

/// Cheap health status. NEVER errors — a missing/broken CLI yields
/// `{ installed:false, .. }`, not an `Err`. Wire type mirrored in TS (§7).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAvailability {
    /// `claude --version` spawned and exited 0.
    pub installed: bool,
    /// v1: reported EQUAL to `installed` (subscription auth is NOT verified in a
    /// cheap probe — a real auth check would cost a billable call). Actual
    /// logged-out state surfaces as `AiFailed` on the first resolve (§9 note).
    pub logged_in: bool,
    /// Parsed from `--version` stdout when installed, else `None`.
    pub version: Option<String>,
    /// Human one-liner for the settings UI ("Claude Code 2.1.220 ready" /
    /// "Claude Code CLI not found on PATH").
    pub detail: String,
}

/// Blocking. Spawns claude as a headless text transform, pipes `stdin_payload`
/// to its stdin, waits up to `opts.timeout`, parses the JSON envelope, returns
/// the model text. `cwd` is the child's working dir (the repo workdir).
///
/// Argv (LOCKED, verified on CLI v2.1.220):
///   claude -p <prompt> --output-format json --safe-mode --tools ""
///          --no-session-persistence --model <model>
///          [--append-system-prompt <system_prompt>]
///
/// - `--safe-mode` (NOT `--bare`): keeps subscription auth, disables
///   CLAUDE.md/hooks/skills/MCP so the repo's own CLAUDE.md never pollutes the
///   prompt. `--bare` is FORBIDDEN (forces ANTHROPIC_API_KEY, breaks no-key req).
/// - `--tools ""` (NOT `--allowedTools`): disables all built-in tools → the
///   model can only emit text; it cannot touch disk/network/git.
/// - `prompt` is passed as the `-p` positional; large content goes via stdin
///   (claude concatenates piped stdin with the `-p` prompt).
///
/// Errors:
///   spawn fails with NotFound         -> AiUnavailable("Claude Code CLI not found …")
///   other spawn failure               -> AiUnavailable(io message)
///   timeout (child killed)            -> AiFailed("Claude timed out after Ns")
///   non-zero exit OR envelope is_error-> AiFailed(result/stderr message)
///   stdout not valid envelope JSON    -> AiFailed("could not parse Claude output: …")
///   empty/absent result               -> AiFailed("Claude returned no output")
pub fn run_claude(
    cwd: &Path,
    prompt: &str,
    stdin_payload: Option<&str>,
    opts: RunOpts,
) -> Result<AiResult, AppError>;

/// Blocking, never errors. Spawns `claude --version` (`AVAILABILITY_TIMEOUT`);
/// returns a populated `AiAvailability`. Respects `CLAUDE_BIN_ENV`.
pub fn check_availability() -> AiAvailability;
```

---

## 3. `run_claude` internals (locked specifics for P13a)

### 3.1 Binary resolution
`std::env::var(CLAUDE_BIN_ENV)` if set (tests), else `"claude"`. `Command::new(bin)` PATH-resolves
the Windows `claude.cmd` shim automatically. `.current_dir(cwd)`, `.stdin(piped)`, `.stdout(piped)`,
`.stderr(piped)`.

### 3.2 Timeout without a new crate (LOCKED, §9.4)
`std::process` has no wait-with-timeout. Use the standard drain-and-poll pattern (avoids the
classic pipe-buffer deadlock where the child blocks writing stdout while we block writing stdin):

```text
child = cmd.spawn()?               // map NotFound -> AiUnavailable
stdin  = child.stdin.take()        // Option
stdout = child.stdout.take().unwrap()
stderr = child.stderr.take().unwrap()

// 1. writer thread: write stdin_payload bytes, then DROP stdin (EOF). Ignore
//    BrokenPipe (child may exit early).
// 2. reader thread A: read stdout to end -> Vec<u8>
// 3. reader thread B: read stderr to end -> Vec<u8>
deadline = Instant::now() + opts.timeout
loop {
    match child.try_wait()? {
        Some(status) => break,
        None => {
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AiFailed(format!("Claude timed out after {}s", timeout.as_secs())));
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
}
join the three threads  // readers finish once the child's pipes close
```

Concurrent readers guarantee no deadlock for payloads larger than the OS pipe buffer.

### 3.3 Envelope parsing (lenient serde)
The CLI JSON fields are already snake_case, so no rename map is needed; unknown fields are ignored
by default. Parse only what we use:

```rust
#[derive(serde::Deserialize)]
struct ClaudeEnvelope {
    result: Option<String>,
    #[serde(default)] is_error: bool,
    #[serde(default)] total_cost_usd: Option<f64>,
    #[serde(default)] session_id: Option<String>,
    #[serde(default)] subtype: Option<String>,
}
```

Mapping after the child exits:
1. Non-zero exit status **and** stdout not parseable → `AiFailed(<stderr, trimmed, first 500 chars>)`.
2. Parse stdout → `ClaudeEnvelope`; parse failure → `AiFailed("could not parse Claude output: …")`.
3. `is_error == true` → `AiFailed(result.unwrap_or(subtype).unwrap_or("Claude reported an error"))`.
4. `result` empty/`None` → `AiFailed("Claude returned no output")`.
5. Else `Ok(AiResult { text: strip_fence(result), cost_usd: total_cost_usd, session_id })`.

`strip_fence`: if `text.trim_start()` begins with a ```` ``` ```` line (optionally ```` ```lang ````)
and ends with a ```` ``` ```` line, remove those two fence lines only; otherwise return the text
unchanged. Defensive against a model that wraps the file body despite the system-prompt instruction.

### 3.4 `check_availability`
Spawn `<bin> --version` with `AVAILABILITY_TIMEOUT` (same poll loop). On spawn NotFound / non-zero /
timeout → `{ installed:false, logged_in:false, version:None, detail:"Claude Code CLI not found on
PATH" }`. On exit 0 → `installed:true`, `logged_in:true` (§9 caveat), `version` = the trimmed
stdout (or first whitespace-split token), `detail:"Claude Code <version> ready"`. Never returns
`Err`.

---

## 4. Settings + UiSettings wiring (P13b)

### 4.1 `src-tauri/src/settings.rs` — additive fields (SETTINGS_VERSION stays 1)
Follows the shipped `theme`/`list_view`/`auto_fetch` additive precedent (§ doc comment at
`settings.rs:155-167`). Add the enum and three `#[serde(default)]` fields to `Settings`
(+ mirror in `Settings::default()`):

```rust
/// AI conflict-resolution autonomy (P11). ProposeReview = user accepts before
/// anything is written/staged (default); AutoResolve = write+stage immediately,
/// user reviews the staged diff before commit_merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiAutonomy {
    #[default]
    ProposeReview,
    AutoResolve,
}

// Settings gains, all #[serde(default)]:
//   pub ai_enabled: bool,               // default true
//   pub ai_conflict_autonomy: AiAutonomy,
//   pub ai_consented: bool,             // default false  (one-time consent gate)
```

`ai_enabled` defaults `true`, but the **consent gate** (`ai_consented` default `false`) is what
actually unlocks the feature — see §9.6. No new clamp fn needed (bools + enum). Extend the
`Settings::default()` body and the on-disk wire-format doc comment with the three keys.

### 4.2 `src-tauri/src/commands.rs` — extend the existing structs (no new commands)
`UiSettings` (`commands.rs:162`), `UiSettingsPatch` (`:174`), `apply_patch` (`:188`),
`get_ui_settings` (`:210`) and `set_ui_settings` (`:232`) all pass the whole struct through — the
AI fields ride along once added:

```rust
// UiSettings gains:  pub ai_enabled: bool,
//                    pub ai_conflict_autonomy: AiAutonomy,
//                    pub ai_consented: bool,
// UiSettingsPatch gains (all Option<..>):
//                    pub ai_enabled: Option<bool>,
//                    pub ai_conflict_autonomy: Option<AiAutonomy>,
//                    pub ai_consented: Option<bool>,
// apply_patch: three `if let Some(x) = patch.x { s.x = x; }` arms (no clamp).
// get_ui_settings / set_ui_settings: copy the three fields into the returned UiSettings.
```

Unit tests (extend the existing `commands.rs` test module, matching
`set_ui_settings_patch_is_partial` / `..._auto_fetch_and_graph`):
1. `ai_settings_roundtrip` — save/load a `Settings` with non-default AI fields.
2. `set_ui_settings_patch_ai_is_partial` — patching only `ai_enabled` leaves autonomy + consent
   untouched, and vice versa; `UiSettingsPatch::default()` mutates nothing.

---

## 5. `src-tauri/src/git/ai_resolve.rs` — the first consumer (P13c)

Register `pub mod ai_resolve;` in `git/mod.rs`. Pure git2 + `crate::ai`, no Tauri types.

```rust
//! AI merge-conflict resolution. ai_resolve_conflict builds a prompt from the
//! conflict's three index stages + marker view, calls the CLI, and returns the
//! proposed merged body. It WRITES NOTHING — applying is the caller's separate
//! resolve_conflict_text step (P12), so ProposeReview holds the bytes before
//! touching disk.

use std::path::Path;
use crate::error::AppError;
use crate::ai::RunOpts;

/// The model's proposed fully-merged file body for one conflicted path.
/// Serialized camelCase (mirrored in TS §7).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiResolveProposal {
    pub path: String,
    pub proposed_text: String,
    pub cost_usd: Option<f64>,
}

/// Blocking. Produces a resolution proposal for one CURRENTLY conflicted path.
///
/// - Reuses `conflict::get_conflict(workdir, path)` for the marker view + the
///   binary/too_large/missing guards; any of those -> `AiFailed` (SKIP AI,
///   manual only — §9). Only text kinds are eligible (see caller-side kind
///   filter §8.2; a deletion/binary kind reaching here -> `AiFailed`).
/// - Reads the three sides via the index: base = get_path(rel,1), ours =
///   get_path(rel,2), theirs = get_path(rel,3) + find_blob (same pattern as
///   conflict.rs `resolve_conflict`). Absent side rendered as "(absent)".
/// - Builds the stdin payload (§5.1), calls
///   `ai::run_claude(workdir, RESOLVE_PROMPT, Some(&payload),
///                   RunOpts { system_prompt: Some(SYSTEM_PROMPT), ..opts })`.
/// - Returns the proposal; DOES NOT write or stage.
///
/// Errors: `aiUnavailable` | `aiFailed` | `git` (path not conflicted, via
/// get_conflict) | `invalidName` (validate_rel_path, same guard as
/// resolve_conflict).
pub fn ai_resolve_conflict(
    workdir: &Path,
    path: &str,
    opts: RunOpts,
) -> Result<AiResolveProposal, AppError>;
```

### 5.1 Prompt + payload format (LOCKED — §9.3)
`SYSTEM_PROMPT` (const, via `--append-system-prompt`):

```
You are a Git merge-conflict resolver. You are given the common ANCESTOR, OURS,
and THEIRS versions of a single file, plus the file with Git conflict markers.
Produce the fully merged file that integrates the intent of both sides, with NO
conflict markers left. Output ONLY the raw merged file contents — no
explanations, no commentary, and no markdown code fences.
```

`RESOLVE_PROMPT` (const, `-p` positional): `Resolve the merge conflict in the file provided on
standard input. Output only the merged file body.`

STDIN payload (built in Rust; sides are lossy UTF-8 of the stage blobs, marker text from
`get_conflict`):

```
FILE: <path>
CONFLICT KIND: <kind>

===== ANCESTOR (base) =====
<base text or (absent)>

===== OURS =====
<ours text or (absent)>

===== THEIRS =====
<theirs text or (absent)>

===== CONFLICTED (worktree, with markers) =====
<ConflictFile.text>
```

`proposed_text` = `AiResult.text`; `cost_usd` = `AiResult.cost_usd`.

---

## 6. Commands (`src-tauri/src/commands.rs` + `lib.rs generate_handler!`)

Two new commands, established `async` wrapper → runtime-free `_inner` → `spawn_blocking` shape.
**Apply is NOT a new command — reuse the existing `resolve_conflict_text(repoId, path, content)`
(§9.5).**

```rust
/// Cheap CLI health probe. No repo, no state. Never rejects for CLI state
/// (only a task-join error can Err). Errors: (join only).
#[tauri::command]
pub async fn check_ai_availability() -> Result<AiAvailability, AppError>;
// body: spawn_blocking(|| ai::check_availability()).await.map_err(join)

/// Proposes an AI resolution for one conflicted path. Loads settings and
/// REFUSES with AiUnavailable unless ai_enabled && ai_consented (§9.6 — the
/// authoritative gate; the frontend also gates for UX). Errors:
/// aiUnavailable | aiFailed | git | invalidName | noRepo.
#[tauri::command]
pub async fn ai_resolve_conflict(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<AiResolveProposal, AppError>;
```

`ai_resolve_conflict_inner(app, state, repo_id, path)`:
1. `let s = settings::load_from(&settings::settings_file(&app)?);`
2. `if !(s.ai_enabled && s.ai_consented) { return Err(AppError::AiUnavailable("AI features are
   disabled or not yet consented to")); }`
3. `let workdir = repo_path(state, repo_id)?;`
4. `spawn_blocking(move || ai_resolve::ai_resolve_conflict(&workdir, &path, RunOpts::default()))`
   (`RunOpts::default()` → model `sonnet`; system prompt set inside `ai_resolve_conflict`).

Register `check_ai_availability` and `ai_resolve_conflict` in `generate_handler![]`. Extend the
existing NoRepo command-guard test with `ai_resolve_conflict` (skip in the pure-inner test if it
needs `AppHandle`; otherwise assert the disabled-gate path returns `AiUnavailable`).

### 6.1 `src-tauri/src/error.rs` — two additive variants
Add to `AppError` (with `kind()`/`message()` arms and the doc-comment kind list), following the
`OperationInProgress` pattern exactly:

```rust
#[error("{0}")] AiUnavailable(String),   // kind "aiUnavailable"
#[error("{0}")] AiFailed(String),        // kind "aiFailed"
```

Both are `String`-carrying → add to the shared `message()` match arm alongside `Git(m) | …`.

---

## 7. IPC mirror + mock (P13d)

### 7.1 `src/ipc/types.ts` additions (verbatim)
```ts
export type AiAutonomy = 'proposeReview' | 'autoResolve';

export interface AiAvailability {
  installed: boolean;
  loggedIn: boolean;
  version: string | null;
  detail: string;
}

export interface AiResolveProposal {
  path: string;
  proposedText: string;
  costUsd: number | null;
}
```

`UiSettings` gains: `aiEnabled: boolean; aiConflictAutonomy: AiAutonomy; aiConsented: boolean;`
`UiSettingsPatch` gains: `aiEnabled?: boolean; aiConflictAutonomy?: AiAutonomy; aiConsented?: boolean;`
`AppError.kind` union gains: `'aiUnavailable' | 'aiFailed'`.

`IpcApi` gains (mirror the Rust error lists):
```ts
/** Cheap Claude Code CLI health probe. Never rejects for CLI state. */
checkAiAvailability(): Promise<AiAvailability>;
/** Propose an AI merge resolution for one conflicted path. Writes nothing.
 *  Rejects aiUnavailable | aiFailed | git | invalidName | noRepo. */
aiResolveConflict(repoId: string, path: string): Promise<AiResolveProposal>;
```

There is **no `applyAiResolution`** — the apply step is the existing
`resolveConflictText(repoId, path, content)` (§9.5). Re-export new types from `src/ipc/index.ts`.

### 7.2 `src/ipc/tauri.ts` (beside `resolveConflict`, `resolveConflictText`)
```ts
checkAiAvailability: () => invoke('check_ai_availability'),
aiResolveConflict: (repoId, path) => invoke('ai_resolve_conflict', { repoId, path }),
```

### 7.3 `src/ipc/mock.ts` — stateful twin, works with NO claude installed
- Read a `?ai=off` query flag once at module init (composable with `?op=merge`).
- `checkAiAvailability()` → `delay(150)` → when `?ai=off`: `{ installed:false, loggedIn:false,
  version:null, detail:'Claude Code CLI not found on PATH' }`; else `{ installed:true,
  loggedIn:true, version:'2.1.220', detail:'Claude Code 2.1.220 ready' }`.
- `aiResolveConflict(repoId, path)` → `delay(600)` (simulate latency) → look up the seeded conflict
  (`?op=merge` fixture, §P3c mock). If the path is not a text conflict (`bothModified`/`bothAdded`)
  or absent → reject `{ kind:'aiFailed', message:'AI resolution unavailable for this file' }`. Else
  return `{ path, proposedText: <markerless merged body derived from conflictTexts[path]>,
  costUsd: 0.012 }`. Do NOT mutate state (the proposal is not applied yet).
- Apply path already exists: the existing `resolveConflictText` mock clears the path from
  `conflicts` + `status.conflicted`. No new mock method for apply.
- `getUiSettings`/`setUiSettings` mock: include the three AI fields (defaults `aiEnabled:true`,
  `aiConflictAutonomy:'proposeReview'`, `aiConsented:false`) so the settings UI + consent flow work
  in the harness.

---

## 8. Frontend (P13e)

### 8.1 `SettingsPanel.tsx` — new "AI assistance" section
Extend `SettingsPanelProps` (additive):
```ts
aiEnabled: boolean;
aiConflictAutonomy: AiAutonomy;
aiConsented: boolean;
aiAvailability: AiAvailability | null;   // App fetches on panel open; null = probing
onRequestEnableAi(): void;               // App shows the consent ConfirmDialog then patches
```
- **Enable toggle** bound to `aiEnabled`. Turning it ON when `aiConsented === false` does NOT patch
  directly — it calls `onRequestEnableAi()`, which opens a consent `ConfirmDialog` (§8.4); only on
  confirm does App patch `{ aiEnabled:true, aiConsented:true }`. Turning OFF patches
  `{ aiEnabled:false }` immediately (consent stays recorded).
- **Autonomy radio** (disabled unless enabled+consented): "Propose & review" (`proposeReview`) /
  "Auto-resolve, then review" (`autoResolve`) → `onChange({ aiConflictAutonomy })`.
- **Availability line**: from `aiAvailability` — "Claude Code 2.1.220 ready" or, when
  `!installed`, an amber note "Claude Code CLI not found on PATH — install it and log in to use AI
  features" (guidance, not a dead control).

### 8.2 `StatusPanel.tsx` — "Resolve with AI" conflict-row action
`ConflictRow` gains one hover action button after `resolved`: `✨ AI` (title "Resolve with AI").
Thread new props through `ConflictsSection` → `StatusPanel` → `RepoWorkspace`:
```ts
aiEligible: boolean;                 // aiEnabled && aiConsented && aiAvailability?.installed
onAiResolve(path: string): void;
```
- **Hidden** unless `kind === 'bothModified' || kind === 'bothAdded'` (the only text-mergeable kinds;
  matches the `ConflictEditor` mount guard). Deletion/binary/too-large kinds never show it.
- **Disabled** when `!aiEligible || disabled` (`disabled` = `mutating`). Manual ours/theirs/resolved
  buttons ALWAYS remain (fallback).

### 8.3 `RepoWorkspace.tsx` — `handleAiResolveConflict(path)`
Per-path busy state (calls take seconds). `try` → `const proposal = await ipc.aiResolveConflict(
repoId, path)`; then branch on `settings.aiConflictAutonomy`:
- **`proposeReview`** — open the review overlay under a new diffSlot key `ai-proposal:<path>`
  (register the prefix in the `overlayMeta` mapper beside `conflict:` — meta `{ path, origPath:null,
  status:'conflicted', kind:'aiProposal' }`). **Reuse `ConflictEditor`** (§9.5): mount it with a
  synthesized `ConflictFile` `{ ...fetchedConflictFile, text: proposal.proposedText }` so the editor
  shows the markerless proposed body, the user reviews/edits, and **Accept** (its existing
  `onResolve`) calls `resolveConflictText(repoId, path, editedText)` → `refreshAll()` → slot
  collapses (same post-resolve rule as today). **Reject/close** discards (collapse, nothing written).
- **`autoResolve`** — `await ipc.resolveConflictText(repoId, path, proposal.proposedText)` →
  `refreshAll()` → `pushToast('success', `Resolved ${path} with AI — review the staged result`)`.
- Errors (`aiUnavailable`/`aiFailed`) → existing sticky-error-toast path; manual buttons remain.

App also owns the consent `ConfirmDialog` and an `aiAvailability` fetch (on Settings open and on
repo open); reuse the Sidebar dialog-open lift so global shortcuts go inert while the dialog is up.

### 8.4 Consent dialog copy (locked)
Title `Enable AI features?`; body `Bonsai will send the contents of conflicted files to the Claude
Code CLI installed on this machine, under your Claude subscription. Nothing is sent to Bonsai's own
servers, and no files are changed without your review. Enable AI features?`; confirm `Enable`.

---

## 9. Ambiguities resolved here (flag to orchestrator if disagreed)

1. **Read `result` prose, NOT `--json-schema` (v1).** Forcing `{"resolved":"<body>"}` double-encodes
   the entire file body as an escaped JSON string inside the envelope's already-JSON `result` field
   — larger payloads, more escaping edge cases, and higher truncation risk for big files. A firm
   `SYSTEM_PROMPT` ("output ONLY the raw merged file contents") plus defensive fence-stripping
   (§3.3) is simpler and deterministic. `RunOpts.json_schema` is wired but unused, so a later
   feature can opt in without an API change. **Recommendation: read `result`.**
2. **Default model `sonnet`.** Best cost/quality/latency balance for per-file conflict reasoning:
   ~1/5 the cost of `opus` (default `opus` ≈ $0.037/trivial call), far stronger than `haiku` on
   merge logic. Configurable via `RunOpts.model`; no user-facing model setting in v1 (flagged as a
   trivial future addition). **Recommendation: `sonnet`.**
3. **Prompt/payload format** — §5.1, labeled ANCESTOR/OURS/THEIRS/CONFLICTED sections on stdin +
   the strict system prompt. Chosen for unambiguous section boundaries and to give the model both
   the clean sides and the marker view.
4. **Timeout 90 s, std-only wait** via the drain-and-poll pattern (§3.2) — writer + two reader
   threads to avoid pipe-buffer deadlock, `try_wait` poll at 50 ms, `child.kill()` on deadline. No
   new crates.
5. **No `apply_ai_resolution` command — reuse the existing `resolve_conflict_text`.** The codebase
   already ships `resolve_conflict_text(repoId, path, content)` (P12) whose backend fn does exactly
   the required `validate_rel_path` → `fs::write` → `index.add_path` → `index.write()`, AND already
   requires a **live index conflict** (`find_conflict` guard, `conflict.rs:329`). This answers task
   decision 5 (the guard exists and is desirable) and shrinks the new IPC surface to two commands.
   The proposal review likewise reuses the existing `ConflictEditor` (seeded with the proposed body)
   rather than a bespoke read-only accept/reject overlay — the user gets edit-before-accept for
   free, and Save is already gated on `hasUnresolvedMarkers` (AI output is markerless, so Save is
   enabled). **Recommendation: reuse both; do not add `apply_ai_resolution`.** If the orchestrator
   wants a distinct read-only diff overlay instead, that is a frontend-only change with no backend
   impact.
6. **Consent/enable gate is authoritative in the BACKEND.** `ai_resolve_conflict` loads settings and
   refuses (`AiUnavailable`) unless `ai_enabled && ai_consented`, because it is the command that
   performs the outward action (sending repo content to a subprocess); a frontend-only gate can race
   or be bypassed. `check_ai_availability` (a harmless probe) and `resolve_conflict_text` (identical
   to manual text resolution — content origin is irrelevant once the user accepted) are NOT gated.
   The frontend also gates for UX (hide/disable the button, consent dialog). **Recommendation: gate
   in `ai_resolve_conflict` only.**
7. **Milestone-number collision (BLOCKER-level flag).** `P11`/`P12` are already used by shipped
   contracts and code comments (auto-fetch, graph knobs, conflict editor). This contract uses the
   requested filename but recommends new-code comments tag this work **`P13-ai`** to avoid label
   collisions. Orchestrator to confirm the number/tag before senior-dev writes comments.
8. **`logged_in` is not cheaply verifiable** — a real auth check would cost a billable call, so
   `check_availability` reports `logged_in == installed` and the first real resolve surfaces a
   logged-out session as `AiFailed` (with the CLI's own auth message). The settings UI shows the
   `installed`/`detail` state; a genuine "logged out" distinction is deferred. Flagged.
9. **AI offered only for `bothModified`/`bothAdded`.** Deletion/add-conflict/binary/too-large kinds
   have no meaningful text merge; the button is hidden and the backend returns `AiFailed` if forced.
   Matches the existing `ConflictEditor` eligibility.

---

## 10. Testing contract

Conventions (USER MANDATES): scratch repos under `D:\Temp\bonsai-scratch`; `TMP`/`TEMP` = `D:\Temp`;
run `cargo test` and `cargo clippy` **sequentially, never concurrently** (target-dir race).

### 10.1 Stub `claude` (P13a) — enables all Rust tests with no real CLI, no network
A tiny fixture binary/script selected via `BONSAI_CLAUDE_BIN`. It must support, keyed by an env var
the test sets (e.g. `BONSAI_STUB_MODE`):
- `success` — read stdin, echo a canned envelope on stdout with a known `result` body, exit 0.
- `error` — emit `{"is_error":true,"result":"boom","type":"result"}`, exit 0.
- `nonzero` — write to stderr, exit 1.
- `slow` — sleep longer than the (test-shortened) timeout to exercise the kill path.
- `version` — for `check_availability`: print a version string, exit 0.
Provide it as a `.cmd` (Windows) and `.sh` (CI/POSIX) pair, or a small Rust test-bin — senior-dev's
choice; document which under `tests/fixtures/`.

### 10.2 `ai/mod.rs` unit tests (P13a)
1. `run_claude` success → `AiResult.text` == stripped canned body; `cost_usd`/`session_id` parsed.
2. Fence-stripping: a canned `result` wrapped in ```` ```lang … ``` ```` returns the inner body.
3. `is_error:true` → `AiFailed`; non-zero exit → `AiFailed`; slow stub + short `RunOpts.timeout` →
   `AiFailed("timed out …")` and the child is reaped.
4. Missing binary (`BONSAI_CLAUDE_BIN` = a non-existent path) → `AiUnavailable`.
5. `check_availability`: `version` stub → `installed:true`, version parsed; missing binary →
   `installed:false`, never `Err`.
6. Large stdin payload (> 128 KiB) round-trips without deadlock (drain-and-poll proof).

### 10.3 `tests/ai_resolve_cli.rs` (P13c)
With the `success` stub returning a known merged body, drive a real scratch-repo `bothModified`
conflict (reuse the `merge_cli`/`conflict_cli` harness):
1. `ai_resolve_conflict` returns `AiResolveProposal { proposed_text == stub body }` and writes
   NOTHING (index still conflicted, worktree unchanged, path still in `Index::conflicts()`).
2. Feeding that `proposed_text` to `conflict::resolve_conflict_text` (the apply step) clears the
   conflict: path gone from `Index::conflicts()`, stage-0 blob == the applied bytes, worktree bytes
   match, and the subsequent existing `commit_merge` finalizes a 2-parent commit cleanly.
3. Binary / too-large / deletion-kind conflict → `ai_resolve_conflict` → `AiFailed` (no CLI call
   needed; guard fires first).
4. Non-conflicted path → `git`; `../escape` → `invalidName`.

### 10.4 `commands.rs` unit tests (P13b) — §4.2 roundtrip + partial-patch.

---

## 11. Acceptance

**AI gate (orchestrator-verifiable, autonomous):**
- `cargo test` green incl. §10.2–§10.4 (stub `claude` via `BONSAI_CLAUDE_BIN`); `cargo clippy
  -- -D warnings` clean; `pnpm build` clean — after every sub-increment.
- Browser harness (`pnpm dev:mock`, `?op=merge`): the `bothModified` conflict row shows "✨ AI";
  ProposeReview opens the proposal overlay (ConflictEditor seeded with the merged body) → Accept
  stages the file and clears the row; switching autonomy to Auto stages directly with a toast; the
  consent dialog appears on first enable and persists `aiConsented`; `?ai=off` disables the button
  with guidance text; no console errors. Plain (no `?op`) harness unchanged (regression).
- `src/ipc/mock.ts` compiles and implements `checkAiAvailability` + `aiResolveConflict` statefully.

**USER CHECKPOINT (native `pnpm tauri dev`, real logged-in `claude` — never self-declared):**
1. Real conflicted merge → "Resolve with AI" → ProposeReview: review the proposed file, Accept →
   `commit_merge` finalizes; `git status` clean; `git log` shows the 2-parent merge.
2. Switch to Auto-resolve → repeat: the file is staged directly; the staged diff is reviewable
   before commit.
3. With `claude` logged out / absent, the feature disables cleanly with a helpful message; the
   manual ours/theirs/resolved buttons still work.

---

## 12. File touch list

- **New:** `src-tauri/src/ai/mod.rs`, `src-tauri/src/git/ai_resolve.rs`,
  `src-tauri/tests/ai_resolve_cli.rs`, `tests/fixtures/` stub `claude` (§10.1), and (frontend)
  the AI section additions.
- **Edit (Rust):** `src-tauri/src/lib.rs` (`pub mod ai;` + 2 handlers), `src-tauri/src/git/mod.rs`
  (`pub mod ai_resolve;`), `src-tauri/src/commands.rs` (`UiSettings`/patch/`apply_patch`/getter/
  setter + 2 commands), `src-tauri/src/settings.rs` (`AiAutonomy` + 3 fields), `src-tauri/src/error.rs`
  (2 variants).
- **Edit (frontend):** `src/ipc/{types,tauri,index,mock}.ts`, `src/components/SettingsPanel.tsx`,
  `src/components/StatusPanel.tsx`, `src/components/RepoWorkspace.tsx` (+ `overlayMeta`
  `ai-proposal:` prefix).
- **Reuse (do not reinvent):** `conflict::resolve_conflict_text` (apply step) + the
  `resolve_conflict_text` command; `conflict::get_conflict` (marker view + guards); `ConflictEditor`
  (proposal review); the `spawn_blocking(_inner)` command pattern; `settings.rs` additive-field
  pattern; `SettingsPanel` page; the `commit_merge`/`OpBanner`/conflict-overlay flow (UNCHANGED).
