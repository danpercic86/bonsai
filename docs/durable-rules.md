# Bonsai — durable rules

> **Live reference, not an archive.** This is board content, moved out of `TODO.md` to stop a
> 211-line block being re-read at the start of every session that does not need it. Read it when
> you are about to make a claim about tests, measurements, gates, coverage or evidence.

**Provenance.** Moved verbatim on **2026-09-23** from `TODO.md:1474-1684` at `473d9fa`, byte-identical,
by `docs-curator` on an explicit user decision (the structural option the 2026-09-16 and 2026-09-22
curator notes both escalated). Nothing was reworded, reordered or dropped. `TODO.md` keeps a pointer
section in its place.

**One sentence below predates the move and is now false in its own terms:** the opening line says the
rules are "on the board, not in the archive". They are still not in the archive — they are here, a
live reference under `docs/` — but they are no longer on the board. Left standing rather than
silently edited, because a verbatim move that quietly rewrites a sentence is not a verbatim move.

**The stories, worked numbers and measurement narrative behind these rules are archive Part 53**
(`docs/history/todo-archive-2026-09.md`) — cite the rule here, read the story there. The later
narratives are Parts 46, 51, 58, 71, 73, 76.1, 78 and 79, named per section below.

**Owner:** `docs-curator` curates the file; the rules themselves are earned by sessions and are
added by whoever earns one. Do not delete a rule because it looks obvious — every one of them was
learned by a claim that was green the whole time it was wrong.

---

## Durable lessons — the rules

The reusable rules. They are on the board, not in the archive, because every one was learned by a
claim that was green the whole time it was wrong. **The stories, worked numbers and measurement
narrative behind them are archive Part 53** — cite the rule here, read the story there.

### The six failed app-wide claims

Every one had the same shape: **a sentence claiming an app-wide property, with a call-site count that
nobody enumerated.** The first five failed for want of an enumeration. The sixth is worse — the
enumeration **existed** and was still blind, because it was **scoped by token name**.

1. **P95** — the enabled-control class; 3 escapes found by P101.
2. **P98** — "`--text-3` family closed"; 122 declarations were never classified.
3. **P74** — the hue-as-text sweep; became P105.
4. **`ui-reference.md` §2** — "6 live hue-over-own-tint instances"; the real population is **38**.
5. **P106's hand-over count of 48** for P108; the real inventory was **62**.
6. **P101** — an *exhaustive* `--text-3` audit recorded as CLOSED, which still missed a **2.96 light**
   glyph, because **`--badge-unknown` is byte-identical to `--text-3`**.

**The rule:** a bucket + verdict per call site, P101 §3 style, or it is not closed. Enumerate,
bucket, record a verdict per site, predict the post-fix residue, then verify the prediction.
**Do not accept a "~N call sites and it's fine" sentence as evidence.**

### The aliasing rule

- **An audit scoped by token NAME cannot see an alias. Scope by resolved VALUE.**
- Token aliasing has hidden instances three times: `--badge-good`/`--badge-warn` are byte-identical
  to `--success`/`--danger` (the first pair), then `--badge-unknown` to `--text-3`.
- A `var()` **fallback masking a missing token is invisible to any hue-name search**, because the hue
  name appears only in the fallback (`--warn`, which is defined nowhere).
- A naive probe of an **undefined** custom property returns the *inherited* value, not the value the
  contract cites — same trap class.

### The grep-counting rules

- **An acceptance grep counts *text*.** Prose comments and `var()` fallbacks inflate it. P106's R10
  read 0 predicted vs **12** measured: 7 hex literals quoted inside comments, 5 `var()` fallbacks.
- A residue prediction must state whether it counts **declarations or raw matches**.
- **A baseline must be measured against the real pre-fix tree**, never inherited from a prior
  contract. Both of P106's misses were the contract's *baselines* being wrong, not the fix.
- **When a grep and a prediction disagree, re-measure the baseline BEFORE touching code.**
- Where a prediction names `file:line`, **the count is the criterion** — comments added by the fix
  itself shift the lines (216/227 → 220/231 in P106; count unchanged).
