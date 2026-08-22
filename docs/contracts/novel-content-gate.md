# Contract — AI conflict novel-content gate (P68 #7 / H1)

Closes H1 structurally: under `aiConflictAutonomy: autoResolve`, a model-resolved file whose body
contains lines present in **none** of base/ours/theirs is demoted to *needs review* and never
auto-written/auto-staged. WHAT is fixed in `P68-security-audit.md` §H1/#7; this is the HOW.

## Enforcement architecture (the load-bearing decision)

The autoResolve write today: `aiRunSettle.ts` → `applyResolution` → `resolveConflictText` command →
`conflict::resolve_conflict_text` (`conflict.rs:334`). That command is **shared** with the manual
ConflictEditor, where novel lines are legitimate — so the gate **cannot** live inside
`resolve_conflict_text`. `ConflictSides` (base/ours/theirs) + the proposed body coexist only in Rust
at batch assembly (`ai_resolve_stream.rs`). Therefore **two layers, one predicate**:

1. **Authoritative write-path enforcement (required by the audit).** A NEW gated command
   `ai_apply_resolution` re-reads the conflict sides server-side and refuses a novel body. The
   autoResolve auto-stage path switches to it; the manual editor keeps `resolveConflictText` ungated.
   This is authoritative because it recomputes from freshly-read sides — it never trusts a
   frontend-passed flag.
2. **Backend classification + frontend demotion (defense-in-depth + correct UX).** `AiResolveBatch`
   proposals carry `needsReview`, computed where sides+body coexist. `settleBatch` excludes
   `needsReview` files from `stageable` (so they never reach step 1's write in the normal path, and
   the row shows "needs review").

Both layers call the **same pure predicate** `resolution_is_novel`, so they cannot disagree.

## The predicate (Rust) — `crates/bonsai-core/src/git/ai_resolve.rs`

Add next to `ConflictSides` (keeps the predicate beside the data it reads; file stays < 500 lines):

```rust
/// True iff `proposed` has ≥1 non-blank line whose NORMALIZED form appears in NONE
/// of base/ours/theirs. Per-file verdict (not per-line). Pure — the union is built
/// from the three sides already held. The `ABSENT` sentinel contributes only the
/// literal "(absent)" line and is harmless.
pub(crate) fn resolution_is_novel(sides: &ConflictSides, proposed: &str) -> bool
```

Normalization rule (justified):
- Split every string with `str::lines()` — this already splits on `\n` AND strips a trailing `\r`,
  so CRLF vs LF never causes a false positive.
- `normalize(line) = line.trim()` (leading/trailing Unicode whitespace only).
- **Skip blank lines** (`normalize(line).is_empty()`) on both sides — blank lines are never novel and
  never contribute to the allowed set.
- Allowed set = `HashSet<&str>` of `normalize` over every non-blank line of `base`, `ours`, `theirs`.
- Novel iff any non-blank `normalize(proposed_line)` is absent from the allowed set. First hit wins.

Why **trim-only** and nothing more: reindentation and trailing-whitespace changes only touch
leading/trailing whitespace, which `trim` removes — so a legitimate reindent is not flagged. We do
**not** collapse interior whitespace, lowercase, strip punctuation/comments, or compare token sets:
any of those would let an injected payload line collide with an innocuous allowed line while carrying
different bytes (evasion). Trim-only keeps full interior content significant, so a novel payload line
cannot alias an allowed line.

Accepted tradeoff (from the audit): a resolution that *rewrites* whole lines rather than recombining
existing ones is flagged as needs-review. Conflict resolution is overwhelmingly line recombination
(picking/keeping whole lines), which passes; a wholesale rewrite is both rare AND exactly the case a
human should review. Demotion routes to review — it never destroys the proposal.

## Rust changes

### 1. `AiResolveProposal` gains the flag — `ai_resolve.rs:37-43`
```rust
#[serde(rename_all = "camelCase")]
pub struct AiResolveProposal {
    pub path: String,
    pub proposed_text: String,
    pub cost_usd: Option<f64>,
    pub needs_review: bool, // NEW — serialized `needsReview`
}
```
Set it at every construction site (each already holds the sides):
- `ai_resolve_stream.rs` `resolve_single` (~:215): `needs_review: resolution_is_novel(sides, &res.text)`.
- `ai_resolve_stream.rs` `resolve_bulk` (~:302): build `path -> &ConflictSides` map once from the
  `sides` slice; per attributed `(path, body)` set `needs_review: resolution_is_novel(s, &body)`
  (a proposal with no matching side — impossible by construction — defaults `false`).
