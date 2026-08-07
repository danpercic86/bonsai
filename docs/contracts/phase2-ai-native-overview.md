# Phase 2 — AI-native edge — shared conventions (P53–P57 anchor)

The consistency anchor for the Phase-2 AI milestones (P53 why-layer, P54 commit composer, P55
NL→safe-op, P56 changelog, P57 semantic search). It does NOT design those features — it records the
SHARED conventions every Phase-2 contract must follow so they stay coherent, and it documents the
one strategic OPEN DECISION (model tiers) the user must confirm before the build-out starts.

References read (verified, not guessed): `crates/bonsai-core/src/ai/mod.rs` (`run_claude`, `RunOpts`,
`AiResult`, `check_availability`, `DEFAULT_MODEL`, `CLAUDE_BIN_ENV`, `resolve_bin`), `ai/payload.rs`
(`render_file_diffs` / `render_headers` / `render_commit_list`, `MAX_PAYLOAD_*`), `git/ai_explain.rs`
(`analyze_diff`, `AiDiffTarget`, `AiAnalysisMode`, `AiAnalysis`, `digest_changes`, `cap_review_payload`),
`git/ai_summary.rs` (`summarize_range`, `AiSummary`), `git/ai_commit.rs` (`generate_commit_message`),
`git/blame.rs` (`blame_file`, `BlameLine`), `src-tauri/src/commands/ai.rs` (the command triple + consent
gate), `src/components/AiOutputPanel.tsx`, `src/ipc/mock/handlers/ai.ts`. Established contracts:
`docs/contracts/P13-ai-foundation.md`, `P15-ai-features.md`, `P25-ai-review-stale-branches.md`,
`P28-what-changed-digest.md`.

---

## C1 — Grounding payload assembly (build on `ai/payload.rs`; do NOT reinvent)

Every AI feature turns PRECOMPUTED, typed git data into a single labeled **stdin payload** and hands
it to `ai::run_claude`. The pattern is already established by `ai_explain`/`ai_summary`/`ai_commit`:

1. Gather typed data with EXISTING git2 helpers — never re-shell git, never re-walk raw objects for
   AI: typed diffs via `git/diff.rs` (`commit_diff`, `commit_file_diff`, `workdir_file_diff`,
   `collect_file_diffs`/`collect_headers`), blame via `git/blame.rs`, commit metadata via git2,
   graph topology / branch ahead-behind via `graph.rs` / `health.rs`.
2. Render with `ai/payload.rs`: `render_file_diffs` (hunk-level), `render_headers` (diffstat),
   `render_commit_list` (short-oid · summary · author). These already enforce
   `MAX_PAYLOAD_LINES`/`MAX_PAYLOAD_FILES` and emit a truncation note.
3. Prefix labeled context sections (the **grounding vocabulary**, below), then apply the shared
   byte-cap idiom (`cap_review_payload`, `MAX_REVIEW_PAYLOAD_BYTES = 256 KiB`) over the WHOLE string.
4. Payloads are multi-line ⇒ **stdin ONLY, never argv** (Windows `claude.cmd` rejects newline args).
   Prompts / system-prompts are single-line consts (assert with a `prompts_are_single_line` test).

**Grounding vocabulary** — labeled uppercase section headers so the model can parse context. Reuse
these spellings; add a new one only when a feature needs it, and record it here:

| Header | Emitted by | Meaning |
|---|---|---|
| `COMMIT <short7>  <summary>` / `AUTHOR …` / `MESSAGE:` | ai_explain (commit), P53 line/commit | commit identity + intent |
| `BRANCH <name> vs <base> (merge-base)` | ai_explain (branch) | branch review scope |
| `RANGE <from>..<to> (<n> commits)` | ai_explain digest, ai_summary | range scope |
| `COMMITS …` / `NET CHANGES (diffstat):` | ai_summary | commit list + aggregate stat |
| `STAGED CHANGES (git diff --cached):` | ai_commit | staged payload |
| `===== FILE: <path> (<status>[, was <orig>]) =====` | render_file_diffs | per-file diff block |
| `LINE <n> of <path>:` | P53 explain-line | line-centric anchor |

**"WHY, not WHAT" (mandatory).** Grounding MUST include intent signals, not just the diff: commit
MESSAGE bodies, commit summaries for ranges, branch/base names. A feature that only renders the diff
and asks "summarize this" is a dead feature — the research verdict. Every Phase-2 system prompt asks
for intent/why first, specifics second, and forbids code fences.

## C2 — UX: generate → review → accept/edit; host = `AiOutputPanel`

- **Read-only prose features** (explain, review, digest, summary, line-why) render in
  `AiOutputPanel.tsx` — a dismissible prose card layered over the graph pane. RepoWorkspace owns the
  call + `{ title, text, loading, error, costUsd }` state via the `aiPanelReqId` **last-wins req-id
  guard** (a stale/slow response for a superseded or closed panel is dropped). Reuse `runAnalyze`'s
  shape for any new prose call; do not add a second panel component.
- **Editable-output features** (commit message P15a, branch name P53c, composer P54) do NOT use
  `AiOutputPanel`: the proposal lands in an editable field/dialog the user edits before the real
  git op. Bonsai never writes on the model's say-so — the generate step WRITES NOTHING; a separate,
  already-confirmed command performs the mutation.
- **Cancellation** = close the panel / dismiss the proposal: the req-id invalidates the pending
  response. The backend `spawn_blocking` call still runs to completion or its `DEFAULT_TIMEOUT`
  (90 s) and its result is discarded. True child-process kill is a future enhancement (would need a
  cancellation-token / process-handle registry) — flag if any Phase-2 feature needs it.