- An implementer must deliberately avoid literal strings in its own comments: without that care two
  P106 counts would have read 11 and 2 instead of 8 and 1.

### The BASE rule

- **A contrast figure is meaningless without its composited base.** Every ratio must name the ink,
  the tint, **AND** the base.
- A figure naming only the tint is **incomplete evidence and may not be used to close an AC**.
- Proof: `--accent-strong` on a 14% accent tint measures **6.42/5.19 over `--bg-0`**, **5.85/4.87
  over `--bg-1`** (P107's figure) and **5.16/4.52 over `--bg-2`** (P106's) — one method, three bases.
  The two contracts never disagreed; **neither stated its base**, and the base alone accounts for
  **1.26** of dark-theme spread.

### The three P91 testing rules — now in the contract, not only in session memory

1. **A writer rule is covered only by a `LogRecord` → `append_record` → read-back round-trip.**
   Synthetic-`Value` tests are additive, never substitutive. This is what let W6 ship dead in
   production while its test passed — the one rule of six that skipped the round-trip.
2. **A validator's tests must use inputs the REAL producer emits**, and a cross-boundary vocabulary
   must be pinned by a drift test that re-derives it from the producer's own source. *A predicate
   that rejects everything is indistinguishable from one that works, unless something asserts a real
   input is ACCEPTED.* This is the `cmd.*` camelCase bug — the **entire `cmd.*` histogram family
   recorded NOTHING in production** while every test was green.
3. **A negative test must be PROVEN to fail on the unfixed code.** Saying "this must fail on today's
   code" is not proof. AC6 as originally written specified a payload the scrubber **already caught**,
   so it would have been green on the buggy code — a negative test that could not go red.

### Two more failure modes, each named after it cost a session

- **Evidence lost in transcription** (named at `ef06e6b`) — a figure or a qualifier that survives the
  measurement and dies in the summary. P108's AC11 is the live example: **source-derived and
  unverified** must travel with the number.
- **The specificity trap** — **a contrast fix that is out-specified by an existing rule is a no-op
  that still passes a grep.** `.diff-stage-float button` (0,1,1) out-specified `.diff-float-discard`
  (0,1,0), so the destructive discard button rendered in accent blue with no danger hue at all while
  passing every AC grep. Sibling to the child-rule trap; both are in `ui-reference.md` §2.

### The measurement rules (numbers: archive Part 46; the worked narrative: archive Part 53)

- **Optimising the measured-slowest test may not move wall clock, because a different test becomes
  the floor.** Net for the 2026-09-03 pass: `cargo nextest --workspace` **106.5s → 92.5s** with tests
  *increasing* 2298 → 2316.
- **Concurrent agents on this box produce outliers** — single-run numbers are worthless; use paired
  or repeated runs.
- **proptest regression seeds can be worse than useless**: proptest keys its persistence file **per
  source file, not per test fn**, and a `cc` seed regenerates values through the *current* strategy.
  Random cases wearing a regression label.
- **Do not run the e2e suite concurrently with other heavy jobs.** `gate.mjs` is strictly serial, so
  the gate itself is safe.
- **A flake that reproduces deterministically in a production bundle is not a flake** (P103).

### The gate-running rules (the gate states that earned them: archive Parts 51 and 58)

- **Before trusting any timing-sensitive failure, verify machine state AT THE TIME IT RAN** — sample
  CPU repeatedly, look for scratch/load processes, confirm no agent is mid-run.
- A slow timing number is **evidence about the machine** until proven otherwise. Timing analogue of
  the grep-counting rule: *measure the baseline, do not infer it.*
- **Verifying machine state before running the gate is a standing pre-gate step**, not a nicety.
- The e2e leg **will** fail intermittently on a loaded machine — expected, documented at P104 (Edge
  misses a hardcoded 30 s CDP close window, then a blocking `taskkill` runs).
  `FIRST_PAINT_TIMEOUT = 15 s` (`c6cd7dd`) removed the largest source, measured at 7.6× p99.
