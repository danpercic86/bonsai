# P68 — streaming/interactive/bulk AI conflict resolution: native USER CHECKPOINT checklist

Milestone P68 (user items **3–7** from 2026-08-17): **3** "Propose & review" appears to do nothing,
**4** resolve *all* conflicts with AI at once, **5** no feedback during a run and the result is
destroyed if you switch files, **6** Claude timed out at 90 s on an i18n JSON conflict, **7** watch
the AI's logs live and answer a question it asks mid-run.
Contract: `docs/contracts/P68-ai-conflict-streaming.md`.

Sub-increments (all landed — commit SHAs for cross-reference):
**P68a** `0f154b2` Rust streaming runner core (`ai/stream.rs`, `ai/session.rs`, `ai/registry.rs`,
`RunLimits`, `AppError::AiCancelled`, NDJSON stub modes) · **P68b** `451457e` streaming conflict
resolve + the 3 commands (`commands/ai_stream.rs`, managed registry, new settings, read-only
allowlist) · **P68c** `8d727de` TS types + Channel bridge + `mock/handlers/aiStream.ts` · **P68d**
`76de1bb` per-path run store `useAiRuns.ts` + row feedback (**the item-5 fix**) · **P68e** `a75a585`
the bottom dock (`AiActivityPanel` / `AiActivityLog` / `AiRunQueue`) · **P68f** `f1096aa` bulk
single-run resolve (`useBulkAiResolve.ts`, `BulkAiResolveButton.tsx`, `BulkAiConfirmDialog.tsx`) ·
**P68g-1** `96295ef` security hardening of the AI conflict surface
(`docs/contracts/P68-security-audit.md`) · **P68g-2** `44067af` AI run settings UI + honest consent
copy + ask-block hardening (`docs/contracts/P68g-ui.md`).
Supporting: `cb85e55` `useUiSettings`/`usePartialStaging` extraction · `8254b46` e2e persistence-race fix.

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

> **STATUS 2026-08-18 — every sub-increment has landed and the AI gate is GREEN at HEAD `44067af`.**
> The ⏳ markers below are the original contract-time planning notation; they are kept so each
> planned gate is still traceable, but the gate as a whole was measured and passed:
> tsc **0** · vitest **1580 passed / 128 files** · e2e **104 passed / 1 skipped / 0 failed** ·
> eslint **29 warnings, 0 errors** · `check-file-size` exit **0** ·
> cargo `bonsai-core --lib` **764 passed / 0 failed / 1 ignored** · cargo `bonsai --lib` **238 / 0** ·
> `cargo clippy --workspace --all-targets -- -D warnings` clean · IPC commands **160**.
> Pre-P68 baselines for comparison (measured 2026-08-17): vitest **1344/0** across 111 files after
> P67a; cargo workspace green **except** the one pre-existing load-sensitive flake noted below (now
> fixed in P68a).
> **The NATIVE section below is the only outstanding half of this milestone. Nobody has run the real
> `claude` CLI against a real conflict, and the harness cannot produce a single pixel — see above.**

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
    *(Caveat, and it is stated in the confirm dialog: if the total payload is large, Bonsai sends it
    as several sequential batches, so one click can mean more than one metered Claude run. It is
    still one dock entry with one `Cancel all` — see item 18.)*
16. **Per-file outcomes are independent.** Each file ends `✓ review` or `⚠`. A file the model failed
    on does **not** invalidate the others; you can review and stage the good ones and handle the
    failed one by hand.
17. **Cancel all works** and behaves like item 4 (log kept, nothing staged).
18. **A very large set splits into batches rather than truncating.** With many/large conflicts, the
    log announces `batch 1/2`, `batch 2/2` etc. No file is ever silently cut short; a single file too
    large to send is reported as failed for that file only.

### Settings and persistence

19. **All eight AI-run settings are present and read sensibly.** Settings → AI, the **AI runs**
    section (P68g-2, `docs/contracts/P68g-ui.md` §1.3). Tick each one:
    - **Repository access** — shows `Read-only`; toggling offers **only** `Read-only` and
      `No file access`. **No write / edit / bash option may exist anywhere in this section.**
    - **Stream the live log** — on.
    - **Include partial messages** — the finer-grained streaming toggle.
    - **Idle timeout** — checkbox on, **300 seconds**.
    - **Absolute time cap** — checkbox **off** by default (this is the "no hard timeout" decision).
    - **Maximum turns** — **6**.
    - **Spend limit** — checkbox **off** by default; when enabled it accepts decimals (e.g. `12.50`).
    - **Bulk payload cap** — in **KB**.
