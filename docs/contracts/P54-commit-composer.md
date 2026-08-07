# P54 — Commit composer (WIP → N logical commits)

Propose grouping the working tree into N logical commits (each = a set of changed files + a
generated message), let the user review/edit, then apply as an ordered stage+commit sequence.
Differentiated (GitKraken charges for it); great for cleaning up messy AI-agent history. Obeys the
Phase-2 shared conventions in `docs/contracts/phase2-ai-native-overview.md` and mirrors the sibling
`P53-ai-why-layer.md`.

References read (verified, not guessed): `crates/bonsai-core/src/git/ai_commit.rs`
(`generate_commit_message`, single-line prompt consts, `CommitMessageProposal`,
`proposal_wire_shape_is_camel_case`), `git/ai_explain.rs` (`gather_worktree` =
`diff_tree_to_workdir_with_index(head, include_untracked)` → `collect_file_diffs`; `cap_review_payload`
+ `MAX_REVIEW_PAYLOAD_BYTES`; consent-gate test idiom), `ai/payload.rs` (`render_file_diffs`,
`RenderedPayload`, `MAX_PAYLOAD_LINES/FILES`), `ai/mod.rs` (`run_claude`, `RunOpts` incl. reserved-unused
`json_schema`, `AiResult`, `DEFAULT_TIMEOUT`), `git/stage.rs` (`stage_paths`, `unstage_paths`,
`validate_rel_path`), `git/stage_partial.rs` (blob-reconstruction: `reconstruct`, `assemble`,
`split_keep_terminator`, `index_blob_bytes` — all `pub(crate)`; `LineSelection`), `git/commit.rs`
(`create_commit`, `resolve_signature`, guards: `require_no_bisect`, `state()==Clean`, `has_conflicts`,
`NothingToCommit`, `EmptyMessage`), `git/status.rs` (`read_status`, `StatusSnapshot`, `StatusEntry`,
`FileStatus`), `git/diff.rs` (`FileDiff`, `Hunk`, `DiffLine`, `LineKind`, `collect_file_diffs`),
`src-tauri/src/commands/ai.rs` (triple + consent gate), `commands/staging.rs` (`commit`/`stage_partial`
house shape; "no `repo-changed` emit; frontend refetches"), `src/components/AiOutputPanel.tsx`,
`src/ipc/mock/handlers/{ai,status}.ts` (`AI_OFF`, `requireRepo`, mock commit mutation).

**Tauri command count: +2** (`ai_compose_commits`, `apply_composed_commits`). Continues from the
post-P53 total (131 if P53 landed → 133); **verify against `lib.rs` at implementation**. Open questions
in §10.

---

## 0. Key decisions (with rationale)

**D1 — Two commands, split by concern. `ai_compose_commits` = PROPOSE (AI, consent-gated, WRITES
NOTHING). `apply_composed_commits` = APPLY (pure git, NOT AI-gated).** Propose returns a structured
proposal; apply performs the mutation on the user's finalized, edited plan. Keeping apply un-gated by AI
means the reviewed plan still applies if AI is later toggled off, and apply is unit-testable without a
CLI (mirrors `commit`, not `generate_commit_message`). Both are request/response **commands** — the
proposal is bounded structured data and the apply output is ≤ `MAX_COMPOSE_GROUPS` rows created in
sub-second; **no channel, no event** (§C3). Neither emits `repo-changed`; the composer hook refetches
graph+status on success (house pattern for `commit`).

**D2 — v1 granularity = FILE-LEVEL (each changed file belongs to exactly one group).** Both the AI
proposal and apply operate on whole files. Rationale: (a) it sidesteps the line-number-shift problem
entirely (a file split across commits renumbers after each staging); (b) apply reuses `stage_paths`,
which handles every file kind (add/modify/delete/rename/typechange/binary/untracked) — whereas
`stage_partial` REFUSES renames/binaries/too-large; (c) models are unreliable at exact line ranges but
reliable at file grouping; (d) it satisfies the dominant "clean up my WIP" use case. Hunk/line-level
splitting is a fully-specified follow-up (OQ2) that leans on `stage_partial`'s `reconstruct` primitive.

**D3 — Rust is the referee: the returned proposal is ALWAYS an apply-able partition, regardless of
model output.** The model returns JSON; the backend parses robustly and NORMALIZES against the ACTUAL
change set (§3.3): unknown/hallucinated paths dropped, overlaps resolved first-wins, uncovered files
collected into `unassigned`, groups capped at `MAX_COMPOSE_GROUPS`. The frontend never trusts raw model
output. Unparseable JSON is **not** a hard error — it degrades to `groups:[]` + all files in
`unassigned` (feature stays usable via manual grouping); only a CLI hard-failure (timeout/nonzero/empty)
propagates as `AiFailed`.

**D4 — Apply is ATOMIC (all-or-nothing) with ref+index rollback; the working tree is NEVER touched.**
Validate the whole plan first (identity, non-empty messages, every path present + assigned once), record
the original HEAD as a rollback anchor, reset the index to HEAD (so uncovered changes can't leak into any
commit), then commit each group in order. Any mid-sequence failure restores HEAD + the index to the
anchor so NOTHING is committed. No workdir mutation ever (no hard reset) ⇒ no data-loss risk. Files in no
group are simply left uncommitted (unstaged) in the working tree.