- `ai_resolve.rs` `ai_resolve_conflict` (~:197, P13 fallback): `resolution_is_novel(&sides, &result.text)`.

`needsReview` is **autonomy-independent** (a property of body vs sides). Autonomy decides only what
`settleBatch` does with it.

### 2. New error variant — `crates/bonsai-core/src/error.rs`
```rust
/// A body the novel-content gate refused to auto-stage. Distinct from AiFailed so
/// the frontend can route it to review instead of showing a raw error toast.
AiNeedsReview(String),
```
Add to `kind()` → `"aiNeedsReview"` and to the message arm (mirror `AiFailed`).

### 3. New write-path command — `src-tauri/src/commands/merge.rs` (beside `resolve_conflict_text`)
```rust
#[tauri::command]
pub async fn ai_apply_resolution(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    content: String,
) -> Result<(), AppError>;

pub(crate) async fn ai_apply_resolution_inner(
    state: &AppState, repo_id: &str, path: String, content: String,
) -> Result<(), AppError>;
```
Body (mirror `resolve_conflict_text_inner`, add the gate; all under one `spawn_blocking`):
1. `let workdir = repo_path(state, repo_id)?;`
2. `let sides = ai_resolve::read_conflict_sides(&workdir, &path)?;` — re-reads base/ours/theirs from
   the still-conflicted index (guaranteed still conflicted: `read_conflict_sides` requires it).
3. `if ai_resolve::resolution_is_novel(&sides, &content) { return Err(AppError::AiNeedsReview(format!("AI introduced content not present in any version of '{path}' — opened for review"))); }`
4. else `conflict::resolve_conflict_text(&workdir, &path, &content)` — the SAME single core writer, so
   the D4 marker gate + symlink guard are unchanged and there is still exactly one write body.

Register in `src-tauri/src/lib.rs` `invoke_handler`. `read_conflict_sides` / `resolution_is_novel`
are already `pub(crate)`; expose via the existing `crate::git::ai_resolve` path.

Errors: `noRepo | git | invalidName | aiFailed` (ineligible/binary/too-large, from
`read_conflict_sides`) | `aiNeedsReview`.

## TypeScript surface