20. **`0` reads as a deliberate "off", not as a broken field.** For the two unbounded defaults —
    **Absolute time cap** and **Spend limit** — the value is `0` on disk, and the UI shows this as an
    **unchecked checkbox beside a dimmed number box that still displays a sensible resume value**
    (1800 s / 5 USD), plus a line explaining that with neither limit on, a run is bounded only by the
    idle timeout and Cancel. Confirm it reads as "switched off on purpose" rather than "empty/zero
    because something failed". Tick the checkbox: the resume value takes effect. Untick it: back to
    unbounded. Same for **Idle timeout** if you turn it off.
21. **Turning the live log off** still runs and still resolves; the dock shows status only, no line
    spam.
22. **Repository access → No file access** reproduces the old blind behaviour (a much less useful
    answer on the i18n conflict) and does not error. Set it back to **Read-only**.
23. **A value out of range is clamped on load, not rejected.** (Optional, only if you hand-edit the
    settings file.) Nonsense values come back as the nearest legal ones rather than breaking the
    section.
24. **The dock is collapsible and resizable, and it remembers.** Collapse it, drag its top edge to a
    comfortable height, quit the app completely, relaunch: the height and collapsed state are as you
    left them.
25. **An old settings file is not disturbed.** The first launch after updating keeps every existing
    setting — theme, pane widths, panel density, graph prefs, AI enable/consent/autonomy — and the
    new fields appear with their defaults.

### Consent, disclosure, and the read-only fence (P68g-1 · P68g-2)

Copy is specified verbatim in `docs/contracts/P68g-ui.md` §2; the security findings behind it are in
`docs/contracts/P68-security-audit.md`. Read the words on screen and confirm they match reality.

26. **The consent dialog now tells the truth.** Turn AI features off, then on again to re-open
    **Enable AI features?**. It must state four things, in this order:
    (a) resolution runs through the **Claude Code CLI on this machine, under your Claude
    subscription, and nothing goes to Bonsai's own servers**;
    (b) Claude receives the conflicting versions **and may read other files in this repository**,
    and **whatever it reads is sent to Anthropic with the request**;
    (c) its tools are **read-only** — no writing, no staging, no commands — and **reads outside the
    repository folder are refused, with refusals shown in the AI activity dock**;
    (d) Bonsai changes files only when you apply a result, **except** `Resolve automatically`, which
    writes and stages with no review step.
    The old copy claimed only conflicted-file contents were sent and that nothing changes without
    review — both were false. The `Enable` button is the normal primary style (**not red**), focus
    starts on **Cancel**, and Esc cancels.
27. **The `Resolve automatically` caveat is visible at the point of choice.** Settings → AI
    assistance shows a hint under **each** autonomy radio, both always visible — `Propose & review`
    says nothing is written or staged until you apply it; `Resolve automatically` says marker-free
    results are written and staged **with no review step**, and markerful ones open as proposals
    instead. You should be able to make the choice without opening the consent dialog.
28. **The bulk dialog does not promise a single run.** `Resolve all with AI` → the confirm dialog
    says Bonsai makes **one or more** Claude runs one after another depending on total size, that
    Claude may also read other repository files, and that **Cancel all** stops the rest. It must not
    claim "one AI run" or "runs the Claude CLI once".
29. **An out-of-repository read is refused, visibly.** Give the model a reason to look outside the
    repo — e.g. a conflicted file whose content references an absolute path such as
    `C:\Users\...\other-project\...` or `../../secrets.env`, or answer a mid-run question by naming a
    file outside the repository. When it tries, the dock shows a **refusal line** for that read and
    the run continues. Nothing outside the repository folder is ever read.
    *(This is explicitly flagged as native-only in `P68g-ui.md` §5 — the harness cannot spawn a
    process, so no one has seen a real refusal line yet.)*
