# P68 — streaming/interactive/bulk AI conflict resolution: native USER CHECKPOINT checklist

Milestone P68 (user items **3–7** from 2026-08-17): **3** "Propose & review" appears to do nothing,
**4** resolve *all* conflicts with AI at once, **5** no feedback during a run and the result is
destroyed if you switch files, **6** Claude timed out at 90 s on an i18n JSON conflict, **7** watch
the AI's logs live and answer a question it asks mid-run.
Contract: `docs/contracts/P68-ai-conflict-streaming.md`.

Sub-increments: **P68a** Rust streaming runner core (`ai/stream.rs`, `ai/session.rs`,
`ai/registry.rs`, `RunLimits`, `AppError::AiCancelled`, NDJSON stub modes) · **P68b** streaming
conflict resolve + the 3 commands (`commands/ai_stream.rs`, managed registry, 10 new settings,
read-only allowlist) · **P68c** TS types + Channel bridge + `mock/handlers/aiStream.ts` · **P68d**
per-path run store `useAiRuns.ts` + row feedback (**the item-5 fix**) · **P68e** the bottom dock
(`AiActivityPanel` / `AiActivityLog` / `AiRunQueue`) · **P68f** bulk single-run resolve · **P68g**
Settings UI + docs + board.

**Command count: 157 → 160** (+3, all in P68b: `ai_resolve_conflict_stream` = Channel,
`ai_cancel_run`, `ai_reply_run`). Existing `ai_resolve_conflict` stays registered and unchanged.

This splits the P68 acceptance into what the orchestrator can prove by AI gate versus what only a
human at the native window, with the real `claude` CLI and a real merge conflict, can confirm.
**The orchestrator must never self-declare the NATIVE section** — present the AI-gate evidence, then
ask the user to run `pnpm tauri dev`.

## Harness limitation (why these are USER CHECKPOINTs)

The mandatory browser harness (`pnpm dev` + `VITE_MOCK_IPC=1`) runs **headless**: the Browser pane
composites at 0×0, so `document.visibilityState === "hidden"` and the browser **pauses
`requestAnimationFrame`**. Event **ordering, state transitions, the log cap, the batching and the
cancel/reply plumbing are all fully machine-provable** through the mock (`?aiSlow` / `?aiAsk` /
`?aiFail` / `?ai=off`) — and they are, below. What cannot be proved there:

- whether a **live log actually reads as live** at real CLI output speed,
- whether the **elapsed timer and the dock height** feel right at the user's display/DPR,
- anything involving the **real `claude` CLI**: a run past 90 s, real read-only tool use against a
  real repository, a real mid-run question, real cost.

The mock never spawns a process. Every item in the NATIVE section requires the real binary.

---

## AI GATE — the automated half (listed for context, do NOT re-ask the user)

> **STATUS DISCIPLINE:** this section is written at contract time, **before any P68 code exists**.
> Every line below is a **planned** gate, not a result. Tick an item only once its sub-increment has
> actually landed and the number has been measured. Do not present any of it to the user as evidence
> until then.
> **P68a ⏳ NOT YET IMPLEMENTED · P68b ⏳ · P68c ⏳ · P68d ⏳ · P68e ⏳ · P68f ⏳ · P68g ⏳.**
> Pre-P68 baselines to compare against (measured 2026-08-17): vitest **1344/0** across 111 files
> after P67a; cargo workspace green **except** the one pre-existing load-sensitive flake noted below.

Backend (cargo):
- **[P68a ⏳]** `classify_line` known answers for **every** row of the contract §3.2 NDJSON mapping
  table, including a non-JSON line and an unknown `type` → `kind:'log'`, **never** an error.
- **[P68a ⏳]** `sentinel_question`: positive, negative, and "token appears mid-body ⇒ `None`".
- **[P68a ⏳]** `parse_result_envelope` reproduces all five branches of the old inline parse
  (`ai/mod.rs:332-366`) — the extraction is behaviour-free.