**D5 — "Compose commits" takes over the index (documented).** Apply resets the index to HEAD as step 1;
on cancel/failure the index ends at HEAD (unstaged). Working-tree content is preserved. The review UI
states this. (OQ1: auto-reset vs refuse-when-dirty — recommend auto-reset.)

**D6 — Human-in-the-loop, verbatim §C2.** Propose WRITES NOTHING. The proposal lands in a NEW review UI
(its own component, NOT `AiOutputPanel` — that panel is read-only prose). The user reassigns files, edits
messages, drops/merges/adds groups. `apply_composed_commits` fires ONLY on an explicit final "Create N
commits" confirm.

**D7 — No new `AppError` variant.** Propose: `AiUnavailable` (gate) | `AiFailed` (CLI fail/empty) |
`NothingToCommit` (clean tree, before any CLI call — mirrors `gather_staged`) | `Git` | `NoRepo`. Apply:
`NoRepo` | `OperationInProgress` | `Git` (conflicts) | `EmptyMessage` | `ConfigMissing` |
`NothingToCommit` (empty plan / a no-op group) | `Other` (unknown path, path in >1 group, no-op group,
drift/stale). All reuse existing kinds.

---

## 1. Module boundaries / files

**New (Rust)**
- `crates/bonsai-core/src/git/ai_compose.rs` — PROPOSE side (~260 lines): `ComposeGroup`,
  `ComposeProposal`, `MAX_COMPOSE_GROUPS`, `compose_commits`, grounding render, `parse_compose_response`
  (pure) + response-normalizer, prompt consts + tests.
- `crates/bonsai-core/src/git/compose_apply.rs` — APPLY side (~240 lines): `ComposePlan`,
  `ComposeApplyResult`, `ComposeCommit`, `apply_composed_commits`, validation / reset / commit-loop /
  rollback helpers + tests. Kept separate from `ai_compose.rs` for single-responsibility and file size.

**New (Frontend)**
- `src/components/ComposerDialog.tsx` — the review overlay container (its own file): header, the group
  list, the unassigned bucket, the "+ New group" + final "Create N commits" confirm; loading/error inline.
- `src/components/ComposerGroupCard.tsx` — one presentational group card: editable message + assigned
  file rows (path + status badge + "move to…" select + "preview" + remove) + drop/merge controls.
- `src/components/repoWorkspace/useCommitComposer.ts` — state hook (open/close, propose call with the
  `reqId` last-wins guard, the editable plan reducers, apply call, refetch-on-success). Keeps
  `RepoWorkspace` lean (mirrors `useCommitSearch`/`useReadOverlays`).
- `src/ipc/mock/handlers/compose.ts` — mock `applyComposedCommits` (mutation).

**Edited (Rust)**
- `crates/bonsai-core/src/git/mod.rs` — `pub mod ai_compose; pub mod compose_apply;`.
- `crates/bonsai-core/src/git/ai_explain.rs` — ensure `gather_worktree` is `pub(crate)` (P53 already
  promotes it; if P53 not merged, promote here) and `cap_review_payload` is reachable (`pub(crate)`, or
  duplicate the tiny idiom — flag OQ4).
- `src-tauri/src/commands/ai.rs` — `ai_compose_commits` + `_inner` (consent-gate triple).
- `src-tauri/src/commands/staging.rs` (or a new `commands/compose.rs`) — `apply_composed_commits` +
  `_inner` (NO consent gate; house shape of `commit`). **Recommend a new `commands/compose.rs`** to keep
  `staging.rs` focused; register `mod compose; pub use compose::*;` in `commands/mod.rs`.
- `src-tauri/src/commands/shared.rs` — re-export `ComposeGroup`, `ComposeProposal`, `ComposePlan`,
  `ComposeApplyResult`, `ComposeCommit`.
- `src-tauri/src/lib.rs` — register `ai_compose_commits` (after `ai_digest`) and `apply_composed_commits`
  in `generate_handler!` (+2).

**Edited (Frontend)**
- `src/ipc/types.ts` — the five wire types + `IpcApi.aiComposeCommits` / `applyComposedCommits`.
- `src/ipc/tauri.ts` — the two invoke wrappers.
- `src/ipc/mock/handlers/ai.ts` — add `aiComposeCommits` (honors `AI_OFF`).
- `src/ipc/mock.ts` — import + spread `composeHandlers` (the new `compose.ts`).
- `src/components/CommitPanel.tsx` (working-changes panel) — a "Compose commits ✨" button, gated on
  `aiEligible && workingDirty` → opens the composer.
- `src/components/RepoWorkspace.tsx` — wire `useCommitComposer`; render `ComposerDialog`; feed its open
  flag into the keyboard gates; pass `aiEligible`/`workingDirty`.
- `src/components/repoWorkspace/useWorkspaceKeyboard.ts` — add `composerOpenRef` to Esc-layering (near
  the top, above `diffSlot`); add `composerOpen` to the shortcut gate (nav keys inert while open).
- `styles.css` — `.composer-*` classes (dialog, group card, file row, unassigned bucket).

---

## 2. Wire types

### 2.1 Rust