- **Errors** surface as `AppError` kinds → the panel's error banner / a toast: `aiUnavailable`
  (CLI missing or consent off), `aiFailed` (CLI error/empty/timeout), `git` (bad ref/oid),
  `invalidName` (bad path/arg), `noRepo`. Never a raw string kind.

## C3 — IPC naming + shape (align with existing `ai_*`)

- **Commands only.** AI results are bounded prose / small lists ⇒ request/response `#[tauri::command]`,
  never a channel, never an event (read-only ⇒ no `repo-changed`). If a future feature wants streamed
  tokens, THAT is a channel — out of scope until asked.
- **Naming:** snake_case `ai_<verb>[_<noun>]` (`ai_explain_line`, `ai_suggest_branch_name`,
  `ai_compose_commits`, …); TS camelCase mirror (`aiExplainLine`, …). Read-only verbs may drop the
  `ai_` only when they are not primarily AI (not the case in Phase 2 — keep `ai_`).
- **Triple + consent gate (verbatim from `commands/ai.rs`):** `ai_x(app,state,…) → settings_file(&app)
  → ai_x_inner(state, &file, …)`. The inner loads settings and REFUSES with
  `AiUnavailable` unless `ai_enabled && ai_consented` — enforced BEFORE `repo_path`, then
  `spawn_blocking(core::…(RunOpts::default()))`, then `map_err(join)`. This backend gate is
  authoritative; the frontend also gates affordances on `aiEligible` (installed && enabled &&
  consented) for UX.
- **Serde:** inputs `#[derive(Deserialize)] #[serde(rename_all="camelCase")]` (discriminated unions use
  `#[serde(tag="kind", …)]`, matching a TS `{ kind: … }` union); outputs `Serialize` camelCase with a
  `*_wire_shape_is_camel_case` test. Reuse `AiAnalysis { text, costUsd }` for any read-only prose
  result; add a new result type only for structured output (lists, grouped commits).
- **Mock parity (mandatory):** every new `ai_*` command gets a handler in
  `src/ipc/mock/handlers/ai.ts` that honors the `AI_OFF` (`?ai=off`) sentinel → throw
  `{ kind:'aiFailed', … }`, else returns canned, shape-correct output. The harness must exercise the
  same panel/dialog plumbing with no CLI.

## C4 — Privacy / consent (gate AND differentiator)

LOCAL-FIRST is a selling point, not a footnote. Phase-2 AI runs the local `claude` CLI on the user's
own subscription (`resolve_bin()` → PATH `claude`); code is piped to a LOCAL child process, never a
network API. The `ai_enabled && ai_consented` gate (C3) is the single consent point today. Any change
that could send code off-device is out of scope for Phase 2 and gated by C5.

## C5 — Model-tier extension point (documented seam; NOT built in Phase 2)

Two seams already exist and MUST be preserved: `RunOpts.model: Option<String>` (per-call model alias;
`None ⇒ DEFAULT_MODEL="sonnet"`) and `resolve_bin()` (which binary to spawn). A future tier system
would add a trait:

```rust
pub trait AiBackend { fn run(&self, req: AiRequest) -> Result<AiResult, AppError>; }
// LocalClaudeCli (wraps today's run_claude) | ByoKeyApi | HostedApi | LocalModel
```

core `ai_*` fns would take `&dyn AiBackend` instead of calling the free `run_claude`; the command
layer would select the impl from a new `ai_backend` setting. Privacy invariant to bake in THEN:
`LocalClaudeCli` is the default and the only tier needing no extra consent; every non-local tier is
explicit opt-in behind a distinct "this sends your code to `<provider>`" consent, separate from the
C4 gate.

**Recommendation (flagged OPEN DECISION — confirm before P54 starts):** ship ALL of Phase 2
(P53–P57) on local-`claude`-CLI ONLY. Do NOT introduce the trait now — a tier refactor is a single
cross-cutting change touching every `ai_*` feature uniformly, so P53–P57 keep calling `run_claude`
directly (consistent with P13/P15). Introduce `AiBackend` only if/when the user greenlights tiers.
Privacy is both the gate and the differentiator — this is the user's call, not the architect's.

---

## Reuse map (what each Phase-2 milestone leans on)

- **P53 (why-layer):** `blame.rs`, `ai_explain::analyze_diff`, `commit_file_diff`, payload renderers,
  `AiAnalysis`, `AiOutputPanel`, branch-create dialog. (Full contract: `P53-ai-why-layer.md`.)
- **P54 (composer):** typed status/diff, line-selection staging (`stage_partial`), payload renderers,
  confirm-gated commit; NEW structured proposal type (groups of files + messages).
- **P55 (NL→safe-op):** `crates/bonsai-mcp` typed mutation tools + confirm gates; a NEW previewable
  operation descriptor (diff + affected refs) — never a raw shell string.
- **P56 (changelog):** `ai_explain::resolve_digest_range` (range resolver), `render_commit_list`,
  `render_headers`; reuses `AiAnalysis` or a grouped-notes type.
- **P57 (semantic search):** NEW embedding index (highest cost); still routes generation through the
  C1 grounding + C3 command conventions; local-embedding option preserves C4.

## Open decisions (flag to orchestrator)

- **OD1 — Model tiers (C5).** Confirm Phase 2 is local-CLI-only and the `AiBackend` trait is deferred.
  Recommendation above. This blocks the start of P54+.
- **OD2 — Streaming.** Prose is single-shot commands today. If any feature wants live token streaming,
  add a channel then. Recommend: not in Phase 2.
