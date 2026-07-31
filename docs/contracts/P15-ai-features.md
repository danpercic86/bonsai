# P15 — In-app AI features (Tier 1): commit-message gen, explain/review, branch/range summary

Status: authoritative for this milestone. Scope: THREE new user-facing AI features, each a thin
consumer of the P13 `run_claude` primitive. No new process-spawning code, no MCP exposure, no new
crates. Comment tag for new code: **`P15`**.

Builds on / reuses (READ THESE — they are the contract's substrate):
- `docs/contracts/P13-ai-foundation.md` — the `run_claude` primitive, `RunOpts`, `AiResult`,
  `AiAvailability`, `check_availability`, the stub-`claude` test harness (`BONSAI_CLAUDE_BIN` +
  `tests/fixtures/claude_stub.cmd`), the consent gate, and the `ai_resolve.rs` consumer pattern.
  **Mirror `ai_resolve.rs` for each new feature: a serde camelCase result struct + a pure
  bonsai-core fn that assembles a labeled stdin payload and calls `run_claude`.**
- `crates/bonsai-core/src/ai/mod.rs` — `run_claude(cwd, prompt, stdin_payload, opts)`,
  `RunOpts { model, timeout, system_prompt, json_schema }`. Windows constraint (LOCKED): the
  `claude` CLI is a `.cmd` shim; Rust's `Command` REFUSES an argv arg containing a newline. Every
  `system_prompt` and `-p` prompt in this contract is a **single line**. Multi-line content ONLY via
  the stdin payload.
- `crates/bonsai-core/src/git/diff.rs` — the typed diff engine reused for every payload:
  `workdir_file_diff` (staged/unstaged, per file, full hunks), `commit_diff` (headers vs first
  parent), `commit_file_diff` (per-file hunks vs first parent), `compare_head_diff`, the private
  `collect_headers`/`build_diff_options`/`apply_find_similar`/`commit_details` helpers (P15c makes
  three `pub(crate)`), and `MAX_FILE_DIFF_LINES`.
- `crates/bonsai-core/src/git/status.rs` — `read_status` (staged file list for P15a / analyze-staged).
- `crates/bonsai-core/src/git/branches.rs` — `list_refs` (BranchesSnapshot; the P15c Sidebar hook).
- `crates/bonsai-core/src/error.rs` — reuse `AiUnavailable` / `AiFailed` / `Git` / `InvalidName` /
  `NoRepo` / `NothingToCommit`. **No new variants** (justified §7.1).
- `src-tauri/src/commands.rs` — the `#[tauri::command] async fn` → runtime-free `_inner` →
  `spawn_blocking` shape; `ai_resolve_conflict_inner` is the exact template (settings gate first,
  then `repo_path`, then core fn under `spawn_blocking`). `repo_path(state, repo_id)` helper.
- `src-tauri/src/settings.rs` — `ai_enabled` / `ai_consented`. **No new settings** (justified §7.2).
- IPC: `src/ipc/{types,tauri,index,mock}.ts` — every new command needs a TS type, an `index.ts`
  re-export + `tauri.ts` invoke wrapper, and a canned `mock.ts` response (works with NO `claude`).

Invariants (non-negotiable, enforced in review): Rust owns ALL Git logic AND subprocess logic;
React only renders. Commands = request/response. blocking work (git2 + `std::process`) runs under
`spawn_blocking` via `*_inner`. No git2 handle crosses `.await`. All three features are read-only
"propose text to the user" — they WRITE NOTHING and stage NOTHING (no autonomy/write gate needed).

---

## 1. Scope split (sub-increments)

| # | Increment | Content | §§ |
|---|-----------|---------|-----|
| 1 | **P15a — commit-message gen** | `ai/payload.rs` (shared payload renderer, introduced here); `git/ai_commit.rs` (`CommitMessageProposal`, `generate_commit_message`); command `generate_commit_message`; IPC + mock `generateCommitMessage`; `CommitBox` "✨ Generate" button. | §2, §3, §5, §6 |
| 2 | **P15b — explain / review** | `git/ai_explain.rs` (`AiAnalysis`, `AiDiffTarget`, `AiAnalysisMode`, `analyze_diff`); command `ai_analyze_diff`; IPC + mock `aiAnalyzeDiff`; `AiOutputPanel` component + Explain/Review affordances. | §2, §4, §5, §6 |
| 3 | **P15c — summarize branch/range** | `git/ai_summary.rs` (`AiSummary`, `summarize_range`); diff.rs `pub(crate)` exposure; command `ai_summarize_range`; IPC + mock `aiSummarizeRange`; Sidebar branch context-menu action. | §2, §4.4, §5, §6 |

Each is one fresh-context senior-dev pass (this file + the exact source paths). Tester runs after
each lands (§8). Orchestrator commits each approved increment (`wip(P15a): …` etc.).

---

## 2. Shared payload renderer — `crates/bonsai-core/src/ai/payload.rs` (NEW, P15a)

Pure Rust, no git2, no Tauri — turns already-computed `FileDiff`/header data into a token-efficient,
budget-capped, labeled text block for stdin. Unit-testable with fabricated `FileDiff` values.
Register `pub mod payload;` in `ai/mod.rs`.

```rust
//! Renders precomputed diff data into labeled stdin payloads for `run_claude`.
//! Pure (no git2 / no Tauri): callers gather typed diffs via git/diff.rs, then
//! render here. All payloads are newline-bearing => stdin ONLY, never argv. (P15)

use crate::git::diff::{FileDiff, FileDiffHeader, LineKind};

/// Total emitted diff-content lines (add/del/context) across ALL files in one
/// payload. Past this the render stops adding files and appends a truncation
/// note. Chosen ~= `MAX_FILE_DIFF_LINES` so a payload is at most a few files of
/// max size — comfortably inside the model context and the 90 s call budget.
pub const MAX_PAYLOAD_LINES: usize = 6_000;
/// Hard cap on files rendered in one payload (diffstat/commit-heavy changes).
pub const MAX_PAYLOAD_FILES: usize = 300;

/// One rendered payload plus whether it was clipped (callers may note it).
pub struct RenderedPayload {
    pub text: String,
    pub truncated: bool,
    pub files_shown: usize,
    pub files_total: usize,
}

/// Render a list of full `FileDiff`s (with hunks) as labeled sections:
///   ===== FILE: <path> (<status>[, was <origPath>]) =====
///   <unified-ish body: " ctx", "+add", "-del" per DiffLine, hunk headers @@ …>
/// binary/too_large files render a one-line placeholder (no body). Stops once
/// `MAX_PAYLOAD_LINES`/`MAX_PAYLOAD_FILES` is hit; on truncation appends
/// "\n... (diff truncated: showed N of M files) ...". Deterministic; input order
/// preserved.
pub fn render_file_diffs(files: &[FileDiff]) -> RenderedPayload;

/// Compact diffstat block (no hunks): one line per header
///   <path>  +<additions> -<deletions>[  (binary)][  was <origPath>]
/// Capped at `MAX_PAYLOAD_FILES`. Used by P15c (aggregate range change).
pub fn render_headers(files: &[FileDiffHeader]) -> RenderedPayload;

/// One commit line per entry for the P15c commit-list section:
///   <short7 oid>  <summary>  (<author>)
/// Caller pre-caps the slice (see AI_SUMMARY_MAX_COMMITS).
pub fn render_commit_list(lines: &[CommitLine]) -> String;

/// Minimal commit descriptor for `render_commit_list` (assembled by ai_summary
/// from a revwalk — NOT a wire type).
pub struct CommitLine {
    pub short_oid: String, // first 7 hex chars
    pub summary: String,   // first message line
    pub author: String,    // author name
}
```

`LineKind` mapping in `render_file_diffs`: `Context` → `" "`, `Add` → `"+"`, `Del` → `"-"` prefix,
then `DiffLine.content` (already newline-stripped). Emit a hunk header
`@@ -{oldStart},{oldLines} +{newStart},{newLines} @@` before each hunk's lines. Count every emitted
add/del/context line against the budget.

---

## 3. P15a — commit-message generation (`crates/bonsai-core/src/git/ai_commit.rs`, NEW)

Register `pub mod ai_commit;` in `git/mod.rs`. Mirrors `ai_resolve.rs`.

```rust
//! AI commit-message generation. Reads the STAGED diff (HEAD tree vs index),
//! renders a payload, and asks the local `claude` CLI for a concise
//! Conventional-Commits message. WRITES NOTHING — the user edits the returned
//! text in the commit box before committing. Pure git2 + crate::ai. (P15)

use std::path::Path;
use crate::ai::RunOpts;
use crate::error::AppError;

/// The model's proposed commit message. Serialized camelCase (mirrored in TS).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitMessageProposal {
    pub message: String,        // trimmed; may contain newlines (summary + body)
    pub cost_usd: Option<f64>,
}

/// Blocking. Gathers the staged diff and returns a proposed message.
/// - Empty staged set (index matches HEAD) => `AppError::NothingToCommit`
///   BEFORE any CLI call (§7.1).
/// - Otherwise renders the staged payload (§3.1) and calls run_claude.
/// Errors: `aiFailed` (CLI error/empty/timeout) | `nothingToCommit`
///   (empty staged) | `git` (repo open) | (`aiUnavailable` is enforced by the
///   command gate, not here).
pub fn generate_commit_message(
    workdir: &Path,
    opts: RunOpts,
) -> Result<CommitMessageProposal, AppError>;
```

### 3.1 Payload assembly (reuse existing public fns — no new diff plumbing)
1. `let staged = crate::git::status::read_status(workdir)?;` collect entries whose staged status is
   present (Added/Modified/Deleted/Renamed/Typechange in the index — the "staged" list the status
   panel already shows). If none → `Err(NothingToCommit)`.
2. For each staged entry, `crate::git::diff::workdir_file_diff(workdir, &path, orig_path, /*staged=*/true)`
   → `Vec<FileDiff>` (respect the entry's `origPath` for renames).
3. `let payload = payload::render_file_diffs(&file_diffs);` prefix with a one-line header
   `STAGED CHANGES (git diff --cached):\n\n` then `payload.text`.
4. `run_claude(workdir, COMMIT_PROMPT, Some(&payload_text), RunOpts { system_prompt:
   Some(COMMIT_SYSTEM_PROMPT.into()), ..opts })`. `message = result.text` (already fence-stripped),
   `cost_usd = result.cost_usd`.

### 3.2 Prompts (LOCKED, single-line — argv-safe)
`COMMIT_SYSTEM_PROMPT` (const, `--append-system-prompt`):
> You are a Git commit-message author. Given a staged diff on standard input, write ONE concise commit message in Conventional Commits style: a short imperative summary line of at most 72 characters (for example 'feat(scope): ...', 'fix: ...', 'refactor: ...'), then, only if warranted, a blank line followed by a brief body of one-line bullet points. Output ONLY the commit message text — no explanations, no preamble, no surrounding quotes, and no markdown code fences.

`COMMIT_PROMPT` (const, `-p`):
> Write a commit message for the staged changes provided on standard input.

---

## 4. P15b — explain / review (`crates/bonsai-core/src/git/ai_explain.rs`, NEW)

**Decision: ONE command with two orthogonal params (`target` + `mode`), not two commands.**
Justification: both actions are "prose about a diff"; only the diff SOURCE (`target`) and the system
prompt (`mode`) vary. Folding them keeps one IPC command, one `_inner`, one core fn, one payload
builder, one mock, one size cap — minimal surface for maximal coverage. The frontend chooses which
combinations to surface (Explain on a commit or a file; Review on the staged set); the backend is
generic. Register `pub mod ai_explain;` in `git/mod.rs`.

```rust
//! AI explain/review of typed diff data. `analyze_diff` selects a diff source
//! (a commit, a working-dir file, or the whole staged set), renders a payload,
//! and asks the CLI to either EXPLAIN (plain English) or REVIEW (risks/bugs/
//! style) it. Read-only prose out; WRITES NOTHING. Pure git2 + crate::ai. (P15)

use std::path::Path;
use crate::ai::RunOpts;
use crate::error::AppError;

/// Which diff to analyze. `#[serde(tag="kind", rename_all="camelCase")]` — this
/// is a COMMAND INPUT (Deserialize); TS mirror is a discriminated union (§5).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AiDiffTarget {
    /// Commit vs its first parent (root => vs empty tree). `oid` = 40-hex.
    Commit { oid: String },
    /// One working-dir file. `staged=false` => index vs workdir; `staged=true`
    /// => HEAD vs index. `orig_path` for renames.
    WorkdirFile {
        path: String,
        #[serde(default)]
        orig_path: Option<String>,
        staged: bool,
    },
    /// The whole staged set (HEAD tree vs index) — the natural Review target.
    Staged,
}

/// Explain (teammate-friendly summary) vs Review (risks/bugs/style).
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiAnalysisMode {
    Explain,
    Review,
}

/// Prose result. Serialized camelCase (mirrored in TS).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAnalysis {
    pub text: String,
    pub cost_usd: Option<f64>,
}