`ai_compose.rs`:
```rust
/// Cap on proposed / applied groups (overflow folds into `unassigned`). Bounds
/// output size and keeps the review UI sane.
pub const MAX_COMPOSE_GROUPS: usize = 10;

/// One proposed logical commit: a set of changed files + a message. v1 =
/// FILE-LEVEL (each changed file appears in exactly ONE group across the plan;
/// enforced by the normalizer/apply-validator). Both Serialize (proposal out)
/// and Deserialize (edited plan in) so the review UI round-trips one shape.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeGroup {
    /// Repo-relative paths (NEW path for renames), forward slashes.
    pub files: Vec<String>,
    /// Commit message (summary + optional body); trimmed.
    pub message: String,
}

/// The NORMALIZED composer proposal — ALWAYS an apply-able partition of the
/// real change set (§3.3), whatever the model returned. Serialize only.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeProposal {
    pub groups: Vec<ComposeGroup>,
    /// Changed files the model did NOT place (or overflow past the group cap).
    /// Surfaced so nothing is silently dropped — the UI shows a distinct bucket.
    /// Empty on full coverage.
    pub unassigned: Vec<String>,
    /// Human notes about what the normalizer changed (dropped unknown path,
    /// resolved an overlap first-wins, capped groups, unparseable output). For
    /// the UI info line; never an error.
    pub notes: Vec<String>,
    pub cost_usd: Option<f64>,
}
```

`compose_apply.rs`:
```rust
/// User-finalized plan to apply: an ORDERED list of groups (first = oldest
/// commit). A changed file absent from every group is intentionally left
/// uncommitted in the working tree. COMMAND INPUT (Deserialize).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposePlan {
    pub groups: Vec<ComposeGroup>,
}

/// Result of applying a plan: created commits, oldest→newest. Serialize only.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeApplyResult {
    pub commits: Vec<ComposeCommit>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeCommit {
    pub oid: String,     // full 40-hex
    pub summary: String, // first message line
}
```

### 2.2 TypeScript (`src/ipc/types.ts`)

```ts
/** One proposed logical commit (P54). v1 is file-level: each changed file is in
 *  exactly one group across the plan. Round-trips as both proposal and plan. */
export interface ComposeGroup { files: string[]; message: string; }

/** Normalized composer proposal — always an apply-able partition of the change
 *  set (backend-enforced). Mirrors Rust `ComposeProposal`. */
export interface ComposeProposal {
  groups: ComposeGroup[];
  unassigned: string[];   // changed files the AI did not place
  notes: string[];        // normalizer notes (informational)
  costUsd: number | null;
}

/** User-finalized plan to apply (ordered; first group = oldest commit). */
export interface ComposePlan { groups: ComposeGroup[]; }

export interface ComposeCommit { oid: string; summary: string; }
export interface ComposeApplyResult { commits: ComposeCommit[]; }
```

`IpcApi` gains (near `generateCommitMessage`):
```ts
/** AI: propose grouping the working-tree changes (HEAD vs working tree, incl.
 *  untracked) into logical commits. Read-only; WRITES NOTHING. `guidance` = an
 *  optional free-text hint (e.g. "keep tests separate"). The result is ALWAYS an
 *  apply-able partition (unknown paths dropped, overlaps first-wins, uncovered
 *  files in `unassigned`). Unparseable model output is NOT an error — it resolves
 *  with groups:[] + all files unassigned. Rejects aiUnavailable | aiFailed (CLI
 *  fail/empty) | nothingToCommit (clean tree) | git | noRepo. */
aiComposeCommits(repoId: string, guidance: string | null): Promise<ComposeProposal>;

/** Apply a reviewed plan as an ORDERED stage+commit sequence. ATOMIC: validates
 *  fully, resets the index to HEAD (working tree UNTOUCHED), commits each group;
 *  ANY mid-sequence failure rolls HEAD+index back so NOTHING is committed. Files
 *  in no group are left uncommitted. Called ONLY on the user's explicit final
 *  confirm. Does NOT emit repo-changed (caller refetches). Not AI-gated. Rejects
 *  noRepo | operationInProgress | git | emptyMessage | configMissing |
 *  nothingToCommit | other (unknown/duplicate path, no-op group, drift). */
applyComposedCommits(repoId: string, plan: ComposePlan): Promise<ComposeApplyResult>;
```

`tauri.ts`:
```ts
aiComposeCommits: (repoId, guidance) => invoke('ai_compose_commits', { repoId, guidance }),
applyComposedCommits: (repoId, plan) => invoke('apply_composed_commits', { repoId, plan }),
```

---

## 3. Backend — PROPOSE (`ai_compose.rs`)

### 3.1 `compose_commits`

```rust
/// Blocking. Gathers the HEAD→working-tree change set, asks the CLI to group it
/// into logical commits, and returns a NORMALIZED, apply-able proposal.
/// - Clean tree (no changes) => NothingToCommit BEFORE any CLI call.
/// - CLI hard-failure (timeout/nonzero/empty) => AiFailed (propagated).
/// - CLI returned text => parse+normalize (never errors on bad grouping).
pub fn compose_commits(workdir: &Path, guidance: Option<&str>, opts: RunOpts)
    -> Result<ComposeProposal, AppError>;
```
Steps (no bodies):
1. `let files = ai_explain::gather_worktree(workdir)?;` (HEAD→workdir incl. untracked). Empty ⇒
   `Err(AppError::NothingToCommit)`.