- **Run the gate on an otherwise-idle machine, or run e2e with `--workers=1`.**
- **Give subagents `--workspace`, not `-p <crate>`, whenever a change crosses a crate boundary.**
  A remediation brief scoped to `cargo nextest -p bonsai-core` never ran the `bonsai` crate's tests
  under `src-tauri/`, and one of them asserted the exact behaviour the change removed — the gate's
  first run failed on it.
- **Redirect the whole gate log to a file**; read the summary from there. Piping through `tail -60`
  lost the failure detail and cost a re-run of the Rust leg.
- **Read the `gate summary` block, never the exit status alone** — a background wrapper reported
  exit 0 while the gate had failed (that was the pipeline's exit code, not the gate's).
- **That rule applies to a bare `cargo test` too, not only to `pnpm gate` — 2026-09-11 cost proves
  it.** The orchestrator piped three `cargo test` runs through `head`/`tail`, read exit 0, saw a
  truncated log with six `FAILED` lines and no `test result:` summary, and reported six phantom
  `h_ai` failures as a possible regression. Serialized and unpiped, `h_ai` was **45/57 passed, 0
  failed** — the truncating pipe manufactured the failure *and* hid the evidence that would have
  disproved it. **A cargo run whose log has no `test result:` line has not finished; it has been
  cut off.**
- **SERIALIZE cargo. Concurrent `cargo` invocations queue on the build-directory lock, and a queued
  cargo is INDISTINGUISHABLE FROM A HANG** — several `cargo.exe`, **zero `rustc`**, negligible CPU,
  no output for 20+ minutes. That is what *waiting for a lock* looks like, not a deadlock. On
  2026-09-11 the orchestrator read that signature as wedged and killed the processes **twice**;
  they would have drained on their own, and each kill discarded build progress and forced a cold
  vendored-libgit2 rebuild. **Do not kill them. Run one cargo at a time and wait** — CLAUDE.md's
  "never conclude failure from a timeout" covers this exact case.
- **Check port 1420 after any harness-heavy pass** — `Get-NetTCPConnection -LocalPort 1420` is the
  whole check. An agent that drives `pnpm dev` by hand **orphans** it (once for **3.5 hours**,
  2026-09-14, PID 12712, 13:18:31 → 16:50:55), and `vite.config.ts` sets **`strictPort: true`**, so
  `pnpm tauri dev` then **fails outright** rather than falling back — i.e. it breaks the USER
  CHECKPOINT. Brief agents to the Playwright-managed path (`scripts/e2e-server.mjs`), never a
  hand-run dev server. `playwright.config.ts` already carries the warning. Narrative: Part 71.2.
- **❌ VOID as of 2026-09-17 (`8ad3c72`): `cargo fmt --check` IS a gate step and the tree IS clean.**
  This line used to say it was not a gate step and dirty at baseline, so never to read its output on
  a diff as a regression. All of that is now false — a fmt failure is a real regression.

### The coverage and evidence rules (earned 2026-09-14/15; narratives archive Parts 71 and 73)

- **A test that passes in the correct AND the broken state is not coverage.** Four instances in the
  P112 work alone. The negative must be proven red on the unfixed code first.
- **The limit case is a test that is never in any state.** `external_picked_tests.rs:186` is
  `#[cfg(not(debug_assertions))]`, so it compiles only under `--release`, which the gate never runs.
- **A green gate says nothing about a Rust/TS DTO change.** Every frontend tier — vitest, tsc, e2e,
  the browser harness — consumes the mock, so all of them stay green while the real app is broken.
  Only `src-tauri/src/settings_defaults_parity_tests.rs` spans the two languages, so **weakening the
  parity oracle is never routine**, and a DTO change must land its Rust and TypeScript halves in the
  **same** increment. The mock is not inventing anything in this failure mode; it is **stale**, and
  staleness is invisible to every test that consumes it.
- **A green `pnpm gate` is Windows-only evidence.** Any test that passes an explicit `TargetOs` while
  touching the real filesystem is host-bound; the absoluteness rule is what makes it so.
- **When a contract signs an error string that names a recovery, the recovery is part of the same
  contract item.** §16.4a signed `BROWSE_STALE`'s string and its report but **not its verb**, so as
  written the message named an action that did nothing.