### `src/ipc/types/ai.ts`
- `AiResolveProposal` gains `needsReview: boolean;` (mirror the doc comment: "true ⇒ the body has
  lines present in no version; never auto-staged").

### `src/ipc/types/common.ts` (kind union, ~:119)
- Add `| 'aiNeedsReview'`.

### `src/ipc/types/ipc-api.ts`
```ts
/** Stage an AI-proposed resolution, gated server-side by the novel-content check.
 *  Rejects aiNeedsReview (body has lines in no version) | aiFailed | git | invalidName | noRepo. */
aiApplyResolution(repoId: string, path: string, content: string): Promise<void>;
```
Wire it in `src/ipc/tauri/merge.ts` (invoke `ai_apply_resolution`).

## Frontend changes

### `aiRunState.ts` `settleBatch` — ordering is BINDING (`:149-189`)
`SettledBatch` gains `needsReview: AiRunFileState[]`. Compute order (each strictly after the last, so
`stageable` is last — the audit warns hoisting it re-opens H1):
1. build `files` (ready/failed) — unchanged.
2. **marker demotion** (existing, autoResolve only) — unchanged.
3. **NEW novel demotion** (autoResolve only): of the still-`ready` files, those whose matching
   `batch.proposals` entry has `needsReview === true` → set `status:'failed'`,
   `error = "AI introduced content not in any version of ${path} — opened for review"`, collect into
   `needsReview`.
4. `stageable = files.filter(ready && proposal !== null)` — now excludes both demotions.

Under `proposeReview`: `needsReview` stays `[]` and nothing changes (criterion d). `newRun`/mock
default `needsReview:false` keeps old fixtures behaving.

### `aiRunSettle.ts` (`:64-91`, `:96`)
- Auto-stage loop calls `d.applyResolution` → repoint `applyResolution` at the new
  `ipc.aiApplyResolution` (NOT `resolveConflictText`). A defensive `aiNeedsReview` rejection is
  swallowed like any per-file failure (already `try/catch`), so a flag/skew can never stage novel
  content — that is layer-1 enforcement.
- Review-pane fallback opens `[...out.markerful, ...out.needsReview][0]` (unchanged for markerful).
- Bulk summary counts exclude `needsReview` files (they are not "resolved").
- `handleResolveConflictText` in `useMergeActions.ts` stays pointed at `resolveConflictText` for the
  **manual** ConflictEditor Save (ungated).

## Mock IPC (`VITE_MOCK_IPC=1`)

- `src/ipc/mock/handlers/merge.ts`: add `aiApplyResolution(repoId, path, content)` — reuse the
  `resolveConflictText` body, but first run the same predicate against the mock's stored
  base/ours/theirs (`state.conflictTexts` / conflict sides) and `throw err('aiNeedsReview', …)` when
  novel. Keeps the harness honest (the gate is real in-browser).
- `src/ipc/mock/handlers/aiStream.ts` (~:259): every proposal sets `needsReview`. Add a `?aiNovel`
  URL seam (mirror `?aiMarkers`) that appends one line absent from all sides to the last eligible
  path's `proposedText` and marks that proposal `needsReview:true`; default proposals `false`.
- Export a `resolutionIsNovel(sides, proposed)` TS twin in a small shared mock util so both mock
  handlers agree (keep it byte-for-byte with the Rust rule: `\n`-split, `trim`, skip blank).

## Acceptance criteria

- (a) A resolution using only lines from base/ours/theirs (recombination, reindented, blank-line
  churn, CRLF↔LF) stays `stageable` and auto-stages under autoResolve — `resolution_is_novel` false.
- (b) A resolution with ≥1 line matching no side is demoted to needs-review even under autoResolve:
  it is excluded from `stageable`, its row shows the review affordance, and `ai_apply_resolution`
  rejects `aiNeedsReview` if called directly.
- (c) Reindentation / trailing-whitespace / blank-line-only differences are NOT flagged.
- (d) `proposeReview` is byte-for-byte unchanged: `needsReview` bucket empty, proposals open for
  review as before.
- Ordering invariant: `stageable` is computed strictly AFTER both demotions (P68 clean-surfaces note).
- Preserve: D4 single-core-writer (both commands funnel through `resolve_conflict_text`), exact-path
  bulk attribution, the marker gate, and the P13 single-file verbatim body (only the flag is added).

## Test list

Rust (`ai_resolve.rs` tests):
- `novel_recombination_is_not_flagged` — proposed = interleave of ours+theirs lines → false.
- `injected_line_is_flagged` — proposed = ours + one foreign line → true.
- `reindent_and_crlf_are_not_flagged` — same lines, changed indent / `\r\n` → false.
- `blank_lines_never_novel` — proposed adds blank lines only → false.
- `absent_sentinel_side_is_harmless` — bothAdded (base absent) resolves from ours/theirs → false.

Rust (command): `ai_apply_resolution_inner` — clean body writes (stage-0, no conflict stages);
novel body → `AiNeedsReview` and the file stays conflicted (nothing written).

Frontend (`aiRunState.test.ts` / settle tests):
- autoResolve + `needsReview` proposal → not in `stageable`, in `needsReview`, row failed.
- autoResolve mixed (clean + markerful + novel) → only clean stages; ordering holds.
- proposeReview + `needsReview` → unchanged.

Mock: `?aiNovel` drives (b) in the browser harness; `aiApplyResolution` rejects `aiNeedsReview`.

## Flags for the orchestrator

- **Re-disclose on hook/side change:** none — classification is per-run over current sides; no state.
- **P13 fallback (`ai_resolve_conflict`):** it now also computes `needsReview`; it is proposeReview-only
  in practice, so the flag is informational there. Left set (not defaulted) for consistency — confirm
  acceptable.
- **`AiNeedsReview` vs reusing `AiFailed`:** recommend the new variant (clean frontend routing, no
  scary "failed" toast). If minimizing error-surface churn is preferred, `AiFailed` with a stable
  message substring works but is less clean — flagged.