2. `let changed: Vec<String> = files.iter().map(|f| f.path.clone()).collect();` — the authoritative path
   list the normalizer validates against.
3. Render grounding (§3.2), `cap_review_payload` the whole string.
4. `let result = ai::run_claude(workdir, COMPOSE_PROMPT, Some(&payload), RunOpts{ system_prompt:
   Some(COMPOSE_SYSTEM_PROMPT), ..opts })?;` (`?` propagates CLI hard-failure as `AiFailed`).
5. `let parsed = parse_compose_response(&result.text, &changed);` (pure; never errors).
6. Return `ComposeProposal { groups: parsed.groups, unassigned: parsed.unassigned, notes: parsed.notes,
   cost_usd: result.cost_usd }`.

Consts (single-line; `prompts_are_single_line` test — the JSON braces/quotes are fine, no newline):
- `COMPOSE_SYSTEM_PROMPT` ≈ "You are organizing a messy working tree into a small number of clean,
  logical git commits. Standard input lists the changed files (use these EXACT paths) and their diffs
  (HEAD vs working tree). Group the files into 1 to 10 logical commits so each commit is one coherent,
  self-contained change (a feature, a fix, a refactor, tests, docs, or formatting). Prefer a few
  well-scoped commits over many tiny ones. For each group write a Conventional Commits message: a short
  imperative summary of at most 72 characters, then, only if warranted, a blank line and brief bullet
  points explaining WHY the change was made. Assign every changed file to exactly one group; never invent
  a path that is not in the list; never place a file in two groups. Output ONLY a JSON object of the form
  {\"groups\":[{\"message\":\"...\",\"files\":[\"path\",...]}]} — no prose, no explanation, no markdown,
  no code fences."
- `COMPOSE_PROMPT` = "Group the changed files described on standard input into logical commits and return
  the JSON object." (When `guidance` is `Some`, append " Extra guidance: <guidance>" — kept single-line;
  guidance is user free-text, never a path/arg.)

### 3.2 Grounding payload (normative template — WHY-not-WHAT, §C1)

The explicit path list constrains the model to real paths (cuts hallucination); the hunk-level diffs give
intent context. Reuse `payload::render_file_diffs` (enforces `MAX_PAYLOAD_LINES/FILES` + a truncation
note).
```
WORKING CHANGES (HEAD vs working tree):

CHANGED FILES (assign each to exactly one group; use these exact paths):
<path1>
<path2>
...

DIFFS:
===== FILE: <path> (<status>[, was <orig>]) =====
<render_file_diffs body>
...
```

### 3.3 `parse_compose_response` — the referee (pure, unit-tested; normative)

```
parse_compose_response(raw: &str, changed: &[String]) -> ParsedCompose:
    # ParsedCompose { groups: Vec<ComposeGroup>, unassigned: Vec<String>, notes: Vec<String> }
    changed_set := set(changed)

    # 1. Extract JSON: strip ```json / ``` fences; trim; take the substring from the
    #    first '{' to the last '}' (fallback: first '[' .. last ']' for a bare array).
    json := extract_json(raw)

    # 2. Deserialize leniently into RawCompose { groups: Vec<RawGroup{ message, files }> }
    #    (also accept a bare top-level array of RawGroup).
    parsed := serde_json::from_str::<RawCompose>(json)
    if parsed is Err:
        # NOT a hard error (D3): degrade to manual grouping.
        return ParsedCompose { groups: [], unassigned: changed.to_vec(),
                               notes: ["AI output could not be parsed into groups; group the files manually."] }

    # 3. Normalize into a PARTITION of `changed`.
    assigned := {}; out := []; notes := []
    for rg in parsed.groups:
        if out.len() == MAX_COMPOSE_GROUPS:
            notes.push("reached the group limit; remaining files left unassigned"); break
        files := []
        for p in rg.files:
            q := normalize_path(p)                       # trim, backslashes->slashes
            if q not in changed_set: notes.push("dropped unknown path "+q); continue
            if q in assigned:        notes.push("path "+q+" already assigned; kept in the earlier group"); continue
            assigned.insert(q); files.push(q)
        if files.is_empty():
            notes.push("dropped an empty group"); continue
        out.push(ComposeGroup { files, message: rg.message.trim() })   # empty message allowed here; UI+apply validate

    # 4. Overflow groups' files + never-mentioned files => unassigned (input order).
    for rg beyond MAX_COMPOSE_GROUPS: fold its still-unassigned files into `assigned`-aware unassigned
    unassigned := changed.filter(|c| c not in assigned)               # preserves `changed` order

    return ParsedCompose { groups: out, unassigned, notes }
```
- **Coverage** guaranteed: `groups ∪ unassigned == changed`, disjoint. **Overlap** → first-wins.
  **Unknown path** → dropped + noted. **Empty group** → dropped. **Unparseable** → all-unassigned + note.
  **> cap** → tail folded into unassigned + note. The proposal is therefore ALWAYS apply-able as-is.

---

## 4. Backend — APPLY (`compose_apply.rs`)

### 4.1 `apply_composed_commits` (normative pseudocode)