/// Blocking. Gathers `target`'s diff, renders a payload, calls run_claude with
/// the `mode` system prompt. An EMPTY target diff (no changes) => `AiFailed(
/// "no changes to analyze")` before any CLI call (§7.1). Errors: `aiFailed`
/// | `git` (bad oid) | `invalidName` (bad path) | (`aiUnavailable` via gate).
pub fn analyze_diff(
    workdir: &Path,
    target: AiDiffTarget,
    mode: AiAnalysisMode,
    opts: RunOpts,
) -> Result<AiAnalysis, AppError>;
```

### 4.1 Payload assembly per target (reuse existing public fns)
- `Commit { oid }`: `commit_diff(workdir, &oid)?` for headers → for each header
  `commit_file_diff(workdir, &oid, &h.path, h.orig_path)` → `Vec<FileDiff>`; prefix with
  `COMMIT <short7>  <summary>\nAUTHOR <name>\n\n` from `CommitDiff.details`, then
  `render_file_diffs`.
- `WorkdirFile { path, orig_path, staged }`: `workdir_file_diff(workdir, &path, orig_path, staged)`
  → single-element `Vec<FileDiff>` → `render_file_diffs`.
- `Staged`: identical gather to §3.1 steps 1–3 (staged file diffs); empty → `AiFailed`.
- Any gathered payload with zero add/del lines → `AiFailed("no changes to analyze")`.

### 4.2 Prompts (LOCKED, single-line)
`EXPLAIN_SYSTEM_PROMPT`:
> You are a senior engineer explaining a code change to a teammate. Given a diff on standard input, explain in clear plain English what the change does and, where inferable, why — a one or two sentence high-level summary first, then the key specifics grouped by file. Be concise and concrete. Output prose only — no markdown code fences.

`REVIEW_SYSTEM_PROMPT`:
> You are a meticulous senior code reviewer. Given a diff on standard input, review it for likely bugs, correctness and edge-case risks, security issues, and notable style or maintainability problems. Be concise and specific and cite file names. If you find nothing significant, say so briefly. Output prose only — no markdown code fences.

`EXPLAIN_PROMPT` (`-p`): `Explain the change provided on standard input.`
`REVIEW_PROMPT` (`-p`): `Review the change provided on standard input.`
`analyze_diff` picks the (system_prompt, prompt) pair from `mode`.

### 4.4 P15c — summarize branch / range (`crates/bonsai-core/src/git/ai_summary.rs`, NEW)

Register `pub mod ai_summary;` in `git/mod.rs`.

```rust
//! AI branch/range summary. Given a base ref and a target ref, gathers the
//! commits unique to target (base..target) plus the net diffstat, renders a
//! compact payload, and asks the CLI to summarize what the branch/range
//! introduces. Read-only prose out; WRITES NOTHING. Pure git2 + crate::ai. (P15)