- **[P68a ⏳]** Stub-driven session tests: `stream_success` emits
  `started → log+ → turnEnd → done` with monotonic `seq` from 0 and `runId` on seq 0;
  `stream_ask` completes after a programmatic reply; `stream_partial` ⇒ `aiFailed`;
  `stream_garbage` still succeeds.
- **[P68a ⏳]** **The D2 guards:** an idle-watchdog fire and a mid-run cancel each leave a
  **non-empty** collected log (today's `run_process` returns empty output on timeout —
  `ai/mod.rs:180-190`), and no child survives a cancel.
- **[P68a ⏳]** **The D3 guard:** with `idle_timeout = 1s`, a run parked on `awaitingInput` for 3 s
  still completes after the reply — the watchdog must never fire while waiting on a human.
- **[P68a ⏳]** Turn budget: the `(max_turns + 1)`-th question fails with a message naming the count.
- **[P68a ⏳]** The pre-existing flake `ai::tests::run_claude_slow_times_out_and_reaps_child`
  (`ai/mod.rs:577`) is rewritten to a monotonic lower bound + a generous upper bound. It measured
  **2.97 s** for a 1 s deadline under parallel load at the P67 baseline — **not** a P68 regression.
- **[P68b ⏳]** `parse_bulk_response`: happy 3-file, missing path ⇒ `failed`, unknown extra path ⇒
  ignored, markerful body ⇒ `failed`, zero blocks with >1 requested ⇒ `aiFailed`, fenced bodies
  stripped. `pack_batches` splits by the byte cap and marks a single oversize file `failed`
  **without truncating**.
- **[P68b ⏳]** `ai_resolve_conflict_stream_inner` rejects `aiUnavailable` when consent is off,
  **before** touching the repo; empty `paths` ⇒ `aiFailed`.
- **[P68b ⏳]** `--tools` is `Read,Grep,Glob` by default and `""` under `aiConflictTools: 'none'`
  (asserted through an argv-echoing stub mode).
- **[P68b ⏳]** camelCase wire-shape tests for `AiRunEvent` / `AiResolveBatch` / `AiResolveFailure`;
  `ai_conflict_tools_roundtrips_both_variants`;
  `old_settings_file_without_ai_run_fields_loads_defaults` (the **no version bump** guard); a
  `set_ui_settings` arm proving the new fields patch independently of `graph`/`listView`/
  `panelDensity`.
- `cargo clippy --workspace --tests -- -D warnings` clean; `clamp_graph_prefs` untouched;
  `commands/ai.rs` unmodified and its 13 `RunOpts::default()` sites still on the 90 s default.
- **[P68b ⏳]** `generate_handler!` recounted: **160**.