```
apply_composed_commits(workdir, plan) -> Result<ComposeApplyResult, AppError>:
    repo := open_workdir_repo(workdir)
    require_no_bisect(&repo)?                                  # reuse commit.rs guard
    if repo.state() != Clean: return OperationInProgress(...)
    if repo.index()?.has_conflicts(): return Git("cannot compose: unresolved conflicts")

    # ---- validate the WHOLE plan first; nothing mutates yet (D4) ----
    if plan.groups.is_empty(): return NothingToCommit
    resolve_signature(&repo.config()?.snapshot()?)?           # ConfigMissing EARLY (before any commit)
    changed := set(path for fd in ai_explain::gather_worktree(workdir)?)   # current HEAD->workdir set
    seen := {}
    for g in &plan.groups:
        if g.message.trim().is_empty(): return EmptyMessage
        if g.files.is_empty():          return Other("a group has no files")
        for f in &g.files:
            validate_rel_path(f)?
            if f not in changed: return Other("file '"+f+"' is not in the working changes; refresh the composer")
            if f in seen:        return Other("file '"+f+"' is assigned to more than one group")
            seen.insert(f)

    # ---- rollback anchor ----
    orig_head: Option<Oid> := repo.head().ok().and_then(peel_to_commit).map(oid)   # None on unborn HEAD

    # ---- take over the index so uncovered changes cannot leak (D5) ----
    reset_index_to_head(&repo, orig_head)?     # index := HEAD tree (or clear() if unborn); workdir untouched

    # ---- create commits in order; atomic on failure ----
    commits := []
    for g in &plan.groups:
        step := (|| -> Result<CommitResult, AppError> {
            stage_paths(workdir, &files_with_rename_origs(&repo, &g.files))?   # stage ONLY this group
            create_commit(workdir, &g.message)                                 # commits whole index = cumulative
        })()
        match step:
            Ok(cr)  => commits.push(ComposeCommit { oid: cr.oid, summary: cr.summary })
            Err(e)  => { rollback(&repo, orig_head)?; return Err(annotate(e, group_index)); }

    Ok(ComposeApplyResult { commits })
```

Helpers (in-file):
- `reset_index_to_head(repo, orig_head)`: `Some(h)` ⇒ read HEAD tree into the index and `index.write()`
  (equivalent to `git reset --mixed HEAD`); `None` (unborn) ⇒ `index.clear(); index.write()`. WORKDIR IS
  NEVER TOUCHED (no checkout, no `--hard`).
- `files_with_rename_origs(repo, files)`: for any file whose status is `Renamed`, also include its
  `orig_path` so `stage_paths` stages both the deletion (old) and add (new) sides.
