# P53 — AI "why" layer (blame-why + explain-commit + branch naming)

Three read-only / low-risk AI features that establish the Phase-2 **grounding plumbing** the rest of
the phase reuses (see `docs/contracts/phase2-ai-native-overview.md` for the shared conventions this
contract obeys):

- **(a) blame "why did this change"** — from a blame gutter block, explain WHY the line/region exists,
  grounded in the commit that last touched it (line-focused, not whole-commit). NEW command.
- **(b) "Explain this commit" from a graph node** — reuse the existing `ai_analyze_diff` explain path;
  add the graph-node context-menu entry point and enrich commit grounding with the commit MESSAGE
  (intent), not just the diff. NO new command.
- **(c) AI branch naming** — propose valid kebab-case branch-name candidates from the working changes
  (or a commit range); the user picks/edits in the branch-create dialog. NEW command.

References read (verified): `crates/bonsai-core/src/git/blame.rs` (`blame_file`, `BlameLine`,
`MAX_BLAME_LINES`), `git/ai_explain.rs` (`analyze_diff`, `build_payload` Commit arm, `AiDiffTarget`,
`AiAnalysis`, `gather_worktree`/`gather_staged` — currently private, `has_analyzable_content`,
`cap_review_payload`), `git/ai_summary.rs` (range walk + `render_headers`/`render_commit_list` idiom),
`git/diff.rs` (`commit_file_diff`), `ai/mod.rs` (`run_claude`, `RunOpts`), `ai/payload.rs`,
`commands/ai.rs` (command triple + consent gate), `src/components/AiOutputPanel.tsx`, `CommitPanel.tsx`
(existing `onExplain` / `aiEligible`), `workspaceMenus.ts` (`commitMenuItems`, the shared oid-action
set, `runAnalyze` already in deps), `BlameView.tsx` (per-block gutter), `src/ipc/mock/handlers/ai.ts`.

**Tauri command count: 129 → 131** (`ai_explain_line` P53a, `ai_suggest_branch_name` P53c; P53b adds
none). Open questions in §10.

---

## 0. Key decisions (with rationale)