Frontend (vitest):
- **[P68c ⏳]** Mock stream: default run's event order; `?aiSlow` + `aiCancelRun` ⇒ `cancelled` event
  + `aiCancelled` rejection; `?aiAsk` ⇒ `awaitingInput` then completion after `aiReplyRun`;
  `?aiFail` single ⇒ rejection, bulk ⇒ one `failed[]` entry with the rest in `proposals[]`;
  **`?ai=off` ⇒ `aiUnavailable`** (which today's `mock/handlers/ai.ts:29` ignores);
  `aiCancelRun('unknown')` resolves.
- **[P68d ⏳]** **The item-5 regression pair:** (a) a run on file A never disables file B's ✨AI
  button; (b) a proposal computed while `fileDiffReqId` was bumped by an unrelated diff open is
  **still in the store** and re-openable.
- **[P68d ⏳]** D5: 300 log events ⇒ ≤1 state commit per 50 ms and ≤500 retained lines with
  `logDropped > 0`; a stale/duplicate `seq` is ignored.
- **[P68d ⏳]** D8: `runId` is taken from the first (`started`) event, and a cancel requested before
  it arrives still fires.
- **[P68d ⏳]** `aiCancelled` ⇒ `status: 'cancelled'` and **no error toast**; `autoResolve` with a
  markerful body still falls back to review with the existing message; `proposeReview` pushes the new
  "proposal ready" toast **and** opens the review editor.
- **[P68d ⏳]** Reviewer greps: no `aiResolvingPath` remains anywhere; no `fileDiffReqId` bump
  precedes any `ipc.ai*` call.
- **[P68e ⏳]** `AiActivityPanel`: `runs: []` renders nothing; per-status header controls; Cancel
  fires; the reply form exists only for `awaitingInput` and submits on click and Ctrl+Enter;
  collapse keeps the header; the dropped-lines note renders.
- **[P68f ⏳]** "Resolve all with AI" issues **exactly one** `aiResolveConflictStream` call with all
  eligible paths (the locked "one run" decision); ineligible kinds excluded; per-file failures mark
  only their own rows; `Cancel all` works.
- **[P68g ⏳]** Settings: each control patches exactly its own field; the "Repo access" control offers
  **only** `Read-only` and `None` — no write option may exist.

Harness (mock IPC, `?op=merge`; seed `localStorage.bonsai.mockUiSettings` with `aiConsented: true`
and reload, or the ✨AI buttons stay dark):
- **[P68e ⏳]** The dock is `.workspace-host`'s **third** element child with `flex: none`, and
  `.panes` height shrinks by exactly the dock height; the height survives a reload.
- **[P68d/f ⏳]** `?aiSlow` → live log + working Cancel, with the pre-cancel lines **still visible**;
  `?aiAsk` → reply completes the run; `?aiFail` → one row fails and the others stay clickable; bulk
  over 3 files → one dock entry with an n-row queue.
- Console clean; `tsc` + `pnpm build` clean.

---

## NATIVE — user must confirm in `pnpm tauri dev` (real `claude` CLI + a real merge conflict)

Set-up: open a repo with a **real conflicted merge**, ideally the item-6 repro (the i18n JSON
conflict that used to time out). Settings → AI must show the Claude Code CLI as found, with AI
enabled and consented.

### The core fixes (items 5, 6, 7)

1. **A run goes past 90 seconds and does not die.** Start ✨AI on the i18n JSON conflict. It keeps
   running past 90 s — the old build failed at exactly 90 s with nothing to show. There is no
   timeout error.
2. **The log is live.** The bottom dock appears, full width, and lines arrive **while** the model
   works — session/model line, thinking, assistant text, tool calls. It reads as live, not as one
   dump at the end.
3. **The elapsed timer runs** in the dock header (and on the conflict row's button), and the cost
   appears when the turn ends.
4. **Cancel works, and nothing is lost.** Press **Cancel** mid-run: the run stops within about a
   second, the row returns to a clickable state, and **every log line from before the cancel is
   still in the dock**. Nothing gets staged.
5. **Read-only tools visibly consult other files.** In the log you can see `Read` / `Grep` / `Glob`
   calls naming files **other than** the conflicted one — that is the actual fix for item 6 (the old
   build ran with no tools at all, so Claude was blind to the repository).
6. **Nothing was written behind your back.** After a completed run and **before** you accept
   anything: `git status` in a terminal shows the conflict still unresolved and the working file
   unchanged. Bonsai only proposes.
7. **Claude asks a question and your answer completes the resolve.** On a genuinely ambiguous
   conflict (or by adding an ambiguity on purpose), the row shows `?` and the dock shows the question
   with a reply box. Type an answer, send it, and the run continues to a proposal.
8. **A question does not time out.** Leave the question **unanswered for at least 6 minutes**
   (longer than the 300 s idle watchdog), then answer it. The run must still be alive and must
   complete. *(This is the D3 invariant — never kill a run waiting on a human.)*
9. **The result survives switching files (the reported bug).** Start ✨AI on file A; while it runs,
   click other files, open their diffs, browse the graph. When the run finishes, file A's row shows
   `✓ review` and clicking it opens the proposal. **Before P68 the proposal was silently destroyed.**
10. **Other conflicts stay usable during a run.** While file A's run is in flight, file B's ✨AI
    button is **not** greyed out and starting a second run works. *(Before P68 one run disabled every
    row.)*

### Item 3 — "Propose & review" is now discoverable

11. **You can tell something happened.** With Settings → AI → Conflict resolution on
    **Propose & review**, clicking ✨AI immediately shows: the row button changes state, the bottom
    dock opens, and when the proposal is ready a **toast** says so. (Before P68 the proposal opened
    silently in the center pane and looked like nothing happened.)
12. **The proposal opens for review and stages only when you say so.** The center pane shows the
    proposed merged file; you can edit it; staging still happens through the existing accept action,
    and only then does `git status` change.
13. **Auto-resolve still works as before.** Switch to **Auto-resolve, then review**: the file is
    staged immediately and you review the staged diff. If the model leaves conflict markers, it is
    **not** staged — you get the warning and the review editor instead.

### Item 4 — bulk resolve

14. **"Resolve all with AI" exists in both places** — in the **Conflicts** section header and in the
    merge banner — and is offered only when there are at least two text conflicts.
15. **It is ONE run, not one per file.** The dock shows a **single** entry (labelled "n conflicts")
    with a per-file list, not n separate runs. The log shows the model reasoning about the files
    together — this is the whole point for a change split across files (your i18n case).
16. **Per-file outcomes are independent.** Each file ends `✓ review` or `⚠`. A file the model failed
    on does **not** invalidate the others; you can review and stage the good ones and handle the
    failed one by hand.
17. **Cancel all works** and behaves like item 4 (log kept, nothing staged).
18. **A very large set splits into batches rather than truncating.** With many/large conflicts, the
    log announces `batch 1/2`, `batch 2/2` etc. No file is ever silently cut short; a single file too
    large to send is reported as failed for that file only.

### Settings and persistence

19. **The new AI-run settings exist and read sensibly.** Settings → AI shows: idle timeout (300 s),
    hard cap (**0 = unbounded** by default), max turns (6), live log on/off, repo access
    (**Read-only** / None), bulk size cap, budget. **There is no write/edit/bash option** — confirm
    that.
20. **Turning the live log off** still runs and still resolves; the dock shows status only, no line
    spam.
21. **Repo access → None** reproduces the old blind behaviour (a much less useful answer on the i18n
    conflict) and does not error. Set it back to **Read-only**.
22. **The dock is collapsible and resizable, and it remembers.** Collapse it, drag its top edge to a
    comfortable height, quit the app completely, relaunch: the height and collapsed state are as you
    left them.
23. **An old settings file is not disturbed.** The first launch after updating keeps every existing
    setting — theme, pane widths, panel density, graph prefs, AI enable/consent/autonomy — and the
    new fields appear with their defaults.

### Regression sweep

24. **The dock does not steal space when unused.** With no AI run ever started in this session, the
    graph and the two side panels are exactly as tall as before P68 — no empty strip at the bottom.
25. **The graph still scrolls smoothly** while an AI run streams in the background (log lines must
    not cause visible hitching in the canvas).
26. **The other AI features are unchanged.** Generate commit message, Explain commit, Ask Bonsai,
    What changed, Changelog and Blame-why all still work exactly as before, in the same output card
    — they intentionally did **not** move to the dock in P68.
27. **Merge completion still works.** After resolving all conflicts (some with AI, some by hand),
    **Commit merge** from the banner succeeds, and `Abort` still aborts cleanly.
28. **Nothing is left running.** Start a long AI run and **close the Bonsai window** while it is in
    flight. Check Task Manager / `ps`: no orphaned `claude` or `node` process is left behind.
    *(With no hard timeout, this is the guard that matters.)*
29. **Cross-platform, if available.** Repeat items 1, 4 and 7 on macOS or Linux: the streaming run,
    Cancel and the reply box behave the same (the Windows `.cmd` shim path is the special case, and
    it is the primary test target).