- `rollback(repo, orig_head)`:
  - `Some(oid)`: point HEAD's target back to `oid` — branch HEAD ⇒ `branch_ref.set_target(oid, "bonsai:
    composer rollback")`; detached ⇒ set the HEAD ref directly — then `reset_index_to_head(repo,
    Some(oid))`. Working tree untouched ⇒ all original changes intact.
  - `None` (started unborn): the loop created a branch tip; delete that branch ref so HEAD is unborn
    again, then `index.clear(); index.write()`.
- `create_commit` is reused wholesale (its guards + CRLF normalization + identity + `NothingToCommit`
  apply per group). A group whose staged files net to no change ⇒ `create_commit` returns
  `NothingToCommit` ⇒ caught ⇒ rollback ⇒ `Err` (the plan referenced a file with no real change / drift).

**Correctness note:** after the reset, `index == HEAD`. Because the file-level partition puts each file in
exactly one group, staging group K only ever advances files untouched by earlier groups; each commit's
delta-to-parent is exactly its group's files. No line renumbering (v1 stages whole files).

---

## 5. Commands

`src-tauri/src/commands/ai.rs` — PROPOSE (consent-gate triple, verbatim shape of `generate_commit_message`):
```rust
#[tauri::command]
pub async fn ai_compose_commits(
    app: tauri::AppHandle, state: tauri::State<'_, AppState>,
    repo_id: String, guidance: Option<String>,
) -> Result<ComposeProposal, AppError> {
    let file = settings::settings_file(&app)?;
    ai_compose_commits_inner(state.inner(), &file, &repo_id, guidance).await
}
// _inner: load settings; refuse AiUnavailable unless ai_enabled && ai_consented (BEFORE repo_path);
//         repo_path; spawn_blocking(ai_compose::compose_commits(&workdir, guidance.as_deref(), RunOpts::default()))
//         map_err(join).
```

`src-tauri/src/commands/compose.rs` (new) — APPLY (house shape of `commit`; NO consent gate; no
`repo-changed` emit):
```rust
#[tauri::command]
pub async fn apply_composed_commits(
    state: tauri::State<'_, AppState>, repo_id: String, plan: ComposePlan,
) -> Result<ComposeApplyResult, AppError> {
    apply_composed_commits_inner(state.inner(), &repo_id, plan).await
}
// _inner: repo_path; spawn_blocking(compose_apply::apply_composed_commits(&workdir, &plan)); map_err(join).
```
Register both in `lib.rs generate_handler!` (`ai_compose_commits` after `ai_digest`;
`apply_composed_commits` after `commit_amend`). Re-export the five types in `commands/shared.rs`.

---

## 6. Frontend — review UI (P54c)

### 6.1 `useCommitComposer.ts` (state hook)
```ts
export function useCommitComposer(deps: {
  repoId: string;
  refetchStatus(): void;                 // after a successful apply
  refetchGraph(): void;
  pushToast(kind: ToastKind, msg: string): void;
}): {
  open: boolean; openComposer(guidance?: string): void; close(): void;
  openRef: { current: boolean };         // Esc-layering
  loading: boolean; error: string | null; // propose in-flight / failure
  notes: string[];                        // proposal.notes
  groups: ComposeGroup[];                 // editable working copy
  unassigned: string[];                   // editable working copy
  // plan reducers (pure, local):
  editMessage(gi: number, message: string): void;
  moveFile(path: string, toGroup: number | 'unassigned'): void;
  addGroup(): void; dropGroup(gi: number): void; mergeInto(gi: number, targetGi: number): void;
  // apply:
  applying: boolean; canApply: boolean;   // >=1 valid group (every group: non-empty msg + >=1 file)
  apply(): Promise<void>;                 // ipc.applyComposedCommits(plan); on ok: toast, refetch*, close
};
```
Behavior: `openComposer` sets `loading`, calls `ipc.aiComposeCommits(repoId, guidance ?? null)` behind a
`reqId` last-wins guard (a stale/closed response is dropped), seeds `groups`/`unassigned`/`notes`. Reducers
mutate only local state. `apply()` builds `ComposePlan { groups }` (unassigned files are intentionally
omitted ⇒ left uncommitted), calls `ipc.applyComposedCommits`, and on success toasts
`Created ${result.commits.length} commit(s)`, refetches graph+status, closes. Errors → `error` / toast.

### 6.2 `ComposerDialog.tsx` + `ComposerGroupCard.tsx`
- `ComposerDialog` (overlay, centered modal card): header "Compose commits" + a note line rendering
  `notes` and, when `unassigned.length > 0`, "N file(s) will be left uncommitted"; a skeleton while
  `loading`; an error banner on `error`; else the group list (`ComposerGroupCard` per group) + an
  **Unassigned** bucket (`ComposerGroupCard` variant with no message, drop disabled) + "+ New group" +
  footer "Cancel" / "Create N commits" (disabled unless `canApply`; spinner while `applying`).
- `ComposerGroupCard` (presentational): editable `<textarea>` message (summary + body); a validation hint
  when empty; the assigned file rows — each row: status badge + path + a "Move to…" `<select>` (other
  groups + Unassigned) + a "Preview" button (reuse the existing working-dir file-diff IPC that `DiffView`
  uses, opening the file's HEAD→workdir diff) + a remove (→ Unassigned); card actions: "Merge into
  next" + "Drop group" (drop returns its files to Unassigned).
- All state via hook props; no direct IPC in the presentational card. Keep each file < 500 lines.

### 6.3 Entry point + keyboard
- `CommitPanel.tsx`: a "Compose commits ✨" button next to the existing commit affordances, `disabled:
  !(aiEligible && workingDirty)`, `onClick: openComposer()`.
- `useWorkspaceKeyboard.ts`: `composerOpenRef` in Esc-layering (near the top, above `diffSlot`); Esc closes
  the composer (discards the in-progress plan — nothing is committed). `composerOpen` in the shortcut gate
  so graph-nav keys are inert while it is open. If `applying`, Esc/close is ignored (op in flight).

---

## 7. Mock

### 7.1 `src/ipc/mock/handlers/ai.ts` — add `aiComposeCommits`
```
aiComposeCommits(repoId, guidance):
  await delay(700); state := requireRepo(repoId)
  if AI_OFF: throw { kind:'aiFailed', message:'Claude Code CLI not found on PATH' }
  changed := unique paths from state.status.{staged,unstaged,untracked} (+ MAIN_RS_PATH if mainRs.workdir != head)
  if changed.length === 0: throw { kind:'nothingToCommit', message:'nothing to compose (working tree clean)' }
  split changed into up to 2 groups (e.g. tests/docs vs code by path heuristic; else first-half/second-half)
  return { groups: [ {files:g1, message:'feat: ...'}, {files:g2, message:'test: ...'} ].filter(nonEmpty),
           unassigned: [], notes: [], costUsd: 0.012 }
```

### 7.2 `src/ipc/mock/handlers/compose.ts` (new) — `applyComposedCommits`
Mutation mock; mirror `status.ts::commit`'s graph+status mutation. Spread `composeHandlers` in `mock.ts`.
```
applyComposedCommits(repoId, plan):
  await delay(200 * plan.groups.length); state := requireRepo(repoId)
  # validation parity with the backend:
  if plan.groups.length === 0: throw { kind:'nothingToCommit', ... }
  for g in plan.groups:
     if g.message.trim()==='' : throw { kind:'emptyMessage', ... }
     if g.files.length===0    : throw { kind:'other', message:'a group has no files' }
  if any g.message contains '#fail': throw { kind:'git', message:'Mock: composer apply failed (rolled back)' }  # nothing mutated
  commits := []
  for g in plan.groups (oldest->newest):
     remove g.files from status.{staged,unstaged,untracked}; if MAIN_RS_PATH in g.files: set mainRs.index=mainRs.workdir=mainRs.head
     oid := randomOid(); summary := g.message.trim().split('\n')[0]
     state.headOid := oid; state.commits.unshift({ oid, summary })   # newest on top, like commit mock
     commits.push({ oid, summary })
  # files NOT in any group stay in status (left uncommitted)
  return { commits }