**D1 — blame-why is a SELF-CONTAINED new command, line-focused grounding (not "explain whole
commit").** `ai_explain_line(repoId, path, lineNo, atOid?)` blames just that line to find the
introducing commit, then grounds on THAT COMMIT'S CHANGE TO THAT FILE (`commit_file_diff`) + the
commit message + the specific line text — NOT the whole multi-file commit. Reusing
`ai_analyze_diff({kind:'commit'})` (OQ1 alt) would explain every file the commit touched (noise) and
would not anchor on the line. Self-contained also makes the command reusable from a future editor/diff
gutter, not just `BlameView`. Uses `blame.rs` (satisfies the roadmap "wire blame + explain").

**D2 — explain-commit (P53b) reuses `ai_analyze_diff` verbatim; the only backend change is grounding
enrichment.** The graph-node menu entry calls the EXISTING `runAnalyze({kind:'commit', oid}, 'explain',
…)`. To serve "WHY not WHAT", extend `ai_explain::build_payload`'s Commit arm to append the FULL commit
message (`MESSAGE:` section) after the existing `COMMIT/AUTHOR` prefix — the author's stated intent is
the strongest why-signal and it is currently dropped. This improves every existing commit
explain/review too (an improvement, not a regression). No new command, no IPC change.

**D3 — branch naming returns a RANKED CANDIDATE LIST the user picks/edits; WRITES NOTHING.**
`ai_suggest_branch_name(repoId, source)` → `BranchNameProposal { names, costUsd }` (best first). The
dialog shows candidates as chips that fill the name field; the actual branch is created by the
existing confirmed create path. The backend SANITIZES each candidate to a valid git ref component and
drops the invalid — the model may propose junk, so we never surface an uncreatable name.

**D4 — branch-name grounding sources: `Working` (default) + `CommitRange`.** `Working` = the
index-aware worktree change set (`gather_worktree`) — the common "I'm about to work" case. `CommitRange
{from,to}` = commit summaries + net diffstat for naming a branch that will carry existing commits.
Drop a distinct `Staged` variant for v1 (Working subsumes it). The branch-create dialog wires
`Working`; `CommitRange` is exposed for a future range entry point (OQ2/OQ6).

**D5 — all three reuse `AiAnalysis`/existing types where possible; one new result type only for the
list.** blame-why → `AiAnalysis` (prose). explain-commit → `AiAnalysis` (existing). branch-name → new
`BranchNameProposal` (a list, not prose). Minimal new wire surface.

**D6 — no new `AppError` variant.** Empty grounding (clean worktree / empty range / line out of range)
→ `AiFailed("no changes to …")` BEFORE any CLI call (mirrors `analyze_diff`/`summarize_range`). Bad
path → `InvalidName`; bad ref/oid → `Git`; consent off / CLI missing → `AiUnavailable` (via the gate).

---

## 1. Module boundaries / files

**New**
- `crates/bonsai-core/src/git/ai_line.rs` — `explain_line` + line-why system/prompt consts + grounding
  render + tests (blame-why). ~180 lines.
- `crates/bonsai-core/src/git/ai_branch_name.rs` — `BranchNameSource`, `BranchNameProposal`,
  `suggest_branch_name`, `sanitize_branch_name`, prompt consts + tests. ~200 lines.
- `src/components/BranchNameSuggest.tsx` — the "Suggest name" button + candidate-chip row for the
  branch-create dialog (own presentational file; loading/error inline).
- `src/ipc/mock/handlers/` — extend `ai.ts` (no new file): `aiExplainLine`, `aiSuggestBranchName`.

**Edited**
- `crates/bonsai-core/src/git/blame.rs` — add `blame_line(workdir, path, line_no, at_oid) ->
  Result<BlameLine, AppError>` (single-line blame via `BlameOptions::min_line/max_line`).
- `crates/bonsai-core/src/git/ai_explain.rs` — (D2) append `MESSAGE:` to the Commit-target grounding
  prefix; promote `gather_worktree` to `pub(crate)` so `ai_branch_name` reuses it (no duplication).
- `crates/bonsai-core/src/git/mod.rs` — `pub mod ai_line; pub mod ai_branch_name;`.
- `src-tauri/src/commands/ai.rs` — `ai_explain_line` + `_inner`; `ai_suggest_branch_name` + `_inner`
  (consent-gate triple, verbatim shape of `ai_analyze_diff`).
- `src-tauri/src/commands/shared.rs` — re-export the two new core types (`BranchNameSource`,
  `BranchNameProposal`) alongside the existing AI re-exports.
- `src-tauri/src/lib.rs` — register `ai_explain_line`, `ai_suggest_branch_name` in `generate_handler!`
  (after `ai_digest`; 129 → 131).
- `src/ipc/types.ts` — `BranchNameSource`, `BranchNameProposal` + `IpcApi.aiExplainLine` /
  `aiSuggestBranchName`.
- `src/ipc/tauri.ts` — the two invoke wrappers.
- `src/ipc/mock.ts` — (already spreads `aiHandlers`) no change beyond the two handlers in `ai.ts`.
- `src/components/BlameView.tsx` — add a per-block "Why?" affordance → `onExplainBlock(oid, lineNo)`;
  gate on a new `aiEligible` prop.
- `src/components/workspaceMenus.ts` — add "Explain this commit" to `commitMenuItems` (and the shared
  oid-action set) → `runAnalyze({kind:'commit', oid}, 'explain', …)`, gated on `aiEligible`.
- `src/components/WorkspaceDialogs.tsx` (branch-create dialog) — render `BranchNameSuggest`; on chip
  click, set the name field.
- `src/components/RepoWorkspace.tsx` — a `runExplainLine` handler (mirrors `runAnalyze`, uses the same
  `aiPanel` state + req-id); pass `aiEligible` to `BlameView`; wire `aiSuggestBranchName` into the
  branch-create dialog props; thread `aiEligible` to the menu builder.
- `styles.css` — `.blame-why`, `.branch-name-suggest`, candidate-chip classes.

---

## 2. Wire types

### 2.1 Rust

`ai_line.rs` reuses `AiAnalysis` (from `ai_explain`) — no new type.

`ai_branch_name.rs`:
```rust
/// Where to draw the branch-name grounding from. COMMAND INPUT (Deserialize);
/// TS mirror is a discriminated union (§2.2).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BranchNameSource {
    /// Index-aware working-tree change set (HEAD tree vs workdir, incl. untracked)
    /// — the common "about to start work" case. Clean tree => AiFailed (§0 D6).
    Working,
    /// Name a branch that will carry `from..to`. Both revparse-able. Empty range
    /// => AiFailed.
    CommitRange { from: String, to: String },
}

/// Ranked branch-name candidates (best first), each a VALID git ref component
/// (already sanitized; never empty). Serialize camelCase (mirrored in TS).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchNameProposal {
    pub names: Vec<String>,
    pub cost_usd: Option<f64>,
}

/// Hard cap on returned candidates (model asked for ~3; §0 D3 / OQ4).
pub const MAX_BRANCH_NAME_SUGGESTIONS: usize = 5;
```

`blame.rs` addition returns the existing `BlameLine` (single line).

### 2.2 TypeScript (`src/ipc/types.ts`)

```ts
/** Grounding source for aiSuggestBranchName — discriminated on `kind` (P53c). */
export type BranchNameSource =
  | { kind: 'working' }
  | { kind: 'commitRange'; from: string; to: string };

/** Ranked branch-name candidates (best first); each is a valid git branch name.
 *  Mirrors the Rust `BranchNameProposal`. Naming writes nothing. */
export interface BranchNameProposal {
  names: string[];
  costUsd: number | null;
}
```

`IpcApi` gains (near `aiSummarizeRange`):
```ts
/** AI "why does this line exist" — blames `lineNo` (as of `atOid`, null => HEAD)
 *  to find the introducing commit, then explains the change to that file focused
 *  on that line. Read-only; WRITES NOTHING; does NOT emit repo-changed. Rejects
 *  aiUnavailable | aiFailed (line out of range / no content) | git | invalidName |
 *  noRepo. */
aiExplainLine(repoId: string, path: string, lineNo: number, atOid: string | null): Promise<AiAnalysis>;

/** AI branch-name suggestions from `source`. Read-only; WRITES NOTHING. Returns
 *  1..5 sanitized candidates. Rejects aiUnavailable | aiFailed (empty grounding) |
 *  git (bad ref) | noRepo. */
aiSuggestBranchName(repoId: string, source: BranchNameSource): Promise<BranchNameProposal>;
```

`tauri.ts`:
```ts
aiExplainLine: (repoId, path, lineNo, atOid) =>
  invoke('ai_explain_line', { repoId, path, lineNo, atOid }),
aiSuggestBranchName: (repoId, source) =>
  invoke('ai_suggest_branch_name', { repoId, source }),
```

---

## 3. Backend cores

### 3.1 `blame.rs::blame_line` (single-line blame)

```rust
/// Blocking. Blames ONLY line `line_no` (1-based) of `path` as of `at_oid`
/// (None => HEAD), returning the introducing commit + that line's text. Uses
/// BlameOptions::min_line(line_no)/max_line(line_no) so large files are not
/// fully blamed. `line_no` out of range or path absent/binary => Git.
pub fn blame_line(workdir: &Path, path: &str, line_no: u32, at_oid: Option<&str>)
    -> Result<BlameLine, AppError>;
```
Reuses `validate_rel_path`, `open_workdir_repo`, `read_tree_blob`, `commit_meta` (already in file).
Resolve `newest` exactly as `blame_file`; set min/max line; take the single hunk covering `line_no`;
read `line_text` from the blob by `line_no`. No new cap logic.

### 3.2 `ai_line.rs::explain_line`

```rust
pub fn explain_line(workdir: &Path, path: &str, line_no: u32, at_oid: Option<&str>, opts: RunOpts)
    -> Result<AiAnalysis, AppError>;
```
Steps (no bodies):
1. `let bl = blame::blame_line(workdir, path, line_no, at_oid)?;` → introducing `oid`, `line_text`,
   author/summary.
2. Gather that commit's change TO THIS FILE: `commit_file_diff(workdir, &bl.oid, path, None, false)?`
   (rename origin not tracked in v1 — OQ7). If it has no analyzable content, still proceed (the line
   text + message are enough context) — do NOT hard-fail on an empty file diff here.
3. Read the introducing commit's full message via git2 (`find_commit(oid).message()` lossy).
4. Render grounding payload (§3.4), `cap_review_payload` it.
5. `run_claude(workdir, LINE_PROMPT, Some(&payload), RunOpts{ system_prompt: Some(LINE_SYSTEM_PROMPT), ..opts })`.
6. Return `AiAnalysis { text, cost_usd }`.

Consts (single-line; `prompts_are_single_line` test):
- `LINE_SYSTEM_PROMPT` ≈ "You are explaining WHY a specific line of code exists to a teammate. Standard
  input gives the line, the commit that introduced it (with its message), and that commit's change to
  the file. Explain the intent behind the line — what problem it solves and why it was written this way
  — grounded in the commit's stated purpose. Do not merely restate the diff. Two or three sentences.
  Output prose only — no markdown code fences."
- `LINE_PROMPT` = "Explain why the line described on standard input exists."

### 3.3 `ai_branch_name.rs::suggest_branch_name`

```rust
pub fn suggest_branch_name(workdir: &Path, source: &BranchNameSource, opts: RunOpts)
    -> Result<BranchNameProposal, AppError>;

/// Map a raw model line to a VALID git branch-name component, or None if it
/// can't be salvaged. Lowercase; spaces/invalid chars => '-'; collapse repeats;
/// trim leading/trailing '-' and '/'; reject empty, '.'-lock/ref-format
/// violations (git check-ref-format rules). Pure; unit-tested.
fn sanitize_branch_name(raw: &str) -> Option<String>;
```
Steps:
1. Build grounding by source:
   - `Working`: `let files = ai_explain::gather_worktree(workdir)?;` (now `pub(crate)`).
     `has_analyzable_content(&files)` false => `AiFailed("no changes to name a branch from")`.
     Payload = `render_file_diffs(&files)` prefixed `WORKING CHANGES:` (cap applies).
   - `CommitRange { from, to }`: revwalk `to` hiding merge-base(from,to) (mirror
     `ai_summary::summarize_range` walk); empty => `AiFailed`. Payload = `render_commit_list(commits
     capped)` + `render_headers(mb..to)` prefixed `COMMITS TO NAME A BRANCH FOR:` / `NET CHANGES:`.
2. `run_claude(workdir, BRANCH_NAME_PROMPT, Some(&payload), RunOpts{ system_prompt:
   Some(BRANCH_NAME_SYSTEM_PROMPT), ..opts })` — keep `opts.model` = default sonnet (OQ3).
3. Parse the result: split lines → `sanitize_branch_name` each → dedup (stable) → drop None →
   truncate to `MAX_BRANCH_NAME_SUGGESTIONS`. Empty after sanitizing => `AiFailed("no usable branch
   name suggested")`. Return `BranchNameProposal { names, cost_usd }`.

Consts (single-line):
- `BRANCH_NAME_SYSTEM_PROMPT` ≈ "You are naming a git branch from a description of code changes on
  standard input. Propose three short, descriptive branch names in kebab-case, most fitting first,
  reflecting the INTENT of the change. Use an optional single type prefix (feat/, fix/, chore/, refactor/)
  then a hyphenated slug. Names must be valid git refs: lowercase, hyphen-separated, at most one '/',
  no spaces or special characters. Output ONLY the names, one per line — no numbering, no explanation,
  no code fences."
- `BRANCH_NAME_PROMPT` = "Suggest branch names for the changes described on standard input."

### 3.4 blame-why grounding payload (normative template)

```
LINE <line_no> of <path>:
    <line_text>

INTRODUCED BY COMMIT <short7>  <summary>
AUTHOR <author_name>  <YYYY-MM-DD>
MESSAGE:
<full commit message>

CHANGE TO <path> IN THAT COMMIT:
===== FILE: <path> (<status>) =====
<render_file_diffs body for that one file>
```
(`YYYY-MM-DD` via the existing `epoch_to_ymd` idiom in `ai_explain.rs` — extract to a shared helper if
reused, else duplicate the tiny fn; flag.)

### 3.5 `ai_explain.rs` Commit-grounding enrichment (D2)

In `build_payload`'s `AiDiffTarget::Commit` arm, after the existing `COMMIT …\nAUTHOR …` prefix, append
`\nMESSAGE:\n<full message>\n` (from `commit_diff` details or a `find_commit` message read; lossy).
Keep it before the per-file blocks. This is the ONLY change to existing behavior; add a test that the
prefix now contains the message body.

---

## 4. Commands — `src-tauri/src/commands/ai.rs`

Both follow the `ai_analyze_diff` triple + consent-gate shape EXACTLY (consent enforced in `_inner`
before `repo_path`; read-only ⇒ no `repo-changed` emit).

```rust
#[tauri::command]
pub async fn ai_explain_line(
    app: tauri::AppHandle, state: tauri::State<'_, AppState>,
    repo_id: String, path: String, line_no: u32, at_oid: Option<String>,
) -> Result<AiAnalysis, AppError> {
    let file = settings::settings_file(&app)?;
    ai_explain_line_inner(state.inner(), &file, &repo_id, path, line_no, at_oid).await
}
// _inner: consent gate → repo_path → spawn_blocking(ai_line::explain_line(&workdir, &path, line_no, at_oid.as_deref(), RunOpts::default()))

#[tauri::command]
pub async fn ai_suggest_branch_name(
    app: tauri::AppHandle, state: tauri::State<'_, AppState>,
    repo_id: String, source: BranchNameSource,
) -> Result<BranchNameProposal, AppError> {
    let file = settings::settings_file(&app)?;
    ai_suggest_branch_name_inner(state.inner(), &file, &repo_id, source).await
}
// _inner: consent gate → repo_path → spawn_blocking(ai_branch_name::suggest_branch_name(&workdir, &source, RunOpts::default()))
```
Register after `ai_digest` in `lib.rs` (131). Re-export `BranchNameSource`/`BranchNameProposal` in
`commands/shared.rs`.

---

## 5. Frontend entry points

### 5.1 blame-why (P53a) — `BlameView.tsx` + RepoWorkspace
- `BlameViewProps` gains `aiEligible: boolean` + `onExplainBlock(oid: string, lineNo: number): void`.
  Each gutter block renders a small "Why?" button (only when `aiEligible`) → `onExplainBlock(block.oid,
  block.lines[0].finalLineNo)`.
- RepoWorkspace `runExplainLine(path, lineNo, atOid, title)` mirrors `runAnalyze`: sets `aiPanel`
  loading, calls `ipc.aiExplainLine`, same `aiPanelReqId` last-wins guard, title e.g.
  `Why line ${lineNo} of ${path}`. `atOid` = the BlameView's current `at_oid` (HEAD => null).
- Result renders in the SAME `AiOutputPanel` (Esc-layering already includes `aiPanelOpenRef`). Blame
  overlay stays open beneath.

### 5.2 explain-commit (P53b) — `workspaceMenus.ts`
- Add to the shared oid-action set (used by commit rows AND branch/tag pills): an "Explain this commit"
  item, `disabled: !aiEligible`, `onSelect: () => runAnalyze({ kind:'commit', oid }, 'explain',
  \`Explain commit ${oid.slice(0,7)}\`)`. `runAnalyze` + a new `aiEligible` are threaded through the
  existing menu-builder deps (both already partly present — `runAnalyze` is used by "Review branch…").
- Ordering: after "Compare with HEAD", before Cherry-pick/Revert (read-only group). No new command,
  no mock change (existing `aiAnalyzeDiff` mock covers `{kind:'commit'}`).

### 5.3 branch naming (P53c) — `BranchNameSuggest.tsx` + branch-create dialog
- `BranchNameSuggest` props: `{ aiEligible; workingDirty; onPick(name): void; suggest(): Promise<BranchNameProposal> }`.
  Renders a "Suggest name ✨" button (disabled unless `aiEligible && workingDirty` — clean tree has no
  grounding, OQ6); on click, calls `suggest()`, shows a tiny inline spinner, then a row of candidate
  chips; clicking a chip calls `onPick(name)` to fill the dialog's name input. Inline error text on
  reject. Own req-id guard (last-wins) inside the component.
- Branch-create dialog (in `WorkspaceDialogs.tsx`, driven by `pendingCreateBranch`) renders
  `BranchNameSuggest` under the name field; `suggest = () => ipc.aiSuggestBranchName(repoId, { kind:
  'working' })`; `onPick` sets the controlled name state. The user still edits + confirms via the
  existing create path (WRITES NOTHING until confirm).
- `aiEligible` + `workingDirty` (status has any staged/unstaged/untracked) are threaded from
  RepoWorkspace.

---

## 6. Mock (`src/ipc/mock/handlers/ai.ts` — extend `aiHandlers`)

Both honor `AI_OFF` (`?ai=off`) → throw `{ kind:'aiFailed', message:'Claude Code CLI not found on
PATH' }`; else canned, shape-correct output; `requireRepo(repoId)`.
- `aiExplainLine(repoId, path, lineNo, atOid)`: `await delay(500)`; return
  `{ text: \`Why line ${lineNo} of ${path}: this line was introduced to … (mock).\`, costUsd: 0.005 }`.
- `aiSuggestBranchName(repoId, source)`: `await delay(400)`; return `{ names: source.kind==='working' ?
  ['feat/ai-why-layer','ai-why-layer','feature/blame-why'] : ['feat/range-work','range-work',
  'topic/selected-commits'], costUsd: 0.003 }`. (Candidates already look like valid refs — no
  sanitizing needed in the mock.)
No change to `mock.ts` (it already spreads `aiHandlers`).

---

## 7. Test plan (`#[cfg(test)]`)

Reuse the AI test idioms: `init_scratch()` (identity + autocrlf off), `create_commit`/`stage_paths`,
`prompts_are_single_line`, `*_wire_shape_is_camel_case`, discriminated-union deserialize locks. Windows
test-runner sets `TMP`/`TEMP=D:\Temp` (MEMORY rule). AI calls use the `claude_stub` via `CLAUDE_BIN_ENV`
(env-locked) — assert grounding/parsing, not model output.

**blame.rs / ai_line.rs**
1. `blame_line_targets_single_line`: 3-commit fixture editing distinct lines → `blame_line(path, k)`
   returns the oid that last touched line k + the right `line_text`; out-of-range line → `Git`.
2. `explain_line_grounding_shape` (stub `success`): payload (captured via a render-only unit or the
   stub echo) contains `LINE <n> of <path>:`, the introducing `COMMIT`/`MESSAGE:`, and the file block;
   returns `AiAnalysis`.
3. `explain_line_bad_path_is_invalid_name`; clean/empty edge handled (line text-only still proceeds).

**ai_branch_name.rs**
4. `sanitize_branch_name_rules` (pure): "Add AI Why Layer" → `add-ai-why-layer`; "feat: X" → `feat-x`
   or `feat/x` (document which); leading/trailing junk trimmed; empty/`..`/control → None.
5. `branch_name_source_deserializes_each_variant` (exact TS JSON: `{"kind":"working"}`,
   `{"kind":"commitRange","from":"main","to":"feature"}`).
6. `branch_name_proposal_wire_shape_is_camel_case` (`names`/`costUsd`, `None`→`null`).
7. `suggest_branch_name_working_empty_fails_before_cli`: clean worktree → `AiFailed`, no CLI (fake bin
   that panics if spawned).
8. `suggest_branch_name_parses_and_caps` (stub returning >5 lines incl. an invalid one): result is
   sanitized, deduped, capped at `MAX_BRANCH_NAME_SUGGESTIONS`, no invalid entry.
9. `prompts_are_single_line` for both files' consts.

**ai_explain.rs (D2)**
10. Commit-target grounding now contains the full `MESSAGE:` body (extend an existing build_payload
    test or add one).

---

## 8. Sub-increment split + acceptance

### P53a — blame-why (backend + IPC + mock + BlameView entry)
Scope: `blame.rs::blame_line`; `ai_line.rs` (+ tests §7.1–§7.3, §7.9); `mod.rs`; `ai.rs`
`ai_explain_line` + `_inner`; `lib.rs` (130); `types.ts` + `tauri.ts` (`aiExplainLine`); `ai.ts`
mock; `BlameView.tsx` + `RepoWorkspace` `runExplainLine` + `aiEligible` thread.
**Acceptance:** (1) `cargo test -p bonsai-core ai_line blame_line` green incl. single-line blame +
grounding-shape; `cargo build` + `clippy -D warnings` clean; `generate_handler!` lists 130. (2) `tsc`
/`pnpm build` clean; no file over ~500 lines. (3) Harness (`VITE_MOCK_IPC=1`): open blame on a file →
each gutter block shows "Why?"; clicking opens `AiOutputPanel` with canned why-prose + cost; `?ai=off`
→ error banner; Esc closes the panel before the blame overlay. (4) Console:
`await ipc.aiExplainLine('r','src/a.ts',3,null)` resolves `{text,costUsd}`.

### P53b — explain-commit from graph node (UI wiring + grounding enrichment)
Scope: `ai_explain.rs` Commit `MESSAGE:` enrichment (+ test §7.10); `workspaceMenus.ts` "Explain this
commit" in the shared oid-action set; `aiEligible`/`runAnalyze` threaded to the menu builder;
RepoWorkspace passes them.
**Acceptance:** (1) `cargo test -p bonsai-core ai_explain` green (message now in Commit grounding);
build/clippy clean; command count UNCHANGED (130). (2) `pnpm build` clean. (3) Harness: right-click a
commit dot/row → "Explain this commit" runs `aiAnalyzeDiff` and opens `AiOutputPanel` (title
`Explain commit <short7>`); the entry is disabled when `?ai=off`/consent off. (4) No destructive item
added; ordering is in the read-only group.

### P53c — branch naming (backend + IPC + mock + dialog entry)
Scope: `ai_branch_name.rs` (+ tests §7.4–§7.9); `ai_explain.rs` `gather_worktree` → `pub(crate)`;
`mod.rs`; `ai.rs` `ai_suggest_branch_name` + `_inner`; `lib.rs` (131); `shared.rs` re-exports;
`types.ts` + `tauri.ts`; `ai.ts` mock; `BranchNameSuggest.tsx`; branch-create dialog wiring;
`RepoWorkspace` thread `aiEligible`/`workingDirty`.
**Acceptance:** (1) `cargo test -p bonsai-core ai_branch_name` green incl. sanitize rules,
deserialize/serialize locks, empty-grounding-fails-before-CLI, parse+cap; build/clippy clean;
`generate_handler!` lists 131. (2) `tsc`/`pnpm build` clean; no file over ~500 lines. (3) Harness:
open "New branch" with a dirty worktree → "Suggest name ✨" enabled → click → candidate chips appear →
clicking a chip fills the name field; clean worktree → button disabled; `?ai=off` → inline error.
(4) Console: `await ipc.aiSuggestBranchName('r',{kind:'working'})` resolves `{names:[…],costUsd}`.

(P53a/b/c are order-independent; b is the smallest and could fold into a if time-boxed — recommend
keeping separate for small review diffs.)

---

## 9. Acceptance criteria (milestone)

- **AI gate:** P53a+b+c acceptance above; `cargo test` (whole crate) green; browser harness proves all
  three entry points against `aiHandlers` (with `?ai=off` error paths); consent gate enforced in each
  `_inner` (unit-cover via the existing `ai_*_inner` gate test pattern if present, else a note);
  command count 129 → 131; the "WHY not WHAT" grounding is present (LINE/MESSAGE sections in payloads).
- **USER CHECKPOINT:** `docs/contracts/P53-user-checklist.md` — with a real `claude` CLI on a real repo:
  blame "Why?" returns a sensible intent-focused explanation (not a diff restatement); graph-node
  "Explain this commit" reflects the commit message; "Suggest name" proposes usable, valid branch
  names that the create flow accepts; disabled/consent-off states behave; no code leaves the device
  (local CLI only).

---

## 10. Open questions (flag to orchestrator)

- **OQ1 — blame-why: self-contained vs frontend-oid.** Recommend self-contained `ai_explain_line`
  (re-blames the single line; reusable beyond BlameView; line-focused grounding). Alt: reuse
  `ai_analyze_diff({kind:'commit'})` with the oid BlameView already has (zero new command, but explains
  the whole multi-file commit — weaker "why"). Confirm the new command.
- **OQ2 — branch-name sources.** Recommend `Working` + `CommitRange`; drop distinct `Staged`. Confirm,
  or ask to add `Staged`.
- **OQ3 — branch-name model.** Recommend DEFAULT (sonnet) — payload is tiny so latency is low and
  quality matters for a name. Alt: `haiku` (cheaper, weaker). Confirm.
- **OQ4 — candidate count.** Recommend ask for 3, cap at `MAX_BRANCH_NAME_SUGGESTIONS = 5`. Confirm.
- **OQ5 — Commit-grounding message enrichment (D2) changes existing explain/review.** Recommend YES
  (serves why-not-what; strictly more context). It alters current commit-explain output — confirm it
  is an accepted improvement, not a regression to guard.
- **OQ6 — clean-tree branch naming.** Recommend disabling "Suggest" when the worktree is clean (no
  grounding to name from) rather than inventing a name from the base commit. Confirm, or wire a
  `CommitRange`/`Commit` fallback UI entry.
- **OQ7 — blame-why rename handling.** v1 passes `orig_path=None` to `commit_file_diff` (no rename
  follow for the introducing commit). Recommend defer rename-follow (blame.rs already degrades). Confirm.
- **OQ8 — shared `epoch_to_ymd`.** It lives in `ai_explain.rs`; blame-why needs a date. Recommend
  promoting it to a tiny shared util (e.g. `git/timefmt.rs`) rather than duplicating. Confirm or accept
  a small duplication.