30. **Nothing prompts you for a secret.** The ask block in the dock always carries the fixed guard
    line that Bonsai never asks for passwords or tokens, and attributes the question to the model
    (untrusted output), even when the model phrases a question as a request for a token. Do not paste
    a credential into the reply box — confirm the warning is there instead.

### Regression sweep

31. **The dock does not steal space when unused.** With no AI run ever started in this session, the
    graph and the two side panels are exactly as tall as before P68 — no empty strip at the bottom.
32. **The graph still scrolls smoothly** while an AI run streams in the background (log lines must
    not cause visible hitching in the canvas).
33. **The other AI features are unchanged.** Generate commit message, Explain commit, Ask Bonsai,
    What changed, Changelog and Blame-why all still work exactly as before, in the same output card
    — they intentionally did **not** move to the dock in P68.
34. **Merge completion still works.** After resolving all conflicts (some with AI, some by hand),
    **Commit merge** from the banner succeeds, and `Abort` still aborts cleanly.
35. **Nothing is left running.** Start a long AI run and **close the Bonsai window** while it is in
    flight. Check Task Manager / `ps`: no orphaned `claude` or `node` process is left behind.
    *(With no hard timeout, this is the guard that matters.)*
36. **Cross-platform, if available.** Repeat items 1, 4 and 7 on macOS or Linux: the streaming run,
    Cancel and the reply box behave the same (the Windows `.cmd` shim path is the special case, and
    it is the primary test target).
37. **Focus rings on the new controls** render correctly in the native window (also flagged
    native-only by `P68g-ui.md` §5).

---

## The four runs that matter most

If you only have time for a short pass, do these four — they are the user-reported failures:

- **A** — item 6: a real-CLI conflict run that goes **past 90 seconds** with a **live log** and a
  **Cancel that actually stops it**, keeping the lines already logged (NATIVE items 1–4).
- **B** — item 7: a run where **Claude asks a question mid-run** and your typed answer carries the
  same run through to a proposal (NATIVE items 7–8).
- **C** — item 6's real cause: **read-only tools visibly consulting other files** in the repository
  (`Read` / `Grep` / `Glob` naming files other than the conflicted one) (NATIVE item 5).
- **D** — item 4: **`Resolve all with AI`** on a genuine multi-file conflict — the reported repro was
  an **i18n JSON set** — reasoning across the files, with independent per-file outcomes
  (NATIVE items 14–18).

---

## Harness seeds (optional, for driving the settings/dialog states yourself)

These are browser-harness states, not native ones: run `pnpm dev` with `VITE_MOCK_IPC=1` and seed
`localStorage['bonsai.mockUiSettings']`. They are documented in full — seed and what each proves — in
**`docs/contracts/P68g-ui.md` §5, states 1–9**. Short form:

| # | Seed | Drives |
|---|---|---|
| 1 | default blob (`aiEnabled:false`) | AI-runs section inert, with the "turn on Enable AI features" note |
| 2 | `{aiEnabled:true, aiConsented:true}` | every control live, all defaults on screen |
| 3 | `{…, aiHardCapSecs:0, aiMaxBudgetUsd:0, aiIdleTimeoutSecs:0}` | the three "0 = off" sentinel rows + the no-limits explainer (NATIVE item 20) |
| 4 | `{aiIdleTimeoutSecs:5, aiBulkMaxBytes:9999999, aiMaxBudgetUsd:-3, aiConflictTools:'bogus'}` | load-time clamping (NATIVE item 23) |
| 5 | `{aiEnabled:false, aiConsented:false}` then click **Enable AI features** | the corrected consent dialog (NATIVE item 26) |
| 6 | availability `installed:false` with `aiEnabled/aiConsented:true` | exactly one CLI-not-found note |
| 7 | bulk confirm with 200 long paths | the bulk dialog's scrolling path list + its three notes (NATIVE item 28) |
| 8 | `[data-theme='light']` at 900px wide | both dialogs + the section in light theme |
| 9 | the `aiStream` mock ask script with a token-asking question | the ask block's attribution + the never-asks-for-secrets guard (NATIVE item 30) |

The two things §5 marks as **not** harness-verifiable are NATIVE items **29** (a real refused read in
the dock) and **37** (native focus rings).