use std::path::Path;
use crate::ai::RunOpts;
use crate::error::AppError;

/// Cap on commits listed in the payload (keeps the call bounded). Beyond it the
/// list is truncated with a "(+N more commits)" note.
pub const AI_SUMMARY_MAX_COMMITS: usize = 200;

/// Prose summary + echoed context. Serialized camelCase (mirrored in TS).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSummary {
    pub text: String,
    pub base: String,        // resolved base ref shorthand, echoed for the UI header
    pub target: String,      // resolved target ref shorthand
    pub commit_count: u32,   // commits unique to target vs base (pre-truncation, capped display)
    pub cost_usd: Option<f64>,
}

/// Blocking. `base`/`target` are ref shorthands/oids (revparse_single).
/// Uses the merge-base of the two (§7.3 decision) so the summary reflects what
/// TARGET introduces since divergence. Empty range (no unique commits) =>
/// `AiFailed("nothing to summarize: <target> has no commits beyond <base>")`
/// before any CLI call. Errors: `aiFailed` | `git` (bad ref) | (`aiUnavailable`
/// via gate).
pub fn summarize_range(
    workdir: &Path,
    base: &str,
    target: &str,
    opts: RunOpts,
) -> Result<AiSummary, AppError>;
```

### 4.5 Gather + payload (reuse diff.rs `pub(crate)` helpers)
1. `revparse_single(base)?.peel_to_commit()?` and same for `target`; on failure → `Git`.
2. `let mb = repo.merge_base(base_oid, target_oid)`; `mb_tree` = mb commit's tree, or the empty tree
   when there is no merge base (unrelated histories — note it in the payload header).
3. Commit list: revwalk `push(target_oid)`, `hide(mb_oid)` (or `hide(base_oid)` when no mb); collect
   up to `AI_SUMMARY_MAX_COMMITS` as `payload::CommitLine`. `commit_count = collected len`
   (pre-truncation total tracked for the "(+N more)" note). Empty → `AiFailed` (see above).
4. Aggregate diffstat: `diff_tree_to_tree(mb_tree, target_tree)` with `build_diff_options(&[])` +
   `apply_find_similar` → `collect_headers` → `Vec<FileDiffHeader>` (all three now `pub(crate)`).
5. Payload = `COMMITS (target since base):\n` + `render_commit_list(...)` + `\n\nNET CHANGES
   (diffstat):\n` + `render_headers(...).text`.
6. `run_claude(workdir, SUMMARY_PROMPT, Some(&payload), RunOpts { system_prompt:
   Some(SUMMARY_SYSTEM_PROMPT.into()), ..opts })`.

### 4.6 diff.rs change (P15c) — expose three helpers
Change `fn collect_headers`, `fn build_diff_options`, `fn apply_find_similar` from private to
`pub(crate)` (signatures unchanged). No behavior change; enables ai_summary to reuse the header
collection against arbitrary trees. (No new public API on the wire.)

### 4.7 Prompts (LOCKED, single-line)
`SUMMARY_SYSTEM_PROMPT`:
> You are summarizing the difference between two Git points for a teammate. Given a list of commits and a diffstat on standard input, summarize what this branch or range introduces: the main themes, the notable changes grouped sensibly, and anything risky or incomplete. Be concise. Output prose only — no markdown code fences.

`SUMMARY_PROMPT` (`-p`): `Summarize the branch or range described on standard input.`

---

## 5. Commands (`src-tauri/src/commands.rs` + `lib.rs generate_handler!`)

Three new commands, ALL following the `ai_resolve_conflict` template EXACTLY: `#[tauri::command]
async fn` resolves the settings file at the `AppHandle` boundary and delegates to a runtime-free
`_inner`; the `_inner` (1) loads settings and enforces the consent gate, (2) resolves `repo_path`,
(3) runs the core fn under `spawn_blocking`. Register all three in `generate_handler![]`.

```rust
/// P15a. Generate a commit message from the staged diff. Gate: ai_enabled &&
/// ai_consented (else AiUnavailable). Errors: aiUnavailable | aiFailed |
/// nothingToCommit | git | noRepo.
#[tauri::command]
pub async fn generate_commit_message(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<CommitMessageProposal, AppError>;

/// P15b. Explain or review a diff target. Gate as above. Errors: aiUnavailable
/// | aiFailed | git | invalidName | noRepo.
#[tauri::command]
pub async fn ai_analyze_diff(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    target: AiDiffTarget,
    mode: AiAnalysisMode,
) -> Result<AiAnalysis, AppError>;

/// P15c. Summarize commits/diff unique to `target` vs `base`. Gate as above.
/// Errors: aiUnavailable | aiFailed | git | noRepo.
#[tauri::command]
pub async fn ai_summarize_range(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    base: String,
    target: String,
) -> Result<AiSummary, AppError>;
```

Each `_inner` (shape identical to `ai_resolve_conflict_inner`):
```text
let s = settings::load_from(settings_file);
if !(s.ai_enabled && s.ai_consented) { return Err(AiUnavailable("AI features are disabled or not yet consented to")); }
let workdir = repo_path(state, repo_id)?;
spawn_blocking(move || <core fn>(&workdir, …, RunOpts::default())).await.map_err(join)?
```
`RunOpts::default()` → model `sonnet`, 90 s timeout; system prompts are set inside each core fn.
Extend the existing NoRepo/gate command test to cover the disabled-gate path of at least one new
command (asserts `AiUnavailable` without needing a real CLI).

Import the new types into `commands.rs` (`use bonsai_core::git::ai_commit::CommitMessageProposal;`
etc., plus `ai_explain::{AiAnalysis, AiDiffTarget, AiAnalysisMode}`, `ai_summary::AiSummary`).

---

## 6. IPC mirror + mock

### 6.1 `src/ipc/types.ts` (verbatim additions)
```ts
export interface CommitMessageProposal {
  /** Trimmed; may contain newlines (summary + body). */
  message: string;
  costUsd: number | null;
}

export type AiAnalysisMode = 'explain' | 'review';

/** Diff source for aiAnalyzeDiff — discriminated on `kind`. */
export type AiDiffTarget =
  | { kind: 'commit'; oid: string }
  | { kind: 'workdirFile'; path: string; origPath: string | null; staged: boolean }
  | { kind: 'staged' };

export interface AiAnalysis {
  text: string;
  costUsd: number | null;
}

export interface AiSummary {
  text: string;
  base: string;
  target: string;
  commitCount: number;
  costUsd: number | null;
}
```
Note: `AiDiffTarget.workdirFile.origPath` is `string | null` on the wire; Rust `#[serde(default)]
Option<String>` accepts a missing key too, but the TS wrapper always sends an explicit `null` for
symmetry with the rest of the IPC surface.

`IpcApi` gains (mirror the Rust error lists):
```ts
/** P15a. Generate a commit message from the staged diff. Never auto-commits.
 *  Rejects aiUnavailable | aiFailed | nothingToCommit | git | noRepo. */
generateCommitMessage(repoId: string): Promise<CommitMessageProposal>;
/** P15b. Explain or review a diff target (read-only prose).
 *  Rejects aiUnavailable | aiFailed | git | invalidName | noRepo. */
aiAnalyzeDiff(repoId: string, target: AiDiffTarget, mode: AiAnalysisMode): Promise<AiAnalysis>;
/** P15c. Summarize commits/diff unique to `target` vs `base`.
 *  Rejects aiUnavailable | aiFailed | git | noRepo. */
aiSummarizeRange(repoId: string, base: string, target: string): Promise<AiSummary>;
```
Re-export `CommitMessageProposal`, `AiAnalysis`, `AiAnalysisMode`, `AiDiffTarget`, `AiSummary` from
`src/ipc/index.ts`.

### 6.2 `src/ipc/tauri.ts` (beside `aiResolveConflict`)
```ts
generateCommitMessage: (repoId) => invoke('generate_commit_message', { repoId }),
aiAnalyzeDiff: (repoId, target, mode) => invoke('ai_analyze_diff', { repoId, target, mode }),
aiSummarizeRange: (repoId, base, target) => invoke('ai_summarize_range', { repoId, base, target }),
```

### 6.3 `src/ipc/mock.ts` — canned twins (work with NO claude installed)
Reuse the existing module-init `AI_OFF = query('ai') === 'off'` flag. Mock does NOT enforce the
consent gate (matches the shipped `aiResolveConflict` mock; the frontend gates the affordances). All
three: `await delay(500)`; when `AI_OFF` → reject `{ kind:'aiFailed', message:'Claude Code CLI not
found on PATH' }` (mirrors the real backend's behavior when the CLI is missing).

- `generateCommitMessage(repoId)`: `const state = requireRepo(repoId)`. If the repo's mock status has
  zero staged entries → reject `{ kind:'nothingToCommit', message:'nothing to commit (index matches
  HEAD)' }`. Else return
  `{ message: 'feat(sidebar): add branch summary action\n\n- wire ai_summarize_range command\n- add
  context-menu entry', costUsd: 0.004 }`.
- `aiAnalyzeDiff(repoId, target, mode)`: `requireRepo(repoId)`. Return
  `{ text: mode === 'review'
      ? 'Review: no blocking issues. Consider a null-check on the new branch lookup in
         Sidebar.tsx; the added revwalk is unbounded — confirm the AI_SUMMARY_MAX_COMMITS cap is
         applied. Style LGTM.'
      : 'This change adds a "Summarize branch" context-menu action in the sidebar and a matching
         ai_summarize_range command that gathers base..target commits plus a diffstat and calls the
         local Claude CLI.',
    costUsd: 0.006 }`.
  (A tiny per-`target.kind` prefix, e.g. "Commit <oid short>: …", is a nice-to-have, not required.)
- `aiSummarizeRange(repoId, base, target)`: `requireRepo(repoId)`. Return
  `{ text: 'This branch introduces the P15 in-app AI features: commit-message generation,
     explain/review of diffs, and branch/range summaries — three thin consumers of the existing
     run_claude primitive. No new settings or process code; all read-only.',
    base, target, commitCount: 3, costUsd: 0.008 }`.

---

## 5.5 Frontend affordances (mirror ConflictEditor's availability-gating + loading/error UX)

`aiEligible` (computed in the App/workspace, reused from P13 §8.2):
`aiEnabled && aiConsented && aiAvailability?.installed === true`. Every affordance below is HIDDEN or
DISABLED unless `aiEligible` (identical gating to the existing "✨ AI" conflict action). A call in
flight shows a spinner/disabled state; `aiUnavailable`/`aiFailed`/`nothingToCommit` surface via the
existing toast/inline-error path. Nothing is ever written automatically.

- **P15a — `CommitBox.tsx`.** Add a `✨ Generate` secondary button in the commit-box header
  (visible only in `mode==='commit'`, not merge). New props: `aiEligible: boolean;
  onGenerate(): Promise<string>` (App calls `ipc.generateCommitMessage(repoId)` and returns
  `proposal.message`). On click: set a local `generating` flag, `const msg = await onGenerate()`,
  set the textarea `message` to `msg` (REPLACING current text; if the box is non-empty, confirm
  first via the existing ConfirmDialog — flagged §7.4). Disabled when `!aiEligible || stagedCount===0
  || busy || generating`. Errors shown in the existing `error-banner`. Never calls commit.
- **P15b — `AiOutputPanel.tsx` (NEW) + two triggers.** `AiOutputPanel` is a lightweight read-only
  panel: props `{ title: string; text: string | null; loading: boolean; error: string | null;
  costUsd?: number | null; onClose(): void }` rendering markdown-free prose in a dismissible
  card (reuse existing panel/toast CSS; NO new IPC). Triggers:
  - **Explain commit** — in the commit-details right panel (shown when a graph node is selected), an
    `✨ Explain` button → `ipc.aiAnalyzeDiff(repoId, { kind:'commit', oid }, 'explain')` → show in
    `AiOutputPanel` (title `Explain commit <short7>`).
  - **Explain file** — in the file-diff header (`DiffOverlay`/changes row), an `✨ Explain` action →
    `aiAnalyzeDiff(repoId, { kind:'workdirFile', path, origPath, staged }, 'explain')`.
  - **Review staged** — in the staged-section header of the changes panel, a `✨ Review` action →
    `aiAnalyzeDiff(repoId, { kind:'staged' }, 'review')` → `AiOutputPanel` (title `Review staged
    changes`). Disabled when `stagedCount===0`.
- **P15c — `Sidebar.tsx` branch context menu.** Add a `Summarize branch…` item to the existing
  local-branch context menu → `ipc.aiSummarizeRange(repoId, base, branch.name)` → `AiOutputPanel`
  (title `Summary: <base> → <branch>`). **Base selection (frontend policy, §7.5):** `base` = the
  repo's primary branch (`main`, else `master`, else the current HEAD branch) UNLESS the target IS
  that branch, in which case `base` = the branch's `upstream` (from `BranchInfo.upstream`) when set,
  else skip the item. Optional secondary hook (NOT required for P15c): two selected graph commits →
  `aiSummarizeRange(repoId, olderOid, newerOid)`.

---

## 7. Ambiguities resolved / decisions to confirm

### 7.1 No new AppError variants (reuse existing)
- Consent/no-CLI → `AiUnavailable` (command gate + `run_claude` NotFound).
- CLI ran but errored/empty/timeout → `AiFailed`.
- Bad oid/ref → `Git`; bad path → `InvalidName` (via `validate_rel_path` inside the diff fns).
- Empty staged set (P15a, and P15b `Staged`) → `NothingToCommit` (message "index matches HEAD" fits
  exactly). Empty commit/file/range diffs (P15b non-staged, P15c) → `AiFailed("no changes to
  analyze" / "nothing to summarize …")`. **Recommendation: reuse; add no variants.** Rationale for
  using `AiFailed` on empty non-staged diffs rather than a new precondition variant: the frontend
  disables these affordances when there is nothing to analyze, so this is a defensive backstop, and
  `AiFailed`'s message channel carries a clear human string. Flagged in case the orchestrator prefers
  a dedicated `nothingToAnalyze` kind (would touch `error.rs` + the TS union + mock).

### 7.2 No new settings
All three features reuse `ai_enabled && ai_consented`. They are read-only "propose text to the user"
— nothing is written or staged — so the `AiAutonomy` (ProposeReview/AutoResolve) knob is irrelevant
and no new autonomy/write gate is introduced. **Confirmed: zero settings changes.**

### 7.3 P15c range semantics — merge-base (recommended) vs direct tree diff
`summarize_range(base, target)` uses `merge_base(base, target)`: commit list = `mb..target`,
diffstat = `diff(mb_tree, target_tree)`. This yields "what TARGET introduces since it diverged",
which is what "summarize this branch vs main" means and avoids counting base's post-divergence
changes as reversed deletions. The alternative (`git diff base target` direct tree compare, matching
`compare_head_diff`) is simpler but noisier for branch summaries. **Recommendation: merge-base.**
For unrelated histories (no merge base) fall back to base_tree (direct) and note it in the payload.

### 7.4 Generate overwrites the commit box
"✨ Generate" replaces existing textarea text. **Recommendation:** if the box is non-empty, show a
one-line ConfirmDialog ("Replace the current message?") before replacing; if empty, replace
silently. Confirm this is desired vs. always-replace.

### 7.5 P15c base-selection heuristic lives in the frontend
The backend stays generic (`base`, `target` strings). The Sidebar computes `base` per §5.5. This
keeps the algorithm out of Rust and lets a future UI offer an explicit base picker without a command
change. **Confirm the main → master → HEAD → upstream fallback order.**

### 7.6 One `ai_analyze_diff` command (not two)
Justified in §4. Confirm you are happy folding explain+review into a `mode` param rather than two
commands.

---

## 8. Testing contract

Conventions (USER MANDATES): scratch repos under `D:\Temp\bonsai-scratch`; `TMP`/`TEMP`=`D:\Temp`;
run `cargo test` and `cargo clippy` **sequentially, never concurrently**. Reuse the P13 stub
(`BONSAI_CLAUDE_BIN` + `tests/fixtures/claude_stub.cmd`, `BONSAI_STUB_MODE`); add a stub mode if a
feature needs a distinct canned body (e.g. a `commitmsg` mode echoing a known message), or reuse
`success` and assert on the stub's fixed body.

### 8.1 `ai/payload.rs` unit tests (P15a) — pure, no git2/CLI
1. `render_file_diffs` emits `+`/`-`/` ` prefixes, hunk headers, and a FILE label per file;
   binary/too_large files render a placeholder line, no body.
2. Budget: a `Vec<FileDiff>` exceeding `MAX_PAYLOAD_LINES` stops early, sets `truncated=true`,
   `files_shown < files_total`, and appends the truncation note.
3. `render_headers` diffstat lines; `render_commit_list` short-oid/summary/author formatting.
4. Wire-shape tests for `CommitMessageProposal` / `AiAnalysis` / `AiSummary` (camelCase, `costUsd`
   null serializes) — mirror `ai_resolve.rs`'s `proposal_wire_shape_is_camel_case`.

### 8.2 `git/ai_commit.rs` + `tests/ai_commit_cli.rs` (P15a) — stub claude
1. Staged scratch repo → `generate_commit_message` returns `message == stub body`, `cost_usd` parsed;
   writes NOTHING (index/worktree unchanged).
2. Empty staged (clean index) → `NothingToCommit`, no CLI call.
3. Payload contains the staged file's added/deleted lines (assert a known line is present).

### 8.3 `git/ai_explain.rs` + `tests/ai_explain_cli.rs` (P15b) — stub claude
1. `AiDiffTarget::Commit{oid}` on a scratch commit → `AiAnalysis.text == stub body`; both `explain`
   and `review` modes call through (assert the mode-selected system prompt path via a stub that
   echoes the received `--append-system-prompt`, or just assert Ok for each mode).
2. `WorkdirFile` (staged and unstaged) and `Staged` targets each build a non-empty payload → Ok.
3. Empty diff (commit vs identical parent is impossible; use a clean workdir file) → `AiFailed("no
   changes to analyze")`, no CLI call. Bad oid → `Git`; `../escape` path → `invalidName`.

### 8.4 `git/ai_summary.rs` + `tests/ai_summary_cli.rs` (P15c) — stub claude
1. Diverged base/feature scratch repo → `summarize_range(base, feature)` returns `AiSummary` with
   `commit_count == unique commits`, `base`/`target` echoed, `text == stub body`; uses merge-base
   (a commit base made AFTER divergence is NOT in the list).
2. Empty range (`target` == `base`, or target has no unique commits) → `AiFailed("nothing to
   summarize …")`, no CLI call. Bad ref → `Git`.
3. `AI_SUMMARY_MAX_COMMITS` truncation note appears when exceeded (small const override or a
   many-commit fixture — reuse the perf-fixture generator if cheap; otherwise assert the cap logic
   with a lowered const behind `#[cfg(test)]`).

### 8.5 `commands.rs` — extend the gate test: at least one new command's `_inner` returns
`AiUnavailable` when `ai_enabled && ai_consented` is false (settings-file-parameterized, no CLI).

---

## 9. Acceptance

**AI gate (orchestrator-verifiable, autonomous) — per sub-increment:**
- `cargo test` green incl. §8 (stub `claude`); `cargo clippy -- -D warnings` clean; `pnpm build` +
  `tsc` clean — after every increment.
- Browser harness (`pnpm dev:mock`), screenshots of each affordance from canned mock data:
  - P15a: staged repo → "✨ Generate" fills the commit textarea with the canned message; empty
    staged → button disabled / `nothingToCommit` surfaced; user can still edit before Commit.
  - P15b: selecting a commit shows "✨ Explain" → `AiOutputPanel` renders the canned explanation;
    staged section "✨ Review" → canned review prose; file-diff "✨ Explain" works.
  - P15c: Sidebar branch context menu "Summarize branch…" → `AiOutputPanel` with base→target header
    + canned summary.
  - `?ai=off` disables/greys the affordances (no `installed`); no console errors; plain harness
    (no `?ai`) unchanged (regression).
- `src/ipc/mock.ts` compiles and implements all three new methods.

**USER CHECKPOINT (native `pnpm tauri dev`, real logged-in `claude` — never self-declared):**
1. Stage real changes → Generate produces a sane Conventional-Commits message; edit + commit works.
2. Select a commit → Explain gives a coherent plain-English summary; Review staged flags real issues.
3. Right-click a feature branch → Summarize produces a sensible branch summary vs the base.
4. With `claude` absent/logged-out, every affordance disables cleanly with the shared guidance; no
   feature ever writes or commits without the user's explicit action.

---

## 10. File touch list

- **New (Rust):** `crates/bonsai-core/src/ai/payload.rs`, `crates/bonsai-core/src/git/ai_commit.rs`,
  `crates/bonsai-core/src/git/ai_explain.rs`, `crates/bonsai-core/src/git/ai_summary.rs`;
  `src-tauri/tests/ai_commit_cli.rs`, `ai_explain_cli.rs`, `ai_summary_cli.rs`.
- **Edit (Rust):** `crates/bonsai-core/src/ai/mod.rs` (`pub mod payload;`),
  `crates/bonsai-core/src/git/mod.rs` (3 `pub mod`), `crates/bonsai-core/src/git/diff.rs`
  (3 helpers → `pub(crate)`), `src-tauri/src/commands.rs` (3 commands + `_inner` + imports + gate
  test), `src-tauri/src/lib.rs` (3 `generate_handler!` entries).
- **New (frontend):** `src/components/AiOutputPanel.tsx`.
- **Edit (frontend):** `src/ipc/{types,tauri,index,mock}.ts`, `src/components/CommitBox.tsx`
  (+ its parent wiring `onGenerate`/`aiEligible`), `src/components/Sidebar.tsx` (branch context
  menu), the commit-details panel + changes/diff panels for the Explain/Review triggers,
  `RepoWorkspace.tsx` (owns the `ipc.*` calls + `AiOutputPanel` state, reusing the existing
  `aiEligible`/`aiAvailability` plumbing from P13).
- **Reuse (do not reinvent):** `run_claude` + `RunOpts` + stub harness; `diff.rs` fns; `read_status`;
  `list_refs`; the `spawn_blocking(_inner)` + settings-gate command pattern; the P13 `aiEligible`
  gating + consent flow + `AiAvailability` fetch; toast/panel CSS.
```