```
`#fail` in a message drives the atomic-rollback error path (mutate NOTHING). Files outside any group
remain in `status` so the harness shows "left uncommitted."

---

## 8. Test plan (`#[cfg(test)]`)

Reuse AI/git idioms: `init_scratch()` (identity + autocrlf off), `create_commit`/`stage_paths`,
`prompts_are_single_line`, `*_wire_shape_is_camel_case`, the `claude_stub` via `CLAUDE_BIN_ENV`. Windows
test-runner sets `TMP`/`TEMP=D:\Temp` (MEMORY rule). No concurrent `cargo test` + `clippy`.

**`ai_compose.rs`**
1. `parse_unparseable_degrades_to_unassigned`: junk / prose input ⇒ `groups:[]`, `unassigned == changed`,
   a note. (Pure, no CLI.)
2. `parse_normalizes_partition`: overlap (a path in two groups) ⇒ first-wins + note; unknown path ⇒
   dropped + note; empty group ⇒ dropped; uncovered path ⇒ `unassigned`; result is disjoint and covers
   `changed`.
3. `parse_caps_groups`: > `MAX_COMPOSE_GROUPS` raw groups ⇒ capped, tail folded into `unassigned` + note.
4. `parse_extracts_fenced_json`: `\`\`\`json { ... } \`\`\`` and leading/trailing prose both parse.
5. `compose_clean_tree_is_nothing_to_commit` (no CLI spawned — fake bin panics if called).
6. `compose_grounding_shape` (stub echo): payload contains `CHANGED FILES (...)`, the exact paths, and
   `===== FILE:` blocks; result is a `ComposeProposal` with `costUsd`.
7. `compose_group_wire_shape_is_camel_case` (`files`/`message`; `ComposeProposal` `unassigned`/`notes`/
   `costUsd`, `None`→`null`) + `ComposePlan`/`ComposeCommit` casing.
8. `prompts_are_single_line`.

**`compose_apply.rs`**
9. `apply_two_groups_creates_two_commits_each_its_own_delta`: 3 changed files split 2+1 ⇒ two commits;
   commit-1 diff-to-parent == group-1 files, commit-2 == group-2 file; HEAD advanced by 2.
10. `apply_leaves_uncovered_files_uncommitted`: a changed file in no group stays dirty in `read_status`
    after apply.
11. `apply_rejects_before_any_commit`: empty message ⇒ `EmptyMessage`; duplicate path across groups ⇒
    `Other`; path not in change set ⇒ `Other`; empty plan ⇒ `NothingToCommit`; unset identity ⇒
    `ConfigMissing`. In every case HEAD is unchanged and NOTHING is committed.
12. `apply_rolls_back_on_mid_sequence_failure`: force a group-2 failure (e.g. a group whose file nets to
    no change ⇒ `create_commit` `NothingToCommit`); assert HEAD == original, the index == HEAD, the
    working tree still holds ALL original changes, and zero commits landed.
13. `apply_first_commits_on_unborn_head`: unborn HEAD + 2 groups ⇒ 2 commits (first is the root); a
    forced rollback from unborn returns HEAD to unborn + empty index.
14. `apply_does_not_touch_workdir`: file bytes on disk are byte-identical before/after a successful apply
    (only the index/refs move).
15. `apply_result_wire_shape_is_camel_case`.

CLI-oracle-style (guard `have_git()`): after `apply_two_groups...`, `git show --stat` per created commit
lists exactly that group's files.

---

## 9. Sub-increments + acceptance

### P54a — Propose backend + IPC + mock
Scope: `ai_compose.rs` (types, `compose_commits`, grounding, `parse_compose_response`, prompt consts,
tests §8.1–§8.8); `git/mod.rs`; `ai.rs` `ai_compose_commits` + `_inner`; `shared.rs` + `lib.rs` (+1);
`types.ts` + `tauri.ts` (`aiComposeCommits`, `ComposeGroup`/`ComposeProposal`); `ai.ts` mock.
**Acceptance:** (1) `cargo test -p bonsai-core ai_compose` green incl. every §8.1–§8.8 (esp. unparseable→
unassigned, overlap first-wins, unknown dropped, coverage, cap); consent gate → `NoRepo` after enable
(reuse the `generate_commit_message_enforces_consent_gate_then_no_repo` pattern). (2) `cargo build` +
`clippy -D warnings` clean; `generate_handler!` count +1. (3) `tsc`/`pnpm build` clean; no file > ~500
lines. (4) Harness console: `await ipc.aiComposeCommits('r', null)` resolves a partition
(`groups`+`unassigned`+`notes`+`costUsd`); `?ai=off` rejects `{kind:'aiFailed'}`.

### P54b — Apply engine + IPC + mock
Scope: `compose_apply.rs` (types, `apply_composed_commits` + reset/validate/loop/rollback, tests
§8.9–§8.15); `git/mod.rs`; `commands/compose.rs` + `mod.rs` + `lib.rs` (+1); `shared.rs` re-exports;
`types.ts` + `tauri.ts` (`applyComposedCommits`, `ComposePlan`/`ComposeCommit`/`ComposeApplyResult`);
`compose.ts` mock + `mock.ts` spread.
**Acceptance:** (1) `cargo test -p bonsai-core compose_apply` green incl. two-commit deltas, uncovered-left-
uncommitted, validate-before-commit, mid-sequence rollback (HEAD+index+workdir intact, zero commits),
unborn-HEAD first-commits, workdir-untouched; `have_git()` oracle per-commit `--stat`. (2) build/clippy
clean; count +1. (3) `tsc`/`pnpm build` clean. (4) Harness console: `applyComposedCommits('r', {groups:[…]})`
resolves `{commits:[…]}`; a `#fail` message rejects `{kind:'git'}` and the mock status is UNCHANGED.