- **Cite from the file, not from a summary.** Four citations in one brief were wrong, and one had
  already propagated a false claim into a contract — where the next reader treats it as established.
  A `file §section` pair is the dangerous shape: the section number can be right for a *different*
  file. Verify a citation **before** delegating on it.
- **Mislabelled evidence is this repository's named defect.** A test message that claims more than
  the test discriminates (`tests_tools_pick.rs:148`: "both fields in ONE update cycle", which two
  sequential `settings::update` calls would also satisfy) is a defect even when the code is right.

### 📌 TWO DURABLE CONSTRAINTS discovered in the fix pass — keep these

1. **`tracing` DOES NOT EXIST in this workspace.** My brief said "`settings.rs` has zero `tracing::`
   calls, so there is nothing to piggyback on" — that understated it: **no crate depends on
   `tracing` at all**, so there is no facade to add a call to. The project's actual non-fatal
   diagnostic facade is `eprintln!("bonsai: …")` (as in `commands::repo`, `lib.rs`,
   `commands::ui_settings`). Use that, and do not write `tracing::` into a brief again.
2. **The settings-load path CANNOT use the observability sink — a bootstrapping constraint.** The
   P91 `obs` sink is **configured from the very settings** that `load_from` is in the middle of
   reading, so it cannot be running yet when migration code executes. Any diagnostic inside
   `load_from` has to be `eprintln!`. This is a real ordering constraint, not a preference, and it
   will bite anyone who tries to route settings-layer diagnostics through `obs`.


### Rules earned 2026-09-16/17 (narratives: archive Parts 76.1, 78 and 79)

- **Commit the moment an increment is approved, even when a MUST-FIX is routed; review the fix
  against a small diff.** Earned by letting one review diff reach **41 modified + 13 new files**, of
  which the reviewer had to name **nine files it did not re-review**.
- **The fixture, not the `expect`, is where "green in both states" hides.** Three misses in one
  session were assertions with **nothing to bite on** (`changedProps` compared two empty objects;
  `settingsToastGuard` asserted on a bare `vi.fn()`; the churn fixture omitted `onReveal`). Require an
  **observed red state per case**, and on review probe a regression *other* than the induced one.
- **A claim that something is *visible* or *clickable* needs `elementFromPoint`, computed style or
  bounding boxes.** Text extraction (`innerText` / `get_page_text`) proves a string is in the DOM and
  is **blind to z-index stacking** — which is how a toast rendered under a scrim, unclickable, passed
  two code reviews and every harness pass.
- **When fixing a defect defined by a code shape, grep for the shape, not the filename.** The third
  swallowing write path (`forge_set_token`, `forge.rs:297-318`) held the **verbatim** pre-fix body and
  was found only because the auditor re-checked after the "fix".
- **A counterfactual against credential-writing code requires the DI seam FIRST.** Proving a fix red
  by running the pre-fix body wrote a test token into the user's real Windows Credential Manager.
- **`cargo fmt -p <crate>` while concurrent Rust edits are live, never `--all`.** A bare `--all`
  rewrapped two files mid-flight and reverting would have destroyed in-flight work.
- **Run `node scripts/check-file-size.mjs` before committing any pass that adds lines to a test
  file.** A 526-line test file was a **hard fail** (not baselined) and would have gone red at gate
  step 7 after a 414 s run.
- **The coverage standard (keep all four):** two counterfactuals, not one — against the **pre-fix**
  defect *and* against the **wrong fix**; compile-gated pins are **declared, not counted**; every
  "this is covered" claim needs a **mutation proof**; restored files are verified with `cmp`, not by
  eye.
- **A test that passes in the correct AND the broken state is not coverage** — of 8 new tests in one
  increment only **2** were fix coverage, and the file header overselling them was itself a finding.
- **`a_failed_connect_leaves_no_repo_override` was ALREADY red** against a production mutation that
  wrote the pin outside the injected closure, because `override_for` reads the settings **file**. The
  prescribed change produced **byte-identical** red output — execution coverage, **no new
  discriminator**. Recorded on the board's own instruction, so nobody re-derives a fix for a
  non-problem.
