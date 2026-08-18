# P68 — streaming, interactive, bulk AI conflict resolution

User items **3–7** from the 2026-08-17 report (board: `TODO.md` § "🐛 USER-REPORTED BATCH
(2026-08-17)"; approved plan: `~/.claude/plans/1-the-dotted-line-cozy-llama.md`, P68 half). Item 3
("Propose & review does nothing") needs **no separate work item** — the proposal already opens in the
*center* pane (`DiffOverlay` → `ConflictEditor`, slot key `ai-proposal:<path>`), invisible from the
right panel where the button is; §C (a toast + a row badge + a per-path store) and §E (the dock) fix
discoverability, and §C fixes the loss.

**Command count: 157 → 160 (+3).** All three land in P68b: `ai_resolve_conflict_stream` (Channel),
`ai_cancel_run`, `ai_reply_run`. **Recount `generate_handler!` in `src-tauri/src/lib.rs` at
implementation** (157 verified 2026-08-17 by counting that macro list; the last entry today is
`commands::forge_commit_statuses`, `lib.rs:228`). One new `AppError` variant (`AiCancelled`), one new
Channel, **zero new Tauri events**.

References read (verified, not guessed):
`crates/bonsai-core/src/ai/mod.rs` (640 lines — `DEFAULT_MODEL` L18, `DEFAULT_TIMEOUT` 90 s L20,
`AVAILABILITY_TIMEOUT` L22, `CLAUDE_BIN_ENV` L25, `RunOpts` L28-40 + `impl Default` L42-46,
`AiResult` L50-55, `AiAvailability` L59-73, `ClaudeEnvelope` L77-88, `ProcOutput` L90-96,
`run_process` L115-197 — `CREATE_NO_WINDOW` L121-126, writer/reader threads L134-156, 50 ms
`try_wait` loop L161-178, **the discard-on-timeout block L180-190**, normal-exit joins L193-196 —
`resolve_bin` L207-213, `kill_child_tree` L223-237, `strip_fence` L243-256, `parse_version` L260,
`run_claude` L279-374 with the locked argv L294-308 and the Windows `.cmd` argv-re-expansion
invariant L288-293, the envelope parse L332-366, `REGISTER_TIMEOUT` L377),
`crates/bonsai-core/src/git/ai_resolve.rs` (`SYSTEM_PROMPT` L21 + its single-line `.cmd` rule
L14-21, `RESOLVE_PROMPT` L24, `ABSENT` L28, `AiResolveProposal` L32-38, `ai_resolve_conflict`
L56-145 — `validate_rel_path` L64, `get_conflict` L68, binary/too_large/missing guards L70-84, kind
gate L88-95, stage 1/2/3 read L99-113, labelled payload L116-128, the single `run_claude` L130-138 —
`proposal_wire_shape_is_camel_case` L154),
`src-tauri/src/commands/ai.rs` (543 lines — `check_ai_availability` L8-13, `ai_resolve_conflict`
L20-32, `ai_resolve_conflict_inner` L36-54 with the consent gate **before** `repo_path`, and the 12
`RunOpts::default()` sites L50,88,129,171,211,257,300,344,387,429,475,539),
`src-tauri/src/commands/history.rs` (the Channel precedent: `history_index_build` L88-102 +
`history_index_build_inner` L107-121 with the Channel abstracted to a plain callback, and the 13th
`RunOpts::default()` at L237),
`src/ipc/tauri.ts` (`historyIndexBuild` L539-548 — `new Channel<IndexProgress>()`),
`src/ipc/mock/handlers/history.ts` (`historyIndexBuild` L48-80, sentinel `HISTORY_FAIL` L41),
`src/ipc/mock/repoState.ts` (`delay` L20, `query` L153, `AI_OFF = query('ai') === 'off'` L160,
`stripConflictMarkers` L164),
`src/ipc/mock/handlers/ai.ts` (485 lines — `aiResolveConflict` L29-43: 600 ms, `bothModified`/
`bothAdded` only, `costUsd: 0.012`, and it **ignores `AI_OFF`**),
`crates/bonsai-core/src/error.rs` (`AiUnavailable` L58-59, `AiFailed` L60-61, the `kind()` table
L96-127 and the `message()` `|`-chain L130-157),
`src-tauri/src/settings.rs` (`clamp_graph_prefs` L279-286, the **no-version-bump bar** L288-303,
`Settings` L304-…, `panel_density` L312-316 as the P67c additive precedent),
`src-tauri/src/commands/ui_settings.rs` (`UiSettings` L11-43, `UiSettingsPatch` L52-84,
`apply_patch` L90-141 with the P67 `panel_density` arm L100-102),
`src-tauri/src/lib.rs` (`.manage(mcp::McpServerState::default())` L20, `generate_handler!` tail
L210-229, `RunEvent::ExitRequested → mcp::shutdown` L232-238), `src-tauri/src/mcp.rs`
(`McpServerState` L104-112 — the managed-state precedent),
`src/components/repoWorkspace/useMergeActions.ts` (219 lines — deps L10-35,
`handleResolveConflictText` L96-108, **`handleAiResolveConflict` L111-169** with the `autoResolve`
staging L127-140, the `hasUnresolvedMarkers` net L126,141-143, and **the shared-`fileDiffReqId`
bug L149-168**),
`src/components/RepoWorkspace.tsx` (3049 lines — `aiEligible` L178, **`aiResolvingPath` L243**,
`aiPanelReqId` L258, **`fileDiffReqId` L538** bumped by ordinary diff opens at
L768,774,777,783,834,840,843,1219,2340, deps hand-off L1557, the six `aiPanel` runners
L1829-1988, the returned fragment L2618, `<WorkspaceToolbar>` L2620-2646, **`<div className="panes">`
L2648**, `aiResolvingPath` passed down L2789, `onAiResolve` L2798),
`src/components/StatusConflictsSection.tsx` (167 lines, **post-P67e split** — `CONFLICT_KIND_LABELS`
L10-18, `ConflictRow` L20-113 with `aiShown` L50, the ✨AI button L99-110, and
`StatusConflictsSection` L117-167 with `aiBusy={aiResolvingPath === entry.path}` L157 /
**`aiDisabled={aiResolvingPath !== null}` L158**),
`src/components/OpBanner.tsx` (merge arm L131-160 — actions row L140-157),
`src/components/SettingsPanel.tsx` (AI section: enable toggle L336-343, autonomy radio group
L345-367, availability line L369-377),
`src/App.tsx` (`.workspace-host` L1004-1008 — `display: flex|none` per tab),
`src/styles.css` (`.workspace-host` L1406-1412 = `flex: 1; min-height: 0; flex-direction: column`,
`.workspace-toolbar` L1414-1426 = `flex: none; height: 40px`),
`src/ipc/types.ts` (`AiAutonomy` L1134, `AiAvailability` L1138-1143, `AiResolveProposal` L1147-1151,
`UiSettings` ai fields L1376-1378, `UiSettingsPatch` ai fields L1408-1410, the `AppError.kind` union
L1812-1813, `aiResolveConflict` L2303),
`crates/bonsai-core/tests/fixtures/claude_stub.cmd` (127 lines — mode dispatch L38-50, `slow` L60-63,
`dump_stdin` L113-118, `emit_file` L120-126).
House format: `docs/contracts/{P67-ux-polish-batch,P62-forge-foundation,P60-parity-batch}.md`.

---

## 0. Key decisions (with rationale) — named invariants D1–D16

**D1 — Rust owns ALL Git logic AND all AI-subprocess logic.** React never spawns, never parses
NDJSON, never splits a bulk response, never decides that a turn ended. The IPC boundary carries
already-computed, compact events and one final batch value. (CLAUDE.md architecture invariant.)

**D2 — Partial output is NEVER discarded.** Today `run_process` returns `stdout: Vec::new()` on
timeout and deliberately never joins the readers (`ai/mod.rs:180-190`), so a 90 s run that was 95 %
done yields *nothing*. The streaming session forwards each line as it arrives, so on cancel or
watchdog fire everything already read is already in the frontend store and in the dock log.
**Scope, stated precisely:** the accumulated assistant *text* is delivered as `log` events and
echoed on the terminal event as `partialText`, and it is shown in the dock **only**. It is NEVER
offered as a stageable proposal — a truncated file body is not markerful, so `hasUnresolvedMarkers`
cannot catch it, and staging it would be silent data loss. "Never discarded" means *never thrown
away unseen*, not *treated as a result*.

**D3 — The idle watchdog is PAUSED while awaiting user input.** Never kill a run that is waiting on
a human. The watchdog measures *time since the last child output*; while `awaiting_input` is set,
that clock is not consulted at all. The optional hard cap (`ai_hard_cap_secs`, default `0` =
unbounded) is **also** paused while awaiting input, for the same reason.

**D4 — AI writes nothing to the working tree; staging is always a separate explicit call after
review.** `ai_resolve_conflict_stream` returns proposed *bytes*. Applying stays the existing
`resolve_conflict_text` command. The tool allowlist is read-only (D10) so the child cannot write
either. This is unchanged from P13 and must stay true for bulk.

**D5 — No per-line React re-render.** `RepoWorkspace.tsx` is 3049 lines; a `setState` per log line
would repaint the whole workspace subtree at CLI output speed. Log lines accumulate in a `useRef`
buffer and flush on a single shared 50 ms timer (`AI_LOG_FLUSH_MS`), capped at 500 retained lines
(`AI_LOG_MAX`, oldest dropped with a `logDropped` counter). Status-changing events
(`started` / `awaitingInput` / `turnEnd` / `done` / `failed` / `cancelled`) flush **immediately** —
they are rare and the UI must react at once. The 1 s elapsed-timer tick exists **only while at least
one run is active** and is cleared when none remain.

**D6 — The 13 existing `RunOpts::default()` call sites keep their 90 s behaviour, untouched.**
`commands/ai.rs:50,88,129,171,211,257,300,344,387,429,475,539` + `history.rs:237`. `run_claude`
keeps its exact signature and its `DEFAULT_TIMEOUT`; the 1 s test override at `mod.rs:564` stays
valid. Streaming is an **additive sibling** (`run_claude_streaming`), not a migration: those are
unrelated features whose latency behaviour must not shift inside a conflict milestone. Migrating the
other six runners is a follow-up TODO, one at a time. `RunLimits` is a **separate parameter**, not a
new `RunOpts` field (see A2).

**D7 — Cancellation is a SECOND command, and every run is reaped.** A Tauri command cannot be
aborted from JS: the `ai_resolve_conflict_stream` promise only settles when the run ends. So
`ai_cancel_run(runId)` flips an `AtomicBool` in a managed registry → the session notices on its next
250 ms tick (`RECV_TICK`) → `kill_child_tree` (whole-tree `taskkill /T /F`, `mod.rs:223`) → `wait()`
→ emit `cancelled` → the original call resolves `Err(AppError::AiCancelled)`, so the frontend has
**one** catch path. On normal completion stdin is dropped, the child is polled for `EXIT_GRACE`
(2 s) and killed if still alive. On app exit the registry's shutdown hook flips every flag **and**
best-effort kills the recorded PIDs — with no hard timeout, a leaked child could otherwise run
forever.

> **AMENDED by P68a implementation (2026-08-17)** — "stdin is dropped" is implemented as **dropping
> the single `WriteTx`**, which makes the writer thread release `ChildStdin`; see D16. Cancel is also
> polled at the top of every loop iteration, not only on the 250 ms tick, so a chatty child (which
> never lets `recv_timeout` expire) still cancels promptly.

**D8 — `runId` arrives on the FIRST channel event (`started`), not as a return value.** This is the
single most easily-missed detail in the milestone: the command promise settles at the *end* of the
run, so there is no other way for the UI to learn the id in time to cancel or reply. The frontend
stores `runId` from `started` and keys `ai_cancel_run` / `ai_reply_run` on it. A `cancelRun` issued
before `started` arrives is queued in the store as `cancelRequested` and fired the moment the id
lands (bounded: `started` is emitted before the child is even spawned).

**D9 — Mid-run questions use a PROMPT-LEVEL sentinel.** Verified dead end: the CLI's `SendMessage`
tool is exposed under `--brief` and *is* enabled by `--tools "SendMessage"`, but in `-p`
main-conversation mode the CLI answers its own tool call and discards an injected `tool_result`
("You are the main conversation…"). So the system prompt carries one line instructing Claude to
reply with a single line beginning `BONSAI_NEEDS_INPUT:` followed by its question.
`post_turn_summary.status_category == "blocked"` / `needs_action` are logged as **corroborating
hints only, never authoritative** — they are undocumented and may change.

**D10 — Read-only tool allowlist; never write, edit or bash.** `--tools "Read,Grep,Glob"` behind
`ai_conflict_tools: 'readOnly' | 'none'` (default `readOnly`; `none` reproduces today's
`--tools ""`). This — not the timeout — is the real fix for item 6: today the conflict run is blind
to the repository, so no deadline increase would let the model "check the whole application".
Verified: the `init` line echoes back exactly the requested subset.

**D11 — Bulk is ONE run for all conflicts, with per-file attribution.** The common case is one
logical change split over several files (the user's i18n JSON), so a single run must see them
together. Guards: a payload byte cap with **split-into-sequential-batches** fallback (never silent
truncation); a file that alone exceeds the cap is marked `failed` and skipped; a path that comes
back missing, empty or markerful is marked `failed` **individually** and never fails the batch;
`--max-budget-usd` is passed when configured.

**D12 — Line interpretation is a PURE, process-free module.** `ai/stream.rs` holds `classify_line`
and nothing else that touches a process; `ai/session.rs` holds lifecycle only. This is what makes
the NDJSON→event mapping unit-testable without spawning anything (mirrors P67's D2: put the
arithmetic where it can be tested). **Unknown line types, unknown subtypes and non-JSON lines
degrade to `kind:'log'` — never an error.**

**D13 — Repo content, prompts payloads and user reply text go through stdin, NEVER argv.** On
Windows `bin` resolves to the npm `claude.cmd` shim and argv reaching a `.cmd` is re-expanded by
cmd.exe (`mod.rs:288-293`); additionally Rust refuses to pass an argument containing a newline to a
batch file (`git/ai_resolve.rs:14-21`), which is why every system prompt is a single line. Every
streaming argv element stays a Bonsai-controlled constant, a vetted model alias, a single-line
system prompt, or a decimal number.

**D14 — The dock is generic over run key, and `AiOutputPanel.tsx` is left untouched.**
`AiOutputPanel.tsx` (140 lines) is a *terminal-state card* (`{title,text,loading,error,costUsd,
editable}` + 5 skeleton rows while loading) with nowhere to put incremental text; the other six
runners keep using it through the single `aiPanel` slot behind `aiPanelReqId`. The dock's props are
keyed by an opaque run key (`conflict:<path>`, `bulk:<n>`, later `analyze:<oid>`) so those runners
can adopt it later without a prop redesign.

**D15 — Mock parity: every event kind and every terminal state is reachable in a plain browser.**
`src/ipc/mock/handlers/aiStream.ts` must be able to produce `started`/`log`/`turnEnd`/
`awaitingInput`/`done`/`failed`/`cancelled`, single and bulk, via `?aiSlow` / `?aiAsk` / `?aiFail`,
and it must honour `?ai=off` (which `mock/handlers/ai.ts:29` ignores today).

**D16 — The session loop thread NEVER blocks on I/O.** *(ADDED by the P68a implementation,
2026-08-17 — the deadlock + unkillable-run fix found in review. Numbering appended, nothing
renumbered.)* Two structural hazards, neither fixable by a comment:

*(a) The pipe-buffer deadlock.* Writing the payload **before** the stdout/stderr reader threads
exist deadlocks as soon as the payload exceeds the OS pipe buffer (~64 KB): the child blocks writing
stdout while we block writing stdin, and nobody drains anything. P68b's ~400 KB bulk payload
(D11/§6.3) hits this every time. **Therefore: readers are spawned BEFORE the first write.**

*(b) The unkillable run.* Even with the readers live, keeping `write_all` on the loop thread means a
child that never drains stdin stops the loop from polling `ctl.cancel` and from running the
watchdog — and streaming has **no wall-clock deadline by design** (D3/D7: the locked user decision
is "no hard timeout + Cancel"). The observed result was a run stuck forever showing only `started`
with a dead Cancel button — precisely the black-box failure P68 exists to eliminate. Verified by
negative control: with the write moved back onto the loop thread the cancellation test fails
deterministically after 20.58 s. **Therefore: a dedicated writer thread owns `ChildStdin`; the
session holds only a `Sender<String>` (`WriteTx`), and `send_write` never blocks.**

*Corollary invariant (stronger than a comment — state it, test it):* **exactly one `WriteTx` value
ever exists.** It is created once, moved into an `Option<WriteTx>`, and **never cloned**. So "drop
stdin" always means "drop the only `WriteTx`", and that drop is the child's EOF signal:
- a one-shot run drops it **immediately after queuing the payload**;
- `complete()` drops it **before** the `EXIT_GRACE` poll, so the child gets EOF and *then* its
  grace, in that order;
- on the cancel / watchdog / hard-cap / max-turns / write-error paths the `WriteTx` may outlive
  `reap()` by a few statements. That is safe: those paths `kill_child_tree` + `wait()` **first**, and
  `child.stdin` was already `take()`n into the writer thread, so `Child::wait()` has no stdin handle
  of its own to drop and cannot deadlock.

*Ordering rule for anyone editing `session.rs::drive`:* `emit(Started)` → `spawn()` → reset
`last_output` → `spawn_reader(stdout)` → `spawn_reader(stderr)` → `spawn_writer(stdin)` → queue the
first turn → enter the loop. **Never move the write earlier.**

---

## 1. Spike findings — RECORDED, do NOT re-verify

Verified against the installed `claude` **v2.1.233** on 2026-08-17. These are inputs to the design,
not open questions. Nobody is asked to re-run them.

1. `-p --output-format stream-json` **requires** `--verbose` (hard error otherwise).
2. Observed NDJSON line order: `system`/`subtype:"init"` (carries `session_id`, `tools[]`, `model`,
   `capabilities[]`) → `rate_limit_event` → `user` (only with `--replay-user-messages`,
   `isReplay:true`) → repeated `system`/`subtype:"thinking_tokens"` heartbeats → `assistant`
   (`message.content[]` of `text` / `tool_use`) → `system`/`subtype:"post_turn_summary"` (carries
   `status_category` `"blocked"`/`"review_ready"` and `needs_action`) → `result`.
3. The `result` line is **byte-compatible** with today's `--output-format json` envelope
   (`is_error`, `result`, `total_cost_usd`, `session_id`, `subtype`), so the existing parse at
   `ai/mod.rs:332-366` is reused **verbatim** — one copy of the is_error / empty / fence-strip logic
   (§3.1 extracts it as `parse_result_envelope`).
4. **A turn ends at the `result` line, NOT at process exit.** With stdin held open the child stayed
   alive and accepted a second turn (one `result` per turn). This is the interactive mechanism.
5. **DEAD END** — `SendMessage` (see D9).
6. `--tools "Read,Grep,Glob"` is a valid allowlist (init echoes the exact subset).
7. Also confirmed available: `--input-format stream-json` (`-p` only), `--include-partial-messages`,
   `--replay-user-messages`, `--max-budget-usd`, `--json-schema`, `--brief`.
8. **Unverified — handle defensively:** whether `result.total_cost_usd` is cumulative across turns
   (observed 0.0238 → 0.0263, so **display the last `result`'s value; never sum within a run**);
   the `--include-partial-messages` delta shape (setting-gated, default **off**; unknown line types
   must degrade to `log`).

### 1a. Sandbox verification — RECORDED (CLI v2.1.234, 2026-08-18)

Required by the security audit's must-fix #1, which treated all three of these as unknowns.
Verified empirically against the installed CLI, non-interactively (`-p`), with a **non-empty**
`--tools` allowlist. These are facts now, not assumptions; do not re-run them.

1. **`--safe-mode` still suppresses the repo's own `CLAUDE.md`, skills and hooks even with a
   non-empty `--tools` allowlist.** A control instruction planted in the repo's `CLAUDE.md` was not
   obeyed under `-p --safe-mode --tools "Read,Grep,Glob" --no-session-persistence`. The pre-P68
   assumption (spike note from v2.1.220) therefore still holds where it now matters most: a hostile
   repo's `CLAUDE.md` is not auto-loaded ahead of Bonsai's own system prompt.
2. **`Read`/`Grep`/`Glob` are NOT fenced to `cwd` by default.** With the same argv the model read a
   file **two directories above `cwd`** and globbed the parent tree, and the `result` line came back
   with `permission_denials: []` — no prompt, no denial. The read grant reaches whatever the Bonsai
   process can reach, not the repository.
3. **`--permission-mode manual` fences them.** With that flag added to the identical argv, an
   in-`cwd` read (`./inside.txt`) still SUCCEEDS while an out-of-`cwd` read is DENIED and recorded
   machine-readably in the `result` line's `permission_denials` array, as
   `{"tool_name":"Read","tool_input":{"file_path":"…"}}`. In non-interactive `-p` there is no human
   to prompt, so out-of-scope requests auto-deny while legitimate in-repo reads keep working.

**Because of (2), `--permission-mode manual` is part of the streaming argv as of P68g-1** (§3.4).
Denials are surfaced: `ai::stream::permission_denial_lines` turns each entry into a `⛔ denied
<tool>(<path>) — outside this repository` dock line, marked `notable` so `ai_stream_log: false`
cannot suppress it (§8.3, M6).

---

## 2. Module boundaries

### 2a. Rust — `crates/bonsai-core`

| File | New/edit | Target | Responsibility |
|---|---|---|---|
| `src/ai/stream.rs` | NEW | ~280 | **Pure** (D12). Raw NDJSON structs, `classify_line`, `sentinel_question`, `AiRunEvent`/`AiRunEventKind` wire types, `StreamLogItem`. No `Command`, no threads, no I/O. |
| `src/ai/session.rs` | NEW | ~320 | `ClaudeSession` lifecycle: argv assembly, spawn, line-reader threads + mpsc, stdin held open, 250 ms tick loop, idle watchdog, hard cap, cancel poll, reply injection, turn accounting, reap. |
| `src/ai/registry.rs` | NEW | ~170 | `AiRunRegistry` (Clone handle over `Arc<Mutex<HashMap<String, AiRunHandle>>>`), `RunControl`, id minting, `cancel` / `reply` / `finish` / `cancel_all` / `active`. |
| `src/ai/mod.rs` | EDIT | 640 → ~715 | **Only**: `RunLimits` + `ToolPolicy`, `run_claude_streaming`, the `parse_result_envelope` extraction (pure move out of `run_claude`), `kill_pid_tree`, `mod stream/session/registry` + re-exports. Nothing else — do not grow it further. |
| `src/git/ai_resolve.rs` | EDIT | 200 → ~245 | Extract `pub(crate) read_conflict_sides` + `ConflictSides` (the stage 1/2/3 read + guards at L64-113) so it exists once. `ai_resolve_conflict` keeps its signature and behaviour **verbatim**. |
| `src/git/ai_resolve_stream.rs` | NEW | ~300 | Payload builders (`build_single_payload`, `build_bulk_payload`), the two single-line system prompts, `parse_bulk_response` (pure), batch packing (`pack_batches`), and `resolve_conflicts_streaming` (the orchestrator that calls `run_claude_streaming` once per batch). |
| `tests/fixtures/claude_stub.{cmd,sh}` | EDIT | — | New NDJSON modes (§9, P68a). |

> **SUPERSEDED by P68a implementation (2026-08-17)** — the two `session.rs` cells above are stale:
> `session.rs` did **not** stay one file (it would have been a ~770-line god-module), and it no longer
> does argv assembly or pipe plumbing. The landed P68a layout, with real line counts:
>
> | File | Lines | Responsibility (as landed) |
> |---|---|---|
> | `src/ai/stream.rs` | 514 (incl. its `#[cfg(test)]` block) | As specified, **plus** `StreamLogItem.assistant_text`, `truncate_text`, `MAX_PARTIAL_TEXT`, `MAX_TOOL_TEXT` (see the §3.2 amendment). Still pure (D12). |
> | `src/ai/session.rs` | **455** | The state machine **only**: the 250 ms tick loop, turn accounting, idle watchdog + hard cap, cancel poll, reply pump, the bounded post-EOF stderr drain, and the exit paths (`complete` / `cancel` / `fail` / `reap`). Declares `RunControl` (public) and the **private** `ClaudeSession`. |
> | `src/ai/session_argv.rs` | **201** | **NEW — not in the original table.** `pub(super) fn build_command(cwd, prompt, opts, limits) -> Command`: the LOCKED §3.4 argv, plus 8 pure argv tests including `argv_never_contains_a_newline` (the executable form of D13). Split out so the flag set is asserted without spawning anything. |
> | `src/ai/session_pipes.rs` | **118** | **NEW — not in the original table.** The three I/O threads and the single mpsc funnel: `Msg` (`Out`/`Err`/`OutEof`/`ErrEof`/`WriteErr`), `WriteTx`, `turn_line`, `send_write`, `spawn_writer`, `spawn_reader`. Interprets nothing and decides nothing — this is where D16 lives. |
> | `src/ai/registry.rs` | 185 | As specified, plus `is_awaiting` (§4.2 amendment). |
> | `src/ai/mod.rs` | 640 → **645** (not ~715) | As specified, plus `DEFAULT_MAX_TURNS`. It barely grew because its inline `#[cfg(test)] mod tests` was extracted to `ai/tests.rs` (245) + `ai/testutil.rs` (101). The old inline refs `mod.rs:564` / `mod.rs:577` now live in `ai/tests.rs`; the **test paths `ai::tests::…` are unchanged**, so §9/§10.1 name them correctly. |
> | `src/ai/session_tests.rs` (489), `src/ai/session_io_tests.rs` (128), `src/ai/session_drain_tests.rs` (102), `src/ai/tests.rs` (245), `src/ai/testutil.rs` (101) | — | `#[cfg(test)]` modules, split per the ~500-line rule. `session_drain_tests.rs` is `#[path]`-included **as a child of `session`** so it can reach `Msg` / `ended_without_result` without widening either's visibility. |
> | `src/ai/payload.rs` | 397 | **Pre-existing (P15), untouched by P68.** Listed only so nobody mistakes it for §6.1's bulk payload builder — that is still P68b's `git/ai_resolve_stream.rs`. |
>
> `git/ai_resolve.rs`, `git/ai_resolve_stream.rs` and the stub rows are unchanged (P68b/P68a scope as
> written).

### 2b. Rust — `src-tauri`

| File | New/edit | Change |
|---|---|---|
| `src/commands/ai_stream.rs` | NEW (~230) | The 3 `#[command]` / `_inner` triples. `ai.rs` is 543 lines — do not grow it. |
| `src/commands/mod.rs` | EDIT | `mod ai_stream; pub use ai_stream::*;` |
| `src/commands/shared.rs` | EDIT | Re-export the names the command layer *names*: `AiRunEvent`, `AiRunEventKind`, `AiResolveBatch`, `AiResolveFailure`, `AiRunRegistry`, `AiConflictTools`. |
| `src/lib.rs` | EDIT | `.manage(bonsai_core::ai::AiRunRegistry::default())` beside L20-21; 3 entries in `generate_handler!`; `RunEvent::ExitRequested` also calls `registry.cancel_all()` beside `mcp::shutdown` (L232-238). |
| `src/settings.rs` | EDIT | 10 additive `#[serde(default)]` fields + `AiConflictTools` enum + `clamp_ai_settings` (§8.3). |
| `src/commands/ui_settings.rs` | EDIT | The same 10 fields on `UiSettings` + `Option<_>` on `UiSettingsPatch` + 10 `apply_patch` arms + **both** builder literals (`get_ui_settings`, `set_ui_settings`). |
| `crates/bonsai-core/src/error.rs` | EDIT | `AiCancelled(String)` variant + `kind()` arm + `message()` `|`-chain arm. |

### 2c. Frontend

| File | New/edit | Target | Responsibility |
|---|---|---|---|
| `src/ipc/types.ts` | EDIT | +~90 | `AiRunEvent`, `AiRunEventKind`, `AiResolveBatch`, `AiResolveFailure`, `AiConflictTools`, `'aiCancelled'` in the `AppError.kind` union, 10 `UiSettings`/`UiSettingsPatch` fields, 3 `IpcApi` methods. |
| `src/ipc/tauri.ts` | EDIT | +~25 | 3 wrappers; `aiResolveConflictStream` builds `new Channel<AiRunEvent>()` (copy `historyIndexBuild` L539-548). |
| `src/ipc/index.ts` | EDIT | +~6 | Re-export the new type names. |
| `src/ipc/mock/handlers/aiStream.ts` | NEW | ~260 | `aiStreamHandlers` (§8.5). `mock/handlers/ai.ts` is 485 lines — new file, spread into `mockIpc` in `src/ipc/mock.ts`. |
| `src/ipc/mock/persistence.ts` | EDIT | +~24 | Defaults + tolerant parse for the 10 settings. |
| `src/ipc/mock/handlers/session.ts` | EDIT | +10 | Patch-merge lines. |
| `src/components/repoWorkspace/useAiRuns.ts` | NEW | ~300 | The per-path run store (§5). Owns the buffered log flush, the elapsed tick, cancel/reply, autonomy routing. |
| `src/components/repoWorkspace/useBulkAiResolve.ts` | NEW | ~140 | Bulk entry points + eligibility (§6.4). Thin — the single-run path does the work. |
| `src/components/AiActivityPanel.tsx` | NEW | ~210 | The dock shell: header (status pill, label, elapsed, cost, Cancel, collapse), body switch, reply box. |
| `src/components/AiActivityLog.tsx` | NEW | ~110 | The log body: capped lines, monospace, stick-to-bottom unless the user scrolled up. |
| `src/components/AiRunQueue.tsx` | NEW | ~150 | Per-file rows for a bulk run (path, per-file status, Review). |
| `src/components/SettingsAiRunSection.tsx` | NEW | ~160 | The new AI-run settings block, used from `SettingsPanel.tsx` after the autonomy fieldset (L367). |
| `src/components/StatusConflictsSection.tsx` | EDIT | 167 → ~215 | `aiResolvingPath` prop → `aiRows` map + `aiBulkBusy`; per-row status affordance; "Resolve all with AI" in the section header (L145-147). |
| `src/components/repoWorkspace/useMergeActions.ts` | EDIT | 219 → ~185 | **Delete `handleAiResolveConflict` (L111-169)** and the `setAiResolvingPath` / `aiConflictAutonomy` deps; **add `openAiProposal(path, proposedText)`**. Keeps `fileDiffReqId` for its own guard. |
| `src/components/RepoWorkspace.tsx` | EDIT | 3049 → ~3080 | Delete `aiResolvingPath` (L243); wire `useAiRuns`; render `<AiActivityPanel>` as the element immediately after `</div>` of `.panes` (L2648…); pass `aiRows` down. |
| `src/components/OpBanner.tsx` | EDIT | +~14 | "Resolve all with AI" in the merge arm's actions row (L140-157), gated + disabled while a bulk run is active. |
| `src/App.tsx` | EDIT | +~8 | `aiDockHeight` / `aiDockCollapsed` state, load, patch line, prop passes. |
| `src/styles.css` | EDIT | +~130 | `.ai-dock*`, `.ai-log*`, `.ai-run-queue*`, `.conflict-action-ai[data-state]`. |
| `src/components/AiOutputPanel.tsx` | **UNTOUCHED** | 140 | D14. |

---

## 3. §A — The streaming runner (Rust)

### 3.1 `ai/mod.rs` additions (and the one extraction)

```rust
/// P68 §A: default idle-output watchdog. A run is killed only after this long
/// with NO output from the child; `Duration::ZERO` disables it. Replaces the
/// wall-clock deadline for STREAMING runs only — `DEFAULT_TIMEOUT` (90 s) still
/// governs every `run_claude` caller (D6).
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(300);
/// P68 §A: how long the session loop blocks on `recv_timeout` before it polls
/// the cancel flag / the watchdog / the reply channel. Bounds cancel latency.
pub const RECV_TICK: Duration = Duration::from_millis(250);
/// P68 §A: grace period between dropping stdin and force-killing the child on a
/// COMPLETED run (the child normally exits on stdin EOF).
pub const EXIT_GRACE: Duration = Duration::from_secs(2);

/// P68 §A/D10: which CLI tools a streaming run may use. `ReadOnly` is the
/// conflict default — the model must be able to look at the rest of the repo
/// (item 6), but NEVER write, edit or run a shell. `None` reproduces today's
/// `--tools ""`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPolicy { ReadOnly, None }

impl ToolPolicy {
    /// The exact `--tools` argument value. `ReadOnly` => "Read,Grep,Glob"
    /// (verified allowlist, spike §1.6); `None` => "".
    pub fn arg(self) -> &'static str;
}

/// P68 §A: limits for ONE streaming run. Deliberately a separate parameter
/// rather than a `RunOpts` field (D6/A2) so the 13 `RunOpts::default()` sites
/// are not touched at all.
#[derive(Debug, Clone)]
pub struct RunLimits {
    /// Kill after this long with no child output. `Duration::ZERO` = disabled.
    /// PAUSED while awaiting user input (D3).
    pub idle_timeout: Duration,
    /// Optional absolute cap. `None` = unbounded (the user's locked default).
    /// Also paused while awaiting user input (D3).
    pub hard_cap: Option<Duration>,
    /// Max `result` lines (turns) per run before a still-questioning model is
    /// failed. >= 1.
    pub max_turns: u32,
    /// Tool allowlist (D10).
    pub tools: ToolPolicy,
    /// `--max-budget-usd` when `Some`; omitted when `None`.
    pub max_budget_usd: Option<f64>,
    /// `--include-partial-messages`. Default false; unknown delta shapes must
    /// degrade to `log` (spike §1.8).
    pub include_partial_messages: bool,
    /// Feed the first turn as a stream-json user message on an OPEN stdin so a
    /// second turn is possible (the interactive mechanism, spike §1.4). false =
    /// one-shot: positional prompt + payload on stdin, then EOF.
    pub interactive: bool,
}

impl Default for RunLimits {
    /// idle 300 s, no hard cap, 6 turns, ReadOnly, no budget, no partials,
    /// interactive = true.
    fn default() -> Self;
}

/// P68 §A: the single streaming entry point. BLOCKING — callers invoke under
/// `spawn_blocking`. `opts.model` and `opts.system_prompt` are honoured;
/// **`opts.timeout` is IGNORED** (streaming is governed by `limits`) — this is
/// documented rather than removed so the caller can pass a `RunOpts` it already
/// has. Emits every event through `on_event` (seq starts at 0 with `started`)
/// and returns the LAST turn's parsed result.
///
/// Errors: `AiUnavailable` (spawn/NotFound) | `AiFailed` (protocol, watchdog,
/// hard cap, turn budget, unparseable/`is_error` result) | `AiCancelled`.
/// On EVERY error path the events already emitted stand (D2).
pub fn run_claude_streaming(
    cwd: &Path,
    /// Positional prompt for one-shot mode; prepended to the stdin user message
    /// in interactive mode (D13 — never argv in interactive mode).
    prompt: &str,
    payload: &str,
    opts: RunOpts,
    limits: RunLimits,
    ctl: RunControl,
    on_event: &(dyn Fn(AiRunEvent) + Send + Sync),
) -> Result<AiResult, AppError>;

/// P68 §A: EXTRACTED VERBATIM from `run_claude` (`mod.rs:332-366`) so the
/// streaming `result` line and the one-shot envelope share ONE copy of the
/// is_error / empty / fence-strip logic (spike §1.3). Behaviour is unchanged:
/// (1) unparseable + non-zero exit -> stderr tail capped at 500 chars;
/// (2) unparseable + zero exit -> "could not parse Claude output";
/// (3) `is_error` -> result|subtype; (4) empty/blank result -> "no output";
/// (5) success -> `strip_fence`.
pub(crate) fn parse_result_envelope(
    stdout: &str,
    success: bool,
    stderr: &str,
) -> Result<AiResult, AppError>;

/// P68 §A/D7: kill a process TREE by pid (the app-exit path, where no `Child`
/// handle survives). Windows: `taskkill /T /F /PID` with `CREATE_NO_WINDOW`;
/// elsewhere: best-effort `kill -9 <pid>`. Best-effort by design — never panics,
/// never blocks longer than the spawn.
pub(crate) fn kill_pid_tree(pid: u32);
```

> **AMENDED by P68a implementation (2026-08-17)** — the const list as landed:
> `DEFAULT_IDLE_TIMEOUT` (`mod.rs:59`), `RECV_TICK` (`:62`), `EXIT_GRACE` (`:65`) and one the
> contract did not name:
> ```rust
> /// P68 §A: turn budget for a streaming run — the settings default
> /// (`ai_max_turns`, §8.3) and `RunLimits::default()` share this ONE number
> /// instead of repeating the literal 6.
> pub const DEFAULT_MAX_TURNS: u32 = 6;
> ```
> `session.rs` additionally owns three **private** consts that the contract left implicit:
> `EXIT_POLL = 50 ms` (the `try_wait` poll interval inside `EXIT_GRACE`) and
> `STDERR_GRACE = 150 ms` / `STDERR_GRACE_TOTAL = 1 s` (the bounded post-EOF stderr drain — see the
> §3.3 amendment). `REPLY_SUFFIX` also lives there (`session.rs:32`), verbatim as specified below.

`run_claude` changes by exactly two things: its parse block becomes a call to
`parse_result_envelope(stdout_str.trim(), output.success, &stderr_str)`, and nothing else. **Its
signature, argv and 90 s default are untouched (D6).**

### 3.2 `ai/stream.rs` — the wire type and the mapping table (PURE)

```rust
/// P68 §F: one push event on the `ai_resolve_conflict_stream` channel. Compact
/// by design (D1) — no libgit2 objects, no per-commit round-trips, at most one
/// line of text. Serialized camelCase; mirrored in TS.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRunEvent {
    /// Stable for the whole run. FIRST delivered on the `Started` event (D8).
    pub run_id: String,
    /// Monotonic from 0, one sequence per run. The frontend drops any event
    /// whose seq <= the last seen (stale/duplicate guard).
    pub seq: u64,
    pub kind: AiRunEventKind,
    /// One log line, the question text, or the terminal message. Never the whole
    /// payload; hard-truncated to 2000 chars per event.
    pub text: Option<String>,
    /// `total_cost_usd` of the turn that just ended (`TurnEnd`) or of the run
    /// (`Done`). LAST value wins — never summed within a run (spike §1.8).
    pub cost_usd: Option<f64>,
    /// Since the run started (not since the turn).
    pub elapsed_ms: u64,
    /// The file this event is about, when known (bulk attribution). `None` for
    /// run-level events.
    pub path: Option<String>,
    /// 1-based turn counter; 0 on `Started`.
    pub turn: u32,
    /// Only on `Cancelled` / `Failed`: the assistant text accumulated so far
    /// (D2). Display-only — NEVER offered as a proposal.
    pub partial_text: Option<String>,
}

/// P68 §F: exactly seven kinds — locked by the approved plan. New line types do
/// NOT add kinds; they map onto `Log` (D12/A3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AiRunEventKind {
    /// Always first, seq 0, emitted BEFORE the child is spawned so the UI has
    /// the runId even if the spawn fails.
    Started,
    /// One human-readable line for the dock. High frequency -> batched (D5).
    Log,
    /// A `result` line arrived and parsed; the run may continue (another turn).
    TurnEnd,
    /// The sentinel was seen; the session is blocked on `ai_reply_run`. The
    /// watchdog is paused (D3).
    AwaitingInput,
    /// Terminal: success. Emitted immediately before the command resolves Ok.
    Done,
    /// Terminal: `text` is the same message as the returned `AiFailed`.
    Failed,
    /// Terminal: user cancel. The command resolves `Err(AiCancelled)`.
    Cancelled,
}

/// One log-ish item produced by classification. `path` is set only when the
/// caller (bulk) knows it.
pub struct StreamLogItem { pub text: String }

/// What one NDJSON line means to the session loop.
pub enum LineOutcome {
    /// Emit these as `Log` events, in order.
    Log(Vec<StreamLogItem>),
    /// This is a `result` line: the session re-parses the RAW line through
    /// `parse_result_envelope` (spike §1.3) and does turn accounting.
    Result,
    /// A heartbeat: it RESETS the idle watchdog but produces no event (A4).
    Heartbeat,
}

/// P68 §A (PURE, D12): classify one NDJSON line. NEVER returns an error —
/// unknown `type`, unknown `subtype`, and non-JSON input all degrade to
/// `Log` with the raw line truncated (D12).
pub fn classify_line(raw: &str) -> LineOutcome;

/// P68 §B: the sentinel. Recognised ONLY when the FIRST non-empty line of the
/// (already fence-stripped) result text starts with `BONSAI_NEEDS_INPUT:`
/// (A9 — a merged file body whose first line is that token is impossible in
/// practice, whereas a body that merely mentions the token mid-text is not a
/// question). Returns the trimmed remainder of that line.
pub fn sentinel_question(text: &str) -> Option<String>;

pub const SENTINEL: &str = "BONSAI_NEEDS_INPUT:";
/// Per-event text cap (chars, not bytes — never split a char boundary).
pub const MAX_EVENT_TEXT: usize = 2000;
```

> **SUPERSEDED by P68a implementation (2026-08-17)** — `StreamLogItem` carries a **second field**.
> The bare `{ text }` shape above cannot express D2/§3.3's "partial accumulates assistant text
> only": once a line has been rendered to a display string, the session can no longer tell assistant
> prose from `⚙ Read(...)`, `system/init` or a `stderr: ` line, so it would either fold decoration
> into `partialText` or need a fragile prefix sniff. Classification is the only place that knows.
> ```rust
> pub struct StreamLogItem {
>     pub text: String,
>     /// True ONLY for `assistant`/`text` content blocks. The session accumulates
>     /// only these into `partial`, so `partialText` stays a plausible truncated
>     /// FILE BODY rather than decoration (D2/A5).
>     pub assistant_text: bool,
> }
> impl StreamLogItem {
>     fn log(text: &str) -> Self;        // decoration -> assistant_text = false
>     fn assistant(text: &str) -> Self;  // real prose -> assistant_text = true
> }
> ```
> `--include-partial-messages` deltas deliberately set `assistant_text: false`: the same text is
> re-sent in the final `assistant` line, so counting the deltas would double the body.
>
> **Consequence, stated once (mirrored in §11.5): `partialText` is a LOSSY echo by construction** —
> every block was already capped at `MAX_EVENT_TEXT` on the way in, partial-message deltas are
> excluded on purpose, and the accumulation is capped again at `MAX_PARTIAL_TEXT`. The **dock log**
> (i.e. every `Log` event) — not `partialText` — is the complete record.
>
> Also landed in `stream.rs`, unspecified before:
> - `pub const MAX_PARTIAL_TEXT: usize = 20_000;` (the `partial` accumulation cap) and
>   `MAX_TOOL_TEXT: usize = 160` (the `⚙` cap the mapping table already required in prose).
> - `pub(crate) fn truncate_text(text: &str, cap: usize) -> String` — char-wise (never bytes: a split
>   char boundary corrupts UTF-8 on the wire), appends `…`, result is exactly `cap` chars.
>   **`truncate_text(x, 0)` returns `""`** — the `…` would otherwise be a 1-char overflow.
> - **`LineOutcome::Log(vec![])` is legal and expected:** an `assistant` line whose `content` array
>   is missing or empty produces **no events and never an error** (D12). Callers must not assume
>   `Log` is non-empty.
> - The mapping table's `system/<subtype>` and `assistant/<type>` rows each have a `None` sibling: a
>   **missing** `subtype`/`type` renders `system/?` / `assistant/?` — still `Log`, still never an
>   error.

Mapping table — **normative**:

| NDJSON line | Outcome | Emitted text |
|---|---|---|
| `type:"system"`, `subtype:"init"` | `Log` | `session <session_id> · model <model> · tools: <tools.join(", ")>` (empty `tools` ⇒ `tools: none`) |
| `type:"system"`, `subtype:"thinking_tokens"` | **`Heartbeat(Some(estimated_tokens))`** | *(no log line — A4; a **metrics-only** event, see the P68d amendment below)* |
| `type:"system"`, `subtype:"post_turn_summary"` | `Log` | `summary: status=<status_category ?? "?"> needsAction=<needs_action ?? false>` — **hint only, never authoritative (D9)** |
| `type:"system"`, any other subtype | `Log` | `system/<subtype>` |
| `type:"rate_limit_event"` | `Log` | `rate limit: <compact re-serialization, ≤200 chars>` |
| `type:"user"` (replay) | `Log` | `» sent <n> bytes to Claude` — **never the content** (A11: `--replay-user-messages` would otherwise dump the whole ≤400 KB payload into the log) |
| `type:"assistant"`, content item `type:"text"` | `Log` (one item per content block) | the block's `text`, truncated |
| `type:"assistant"`, content item `type:"tool_use"` | `Log` | `⚙ <name>(<first string field of input, or "">)`, truncated to 160 chars (A3 — no new event kind) |
| `type:"assistant"`, other content item | `Log` | `assistant/<type>` |
| `type:"result"` | **`Result`** | *(the session re-parses the raw line)* |
| `type:"stream_event"` / any `--include-partial-messages` shape | `Log` if a `text` delta can be found, else `Heartbeat` | the delta, truncated |
| unknown `type` | `Log` | the raw line, truncated |
| non-JSON / blank line | `Log` (blank ⇒ `Heartbeat`) | the raw line, truncated |

stderr lines are not classified: the session emits them as `Log` with a `stderr: ` prefix and keeps
the last 2000 chars as `stderr_tail` for the failure message.

> **AMENDED by P68d (2026-08-17) — the LIVE TOKEN COUNT. Payload VERIFIED, not assumed.**
>
> *Why:* `cost_usd` only exists at a turn boundary (`TurnEnd`/`Done`), so a long single-turn run
> shows `$—` for minutes. The user accepted "no spend cap" **because** spend would be visible, so
> that gap is part of the safety story, not cosmetics (P68e §12-B1). The agreed remedy was a live
> token count *if* the heartbeat actually carries one.
>
> *What the payload actually contains* — run against the installed `claude` **v2.1.233**,
> `-p --verbose --output-format stream-json`, with a prompt that forces extended thinking:
> ```json
> {"type":"system","subtype":"thinking_tokens","estimated_tokens":350,
>  "estimated_tokens_delta":150,"uuid":"…","session_id":"…"}
> ```
> Five such lines in one run: `estimated_tokens` 100 → 200 → 350 → 450 → 600, i.e. **cumulative and
> monotonic**, roughly one line per few seconds. The run's real
> `usage.output_tokens_details.thinking_tokens` on the `result` line was **679**, so the estimate is
> a good live approximation of the final figure.
>
> *Scope limits — state them, do not paper over them:*
> - It is **THINKING tokens only, and estimated**. It is not a total-token count.
> - A run that never enters extended thinking emits **no heartbeats at all** (verified: a trivial
>   prompt produced zero), so the readout legitimately stays absent for short runs.
> - The `assistant` line's `usage` is **NOT** a usable alternative: mid-run it reports
>   `output_tokens: 2` (a placeholder). The real 1398 only appears on `result`.
> - It is **never converted to money.** No price table exists anywhere in Bonsai, and inventing one
>   would be worse than showing nothing (P68e §12-B1 option (c), rejected).
>
> *Wire change (additive):* `AiRunEvent` gains `thinking_tokens: Option<u64>` → `thinkingTokens` in
> TS. `LineOutcome::Heartbeat` becomes `Heartbeat(Option<u64>)`; a missing, non-integer or negative
> `estimated_tokens` degrades to `None` (D12 — never an error).
>
> *Delivery, without breaking the locked 7-kind union:* the session emits a **metrics-only event** —
> `kind: Log` with **`text: None`** and `thinkingTokens: Some(n)`. So:
> - **`text` and `thinkingTokens` are mutually exclusive on a `log` event** (asserted in
>   `ai::session_tests`), which is how a consumer tells the two apart;
> - the dock gets no extra noise (A4 stands: heartbeats never become log lines);
> - `AiRunEventKind` still has exactly seven variants.
>
> A metrics event **bypasses the `ai_stream_log` setting** in `RunEvents::forward`: that switch
> suppresses log *noise*, and silencing the spend readout with it would remove the thing that made
> "no spend cap" acceptable.
>
> Frontend: `useAiRuns` stores it as `AiRunState.thinkingTokens` (last value wins), buffered on the
> same 50 ms flush as log lines (D5). The mock emits one heartbeat every third tick so the harness
> can see it climb.

### 3.3 `ai/session.rs` — `ClaudeSession` lifecycle

```rust
/// P68 §A: one streaming CLI run. Owns the child, two reader threads, the mpsc
/// funnel and the turn state machine. Lifecycle only — all line interpretation
/// lives in `stream.rs` (D12).
pub struct ClaudeSession { /* child, rx, stdin, seq, started, last_output, turn, awaiting, partial */ }

/// Cancel + reply plumbing handed to a session by the registry.
pub struct RunControl {
    pub run_id: String,
    pub cancel: Arc<AtomicBool>,
    /// Set by the session so `ai_reply_run` can reject a reply for a run that is
    /// not awaiting input, and so the UI can show the right affordance.
    pub awaiting: Arc<AtomicBool>,
    /// Set by the session right after spawn so the app-exit hook can kill an
    /// orphan (D7). 0 = not spawned.
    pub pid: Arc<AtomicU32>,
    pub replies: std::sync::mpsc::Receiver<String>,
}
```

> **SUPERSEDED by P68a implementation (2026-08-17)** — `ClaudeSession` is **private** and has **no
> `stdin` field**; `ChildStdin` is owned by the writer thread (D16), and the child + pipes are locals
> of `drive` rather than fields, so no exit path can forget them.
> ```rust
> /// Private on purpose: `run_claude_streaming` (= `session::run`) and `RunControl`
> /// are the ONLY public surface, so a session cannot be half-driven from outside.
> struct ClaudeSession<'a> {
>     ctl: RunControl,
>     on_event: &'a (dyn Fn(AiRunEvent) + Send + Sync),
>     seq: u64,
>     started: Instant,
>     last_output: Instant,
>     turn: u32,
>     awaiting: bool,
>     /// Assistant prose ONLY (D2/A5) — fed by `StreamLogItem.assistant_text`.
>     partial: String,
>     stderr_tail: String,
> }
>
> /// Blocking. The only way to drive a session.
> pub(crate) fn run(
>     cwd: &Path, prompt: &str, payload: &str,
>     opts: RunOpts, limits: RunLimits, ctl: RunControl,
>     on_event: &(dyn Fn(AiRunEvent) + Send + Sync),
> ) -> Result<AiResult, AppError>;   // = `super::run_claude_streaming`
> ```
> `RunControl` is exactly as specified above (unchanged). New in `ai/session_pipes.rs` — the D16
> plumbing:
> ```rust
> /// Everything that reaches the loop. Two reader threads AND the writer thread
> /// report on ONE mpsc, so stdout/stderr interleave in real time and a failed
> /// write is reported without blocking the loop.
> pub(super) enum Msg { Out(String), Err(String), OutEof, ErrEof, WriteErr(String) }
>
> /// The session's ONLY handle on stdin. Exactly one value ever exists — created
> /// once, moved into an `Option<WriteTx>`, NEVER cloned; dropping it is the
> /// child's EOF (D16).
> pub(super) type WriteTx = std::sync::mpsc::Sender<String>;
>
> pub(super) fn turn_line(text: &str) -> String;   // one NDJSON `user` line + '\n'
> /// Queue a turn. NEVER blocks. `Err` only when the writer thread is already gone
> /// (then `*writer = None`, so the state stays honest); a real `io::Error` from
> /// `write_all` arrives later as `Msg::WriteErr`.
> pub(super) fn send_write(writer: &mut Option<WriteTx>, text: String) -> std::io::Result<()>;
> pub(super) fn spawn_writer(stdin: Option<ChildStdin>, reqs: Receiver<String>, tx: Sender<Msg>);
> pub(super) fn spawn_reader<R: Read + Send + 'static>(
>     src: Option<R>, tx: Sender<Msg>, wrap: fn(String) -> Msg, eof: Msg);
> ```
> `Msg::WriteErr` is a **new message kind on the same funnel** and it exists *because* the loop is no
> longer the thread that writes: a fatal stdin failure has to arrive as a message like everything
> else.

Loop — **normative pseudocode**:

```
emit(Started { run_id, seq 0, turn 0 })            // BEFORE spawn (D8)

cmd = build_argv(bin, cwd, opts, limits)           // §3.4
child = spawn(cmd)                                 // NotFound -> AiUnavailable (emit Failed first)
ctl.pid.store(child.id())

if limits.interactive:
    write_turn(stdin, prompt + "\n\n" + payload)   // ONE NDJSON line, stdin STAYS OPEN
else:
    write_all(stdin, payload); drop(stdin)         // EOF, one-shot

spawn reader(stdout) -> tx.send(Out(line)) per BufReader::lines(), then tx.send(OutEof)
spawn reader(stderr) -> tx.send(Err(line)) ...,    then tx.send(ErrEof)

started      = Instant::now()
last_output  = started
turn         = 0
awaiting     = false
partial      = String::new()          // assistant text only (D2)
stderr_tail  = String::new()

loop {
    match rx.recv_timeout(RECV_TICK) {

      Ok(Out(line)) => {
          last_output = Instant::now()
          match classify_line(&line) {
            Heartbeat  => {}                                   // watchdog reset only (A4)
            Log(items) => for it in items { partial.push_str(&it.text); emit(Log{ text: it.text }) }
            Result     => {
                turn += 1
                res = parse_result_envelope(&line, true, &stderr_tail)?   // Err -> FAIL path
                emit(TurnEnd { cost_usd: res.cost_usd, turn })
                match sentinel_question(&res.text) {
                  None    => { final = res; break OK }
                  Some(q) => {
                      if !limits.interactive        -> FAIL("Claude needs more information but the run is not interactive")
                      if turn >= limits.max_turns   -> FAIL("Claude asked <turn> questions without producing a resolution")
                      awaiting = true; ctl.awaiting.store(true)
                      emit(AwaitingInput { text: q, turn })
                  }
                }
            }
          }
      }

      Ok(Err(line)) => { last_output = now(); stderr_tail.push_line(&line)
                         emit(Log { text: "stderr: " + line }) }

      Ok(OutEof) | Ok(ErrEof) => { eofs += 1
                         // stdout EOF before any `result` == the child died mid-turn
                         if eofs_includes_stdout && final.is_none() -> FAIL(stderr_tail or "Claude exited without a result") }

      Err(Timeout) => {
          if ctl.cancel.load()                       -> CANCEL path
          if awaiting {
              if let Ok(text) = ctl.replies.try_recv() {
                  write_turn(stdin, text + REPLY_SUFFIX)      // stdin, NEVER argv (D13)
                  awaiting = false; ctl.awaiting.store(false)
                  last_output = now()
                  emit(Log { text: "» answered (" + text.len() + " bytes)" })
              }
              // D3: NEITHER the idle watchdog NOR the hard cap is consulted here
          } else {
              if limits.idle_timeout != ZERO && now() - last_output > limits.idle_timeout
                                                      -> FAIL_WATCHDOG("Claude produced no output for <n>s — stopped")
              if let Some(cap) = limits.hard_cap { if now() - started > cap
                                                      -> FAIL_WATCHDOG("Claude exceeded the <n>s cap — stopped") }
          }
      }

      Err(Disconnected) => -> FAIL("Claude output stream closed unexpectedly")
    }
}

// ---- exits ----
OK:            drop(stdin); poll try_wait up to EXIT_GRACE; if alive kill_child_tree; wait()
               emit(Done { cost_usd: final.cost_usd, turn }); return Ok(final)
CANCEL:        kill_child_tree(&mut child); wait()
               emit(Cancelled { text: "cancelled", partial_text: Some(partial) })
               return Err(AiCancelled("cancelled by user"))
FAIL(msg):     kill_child_tree(&mut child); wait()
               emit(Failed { text: msg, partial_text: Some(partial) })
               return Err(AiFailed(msg))
FAIL_WATCHDOG: identical to FAIL — the distinction is the message only.
```

> **SUPERSEDED by P68a implementation (2026-08-17)** — the **prologue ordering** and the EOF /
> tick / exit arms below replace their counterparts above; the `Ok(Out(line))` arm is as landed.
>
> **⚠ Do not "restore" the original order.** Writing the first turn before the readers exist
> deadlocks on the OS pipe buffer as soon as the payload exceeds ~64 KB — the child blocks writing
> stdout while we block writing stdin (guaranteed for P68b's ~400 KB bulk payload) — and writing on
> the loop thread makes a stdin-refusing child **unkillable**, because `ctl.cancel` and the watchdog
> are never polled and streaming has no wall-clock deadline by design. Both are D16; the second was
> verified by negative control (cancellation test fails deterministically after 20.58 s).
>
> ```
> emit(Started { run_id, seq 0, turn 0 })            // BEFORE spawn (D8)
>
> cmd   = build_command(cwd, prompt, opts, limits)   // ai/session_argv.rs, §3.4
> child = cmd.spawn()                                // NotFound -> emit Failed, Err(AiUnavailable)
> ctl.pid.store(child.id())
>
> last_output = Instant::now()   // RESET HERE, not in new(). Process creation (cmd.exe + the npm
>                                // shim + node) is not "the child being silent", and charging it to
>                                // the watchdog made a 1 s limit fire on startup alone under load.
>                                // `started` is deliberately NOT touched, so `elapsedMs` stays
>                                // user-perceived. This is what makes §10.1's idle_timeout = 1s
>                                // test achievable — it had to be widened to 2 s before the fix.
>                                // A CLI that never says anything is still reaped, from this instant.
>
> // ---- 1. READERS FIRST (D16a) ----
> (tx, rx) = channel::<Msg>()
> spawn_reader(child.stdout.take(), tx.clone(), Msg::Out, Msg::OutEof)
> spawn_reader(child.stderr.take(), tx.clone(), Msg::Err, Msg::ErrEof)
>
> // ---- 2. THEN the writer thread, which OWNS ChildStdin (D16b) ----
> (wtx, wrx) = channel::<String>()
> spawn_writer(child.stdin.take(), wrx, tx)      // `tx` MOVES here: the last funnel sender
> writer: Option<WriteTx> = Some(wtx)            // the one and only WriteTx — never cloned
>
> // ---- 3. ONLY NOW queue the first turn (never blocks) ----
> first = if limits.interactive { turn_line(prompt + "\n\n" + payload) }   // stdin STAYS OPEN
>         else                  { payload }                                // raw bytes, one-shot
> if send_write(&mut writer, first).is_err() -> FAIL("Claude closed its input: <e>")
> if !limits.interactive { writer = None }       // drop the ONLY WriteTx -> the writer thread
>                                                // releases ChildStdin -> EOF for a one-shot run
>
> loop {
>     if ctl.cancel.load() -> CANCEL             // checked EVERY iteration, not only on the tick,
>                                                // so a chatty child still cancels promptly
>     match rx.recv_timeout(RECV_TICK) {
>       Ok(Out(line))    => { last_output = now(); ... as above (Heartbeat / Log / Result) ... }
>       Ok(Err(line))    => { last_output = now(); stderr_tail += line; trim; emit Log "stderr: …" }
>       Ok(OutEof)       => -> ended_without_result(rx, None)
>       Ok(ErrEof)       => {}                   // never terminal on its own
>       Ok(WriteErr(e))  => -> ended_without_result(rx, Some(e))   // fatal, but compose stderr first
>       Err(Timeout)     => on_tick(&mut writer, limits)?
>       Err(Disconnected)=> -> FAIL("Claude output stream closed unexpectedly")
>     }
> }
>
> on_tick(writer, limits):                       // the reply pump / watchdog / hard cap
>   if awaiting:
>       match ctl.replies.try_recv() {
>         Ok(text)     => { send_write(writer, turn_line(text + REPLY_SUFFIX))   // NEVER blocks
>                           on Err -> FAIL("Claude closed its input: <e>")
>                           awaiting = false; ctl.awaiting.store(false)
>                           last_output = now(); emit Log "» answered (<n> bytes)" }
>         Empty        => {}
>         Disconnected => -> FAIL("Claude asked a question but the reply channel is closed")
>                         // FAIL LOUDLY: D3 has paused the watchdog, so nobody would ever reap this
>                         // run; a closed reply channel means no answer can ever arrive.
>       }
>       return            // D3: NEITHER the idle watchdog NOR the hard cap is consulted
>   ... idle_timeout / hard_cap checks exactly as specified above ...
>
> ended_without_result(rx, write_err):           // the stdout-EOF-vs-stderr race
>   drain_stderr(rx)     // recv_timeout(STDERR_GRACE = 150 ms) until ErrEof, an empty gap, or the
>                        // STDERR_GRACE_TOTAL = 1 s cap. Stdout lines seen here are still LOGGED (D2).
>   tail = stderr_tail.trim()
>   tail non-empty     -> FAIL("Claude exited without a result: <tail>")
>   else + write_err   -> FAIL("Claude closed its input: <e>")
>   else               -> FAIL("Claude exited without a result")
> ```
> **Why the bounded drain:** stdout-EOF and stderr arrive from **different senders**, so mpsc gives
> no ordering guarantee between the last stderr line and `OutEof`. Without the grace, a child that
> printed a real error (bad flag, expired login) and exited — the most likely real-world failure —
> reported the generic "Claude exited without a result" with an empty stderr roughly **half the
> time**, throwing away its only diagnostic. Composing stderr-first fixes that; the cap keeps the
> failure path bounded.
>
> Exits, as landed (the EOF-then-grace ordering is load-bearing):
> ```
> OK:     complete(child, writer, res):
>           drop(writer)                        // the ONLY WriteTx -> writer thread releases
>                                               // ChildStdin -> the child sees EOF and exits
>           poll try_wait every EXIT_POLL (50 ms) up to EXIT_GRACE;
>           if still alive -> kill_child_tree + wait()
>           emit(Done { cost_usd, turn })       // EOF FIRST, then the grace — in that order
> CANCEL: reap(child) = kill_child_tree + wait() + ctl.awaiting.store(false)
>         emit(Cancelled { text: "cancelled", partial_text }); Err(AiCancelled("cancelled by user"))
> FAIL:   reap(child); emit(Failed { text: msg, partial_text }); Err(AiFailed(msg))
> ```
> On the CANCEL / FAIL paths (cancel, watchdog, hard cap, max-turns, write error) the `WriteTx` may
> outlive `reap()` by a few statements. Safe and deliberate: those paths kill the tree and `wait()`
> **first**, and `child.stdin` was already `take()`n into the writer thread, so `Child::wait()` has
> no stdin handle of its own to drop and cannot deadlock.

Notes the implementer must not "simplify" away:
- **Reader threads are never joined** — a surviving grandchild can hold the inherited pipe (the
  reason `run_process` detaches them today). Nothing is lost, because lines were forwarded as they
  arrived: that is precisely the mechanism behind D2.
- `partial` accumulates **assistant text only** (not `⚙`/`system`/`stderr` decoration), so
  `partialText` is a plausible truncated body for display.
- `REPLY_SUFFIX` (single line, appended in stdin): `"\n\n(Answer above. Now output ONLY the merged file contents, with no conflict markers and no commentary.)"`.
- `write_turn` writes exactly one line: `serde_json::json!({"type":"user","message":{"role":"user","content":[{"type":"text","text": <text>}]}})` + `"\n"` + `flush()`. serde_json does the escaping — never hand-build this.
- A `BrokenPipe` from `write_turn` is a FAIL (`"Claude closed its input"`), not a panic.

> **SUPERSEDED by P68a implementation (2026-08-17)** — the first bullet stands verbatim (and the
> writer thread is likewise never joined). The rest, restated:
> - `partial` accumulates assistant text only — now **enforced by `StreamLogItem.assistant_text`**
>   (§3.2 amendment) rather than by re-sniffing an already-rendered line.
> - `REPLY_SUFFIX` is unchanged, verbatim (`session.rs:32`).
> - **`write_turn(stdin, …)` does not exist.** `turn_line(text)` (`session_pipes.rs`) builds exactly
>   one line — `serde_json::json!({"type":"user","message":{"role":"user","content":[{"type":"text","text": <text>}]}})` + `"\n"`
>   (serde_json does the escaping — never hand-build it) — and `send_write(&mut writer, line)`
>   **queues** it for the writer thread, which performs the `write_all` + `flush`. Queuing never
>   blocks (D16).
> - **`drop(stdin)` does not exist either.** EOF is *dropping the single `WriteTx`* (D16).
> - A `BrokenPipe` is still a FAIL and never a panic, but it now arrives on one of two paths:
>   `send_write` returns `Err` immediately (the writer thread is already gone), or `Msg::WriteErr`
>   arrives later (the `write_all` itself failed). Both go through `ended_without_result`, so the
>   child's own stderr wins over our generic `BrokenPipe` text.

### 3.4 The streaming argv (LOCKED)

```
claude [-p <prompt>]                     # positional ONLY when !interactive (spike §1.4)
       -p                                # interactive: bare -p, prompt goes in via stdin (D13)
       --verbose                          # REQUIRED by stream-json (spike §1.1)
       --output-format stream-json
       [--input-format stream-json]       # interactive
       [--replay-user-messages]           # interactive (logged as a byte count only — A11)
       [--include-partial-messages]       # limits.include_partial_messages
       --safe-mode
       --tools <limits.tools.arg()>       # "Read,Grep,Glob" | ""   (D10)
       --permission-mode manual           # the READ FENCE — always (§1a.2/§1a.3)
       --no-session-persistence
       --model <model>
       [--append-system-prompt <sp>]      # SINGLE line (D13)
       [--max-budget-usd <n>]             # limits.max_budget_usd, `{:.4}` formatted
```

`resolve_bin()` (`mod.rs:207`), `CREATE_NO_WINDOW` (`mod.rs:121-126`) and `kill_child_tree`
(`mod.rs:223`) are reused **verbatim**. The D13 invariant note must be copied above the builder, kept
truthful.

> **AMENDED by P68a implementation (2026-08-17)** — this argv landed unchanged, but it lives in
> `ai/session_argv.rs::build_command` (not inline in `session.rs`), and the D13 note above the
> builder is now backed by a test: `argv_never_contains_a_newline`.
>
> **AMENDED by P68g-1 (2026-08-18, security audit H1)** — `--permission-mode manual` added,
> unconditionally (harmless under `ToolPolicy::None`, and a fence that only appears "when it
> matters" is a fence that goes missing when the policy changes). Asserted by
> `argv_always_fences_reads_with_permission_mode_manual`. The 13 non-streaming `RunOpts::default()`
> call sites in `src-tauri/src/commands/ai.rs` are deliberately NOT touched — they run
> `--tools ""` through `run_claude`, so they have nothing to fence.

---

## 4. §B — Interactive Q&A, cancel, and the read-only allowlist

### 4.1 The sentinel protocol (D9)

Both streaming system prompts (single-line, `git/ai_resolve_stream.rs`) end with:

> ` If you cannot resolve without more information, reply with EXACTLY one line beginning BONSAI_NEEDS_INPUT: followed by your question, and nothing else.`

and both carry the read-only clause:

> ` You may READ other files in the repository (Read, Grep, Glob) to understand how the conflicting code is used; never modify anything.`

Rules:
1. Detection is `sentinel_question` (§3.2) on the fence-stripped result of a turn — **first non-empty
   line only** (A9).
2. `post_turn_summary.status_category == "blocked"` / `needs_action` are emitted as a `Log` line and
   **never** drive `AwaitingInput` (D9).
3. Bounded by `limits.max_turns` (`ai_max_turns`, default 6): the *n*-th sentinel with
   `turn >= max_turns` fails the run with a message naming the count.
4. **The idle watchdog and the hard cap are both paused while awaiting input (D3).** A user may take
   ten minutes to answer.
5. A model that ignores the convention just returns a normal answer — still caught by
   `hasUnresolvedMarkers` on the frontend, never silently staged (D4, §11).

> **AMENDED by P68a implementation (2026-08-17)** — rule 4 has one deliberate escape hatch: if the
> reply channel is **closed** (`TryRecvError::Disconnected`) while awaiting, the run **fails loudly**
> instead of waiting forever. With the watchdog paused there is no other reaper, and no answer can
> ever arrive on a closed channel. Message: `"Claude asked a question but the reply channel is
> closed"`.

### 4.2 `ai/registry.rs`

```rust
/// P68 §B: per-run cancel/reply handles. CLONE-able handle over a shared map so
/// it can be `.manage()`d on the Tauri app AND moved into `spawn_blocking`
/// (`tauri::State` only yields a borrow). Mirrors `McpServerState` as managed
/// state (`src-tauri/src/lib.rs:20`, `mcp.rs:104`).
#[derive(Clone, Default)]
pub struct AiRunRegistry { inner: Arc<Mutex<HashMap<String, AiRunHandle>>> }

pub struct AiRunHandle {
    pub cancel: Arc<AtomicBool>,
    pub awaiting: Arc<AtomicBool>,
    pub pid: Arc<AtomicU32>,
    reply_tx: std::sync::mpsc::Sender<String>,
}

impl AiRunRegistry {
    /// Mint a run id and register it. NO new dependency: the id is
    /// `format!("ai-{:x}-{}", <nanos since UNIX_EPOCH>, <process-global AtomicU64>)`
    /// — unique per process and unguessable enough for a local channel key (A7).
    pub fn register(&self) -> (String, RunControl);
    /// Idempotent: an unknown id is `false` (the command still returns Ok).
    pub fn cancel(&self, run_id: &str) -> bool;
    /// `AiFailed` when the id is unknown OR the run is not awaiting input, so a
    /// stray reply can never be silently swallowed.
    pub fn reply(&self, run_id: &str, text: String) -> Result<(), AppError>;
    /// Always called on EVERY exit path of the command (success, failure,
    /// cancel) — use a guard/`finally`-style drop so a panic cannot leak an entry.
    pub fn finish(&self, run_id: &str);
    /// App-exit hook (D7): flip every cancel flag AND `kill_pid_tree` every
    /// recorded pid, then clear. Best-effort, never blocks meaningfully.
    pub fn cancel_all(&self);
    pub fn active(&self) -> usize;
}
```

> **AMENDED by P68a implementation (2026-08-17)** — one addition to `AiRunRegistry`:
> ```rust
>     /// True when the run exists AND its session has set `awaiting` (§3.3).
>     /// P68b's `ai_reply_run` may use it to validate a reply before calling
>     /// `reply`, and a future `ai_active_runs` can report it — without exposing
>     /// the handle. Unknown id => false.
>     pub fn is_awaiting(&self, run_id: &str) -> bool;
> ```
> `reply()` still enforces the rule itself (`AiFailed` when unknown or not awaiting), so
> `is_awaiting` is a **query, not a required pre-check**.

### 4.3 `AppError::AiCancelled`

```rust
    /// P68 §B: the user cancelled a streaming AI run via `ai_cancel_run`. NOT a
    /// failure — the frontend shows no error toast, only a `cancelled` run state.
    /// Distinct from `AiFailed` so the one catch path can tell them apart.
    #[error("{0}")]
    AiCancelled(String),
```
`kind()` ⇒ `"aiCancelled"`; add to the `message()` `|`-chain. TS: add `'aiCancelled'` to the
`AppError.kind` union (`src/ipc/types.ts:1812-1813`). `useAiRuns` maps it to
`status: 'cancelled'` and pushes **no** toast.

---

## 5. §C — The per-path run store (the item-5 fix)

### 5.1 Root cause, restated so the fix is unambiguous

1. `aiResolvingPath` (`RepoWorkspace.tsx:243`) is a single scalar, and
   `StatusConflictsSection.tsx:158` does `aiDisabled={aiResolvingPath !== null}` — so **every**
   conflict row's ✨AI button is disabled during any run.
2. `useMergeActions.ts:149-162` does `++fileDiffReqId.current` → `await ipc.getConflict(path)` →
   `if (id !== fileDiffReqId.current) return;`, and `fileDiffReqId` (`RepoWorkspace.tsx:538`) is
   **shared** with ordinary diff opening (L768,774,777,783,834,840,843,1219,2340). Opening another
   file during the run — *even after the CLI call already succeeded* — hits that `return` and
   discards the computed proposal with no toast, no cache and no retry.

**Rule (binding): `fileDiffReqId` is NEVER bumped before an AI CLI call.** The only AI-path bump is
immediately before the fast, local `getConflict` inside `openAiProposal` — so the guard protects the
*diff slot* and can never destroy a *proposal*. A superseded `openAiProposal` leaves the proposal in
the store; the row's `✓ review` affordance re-opens it.

### 5.2 `src/components/repoWorkspace/useAiRuns.ts`

```ts
/** P68 §C: one AI run, keyed independently of any UI slot. */
export type AiRunStatus = 'running' | 'awaitingInput' | 'ready' | 'failed' | 'cancelled';

export interface AiRunLogLine { seq: number; text: string }

export interface AiRunFileState {
  path: string;
  /** 'pending' until the batch resolves, then one of the terminal three. */
  status: 'pending' | 'ready' | 'failed';
  proposal: string | null;
  error: string | null;
}

export interface AiRunState {
  /** `conflict:<path>` for a single file, `bulk:<startedAt>` for a batch.
   *  Generalises to `analyze:<oid>` when the other six runners adopt the dock (D14). */
  key: string;
  /** Dock header label: the path, or `"<n> conflicts"`. */
  label: string;
  paths: string[];
  /** null until the `started` event (D8). */
  runId: string | null;
  /** A cancel requested before `runId` arrived; fired as soon as it does (D8). */
  cancelRequested: boolean;
  status: AiRunStatus;
  log: AiRunLogLine[];        // capped at AI_LOG_MAX (oldest dropped)
  logDropped: number;
  question: string | null;
  /** Single-run proposal (paths.length === 1). */
  proposal: string | null;
  /** Per-file rows — always populated, length === paths.length. */
  files: AiRunFileState[];
  error: string | null;
  /** LAST result's value within a run; SUMMED across sequential bulk batches (A10). */
  costUsd: number | null;
  /** Display-only partial assistant text on a cancelled/failed run (D2). */
  partialText: string | null;
  startedAt: number;
  endedAt: number | null;
  /** Stale/duplicate event guard: events with seq <= this are ignored. */
  lastSeq: number;
}

export interface AiRunsApi {
  /** key -> state, newest first when iterated via `orderedRuns`. */
  runs: Record<string, AiRunState>;
  orderedRuns: AiRunState[];
  /** Store state for one conflict path, or null. Drives the row affordance. */
  runForPath(path: string): AiRunState | null;
  /** True when the concurrency cap is reached (OQ1). */
  atCapacity: boolean;
  startConflictRun(path: string): void;
  startBulkRun(paths: string[]): void;
  cancelRun(key: string): void;
  replyRun(key: string, text: string): void;
  /** Remove a terminal run from the store (dock ✕). No-op while running. */
  dismissRun(key: string): void;
  /** Nudge counter, incremented every second while any run is active; consumers
   *  derive elapsed from `Date.now() - startedAt` (D5 — one interval, only
   *  while active). */
  tick: number;
}

export function useAiRuns(deps: {
  repoId: string;
  pushToast: (level: 'info' | 'success' | 'error', msg: string) => void;
  aiConflictAutonomy: AiAutonomy;
  /** = `handleResolveConflictText` — the ONLY writer (D4). */
  applyResolution: (path: string, text: string) => Promise<void>;
  /** = `useMergeActions.openAiProposal` — opens the center-pane review editor. */
  openAiProposal: (path: string, proposedText: string) => Promise<void>;
  /** Bulk cost guard / eligibility. */
  aiEligible: boolean;
}): AiRunsApi;

/** P68 §C/D5. */
export const AI_LOG_FLUSH_MS = 50;
export const AI_LOG_MAX = 500;
/** OQ1 — concurrency cap. One CLI process per run; more than a few is a
 *  subscription-rate-limit hazard and unreadable in one dock. */
export const AI_MAX_CONCURRENT_RUNS = 3;
```

> **AMENDED by the P68d implementation (2026-08-17) — the landed layout and the deviations.**
>
> `useAiRuns.ts` as one ~300-line file was not achievable once it also owned the buffered flush, the
> elapsed clock, the autonomy routing and the prune policy: the first working version was **668
> lines**. Split per the ~500-line rule, in the same increment:
>
> | File | Lines | Responsibility |
> |---|---|---|
> | `repoWorkspace/useAiRuns.ts` | **475** | The hook only: `runsRef` + render mirror, the 50 ms log/metrics buffers, the elapsed interval, `start`/`cancel`/`reply`/`review`/`dismiss`, `drive` + `settle`, the prune effect. |
> | `repoWorkspace/aiRunState.ts` | **231** | The state SHAPE (`AiRunState`, `AiRunFileState`, `AiRowState`, `AiRunStatus`) + the pure transforms: **`settleBatch` (which owns the markerful safety gate)**, `deriveRowStates`, `pruneRuns`, `newRun`, `conflictKey`. |
> | `repoWorkspace/aiRunEvent.ts` | **98** | `decideEvent` — the §5.2 table as a PURE decision function (`{patch, logLine, thinkingTokens, flushNow, fireQueuedCancel}`). Mirrors the Rust `stream.rs`/`session.rs` split (D12): interpretation apart from the machinery that acts on it. |
> | `repoWorkspace/aiRunLog.ts` | **69** | `AiRunLogLine` + `AiRunLogKind` + **`classifyLogLine` (P68e §12-A1: kind decided ONCE, at ingest)**, `AI_LOG_FLUSH_MS`, `AI_LOG_MAX`, `AI_EVENT_TEXT_MAX`, `appendCapped`. |
>
> Deviations from §5.2/§5.4, all deliberate:
> 1. **`applyResolution` takes an optional third argument** (`successMessage`). `handleResolveConflictText` toasts `Staged resolution for <path>`; routing autoResolve through it (D4: one writer) would otherwise either lose the P13 copy `Resolved <path> with AI — review the staged result` or emit two toasts. One writer, one toast, exact copy preserved.
> 2. **`tick` is a TIMESTAMP, not a counter.** `deriveRowStates(view, tick)` is then a pure function of state — no `Date.now()` inside a memo, so React and eslint see the real dependency (the counter version needed an `eslint-disable`). Refreshed from exactly two places: the once-a-second interval that runs only while a run is live (D5), and every commit.
> 3. **`AiRowState` carries `key` and `error`** as well as `{status, elapsedSecs}` — the row needs the error for the `⚠` tooltip, and the key to address the dock entry in P68e.
> 4. **The store exposes `rowStates` and `runningCount`**, so `RepoWorkspace` passes `aiRuns.rowStates` / `aiRuns.atCapacity` straight through instead of deriving them in a 3050-line container.
> 5. **`onAiReveal` is an OPTIONAL prop and P68d does not pass it.** §5.4 has a `running`/`awaitingInput` row click "reveal + expand the dock", and there is no dock until P68e. So while `onAiReveal` is absent the live-run button renders as a read-only status badge (`…12s` / `?`, disabled, with an explanatory `title`); P68e passes the handler and it becomes clickable. It never disables any OTHER row — that is the item-5 invariant and it is tested.
> 6. **Two extra prop hops**: §5.2 lists only `StatusConflictsSection.tsx`, but `aiResolvingPath` was also threaded through `StatusPanel.tsx` and `WorkspaceRightPanel.tsx`; both now carry `aiRows` / `aiAtCapacity` / `onAiReview` / `onAiReveal`.
> 7. **Fixture change**: the paused-merge mock fixture gains a second `bothModified` path (`MERGE_DEEP_PATH`, a deep i18n JSON path). The item-5 scenario needs a second AI-eligible file to switch to, P68f's "Resolve all with AI" needs ≥2 to appear at all, and the deep path exercises path truncation in the dock. Five fixture-count assertions were updated with it.
> 8. **New mock seams** `?aiMarkers` (markerful proposal — the only way to exercise the safety gate through the real IPC surface) and `?aiFlood` (~700 lines + one exactly-2000-char line, for the cap / truncation chip / jump-to-latest).

Event handling (normative):

```
onEvent(ev):
  st = runs[keyOf(ev.runId)]                    // runId -> key map built on `started`
  if st === undefined -> ignore
  if ev.seq <= st.lastSeq -> ignore             // stale/duplicate
  switch ev.kind:
    'started'        : st.runId = ev.runId; register runId->key; FLUSH NOW
                       if st.cancelRequested -> void ipc.aiCancelRun(ev.runId)
    'log'            : logBuf.get(key).push({seq, text}); scheduleFlush()      // D5
    'turnEnd'        : st.costUsd = ev.costUsd ?? st.costUsd; FLUSH NOW
    'awaitingInput'  : st.status='awaitingInput'; st.question=ev.text; FLUSH NOW
    'done'           : st.status handled by the PROMISE, not the event (see below); FLUSH NOW
    'failed'         : st.status='failed'; st.error=ev.text; st.partialText=ev.partialText; FLUSH NOW
    'cancelled'      : st.status='cancelled'; st.partialText=ev.partialText; FLUSH NOW
```

The command **promise** is authoritative for the final data (the channel may have been dropped):
`await ipc.aiResolveConflictStream(...)` → on resolve, write `files`/`proposal`/`costUsd`, set
`status: 'ready'` (or `'failed'` when *every* path failed), `endedAt = Date.now()`; on reject, map
`aiCancelled` → `'cancelled'` (no toast) and anything else → `'failed'` + one error toast. The store
is **idempotent**: a terminal status is never overwritten by a later event.

Autonomy routing (unchanged semantics, moved):
- `proposeReview` → for each ready path: `openAiProposal(path, text)` for the **first** path only
  (one center pane), and `pushToast('success', 'AI proposal ready for <path> — opened for review')`.
  For a bulk run with >1 ready file, the toast reads `AI proposals ready for <n> files — review them
  from the AI activity panel` and nothing is auto-opened.
- `autoResolve` → for each ready path where `!hasUnresolvedMarkers(text)`: `applyResolution` then
  one `refreshAll` for the whole batch; markerful paths fall back to `failed` with the existing
  message (`AI left unresolved markers in <path> — opened for review`) and are opened for review.
  **The markerful safety net is preserved verbatim** (`useMergeActions.ts:126,141-143`).

### 5.3 `useMergeActions.ts` — deletions and the one addition

- **DELETE** `handleAiResolveConflict` (L111-169) and the `setAiResolvingPath`, `aiConflictAutonomy`
  deps. `hasUnresolvedMarkers` / `AiResolveProposal` imports move to `useAiRuns.ts`.
- **ADD**:
```ts
  /** P68 §C: open an already-computed AI proposal in the center-pane review
   *  editor. The `fileDiffReqId` guard wraps ONLY the fast local `getConflict`
   *  — never a CLI call (§5.1) — so a superseded open loses the SLOT, never the
   *  proposal (which stays in the run store and can be re-opened). Slot key
   *  `ai-proposal:<path>` is unchanged, so ConflictEditor/DiffOverlay need no
   *  change. */
  async function openAiProposal(path: string, proposedText: string): Promise<void>;
```
  Body shape: `const id = ++fileDiffReqId.current;` → `const file = await ipc.getConflict(repoId, path);`
  → `if (id !== fileDiffReqId.current) return;` → `setDiffSlot({ key: 'ai-proposal:' + path, state: 'ready', diff: null, conflict: { ...file, text: proposedText }, error: null })`.
- `RepoWorkspace.tsx`: delete `aiResolvingPath` (L243) and its pass-down (L2789);
  `onAiResolve={(path) => aiRuns.startConflictRun(path)}` (L2798).

### 5.4 Row affordance — `StatusConflictsSection.tsx`

Prop change (single, mechanical):

```ts
/** P68 §C: per-path AI run state for the row affordance. Replaces the single
 *  `aiResolvingPath` scalar, whose `aiDisabled={aiResolvingPath !== null}`
 *  disabled EVERY row during any run (the item-5 bug, part a). */
export interface AiRowState { status: AiRunStatus; elapsedSecs: number }
// StatusConflictsSection props:  aiRows: Record<string, AiRowState>;  aiAtCapacity: boolean;
//                               aiBulkBusy: boolean;  onAiResolveAll(): void;
```

`ConflictRow` (per status) — label, `title`, `data-state` and click action:

| store status | label | click |
|---|---|---|
| *(none)* | `✨ AI` | `onAiResolve()` |
| `running` | `…<elapsedSecs>s` | reveal + expand the dock for this run |
| `awaitingInput` | `?` | expand the dock and focus the reply box |
| `ready` | `✓ review` | `openAiProposal` for this path |
| `failed` | `⚠` | retry (`onAiResolve()`); `title` carries the error |
| `cancelled` | `✨ AI` | `onAiResolve()` |

`disabled` becomes `!aiEligible || disabled || (status === 'running' || status === 'awaitingInput' ? false : aiAtCapacity)`
— i.e. **a run on another path never disables this row** (only the cap does), and a row with a live
run stays clickable because clicking it reveals the dock.

Section header (`:145-147`) gains a `Resolve all with AI` button: shown when `aiEligible` and ≥2
eligible (`bothModified`/`bothAdded`) conflicts exist; disabled while `aiBulkBusy`; label becomes
`Cancel all` while a bulk run is active (which calls `cancelRun` on the bulk key).

---

## 6. §D — Bulk resolve: ONE run for all conflicts

### 6.1 Payload format (stdin, `build_bulk_payload`)

```
BONSAI BULK CONFLICT RESOLUTION — <n> files, one merge
===== BONSAI FILE 1/<n>: <path> =====
CONFLICT KIND: <ConflictKind:?>
----- ANCESTOR (base) -----
<base or "(absent)">
----- OURS -----
<ours>
----- THEIRS -----
<theirs>
----- CONFLICTED (worktree, with markers) -----
<marker text>
===== BONSAI FILE 2/<n>: <path> =====
…
```
`(absent)` reuses `ABSENT` (`git/ai_resolve.rs:28`); the per-file trio comes from the extracted
`read_conflict_sides`. A single-path run does **not** use this format: it keeps today's payload
(`ai_resolve.rs:116-128`) and today's `SYSTEM_PROMPT` + the two appended clauses — the proven common
case is not perturbed.

### 6.2 Response contract and parse-back (`parse_bulk_response`, PURE)

The bulk system prompt requires, for each file:
```
===== BONSAI RESULT: <path> =====
<merged file contents>
```

```
scan the model text line by line for  ^\s*=====\s*BONSAI RESULT:\s*(.+?)\s*=====\s*$
for each match: body = every line until the next marker or EOF
  - strip ONE leading and ONE trailing blank line
  - apply strip_fence to the body
attribution:
  path matched EXACTLY (byte-equal, forward slashes) against the requested set  -> proposal
  path not in the requested set                                                 -> log + ignore
  requested path with NO block            -> failed(path, "no result block returned")
  body empty / whitespace only            -> failed(path, "empty result")
  body still has conflict markers         -> failed(path, "AI left unresolved conflict markers")
NEVER fail the whole batch for a per-file problem (D11).
Zero blocks parsed AND >1 requested path   -> AiFailed("Claude did not return per-file result blocks")
```
Marker detection in Rust reuses the same rule as the frontend `hasUnresolvedMarkers`: any line
starting with `<<<<<<< `, `=======` or `>>>>>>> ` at column 0.

### 6.3 Byte cap and batch splitting (never truncate)

```
cap = settings.ai_bulk_max_bytes                  // default 400_000
parts = requested paths in the order given, each with its rendered payload part
oversize: a SINGLE part whose bytes > cap  -> failed(path, "too large for AI resolution"), skipped
pack: greedily fill batches so sum(part bytes) + header <= cap; a batch is >= 1 part
run: ONE `run_claude_streaming` per batch, SEQUENTIALLY, under the SAME run_id
     - emit Log "batch <i>/<m>: <k> files (<bytes> B)" before each
     - check ctl.cancel between batches -> AiCancelled (already-parsed proposals are RETURNED? NO:
       cancel is an Err; the events already emitted stand — D2/§11)
     - a batch that fails as a WHOLE marks every path in that batch failed and continues to the
       next batch (a per-batch failure must not lose the other batches' work)
cost: SUM over batches of that batch's last `result` cost (A10 — separate processes, so summing is
      correct here; within one process the last value wins)
turns: reported as the max turn count seen across batches
```

> **NOTE for P68b (added 2026-08-17, from P68a's landed structure)** — a ~400 KB batch payload is
> exactly the case D16(a) exists for: it is written through `send_write` onto the writer thread
> **after** the readers are live, so it cannot deadlock. Do not "optimise" that into an inline
> `write_all`. Also see the §10.1 amendment: the Windows batch stub **cannot** exercise an
> interactive turn at bulk payload size, so that combination has no automated coverage on Windows.

### 6.4 Frontend

`useBulkAiResolve.ts` is thin: it computes the eligible path list (conflicts whose kind is
`bothModified`/`bothAdded`), refuses when `< 1`, and calls `aiRuns.startBulkRun(paths)`. All state
lives in `useAiRuns` (one store, one dock, one cancel). Entry points: the conflicts-section header
(§5.4) and the merge `OpBanner` actions row (`OpBanner.tsx:140-157`) — the banner button is
`Resolve all with AI` / `Cancel all`, disabled while `mutating`.

---

## 7. §E — The bottom dock

### 7.1 Mount point (verified)

`.workspace-host` (`styles.css:1406-1412`) is already `flex: 1; min-height: 0; flex-direction:
column` containing `.workspace-toolbar` (`flex: none; height: 40px`) and `.panes` (`flex: 1`). The
dock is therefore a **clean third child**: `flex: none`, explicit px height, `overflow: hidden`.
Full width, deliberately — it must not compete with the ~115 px P67b just reclaimed in the right
panel.

`RepoWorkspace.tsx` renders it as the element **immediately after the `.panes` closing tag**
(L2648…) inside the returned fragment, so DOM order = flex order. Dialogs/overlays rendered later in
the fragment are absolutely positioned and do not affect the flex layout — but the dock must still
come first among them. When `orderedRuns.length === 0` the component returns `null`: **zero layout
impact when no AI run has ever been started**.

### 7.2 Components

```ts
// AiActivityPanel.tsx (~210) — the dock shell.
export interface AiActivityFile { path: string; status: 'pending' | 'ready' | 'failed'; error: string | null }
export interface AiActivityRun {
  key: string; label: string; status: AiRunStatus;
  elapsedMs: number; costUsd: number | null;
  question: string | null; error: string | null; partialText: string | null;
  log: AiRunLogLine[]; logDropped: number;
  /** [] for a single-path run; per-file rows for a bulk run. */
  files: AiActivityFile[];
}
export interface AiActivityPanelProps {
  /** Newest first. Empty => the component renders null. */
  runs: AiActivityRun[];
  activeKey: string | null;
  onSelectRun(key: string): void;
  collapsed: boolean;
  onToggleCollapsed(next: boolean): void;
  /** Persisted px height of the expanded dock (§8.3 clamp 120..600). */
  height: number;
  onResizeHeight(next: number): void;
  onCancel(key: string): void;
  onReply(key: string, text: string): void;
  onDismiss(key: string): void;
  onReviewFile(key: string, path: string): void;
}
```
Header (class names are part of the contract): `.ai-dock-header` with
`.ai-dock-status` pill (`data-status` = the run status), the label, `.ai-dock-elapsed` (`m:ss`),
`.ai-dock-cost` (`$0.0263`, hidden when null), a `Cancel` `btn-danger` (only while
`running`/`awaitingInput`), a `✕` dismiss (only when terminal), and the collapse chevron
(`aria-expanded`). More than one run ⇒ a `.ai-dock-runs` selector strip.
Body: `AiActivityLog` always; `AiRunQueue` above it when `files.length > 0`; a
`.ai-dock-reply` form (textarea + `Send`, Ctrl/Cmd+Enter submits) when `status === 'awaitingInput'`,
with the question rendered above it in `.ai-dock-question`.
Resizer: a `.ai-dock-resizer` grab bar on the **top** edge (pointer events, min 120 / max 600),
committing through `onResizeHeight` on pointer-up only (debounced settings write).

`AiActivityLog.tsx` — `<ol className="ai-log">` of `log`, `<li className="ai-log-line">`, monospace,
`aria-live="off"` (a chatty live region would spam screen readers); a `.ai-log-dropped` note when
`logDropped > 0` (`"… <n> earlier lines dropped"`); stick-to-bottom via a `stickRef` that turns off
when the user scrolls up more than 24 px and back on at the bottom.

`AiRunQueue.tsx` — `<ul className="ai-run-queue">`, one row per file: status glyph
(`…`/`✓`/`⚠`), the path, and a `Review` button (enabled only when `status === 'ready'`) calling
`onReviewFile(key, path)`.

`AiOutputPanel.tsx` is **untouched** (D14).

---

## 8. §F — IPC surface

### 8.1 Command table (+3: 157 → 160)

| Command | Kind | Rust signature | TS |
|---|---|---|---|
| `ai_resolve_conflict_stream` | **Channel** | `(app: AppHandle, state: State<AppState>, registry: State<AiRunRegistry>, repo_id: String, paths: Vec<String>, on_event: tauri::ipc::Channel<AiRunEvent>) -> Result<AiResolveBatch, AppError>` | `aiResolveConflictStream(repoId: string, paths: string[], onEvent: (e: AiRunEvent) => void): Promise<AiResolveBatch>` |
| `ai_cancel_run` | req/res | `(registry: State<AiRunRegistry>, run_id: String) -> Result<(), AppError>` — **idempotent; unknown id ⇒ `Ok(())`** | `aiCancelRun(runId: string): Promise<void>` |
| `ai_reply_run` | req/res | `(registry: State<AiRunRegistry>, run_id: String, text: String) -> Result<(), AppError>` — `AiFailed` when unknown or not awaiting | `aiReplyRun(runId: string, text: string): Promise<void>` |

**`ai_resolve_conflict` (`commands/ai.rs:20`) stays registered and completely unchanged** — no
signature change, no `RunOpts` change, no deprecation in P68. It remains the fallback and keeps the
existing `commands/tests.rs` coverage valid.

Runtime-free cores (the tauri `test` feature is unusable on this machine —
`STATUS_ENTRYPOINT_NOT_FOUND`; mirror `ai.rs:36-54` and `history.rs:107-121`):

```rust
pub(crate) async fn ai_resolve_conflict_stream_inner(
    state: &AppState,
    registry: &AiRunRegistry,
    settings_file: &std::path::Path,
    repo_id: &str,
    paths: Vec<String>,
    on_event: impl Fn(AiRunEvent) + Send + Sync + 'static,
) -> Result<AiResolveBatch, AppError>;
```
Order of operations (binding, mirrors §9.6 of P13): load settings → **consent gate
(`ai_enabled && ai_consented`) BEFORE `repo_path`** → reject empty `paths` with
`AiFailed("no conflicted paths given")` → `repo_path(state, repo_id)` → `registry.register()` →
`spawn_blocking(move || ai_resolve_stream::resolve_conflicts_streaming(...))` → `registry.finish`
on every exit path. Channel sends are `let _ = on_event.send(ev);` (a dropped channel must not fail
the run — same rule as `history.rs:97-99`).

### 8.2 Wire types

```rust
/// P68 §D: the outcome of one streaming resolve run over 1..n paths.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiResolveBatch {
    /// Echo of the run id (also delivered on the `started` event — D8).
    pub run_id: String,
    /// One entry per successfully attributed path. Reuses the P13 type verbatim.
    pub proposals: Vec<AiResolveProposal>,
    /// Per-file failures; NEVER fatal to the batch (D11).
    pub failed: Vec<AiResolveFailure>,
    /// Last-value-within-a-run, summed across sequential batches (A10).
    pub cost_usd: Option<f64>,
    /// Max turns used across batches (1 when no question was asked).
    pub turns: u32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiResolveFailure { pub path: String, pub reason: String }
```

```ts
/** P68 §F: a push event on the ai_resolve_conflict_stream channel. Mirrors the
 *  Rust `AiRunEvent` exactly. `runId` arrives on the FIRST (`started`) event —
 *  the command promise only settles at the end, so this is the only way the UI
 *  learns the id in time to cancel or reply (D8). */
export type AiRunEventKind =
  | 'started' | 'log' | 'turnEnd' | 'awaitingInput' | 'done' | 'failed' | 'cancelled';

export interface AiRunEvent {
  runId: string;
  seq: number;
  kind: AiRunEventKind;
  text: string | null;
  costUsd: number | null;
  elapsedMs: number;
  path: string | null;
  turn: number;
  /** Display-only accumulated text on `cancelled`/`failed` (D2). NEVER staged. */
  partialText: string | null;
}

export interface AiResolveFailure { path: string; reason: string }
export interface AiResolveBatch {
  runId: string;
  proposals: AiResolveProposal[];
  failed: AiResolveFailure[];
  costUsd: number | null;
  turns: number;
}
/** P68 §B/D10: repo access granted to a conflict-resolution run. */
export type AiConflictTools = 'readOnly' | 'none';
```

`IpcApi` doc comments must name the rejections:
`aiResolveConflictStream` → `aiUnavailable | aiFailed | aiCancelled | git | invalidName | noRepo`;
`aiCancelRun` → *(never rejects for an unknown id)*; `aiReplyRun` → `aiFailed`.

> **NOTE (P68a, 2026-08-17)** — the Rust `AiRunEvent` shape landed exactly as specified above, so the
> TS mirror and the mock (D15) are unaffected by the P68a restructure. `StreamLogItem.assistant_text`
> is **internal to Rust** and deliberately does **not** cross the IPC boundary: the frontend receives
> only the already-decided `partialText`.

### 8.3 New settings (additive, `#[serde(default)]`, **no version bump**)

Meets the documented bar at `settings.rs:288-303` (P67c's `panel_density` is the precedent): every
field is additive with a safe type default, so a pre-P68 `settings.json` deserialises unchanged.
Extend that doc comment's field list. `clamp_graph_prefs` is **NOT** touched.

| Rust field | JSON | Type | Default | Clamp (`clamp_ai_settings`) |
|---|---|---|---|---|
| `ai_idle_timeout_secs` | `aiIdleTimeoutSecs` | `u32` | `300` | `0` (disabled) or `30..=3600` |
| `ai_hard_cap_secs` | `aiHardCapSecs` | `u32` | **`0` = unbounded** | `0` or `60..=86_400` |
| `ai_max_turns` | `aiMaxTurns` | `u32` | `6` | `1..=20` |
| `ai_stream_log` | `aiStreamLog` | `bool` | `true` | — |
| `ai_include_partial_messages` | `aiIncludePartialMessages` | `bool` | `false` | — |
| `ai_conflict_tools` | `aiConflictTools` | `AiConflictTools` | `ReadOnly` | — |
| `ai_bulk_max_bytes` | `aiBulkMaxBytes` | `u32` | `400_000` | `20_000..=4_000_000` |
| `ai_max_budget_usd` | `aiMaxBudgetUsd` | `f64` | `0.0` (⇒ omit the flag) | `0.0..=100.0`, non-finite ⇒ `0.0` |
| `ai_dock_height` | `aiDockHeight` | `u32` | `180` | `120..=600` |
| `ai_dock_collapsed` | `aiDockCollapsed` | `bool` | `false` | — |

```rust
/// P68 §B/D10: repo access granted to a conflict-resolution run. `ReadOnly` maps
/// to `--tools "Read,Grep,Glob"`; `None` to today's `--tools ""`. There is NO
/// write/edit/bash option, by design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiConflictTools { #[default] ReadOnly, None }
```
`ai_stream_log == false` suppresses `Log` events at the **source** (the session emits only
status-changing events) — not in the UI, so a user who turns it off pays no IPC cost.

> **NOTE (P68a, 2026-08-17)** — `ai_max_turns`'s default `6` is now the shared const
> `bonsai_core::ai::DEFAULT_MAX_TURNS` (§3.1 amendment): P68b's settings default should reference it
> rather than repeating the literal.

### 8.4 `src/ipc/tauri.ts`

```ts
  aiResolveConflictStream(
    repoId: string,
    paths: string[],
    onEvent: (e: AiRunEvent) => void,
  ): Promise<AiResolveBatch> {
    const channel = new Channel<AiRunEvent>();
    channel.onmessage = onEvent;
    // Tauri auto-serializes the Channel as the `on_event` command argument
    // (mirrors historyIndexBuild / cloneRepo).
    return invoke<AiResolveBatch>('ai_resolve_conflict_stream', { repoId, paths, onEvent: channel });
  },
  aiCancelRun(runId: string): Promise<void> { return invoke<void>('ai_cancel_run', { runId }); },
  aiReplyRun(runId: string, text: string): Promise<void> {
    return invoke<void>('ai_reply_run', { runId, text });
  },
```

### 8.5 `src/ipc/mock/handlers/aiStream.ts` (D15)

Module-init sentinels (via `query()` from `mock/repoState.ts:153`): `AI_SLOW = query('aiSlow') !== null`,
`AI_ASK = query('aiAsk') !== null`, `AI_FAIL = query('aiFail') !== null`. Module state:
`runs = new Map<string, MockRun>()` holding `{ cancelled: boolean; awaiting: boolean; resolveReply: ((t: string) => void) | null }`.

```
aiResolveConflictStream(repoId, paths, onEvent):
  state = requireRepo(repoId)
  if AI_OFF -> throw { kind:'aiUnavailable', message:'Claude Code CLI not found on PATH' }   // the fix for ai.ts:29
  eligible = paths whose conflict kind is bothModified|bothAdded; others -> failed[]
  if eligible.length === 0 -> throw { kind:'aiFailed', message:'AI resolution unavailable for these files' }
  runId = `mock-run-${++counter}`; seq = 0
  emit('started')                                     // runId lands here (D8)
  emit log: 'session mock-sess · model sonnet · tools: Read, Grep, Glob'
  emit log: '⚙ Grep(pattern: "<first path basename>")'    // proves read-only tool visibility
  for each eligible path: emit log '⚙ Read(<path>)' (path field set)
  if AI_FAIL && eligible.length === 1 -> emit('failed'); throw { kind:'aiFailed', ... }
  if AI_ASK:
      emit('awaitingInput', text: 'Should the German plural form use "Einträge" or "Eintraege"?')
      await new Promise(r => run.resolveReply = r)      // resolved by aiReplyRun
      emit log '» answered (n bytes)'
  ticks = AI_SLOW ? 12 : 3 ;  gap = AI_SLOW ? 1500 : 200
  for i in 1..ticks: await delay(gap)
                     if run.cancelled -> emit('cancelled'); throw { kind:'aiCancelled', message:'cancelled by user' }
                     emit log `analysing… (${i}/${ticks})`
  emit('turnEnd', costUsd: 0.0238)
  proposals = eligible.map(p => ({ path: p, proposedText: stripConflictMarkers(conflictTexts[p].text), costUsd: null }))
  if AI_FAIL && eligible.length > 1 -> move eligible[1] from proposals to failed
                                       (reason: 'no result block returned')   // per-file, not fatal
  emit('done', costUsd: 0.0263)
  return { runId, proposals, failed, costUsd: 0.0263, turns: AI_ASK ? 2 : 1 }

aiCancelRun(runId):  await delay(30); runs.get(runId)?.cancelled = true; resolve (unknown id = Ok)
aiReplyRun(runId, text):
  r = runs.get(runId)
  if r === undefined || r.resolveReply === null -> throw { kind:'aiFailed', message:'run is not awaiting input' }
  r.resolveReply(text); r.resolveReply = null
```
Every event carries a monotonic `seq`, a real `elapsedMs` (`Date.now() - startedAt`) and `turn`.
`mock/persistence.ts` gains the 10 defaults + tolerant parsing (clamp numerics, validate
`aiConflictTools`); `mock/handlers/session.ts` gains 10 `patch.x ?? current.x` merge lines.

---

## 9. §G — Sub-increments

Each is one fresh-context senior-dev pass. Commit after each reviewer approval
(`wip(P68): …`). **+cmd math: P68a +0 · P68b +3 (157 → 160) · P68c–P68g +0.**

### P68a — Rust streaming runner core (no Tauri, no conflict logic)
Scope: `ai/stream.rs`, `ai/session.rs`, `ai/registry.rs`; `ai/mod.rs` gains **only** `RunLimits`,
`ToolPolicy`, `run_claude_streaming`, `parse_result_envelope` (pure extraction), `kill_pid_tree`, the
new consts and the `mod`/re-export lines; `AppError::AiCancelled`; NDJSON stub modes.
Stub modes to add to `crates/bonsai-core/tests/fixtures/claude_stub.{cmd,sh}` — each echoes NDJSON
lines, one per `echo`, draining stdin first:
`stream_success` (init → thinking → assistant text → post_turn_summary → result),
`stream_slow` (init, then `ping`-based silence longer than a 1 s test idle limit, then result),
`stream_ask` (first turn's result body is `BONSAI_NEEDS_INPUT: which one?`; a second `result` with
the real body after a second stdin line arrives),
`stream_partial` (init + assistant text, then exits WITHOUT a `result`),
`stream_garbage` (a non-JSON line, an unknown `type`, then a valid result),
`stream_bulk` (two `===== BONSAI RESULT: … =====` blocks in the result body).
**Acceptance:** (1) `cargo test -p bonsai-core ai::` green with, at minimum:
`classify_line` known-answer cases for **every** row of the §3.2 table incl. non-JSON and unknown
`type` → `Log` (never `Err`); `sentinel_question` positive/negative incl. "token mid-body ⇒ None"
(A9); `parse_result_envelope` reproduces all five branches of the old inline parse;
`stream_success` end-to-end emits `started → …log… → turnEnd → done` with monotonic `seq` starting at
0 and `runId` on seq 0; `stream_ask` completes after a programmatic `registry.reply`;
`stream_slow` with `idle_timeout = 1s` fails **and the collected log is non-empty** (the D2 guard —
this is the test that would have caught today's discard); a cancel flipped mid-run yields
`AiCancelled`, a non-empty log, and no surviving child; `stream_partial` yields
`AiFailed` mentioning the missing result; `stream_garbage` still succeeds.
(2) **Replace the pre-existing wall-clock assertion** in
`ai::tests::run_claude_slow_times_out_and_reaps_child` (`ai/mod.rs:577`) with a monotonic lower bound
(`>= the deadline`) plus a generous upper bound (e.g. `< 30 s`): it measured **2.97 s** for a 1 s
deadline under parallel load and passes only in isolation. This is a **pre-existing flake in the very
code P68a touches** — do not read it as a P68 regression.
(3) `cargo clippy --workspace --tests -- -D warnings` clean. (4) `run_claude`'s signature, argv and
90 s default are byte-identical apart from the extracted parse call (reviewer greps the diff) — D6.
(5) `ai/mod.rs` ≤ ~715 lines.

### P68b — conflict resolve + the 3 commands
Scope: `git/ai_resolve.rs` extraction (`read_conflict_sides`, `ConflictSides`);
new `git/ai_resolve_stream.rs` (payload builders, the two single-line prompts, `parse_bulk_response`,
`pack_batches`, `resolve_conflicts_streaming`); new `commands/ai_stream.rs`; `commands/mod.rs`,
`shared.rs`, `lib.rs` (`.manage` + 3 handler entries + `cancel_all` in `ExitRequested`);
`settings.rs` (10 fields + `AiConflictTools` + `clamp_ai_settings`); `ui_settings.rs` (10 fields ×
patch/arm/2 builders).
**Acceptance:** (1) `generate_handler!` counted at **160**. (2) `cargo test` green with:
`parse_bulk_response` cases — happy 3-file, a missing path ⇒ `failed`, an extra unknown path ⇒
ignored, a markerful body ⇒ `failed`, zero blocks with >1 requested ⇒ `AiFailed`, fenced bodies
stripped; `pack_batches` splits by cap and marks a single oversize file `failed` without truncating;
`ai_resolve_conflict_stream_inner` **rejects with `aiUnavailable` when consent is off, before touching
the repo** (mirror `ai.rs:36-54` / the existing consent test); empty `paths` ⇒ `aiFailed`;
`AiResolveBatch`/`AiRunEvent`/`AiResolveFailure` `*_wire_shape_is_camel_case` tests (model:
`ai_resolve.rs:154`) proving `runId`/`costUsd`/`partialText`/`elapsedMs`;
`ai_conflict_tools_roundtrips_both_variants` + `old_settings_file_without_ai_run_fields_loads_defaults`
(the no-version-bump guard); a `set_ui_settings` partial-patch arm proving the new fields patch
independently of `graph`/`listView`/`panelDensity`. (3) `--tools` is `Read,Grep,Glob` under the
default setting and `""` under `none` (assert via a stub mode that echoes its argv, à la
`check_model`). (4) clippy clean; `commands/ai.rs` **unmodified**.

### P68c — TS types + Channel bridge + mock
Scope: `src/ipc/types.ts`, `tauri.ts`, `index.ts`, new `mock/handlers/aiStream.ts`, `mock.ts` spread,
`mock/persistence.ts`, `mock/handlers/session.ts`; a new `src/ipc/mock/handlers/aiStream.test.ts`.
**Acceptance:** (1) `tsc` + `pnpm build` clean. (2) vitest: the default mock run emits
`started → log+ → turnEnd → done` with monotonic `seq` and a `runId` on the first event; `?aiSlow`
+ `aiCancelRun` ⇒ a `cancelled` event and an `aiCancelled` rejection; `?aiAsk` ⇒ `awaitingInput`
then completion after `aiReplyRun`; `?aiFail` single ⇒ rejection, bulk ⇒ one entry in `failed[]`
and the others in `proposals[]`; `?ai=off` ⇒ `aiUnavailable`; `aiCancelRun('nope')` resolves.
(3) `mock/handlers/ai.ts` **unchanged in size** (new file, D: do not grow a 485-line module).
(4) `getUiSettings()` returns the 10 new fields with the §8.3 defaults and round-trips a patch.

### P68d — per-path store + row feedback (**the item-5 fix**)
Scope: new `repoWorkspace/useAiRuns.ts`; `useMergeActions.ts` (delete `handleAiResolveConflict`, add
`openAiProposal`); `RepoWorkspace.tsx` (delete `aiResolvingPath`, wire the store, pass `aiRows`);
`StatusConflictsSection.tsx` (props + affordance); new `useAiRuns.test.ts` +
`StatusConflictsSection.test.tsx`.
**Acceptance:** (1) `tsc` + `pnpm build` clean; `StatusPanel.test.tsx` passes (adjust only the
conflicts-section props it constructs). (2) vitest, the regression guards:
**a run on file A does not disable file B's ✨AI button**; a proposal that arrives while
`fileDiffReqId` has been bumped by an unrelated diff open is **still in the store** and re-openable
(the exact item-5 scenario); 300 `log` events produce **≤ 1 state commit per 50 ms** and at most
`AI_LOG_MAX` retained lines with `logDropped > 0` (D5); a duplicate/stale `seq` is ignored; an
`aiCancelled` rejection produces `status: 'cancelled'` and **no error toast**; `autoResolve` with a
markerful body falls back to review with the existing message; `proposeReview` pushes the new
"proposal ready" toast and calls `openAiProposal`. (3) Reviewer greps: **no `aiResolvingPath`
anywhere**, and no `fileDiffReqId` bump precedes any `ipc.ai*` call.

### P68e — the bottom dock
Scope: new `AiActivityPanel.tsx`, `AiActivityLog.tsx`, `AiRunQueue.tsx`; `RepoWorkspace.tsx` mount;
`App.tsx` height/collapsed state; `styles.css`; new `AiActivityPanel.test.tsx`.
**Acceptance:** (1) `tsc` + `pnpm build` clean. (2) vitest: `runs: []` renders `null`; the header
shows the status pill / elapsed / cost / Cancel / dismiss per status; Cancel fires `onCancel(key)`;
the reply form appears **only** for `awaitingInput` and submits on click and on Ctrl+Enter;
collapsing hides the body but keeps the header; `logDropped > 0` renders the note.
(3) Harness: the dock is `.workspace-host`'s **third** element child, `flex: none`, and `.panes`
`getBoundingClientRect().height` shrinks by exactly the dock height (⇒ nothing overlaps and the graph
canvas re-lays out); height survives a reload. (4) `AiOutputPanel.tsx` untouched (D14); each new file
≤ ~230 lines.

### P68f — bulk single-run resolve
Scope: new `repoWorkspace/useBulkAiResolve.ts`; `StatusConflictsSection.tsx` header button;
`OpBanner.tsx` merge-arm button; tests.
**Acceptance:** (1) `tsc` + `pnpm build` clean. (2) vitest: the header button appears only with ≥2
eligible conflicts and `aiEligible`; it starts **exactly one** `aiResolveConflictStream` call with
**all** eligible paths (assert call count === 1 — this is the locked "one run" decision); per-file
`failed` entries mark only their own rows; `Cancel all` cancels the bulk key; ineligible kinds are
never included. (3) Harness: `?aiFail` bulk over 3 files ⇒ 2 rows `✓ review`, 1 row `⚠`, the dock's
`AiRunQueue` matching.

> **AMENDED by P68f implementation (2026-08-18) — three deliberate deviations, to fold into the
> P68g doc pass.**
>
> 1. **§5.4 "shown when `aiEligible`" → shown-and-DISABLED with an explanatory title.** Hiding the
>    header button when AI is off would make the affordance vanish rather than explain itself; the
>    per-row ✨AI button already renders disabled with `Enable AI features in Settings to use this`,
>    so this is genuine parity with the row button *and* the better behaviour. The gate that matters
>    (no run can start) is unchanged.
> 2. **§6.4 "disabled while `mutating`" → cancel is EXEMPTED.** A background refresh or any other
>    mutation must never trap a live AI run: the host section ORs its own `busy` into the *Resolve*
>    arm only, so `Cancel all` stays clickable for as long as the run lives.
> 3. **§9-P68f (3)'s 3-file harness case shipped as 2 files, plus unit cases.** The property that
>    clause protects is per-file independence (one bad file never costs another its result), and the
>    1-ready + 1-failed e2e case proves it; a third `bothModified` fixture would re-churn the five
>    fixture-count assertions P68d already churned once. The multi-file properties are asserted at
>    the unit layer instead (`useAiRuns.routing.test.tsx`): 2 markerful + 1 clean opens the centre
>    pane **exactly once**, and an all-markerful bulk stages nothing, refreshes nothing and
>    summarises nothing.
>
> **Also landed (post-review SHOULD-FIX):** `AiRunQueue` offers **Review** on a `failed` row that
> kept a `proposal` (`AiActivityFile.hasProposal`), not just on `ready`. Under `autoResolve` the
> markerful gate demotes every marker-carrying body to `failed` and bulk auto-opens only
> `markerful[0]`, so files 2..N held a paid-for draft with no reachable button — which also made
> `BulkAiConfirmDialog`'s "is opened for review instead" false. `Retry` still renders alongside.

### P68g — Settings UI + docs
Scope: new `SettingsAiRunSection.tsx` used from `SettingsPanel.tsx` after the autonomy fieldset
(L367); `SettingsPanel.test.tsx` extension; this contract + `P68-user-checklist.md`; the `TODO.md`
P68 entry (status + command math **160**) ; a CHANGELOG line.
**Acceptance:** (1) each control patches exactly its own field (`{ aiIdleTimeoutSecs: 600 }` etc.);
out-of-range input is clamped in the UI *and* in Rust. (2) The "Repo access" control offers exactly
two options (`Read-only` / `None`) — **no write option may appear** (D10). (3)
`git status --porcelain -uall` shows the docs (a `--name-only` diff hides untracked files — this has
stranded docs twice).

---

## 10. §H — Acceptance criteria

### 10.1 AI gate — cargo

- `cargo test -p bonsai-core ai::` and the workspace suite green; the per-increment lists in §9 are
  the enumeration. Highlights that must exist by name: a **cancel-keeps-partial-output** test, a
  **watchdog-keeps-partial-output** test, a **watchdog-does-not-fire-while-awaiting-input** test
  (D3: set `idle_timeout = 1s`, drive `stream_ask`, sleep 3 s before replying, assert the run still
  completes), a **turn-budget** test, and a **no-surviving-child** assertion after cancel.
- `cargo clippy --workspace --tests -- -D warnings` clean. **Never run `cargo test` and `clippy`
  concurrently** (target-dir race); set `TMP`/`TEMP` to `D:\Temp`.
- `generate_handler!` recounted: **160**.
- Known pre-existing flake, do NOT attribute to P68:
  `ai::tests::run_claude_slow_times_out_and_reaps_child` (see P68a acceptance (2)).

> **AMENDED by P68a implementation (2026-08-17) — stub coverage + timing notes.**
>
> - **Windows stub limitation — carry this into P68b.** `claude_stub.cmd` reads one turn with
>   `set /p`, which has a cmd.exe accepted-line ceiling of **~1 KB** *and does not consume the rest
>   of an over-long line*: the residue stays in the pipe and the **next** `set /p` (e.g.
>   `stream_ask`'s reply read) would swallow it **instead of the reply**. So the batch stub
>   **cannot exercise an interactive turn with a bulk-sized (~400 KB) payload on Windows.** P68a's
>   ~90-byte turns are fine. **P68b must not assume that coverage exists** — a bulk interactive
>   round-trip needs the real CLI (USER CHECKPOINT) or a small Rust echo helper, not the stub. This
>   is also why D16(a)'s pipe-buffer deadlock is proved Rust-side rather than through the stub.
> - **Two stub modes beyond the §9 list landed**, both for D16 and both required by the tests above:
>   `stream_stderr_fail` (prints a real error on stderr and exits with **no** `result` — the
>   stderr-first / bounded-drain assertion) and `stream_hang_stdin` (never reads stdin and stays
>   alive ~20 s — the unkillable-run negative control: **cancel must still work**).
> - **`idle_timeout = 1 s` is only usable as a test limit because `last_output` is reset right after
>   `spawn()`** (§3.3 amendment). Before that fix the same test needed **2 s** and still fired on
>   process-creation cost under parallel load. Do not "simplify" the reset away — and note that
>   `started` is deliberately *not* reset, so `elapsedMs` stays user-perceived.
> - Test modules, per the ~500-line rule (§2a amendment): `ai/session_tests.rs`,
>   `ai/session_io_tests.rs`, `ai/session_drain_tests.rs`, `ai/tests.rs`, `ai/testutil.rs`. The
>   `ai::tests::…` test paths named above and in §9 are **unchanged** by that split.

### 10.2 AI gate — vitest

The §9 per-increment lists. The three that matter most, restated because they are the milestone's
reason for existing:
1. **Item 5:** start a run for file A, bump `fileDiffReqId` by opening an unrelated diff, let the run
   finish → the proposal is in the store and re-openable; **and** file B's ✨AI button was never
   disabled.
2. **D5:** 300 log events ⇒ ≤1 commit per 50 ms, ≤500 retained lines.
3. **D8:** `runId` is read from the first event and a cancel issued before it arrives still fires.

### 10.3 AI gate — browser harness (`pnpm dev` + `VITE_MOCK_IPC=1`)

Frugal, batched checks; **one** screenshot at the end. Seed
`localStorage.bonsai.mockUiSettings` with `aiConsented: true` and reload (mock default is `false`,
which otherwise leaves `aiEligible` dark — the P53 lesson).
1. `?op=merge&aiSlow` → click ✨AI on one conflict: the dock appears as `.workspace-host`'s third
   child, the log grows, the header elapsed climbs, **Cancel** works → the row shows `⚠`/`✨ AI`
   again and the log lines from before the cancel are **still in the dock** (D2).
2. `?op=merge&aiAsk` → the row shows `?`, the dock shows the question + reply box; a reply completes
   the run and the row shows `✓ review`.
3. `?op=merge&aiFail` → one path fails, the **other rows stay clickable**.
4. **Item-5 regression, by hand:** start on file A, open file B's diff, come back — the proposal is
   still there (`✓ review` opens it in the center pane).
5. Bulk: `?op=merge` with ≥2 conflicts → "Resolve all with AI" issues **one** call
   (assert via a `window.__bonsaiMockCalls`-style counter or by the single dock entry with an
   `AiRunQueue` of n rows); `Cancel all` works.
6. `?ai=off` → the ✨AI buttons are inert and the stream command rejects `aiUnavailable` with no
   dock entry.
7. `getUiSettings()` shows the 10 new fields; Settings → AI patches each one; the dock height
   survives a reload.
8. Console clean (no new warnings/errors); `tsc` + `pnpm build` clean.

**Harness limitation (state it every time):** the Browser pane composites at 0×0, so
`document.visibilityState === "hidden"` and **`requestAnimationFrame` is paused**. Streaming
*visuals* — whether the log actually reads as live, whether the elapsed timer feels right, whether
the dock height is comfortable — are therefore **native-only**. Event **ordering and state
transitions are fully unit-testable** and are proved above; smoothness is not.

### 10.4 USER CHECKPOINT — native only (`pnpm tauri dev`, real `claude` CLI)

The orchestrator must **never** self-declare these. Full numbered list:
`docs/contracts/P68-user-checklist.md`. The four that are the point of the milestone:
- **A real run past 90 s on the item-6 repro** (the i18n JSON conflict) with a live log and a
  **working Cancel** — the old build died at 90 s with nothing to show.
- **A run where Claude asks a question** and the user's typed answer completes the resolve.
- **Read-only tools visibly consulting other files** (`⚙ Read(...)` / `⚙ Grep(...)` lines for files
  other than the conflicted one).
- **Bulk resolve on a real multi-file conflict**, one run, per-file review.

---

## 11. §I — Known limitations (state these up front)

1. **Sentinel-based questions are a convention, not a protocol.** A model that ignores it returns a
   normal (possibly incomplete) answer. That answer is still passed through
   `hasUnresolvedMarkers` and, in `autoResolve`, falls back to review instead of being staged — it
   is **never silently staged** (D4). The CLI's own `SendMessage` tool cannot be used (D9).
2. **No re-attach to an in-flight run after a window reload.** The registry lives in the Rust
   process, but the channel and the store do not survive a frontend reload; the child is left to be
   killed by the exit hook. An `ai_active_runs` listing + re-attach is deferred.
3. **Cost may be cumulative across turns**, so the UI shows the **last** `result`'s value within a
   run and never sums it; it *does* sum across sequential bulk batches (separate processes).
4. **A cancelled run returns no proposals, even for files already parsed.** Cancel is an `Err`; the
   log stands (D2) but the batch value is discarded. Partial-batch harvesting is deliberately out of
   scope — it would invite staging half-reviewed work.
5. **`partialText` is display-only.** Truncated output is not markerful, so no automated check can
   tell it from a complete resolution; it is never offered as a proposal (D2 scope).

   > **AMENDED by P68a implementation (2026-08-17)** — and **lossy by construction**: each block was
   > capped at `MAX_EVENT_TEXT` on the way in, `--include-partial-messages` deltas are excluded on
   > purpose (they would double-count the final `assistant` line), and the accumulation is capped
   > again at `MAX_PARTIAL_TEXT = 20_000` (§3.2 amendment). The **dock log** is the complete record;
   > `partialText` is only what the terminal card shows. Do not build anything that treats it as the
   > full output.

6. **`--include-partial-messages` is off by default** and its line shape is unverified; unknown
   lines degrade to `log` (D12).
7. **The idle watchdog may effectively never fire** while the CLI emits `thinking_tokens`
   heartbeats (they reset it, A4). That is intended — a thinking model is not a hung model — but it
   means the *user's* Cancel, not the watchdog, is the primary stop mechanism (which is exactly the
   locked decision).

   > **AMENDED by P68a implementation (2026-08-17)** — this is precisely why D16 is non-negotiable:
   > with no wall-clock deadline, Cancel is the *only* stop, so the loop thread must never be
   > blocked on I/O when the cancel flag flips.

8. **Only the conflict runner streams.** The other six AI features keep `AiOutputPanel` and the 90 s
   `RunOpts::default()` (D6/D14). Adopting the dock for them is a follow-up, one at a time.

---

## 12. Ambiguities resolved / flagged

Resolved while writing (recorded so a reviewer does not read them as drift):

- **A1 — `ai_resolve_conflict_stream` takes `paths: Vec<String>`, not `path: String`.** The plan's
  table sketched `(repo_id, path)` plus a separate bulk mechanism; a 4th command would have been
  needed, or the bulk parse would have leaked into TypeScript (violating D1). One command with
  `paths` keeps the count at the locked **+3**, keeps the split/attribution in Rust, and makes the
  single-file case literally `paths.len() == 1` (which still uses today's proven single-file prompt
  and payload, not the delimiter format).
- **A2 — `RunLimits` is a separate parameter of `run_claude_streaming`, not a field on `RunOpts`.**
  The plan noted that adding a field would be source-compatible; true, but a separate parameter
  touches the 13 existing call sites **zero** times and avoids a `RunOpts` whose `timeout` means
  different things depending on the callee. `run_claude_streaming` documents that it ignores
  `opts.timeout`.
- **A3 — `tool_use` maps to `kind:'log'`** with a `⚙ Name(arg)` prefix rather than a new event kind.
  Keeps the locked 7-kind union, and the checkpoint item ("read-only tools visibly consulting other
  files") is satisfied by the log line.
- **A4 — `thinking_tokens` heartbeats reset the watchdog but emit NO event.** Emitting them would
  flood the dock with noise at zero information value; swallowing them without resetting the
  watchdog would kill a legitimately thinking model.
- **A5 — D2's scope is spelled out** (§0 D2): partial text is logged and echoed as `partialText`,
  never offered as a proposal. Without this, "keep partial output" could be read as "let the user
  stage it", which is a silent-truncation hazard `hasUnresolvedMarkers` cannot catch.
- **A6 — `elapsedMs` and `turn` are REQUIRED, non-null** on `AiRunEvent` (the plan sketch had
  `elapsedMs?`). They are always known; optionality would just add `?? 0` at every call site.
- **A7 — run ids are minted without a new dependency** (`ai-<nanos hex>-<counter>`); no `uuid` crate
  is added.
- **A8 — dock height/collapsed are top-level settings (`aiDockHeight`, `aiDockCollapsed`), not new
  members of `PaneWidths`.** Reusing `PaneWidths` would have inherited `clamp_pane_widths` for free
  but put a *height* inside a struct named widths and widened an existing wire shape; top-level
  additive fields mirror the `panelDensity` precedent exactly.
- **A9 — the sentinel is recognised only as the FIRST non-empty line** of the fence-stripped result.
  A merged file body whose first line is `BONSAI_NEEDS_INPUT:` is not a thing; a body that mentions
  the token mid-text is not a question.
- **A10 — cost is last-value WITHIN a run, SUMMED ACROSS sequential bulk batches.** Separate
  processes have independent totals, so summing there is correct; within one process the observed
  climb (0.0238 → 0.0263) means summing would double-count.
- **A11 — replayed user messages are logged as a byte count, never verbatim.**
  `--replay-user-messages` would otherwise dump the whole ≤400 KB payload into the dock log.
- **A12 — the contract is written against the POST-P67e layout:** conflicts live in
  `src/components/StatusConflictsSection.tsx` (verified present, 167 lines). Any pre-split
  `StatusPanel.tsx:339-486` line numbers quoted in the plan or the board refer to the old file.
- **A13 (ADDED by the P68a implementation, 2026-08-17) — the writer thread and the reader-first
  ordering are structural requirements, not style.** Recorded as **D16** with its two rationales
  (pipe-buffer deadlock; unkillable run) and its negative control (20.58 s deterministic
  cancellation-test failure when the write moves back onto the loop thread). Amendment blocks in
  §2a, §3.1, §3.2, §3.3, §4.1, §4.2 and §10.1 restate the affected declarations and pseudocode.

**Needs the orchestrator's (or the user's) confirmation:**

- **OQ1 — Concurrency cap.** This contract sets `AI_MAX_CONCURRENT_RUNS = 3`: each run is a separate
  CLI process (subscription rate limits, and >3 live logs is unreadable in one dock). The locked
  decisions only say a run on one file must not disable the others. **Recommend 3**; confirm before
  P68d. (1 would re-create the reported bug; unbounded invites a rate-limit wall.)
- **OQ2 — `ai_max_budget_usd` default `0.0` = no budget flag.** A default cap (e.g. `$1.00` per run)
  would be a guard rail against a runaway unbounded run, at the price of a surprising mid-run stop.
  **Recommend shipping `0.0` (opt-in)** and revisiting after the checkpoint.
- **OQ3 — `autoResolve` + bulk.** This contract stages **every** returned file that is
  marker-free, in one pass, then refreshes once; markerful/failed files fall back to review. The
  alternative is "bulk always behaves as proposeReview". **Recommend as specified** (it is what
  `autoResolve` means), but it is the first place P68 stages several files from one AI call —
  confirm.
- **OQ4 — Where the "Resolve all with AI" button lives.** Specified in **both** the conflicts-section
  header and the merge `OpBanner`. If two entry points feels like clutter, drop the banner one
  (the header is the discoverable place). **Recommend keeping both** — the banner is where the user
  looks during a merge.
- **OQ5 — Dock adoption by the other six runners.** Explicitly deferred (D14/§11.8). Confirm that it
  stays out of P68 rather than being folded into P68e.
- **OQ6 (ADDED 2026-08-17, from the P68a stub finding) — bulk interactive coverage on Windows.**
  `claude_stub.cmd` cannot do an interactive turn at ~400 KB (§10.1 amendment), so "bulk run + mid-run
  question" has **no automated Windows coverage**. Options: (a) accept it as a USER CHECKPOINT item
  only; (b) P68b adds a tiny Rust echo-helper binary used as `BONSAI_CLAUDE_BIN` for that one test.
  **Recommend (a) for P68b** (the stub stays the single fixture surface) and (b) as a follow-up if
  bulk interactivity ever regresses.
