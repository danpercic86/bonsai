# P28 — B3 "what changed" digest

An AI-generated natural-language digest of what changed in the repo over a selectable range:
**(a)** between two refs/commits, **(b)** the last N days on the current branch, **(c)** since a
given commit. **WRITE-FREE** — prose only, rendered in the existing `AiOutputPanel`.

Reuses shipped plumbing verbatim: `ai::run_claude` + consent gate, `ai::payload::render_file_diffs`,
`diff.rs::collect_file_diffs` / `build_diff_options` / `apply_find_similar`,
`ai_explain::{cap_review_payload, MAX_REVIEW_PAYLOAD_BYTES, has_analyzable_content, AiAnalysis}`,
and the frontend `AiOutputPanel` + req-id pattern from `runAnalyze` (RepoWorkspace).

New surface, deliberately minimal: one enum (`AiDigestRange`), one core fn (`digest_changes`),
two prompt consts, **one new command** (`ai_digest`), one IPC method, one small dialog.

Sub-increments (§9): **P28a** core + tests + command + IPC triple · **P28b** UI.

---

## Decisions (defaults chosen; orchestrator may override)

1. **New command + new enum, NOT new `AiDiffTarget` variants.** A digest is range-shaped (needs a
   commit-metadata walk, not just a diff) and has its own mode/prompt; the `AiDiffTarget` arms all
   return `(prefix, Vec<FileDiff>)` and feed Explain/Review prompts. Shoehorning three range kinds +
   a third `AiAnalysisMode` variant into `analyze_diff` would force every existing target to handle
   a `Digest` mode it can't sensibly serve. So: `pub fn digest_changes(...)` in `ai_explain.rs`
   (sibling of `analyze_diff`, sharing its private helpers) + command `ai_digest`. Result type is
   the existing `AiAnalysis` — no new result type.
2. **`sinceCommit` = `betweenRefs { from: oid, to: "HEAD" }`.** One code path; a non-ancestor
   `from` degrades to merge-base semantics automatically (same as any ref pair).
3. **`betweenRefs` semantics = merge-base range** (`from...to` narrative): commits reachable from
   `to` and not `from`; diff = merge-base tree vs `to` tree. Matches `gather_branch`/`ai_summary`
   precedent (incl. the unrelated-histories empty-tree fallback + note).
4. **`lastDays` walks FIRST-PARENT on HEAD.** "The last N days on the current branch" is a
   narrative on the branch's mainline; first-parent gives a well-defined cutoff commit whose tree
   anchors the range diff. Cutoff = committer time `>= now − days·86400`. Rejected: full-graph
   time-filtered walk (no well-defined single diff base; merge noise).
5. **Commit-metadata cap `MAX_DIGEST_COMMITS = 200`.** Metadata lines beyond the cap collapse to
   `... and N more commits`. The diff still spans the WHOLE range (byte-capped at 256 KiB by the
   existing `cap_review_payload`); metadata is capped separately so it never crowds out the diff.