### P54c — Review UI
Scope: `useCommitComposer.ts`, `ComposerDialog.tsx`, `ComposerGroupCard.tsx`; `CommitPanel` entry
(`aiEligible && workingDirty`); `useWorkspaceKeyboard` (Esc-layer + gate); `RepoWorkspace` wiring; styles.
**Acceptance:** (1) `pnpm build` clean; no file > ~500 lines. (2) Harness (`VITE_MOCK_IPC=1`): dirty tree →
"Compose commits ✨" enabled → opens with ≥1 group; reassign a file between groups; edit a message; drop &
merge groups; a group with an empty message disables "Create"; "Create N commits" applies → the graph
gains N rows on top and committed files leave the status list, uncovered files remain with the "left
uncommitted" note. (3) `?ai=off` → error banner in the dialog. (4) Esc closes the composer before any lower
overlay; nav keys inert while open. (5) Clean tree → the entry is disabled.

(a → b → c: b reuses `ComposeGroup` from a; c needs both. a and b are otherwise independent.)

---

## 10. Acceptance criteria (milestone) — AI-gate vs USER CHECKPOINT

**AI gate (orchestrator-verifiable):** P54a+b+c acceptance above; whole-crate `cargo test` green; the
normalizer proves the partition invariant (coverage + disjoint + first-wins + cap + unparseable-degrade);
the apply proves atomic all-or-nothing with ref+index rollback and an untouched working tree (unit +
`have_git()` oracle); consent gate enforced in `ai_compose_commits_inner`; apply is NOT AI-gated and
carries no `repo-changed` emit; command count +2; browser harness exercises propose → review → apply and
the `?ai=off` + `#fail` error paths; grounding is WHY-not-WHAT (CHANGED FILES + per-file diffs + a
message-intent-seeking prompt); no file over the ~500-line soft limit.

**USER CHECKPOINT (`docs/contracts/P54-user-checklist.md`):** with a real `claude` CLI on a real dirty
repo — propose returns sensible, coherent groups with intent-focused messages (not diff restatements);
reassigning/editing/dropping/merging behaves; "Create N commits" produces exactly N commits whose
per-commit diffs match the groups, in order, on the current branch (and first-commits on an unborn HEAD);
a mid-sequence failure (e.g. induced) leaves HEAD and the working tree exactly as before with nothing
committed; files left unassigned remain uncommitted; no code leaves the device (local CLI only).

---

## 11. Open questions (flag to orchestrator)

- **OQ1 — Dirty-index precondition (D5).** Recommend: apply AUTO-RESETS the index to HEAD as step 1
  (working tree untouched; the review UI states "the composer manages staging"). Alt: REFUSE when the
  index has staged content and ask the user to unstage first (less friction-free, more explicit). Confirm
  the auto-reset.
- **OQ2 — Granularity: file-level v1 vs hunk/line-level (D2). RECOMMENDED FORK.** Ship file-level for
  proposal + apply now (robust, `stage_paths`, no renumbering, reliable model output). Line-level (a file
  split across commits) is a clean follow-up: extend `ComposeGroup.files` entries with an optional
  `lines: LineSelection[]` (absent ⇒ whole file), have apply reconstruct each file's cumulative blob FROM
  HEAD using `stage_partial::{reconstruct, assemble, split_keep_terminator}` driven by the ONE HEAD→workdir
  diff computed at apply-start (so line numbers never shift across commits), add a hunk-splitting review
  editor, and add a line-aware prompt. It requires promoting those `stage_partial` helpers' visibility and
  a per-file diff snapshot in the plan. Confirm file-level v1; schedule line-level as P54d/P66 if wanted.
- **OQ3 — Guidance input.** Recommend shipping the optional `guidance` hint (cheap, matches
  `ai_generate_asset`). Alt: drop it for v1 (model chooses N with no hint). Confirm.
- **OQ4 — Shared `cap_review_payload` / `gather_worktree` visibility.** Both currently live in
  `ai_explain.rs` (`cap_review_payload` private; `gather_worktree` promoted to `pub(crate)` by P53).
  Recommend promoting `cap_review_payload` to `pub(crate)` (and depending on P53's `gather_worktree`
  promotion; if P53 is unmerged, promote it here). Alt: duplicate the tiny cap idiom. Confirm.
- **OQ5 — Empty-message groups from the model.** The normalizer KEEPS a valid-file group even if its
  message is empty (the UI requires a message before "Create" is enabled; apply rejects `EmptyMessage`).
  Recommend keep (UI-validated). Alt: drop empty-message groups in the normalizer. Confirm.
- **OQ6 — Progress feedback for apply.** Recommend a single command (N ≤ `MAX_COMPOSE_GROUPS`, sub-second)
  — no channel/event. If a future large-N or slow-hook (P59) case makes apply lengthy, add a progress
  channel then. Confirm not needed now.