6. **Payload order: metadata header first, then rendered diff, then one combined
   `cap_review_payload` over the whole string.** The header is small and bounded (#5), so the
   truncation only ever eats diff tail — the narrative skeleton survives.
7. **Empty range** (no commits AND no diff content) → `AiFailed("no changes in the selected range")`
   before any CLI call. Unborn HEAD for `lastDays`/`sinceCommit`-to-HEAD → same error.
8. **UI entry point: one toolbar affordance "✨ What changed…"** opening a small range-picker
   dialog (§7). Branch-context-menu variants deferred (Polish) — one entry keeps P28b tiny.
9. **No new `AppError` variant, no events, no channels.** Response is small prose → plain command.

---

## 1. Module boundaries

| File | Change | Increment |
|---|---|---|
| `crates/bonsai-core/src/git/ai_explain.rs` (extend) | `AiDigestRange`, `MAX_DIGEST_COMMITS`, `DIGEST_SYSTEM_PROMPT`/`DIGEST_PROMPT`, private `resolve_digest_range` + `format_commit_meta`, `pub fn digest_changes` (§2–§4) | P28a |
| `src-tauri/src/commands.rs` (extend) | `ai_digest` command — consent gate then `spawn_blocking` (§5) | P28a |
| `src-tauri/src/lib.rs` | register `ai_digest` in `generate_handler!` | P28a |
| `src/ipc/types.ts` / `tauri.ts` / `mock.ts` | `AiDigestRange` union + `aiDigest` method + mock (§5.2, §6) | P28a |
| `src/components/WhatChangedDialog.tsx` (new) + `RepoWorkspace.tsx` (extend) | range picker + `runDigest` → `AiOutputPanel` (§7) | P28b |

Nothing else changes. `diff.rs`, `ai.rs`, `error.rs`, `AiOutputPanel.tsx` untouched.

---

## 2. Rust types (`ai_explain.rs`)

```rust
/// Max commits listed in the digest metadata header (Decision #5).
pub const MAX_DIGEST_COMMITS: usize = 200;

/// SINGLE-LINE (Windows claude.cmd argv constraint — same rule as the P15 prompts).
const DIGEST_SYSTEM_PROMPT: &str = "You are a senior engineer writing a change digest for a teammate returning to a repository. Standard input contains a commit list (short hash, date, author, subject) followed by the corresponding combined diff. Write a clear plain-English digest of what changed over this range: a two or three sentence executive summary first, then the main themes or workstreams as short groups, citing file or area names and mentioning authors when several people contributed. Prefer narrative over per-commit listing; skip trivial churn. Output prose only — no markdown code fences.";

const DIGEST_PROMPT: &str = "Summarize what changed in the range provided on standard input.";

/// Which range to digest. Command INPUT (Deserialize); TS mirror is a
/// discriminated union (§5.2).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AiDigestRange {
    /// Commits in `to` but not `from` (merge-base range, `from...to` narrative).
    /// Both accept any revparse-able ref/oid (branch, remote-tracking, tag, hex).
    BetweenRefs { from: String, to: String },
    /// First-parent commits on the current branch (HEAD) with committer time
    /// within the last `days` days. days >= 1 (0 => InvalidName).
    LastDays { days: u32 },
    /// Commits in HEAD but not `oid` — sugar for BetweenRefs{from: oid, to: "HEAD"}.
    SinceCommit { oid: String },
}

/// Blocking. Resolves the range, gathers commit metadata + the range diff,
/// renders the payload, and asks the CLI for a digest. Read-only; WRITES
/// NOTHING. Errors: aiFailed (empty range / CLI failure) | git (bad ref,
/// unborn HEAD for HEAD-anchored ranges) | invalidName (days == 0) |
/// (aiUnavailable via the command-layer gate).
pub fn digest_changes(
    workdir: &Path,
    range: AiDigestRange,
    opts: RunOpts,
) -> Result<AiAnalysis, AppError>;
```

Reuses `AiAnalysis` as the result verbatim. No change to `AiDiffTarget` / `AiAnalysisMode`.

---

## 3. Range semantics (exact git2 rules)

Private helper, one resolution path:

```rust
/// (header_note, commits_newest_first, old_tree: Option<Tree>, new_tree: Tree)
fn resolve_digest_range<'r>(repo: &'r git2::Repository, range: &AiDigestRange)
    -> Result<(String, Vec<git2::Commit<'r>>, Option<git2::Tree<'r>>, git2::Tree<'r>), AppError>;
```

**BetweenRefs { from, to }** (and SinceCommit via `from = oid`, `to = "HEAD"`):
1. `from_c = repo.revparse_single(from)?.peel_to_commit()?`; same for `to` (bad ref → `Git`;
   `"HEAD"` on unborn HEAD → the git2 error surfaces as `Git`).
2. `mb = repo.merge_base(from_c.id(), to_c.id()).ok()`.
3. Revwalk: `push(to_c.id())`; `hide(mb)` when `Some` (hide nothing when unrelated);
   sorting `TOPOLOGICAL | TIME`. Collect commits (all of them for counting; metadata formatting
   caps at `MAX_DIGEST_COMMITS`, §4).
4. `old_tree` = mb commit's tree (`None` → `None`, meaning empty tree); `new_tree = to_c.tree()?`.
5. `header_note` = `"RANGE {from}..{to} ({n} commits)"` + on unrelated histories the same
   "no common ancestor" note wording as `gather_branch`.
6. `from` == `to` (or mb == to) ⇒ zero commits ⇒ empty-range error in `digest_changes` (§4 step 4).

**LastDays { days }**:
1. `days == 0` → `AppError::InvalidName("days must be >= 1")`. Clamp `days` at 3650 (same error
   direction is unnecessary — just clamp).
2. `head = repo.head()?.peel_to_commit()?` (unborn → `Git` from git2; acceptable per Decision #7 —
   the command surfaces `git`, the UI shows the message).
3. `cutoff = now_epoch_secs − days as i64 * 86_400` (`std::time::SystemTime`).
4. Revwalk: `push(head.id())`; `simplify_first_parent()`; sorting `TOPOLOGICAL`. Iterate:
   collect commits while `commit.time().seconds() >= cutoff`; **stop at the first older commit** —
   call it `boundary`. (First-parent order is monotone enough; a single stale-dated commit inside
   the window is an accepted edge case, documented in the code comment.)
5. `old_tree` = `boundary.tree()` when a boundary exists, else `None` (walk exhausted ⇒ the whole
   history is within the window ⇒ diff vs empty tree). `new_tree = head.tree()?`.
6. `header_note` = `"RANGE last {days} day(s) on {branch} ({n} commits)"` where `{branch}` is
   `repo.head().shorthand()` or `"HEAD (detached)"`.

**Range diff** (shared): `build_diff_options(&[], false)` →
`repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut opts))` →
`apply_find_similar` → `collect_file_diffs`. (Exactly the `gather_branch` pipeline.)

---

## 4. `digest_changes` flow

1. `open_workdir_repo(workdir)?`; `resolve_digest_range(...)?`.
2. **Empty range**: if `commits.is_empty() && !has_analyzable_content(&files)` →
   `AiFailed("no changes in the selected range")`, no CLI call. (Commits present but an empty diff
   — e.g. a revert pair — still digests: the metadata alone is a valid narrative.)
3. Format metadata, newest first, one line per commit:
   `- {short7} {YYYY-MM-DD} {author_name}  {subject}` (`subject` = first summary line, lossy UTF-8;
   date from `commit.time()` UTC). After `MAX_DIGEST_COMMITS` lines append
   `... and {n - MAX_DIGEST_COMMITS} more commits`.
4. `payload = cap_review_payload(format!("{header_note}\n\nCOMMITS\n{meta}\n\nDIFF\n{}", render_file_diffs(&files).text))`
   — reuses the existing 256 KiB cap + truncation note (§P25 2.4).
5. `ai::run_claude(workdir, DIGEST_PROMPT, Some(&payload), RunOpts { system_prompt: Some(DIGEST_SYSTEM_PROMPT.into()), ..opts })`
   → map to `AiAnalysis { text, cost_usd }`.

---

## 5. Command + IPC surface

### 5.1 `commands.rs`

```rust
/// Consent-gated (ai_enabled && ai_consented BEFORE repo_path — same order as
/// ai_analyze_diff). Then spawn_blocking(move || ai_explain::digest_changes(...)).
#[tauri::command]
pub async fn ai_digest(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    range: AiDigestRange,
) -> Result<AiAnalysis, AppError>;
```

| Command | IPC method | Args | Returns | Error kinds |
|---|---|---|---|---|
| `ai_digest` | `aiDigest` | `repoId, range` | `AiAnalysis` | `aiUnavailable \| aiFailed \| git \| invalidName \| noRepo` |

`RunOpts` defaults mirror `ai_analyze_diff` (same model/timeout source). No events, no channels
(prose fits a command; existing AI features set the precedent).

### 5.2 TS wire (`types.ts`)

```ts
export type AiDigestRange =
  | { kind: 'betweenRefs'; from: string; to: string }
  | { kind: 'lastDays'; days: number }
  | { kind: 'sinceCommit'; oid: string };
```

`IpcApi`: `aiDigest(repoId: string, range: AiDigestRange): Promise<AiAnalysis>;`
`tauri.ts`: `invoke('ai_digest', { repoId, range })`. Reuses the existing `AiAnalysis` TS type.

---

## 6. Mock (`mock.ts`)

Add `aiDigest` to `mockIpc` (keeps compiling — mandatory):

- `await delay(700); requireRepo(repoId);`
- `AI_OFF` (`?ai=off`) → throw `{ kind: 'aiFailed', message: 'Claude Code CLI not found on PATH' }`
  (identical to `aiAnalyzeDiff`).
- Canned prose keyed on `range.kind`, with the range echoed so the harness shows what was digested:
  - `betweenRefs` → `` `Digest ${from}..${to}: Over this range the team landed the worktrees feature (sidebar section, create dialog, lifecycle commands) and hardened the AI review path; most churn is in src-tauri/src and src/components.` ``
  - `lastDays` → `` `Digest, last ${days} day(s): Mostly polish — mock-harness fixes and docs updates; one behavioral change in the watcher debounce.` ``
  - `sinceCommit` → `` `Digest since ${oid.slice(0, 7)}: Two workstreams — worktree UX and stale-branch cleanup — plus test scaffolding.` ``
- Return `{ text, costUsd: 0.01 }`.

No new mock state; deterministic; browser harness (`VITE_MOCK_IPC=1`) covers dialog → panel flow.

---

## 7. Frontend UX (P28b)

- **Entry**: a toolbar button "✨ What changed…" next to the existing AI affordances, gated by the
  same AI-eligibility check as the other ✨ actions (available + enabled + consented). Opens
  `WhatChangedDialog`.
- **`WhatChangedDialog.tsx`** (new, presentational + local form state):
  - Three radio modes: **Between refs** (two inputs: `from`, `to` — plain text seeded with
    datalist suggestions from the already-loaded branch names; `to` defaults to the current
    branch), **Last N days** (number input, default 7, min 1), **Since commit** (text input for a
    ref/oid).
  - Submit builds the `AiDigestRange` and calls `onSubmit(range, title)`; Cancel closes. No git
    logic in the dialog.
- **`RepoWorkspace.tsx`**: add `runDigest(range, title)` — a sibling of `runAnalyze` sharing the
  SAME `aiPanel` state + `aiPanelReqId` guard, calling `ipc.aiDigest(repoId, range)`. Titles:
  `What changed: {from}..{to}` / `What changed: last {N} days` / `What changed since {short7}`.
- Output + errors render in the existing `AiOutputPanel` (loading spinner, error banner, costUsd)
  — no panel changes.

---

## 8. Error mapping (no `error.rs` change)

| Situation | Variant | TS kind |
|---|---|---|
| AI disabled / not consented / CLI missing | `AiUnavailable` (gate) | `aiUnavailable` |
| Bad ref / unborn HEAD / other git2 failure | `Git` | `git` |
| `days == 0` | `InvalidName` | `invalidName` |
| Empty range | `AiFailed("no changes in the selected range")` | `aiFailed` |
| CLI failed / timed out | `AiFailed` | `aiFailed` |
| Unknown `repoId` | `NoRepo` | `noRepo` |

---

## 9. Sub-increments (each = one fresh-context senior-dev pass)

- **P28a — core + command + IPC triple.**
  Rust: §2–§4 in `ai_explain.rs` + unit tests §10.1 + oracle §10.2 + stub test §10.3; `ai_digest`
  command + `lib.rs` registration. TS: `AiDigestRange` + `aiDigest` in `types.ts`/`tauri.ts`/
  `mock.ts` (§5.2, §6). Acceptance: all new tests green; `tsc` + `pnpm build` clean.
- **P28b — UI.** `WhatChangedDialog` + toolbar button + `runDigest` wiring. Acceptance: harness
  screenshots of the dialog (all three modes) and the resulting `AiOutputPanel` prose; `?ai=off`
  path shows the error banner.

Orchestrator commits each approved sub-increment (`wip(P28a): …`).

---

## 10. Tests (AI gate)

Environment: scratch repos via `crate::testutil::scratch_dir()` / under `D:\Temp\bonsai-scratch`
for integration oracles; `TMP`/`TEMP=D:\Temp`; **never run `cargo test` and `clippy` concurrently**.

### 10.1 Rust unit tests (`ai_explain.rs`)

1. **Serde**: `AiDigestRange` deserializes the exact TS JSON for each variant
   (`{"kind":"betweenRefs","from":"main","to":"feature"}`, `{"kind":"lastDays","days":7}`,
   `{"kind":"sinceCommit","oid":"deadbeef"}`); prompts are single-line (extend the existing test's
   const list).
2. **BetweenRefs walk**: scratch repo, `main` with commits A–B, `feature` branched at B with C–D →
   `resolve_digest_range(BetweenRefs{main, feature})` yields exactly [D, C] newest-first, old_tree
   = B's tree; `from == to` → zero commits → `digest_changes` returns
   `AiFailed("no changes in the selected range")` (assert via `has_analyzable_content` path or the
   stub harness).
3. **SinceCommit** ≡ BetweenRefs to HEAD: since B on `feature` yields [D, C].
4. **Unrelated histories**: two roots → no hide, diff vs empty tree, header carries the note.
5. **LastDays**: build first-parent chain with controlled committer times (git2 `Signature::new`
   with explicit `Time`) — commits at now−1d, now−2d, now−10d; `days=7` collects the two recent,
   boundary = the 10-day-old commit, old_tree = its tree; `days=0` → `InvalidName`; all-in-window
   history → old_tree None.
6. **Metadata cap**: 250 synthetic commit metas → header lists 200 lines + `... and 50 more commits`.
   (Unit-test `format_commit_meta` directly.)

### 10.2 CLI oracle (`crates/bonsai-core/tests/`)

Degrade-skip when `git` absent (existing pattern). Scratch repo under `D:\Temp\bonsai-scratch`:
- BetweenRefs commit set == `git log --format=%h main..feature` (order + membership).
- LastDays commit set == `git log --first-parent --since=<cutoff-iso> --format=%h HEAD`
  (pin commit times explicitly so the oracle is deterministic).

### 10.3 Stub harness

With `BONSAI_CLAUDE_BIN` → `tests/fixtures/claude_stub.cmd`: `digest_changes(BetweenRefs{...})`
returns the stub's canned text; assert the payload written to the stub's capture (if the stub
records stdin) contains both a `COMMITS` line block and a `DIFF` section; empty range errors
BEFORE spawning.

### 10.4 Frontend

`tsc` + `pnpm build` clean; harness (`VITE_MOCK_IPC=1`): dialog opens from the toolbar, each mode
submits and shows mock prose in `AiOutputPanel`; `?ai=off` shows the `aiFailed` banner.

---

## 11. Acceptance — AI gate vs USER CHECKPOINT

**AI gate:** `cargo check` + `clippy` clean (sequential with tests); §10.1–10.3 green; `tsc` +
`pnpm build` clean; harness screenshots per §10.4.

**USER CHECKPOINT (native `pnpm tauri dev`, real `claude` CLI, real repo):**
- "✨ What changed…" between `origin/main` and `main` after a fetch returns a sane digest naming
  real files/authors.
- "Last 7 days" on this repo returns a plausible narrative; "since <old commit>" works with a
  pasted short oid.
- With AI disabled in settings, the affordance is gated/blocked with a clear message.
