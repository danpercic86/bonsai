# Tasks: P120 — Performance & render-stability measurement loop (parts a + c)

**Plan:** ./plan.md
**Status:** ready

Scope: part a (lab detection and ratchets) and part c (reviewer). Part b (dev-mode field records
and anomaly rules) gets its tasks from `docs/contracts/P120-stability-signals.md` once the
architect has ratified it; it is not listed here.

## Track R: Rust layout counters
- [x] 1. Behaviour-preserving move of `layout_walk` (and any private helpers only it uses) out of
  `crates/bonsai-core/src/graph.rs` (499 lines) into a new `crates/bonsai-core/src/graph/walk.rs`.
  No signature or behaviour change. Test counts must be identical before and after
  (`cargo nextest run -p bonsai-core`). — `crates/bonsai-core/src/graph.rs`,
  `crates/bonsai-core/src/graph/walk.rs` — owner: refactorer
- [x] 2. `LayoutCounters` and counted entry point:
  - new `crates/bonsai-core/src/graph/counters.rs` with the counter fields named in plan.md
    §New files → Part a (Rust)
  - thread `&mut LayoutCounters` through `layout_walk` (walk.rs) and `LaneWalker::step`
    (`crates/bonsai-core/src/graph/lane.rs:131`), passing a throwaway default on the uncounted
    path
  - `#[doc(hidden)] pub fn compute_graph_counted`
  - `crates/bonsai-core/tests/graph_layout_counts.rs`: small deterministic fixture, run twice and
    assert equal, compare to `crates/bonsai-core/tests/fixtures/layout-counts-baseline.json`;
    `BONSAI_UPDATE_PERF_BASELINE=1` may only lower values
  - seed the baseline.

  Files: `crates/bonsai-core/src/graph/{counters,walk,lane}.rs`, the test and fixture paths above.
  — owner: senior-dev (after 1)

## Track F: frontend instrumentation (lab side)
- [x] 3. [P] Shared stability detector, `src/obs/stability/`:
  - `regions.ts`: `REGIONS` plus `regionOf(node)`
  - `phase.ts`: `markInteractive()` and `currentPhase()`
  - `layoutShift.ts`: guarded by `PerformanceObserver.supportedEntryTypes?.includes('layout-shift')`
    so it installs nothing on WebKit; per-source region attribution; `hadRecentInput`;
    `resizing` = a resize within 500 ms; coalesced per region per frame
  - `focusLoss.ts`: the classifier truth table from plan.md §New files → `focusLoss.ts`
  - `sinks.ts`: `addStabilitySink(fn): () => void`, the fan-out, and a `startStability()` /
    `stopStability()` lifecycle.

  **The sink event type is the interface part b attaches to.** Shape it as
  `{ kind: 'layout.shift', region, phase, value, hadRecentInput, resizing }` or
  `{ kind: 'focus.lost', fromRegion, to, phase }`, which matches plan.md §Data model. Do **not**
  implement the dev-mode `logRecord` sink (that's part b), and do not touch `src/obs/types.ts`,
  `src/ipc/types/obs.ts` or any Rust.

  Vitest for each file, including the unsupported-API path. — `src/obs/stability/*.ts`,
  `src/obs/stability/*.test.ts` — owner: senior-dev
- [x] 4. Wiring and mock-mode lab hook:
  - `data-obs-region` on the pane roots, per the plan.md region table. Put the attribute on an
    **existing** element and line; `WorkspaceGraphPane.tsx` (505) is size-baselined and must not
    grow, and don't touch `RepoWorkspace.tsx`.
  - call `markInteractive()` at the site that knows the first graph `meta` **and** the first
    status round have landed
  - start the detector at boot, in mock mode only, for now (part b extends this to dev mode)
  - `window.__bonsai.perf = { reset(), snapshot() }` in `src/graph/useMockDevHooks.ts`
  - per-command IPC call counter in `src/ipc/mock.ts`
  - React `<Profiler>` commit counter at the app root, only when `VITE_MOCK_IPC=1`
  - query flags `?perfInjectShift=<region>` / `?perfInjectFocusSteal=1`, mock only, which inject a
    post-interactive shift or a focus drop for the detector proof.

  Keep `pnpm dev:mock` console-clean. — the pane components named in plan.md,
  `src/graph/useMockDevHooks.ts`, `src/ipc/mock.ts`, the app root — owner: senior-dev (after 3)

## Track E: Playwright perf project and ratchet
- [ ] 5. Perf project:
  - `playwright.config.ts`: a `perf` project with `testDir: 'e2e/perf'`, `workers: 1` and
    `fullyParallel: false`; the existing project gets `testIgnore: 'perf/**'`
  - `e2e/perf/perfHelpers.ts`: `resetPerf`, `snapshotPerf`, `withCdpMetrics` (CDP
    `Performance.getMetrics` deltas of `RecalcStyleCount` / `LayoutCount`), `writeCounts` →
    `test-results/perf-counts.json`, keyed by browser channel
  - `e2e/perf/interactions.spec.ts`: the seven interactions from spec.md resolved decision 2. Each
    runs three times in fresh pages, after `waitForGraphSettled` and Refresh-enabled; a metric
    that disagrees across the runs becomes `"nondeterministic"`. Reuse the `e2e/helpers.ts`
    helpers: `openRepo`, `clickGraphRow`, `openPalette`, `openBranchContextMenu`,
    `waitForGraphSettled`.
  - `e2e/perf/detector.spec.ts`: the 20/20 proofs using the task-4 flags (AC 2, AC 3)
  - `package.json`: `test:perf`.

  Measure and report the project's runtime. — owner: senior-dev (after 4)
- [ ] 6. Ratchet checker and gate:
  - `scripts/check-perf-baseline.mjs`, modelled on `scripts/check-file-size.mjs`:
    - fail on any count above its ceiling, naming the interaction, metric, baseline and new
      value
    - pass when counts drop, with a hint to lower the baseline
    - `--update-baseline` lowers only, never raises
    - skip `"nondeterministic"` with a warning
    - with no counts file, fail with instructions
  - `scripts/check-perf-baseline.test.mjs`
  - `package.json`: `lint:perf`
  - `scripts/gate.mjs`: two steps, `perf counts` and `perf ratchet`, in group `e2e`, after
    `playwright e2e`
  - seed `scripts/perf-baseline.json` from a real task-5 run. Today's values, non-zero
    shift/focus counts included, are the starting ceiling (resolved decision 1).

  — owner: senior-dev (after 5)

## Track C: reviewer (part c)
- [x] 7. [P] `.claude/agents/perf-reviewer.md` and `.claude/commands/perf-review.md`, per plan.md
  §New files → Part c:
  - read-only tools
  - per-OS log directory (`<app_config_dir>/logs`)
  - newest `*.jsonl` by default, or a path argument
  - check truncation first
  - ranked findings with `refs`, a `file:line` hypothesis and a proposed failing check
  - explicit no-findings result
  - report to `D:\Data\Temp\bonsai-perf-review\`
  - never files board items.

  Add a one-line `perf-reviewer` bullet to `CLAUDE.md` → "On-demand specialists" (Edit tool).
  — owner: orchestrator (agent and command config, not application code)

## Review, test, verify
- [x] 8. (Approved together with Track F in `1e2b538`; the Track E review is still owed under
  task 9.) Review the Track R diff (tasks 1–2): behaviour preserved, counter placement, no
  per-commit overhead worth measuring on the uncounted path. — owner: reviewer
- [ ] 9. Review the Track F + E diff (tasks 3–6): production builds unaffected (mock-only guards),
  no size-baseline growth, the sink interface matches plan.md §Data model, the checker's
  lower-only guarantee, gate tier placement. — owner: reviewer (run concurrently with 8)
- [ ] 10. Tests and determinism evidence:
  - `pnpm test:perf` twice in a row → identical `perf-counts.json` (AC 4); report which metrics
    were marked nondeterministic
  - detector 20/20 both ways (AC 2, AC 3)
  - checker unit test
  - `graph_layout_counts` (AC 7)
  - a hand-raised count in a temp baseline copy fails the checker (AC 5).

  Report the numbers only. — owner: tester
- [ ] 11. AC 8 evidence (count tracks clock): try one safe reduction of `lane_scans` in
  `LaneWalker::step`, then show the counter drop plus the Criterion `compute_graph_31k` result
  before and after. If there's no safe reduction, record the explicit deferral on the P120 board
  entry, per spec AC 8. — owner: senior-dev, then reviewer
- [ ] 12. Integrate:
  - `pnpm gate` (bare, the default tier, which now includes perf); paste the output
  - browser harness check: `pnpm dev:mock`, then `__bonsai.perf.snapshot()` after one interaction
  - commit `wip(spec-008): …` per approved track
  - update the P120 entry in `TODO.md`
  - tick this file.

  — owner: orchestrator

## Notes
- **Spawn batching** (CLAUDE.md token discipline):
  - task 1 alone (refactorer; never mixes with behaviour change)
  - task 2 alone
  - tasks 3 + 4 as **one** senior-dev spawn (same subsystem)
  - tasks 5 + 6 as **one** senior-dev spawn
  - Tracks R and F/E touch disjoint files, so they can run concurrently (cap 2).
- **Pass every subagent** `spec.md` + `plan.md` paths and its task numbers. Tell it to take shapes
  from plan.md §Data model verbatim, not re-derive them. For tasks 3 + 4 also pass
  `docs/contracts/P120-stability-signals.md` **if it exists by then**: the architect may amend the
  sink event shape, and the contract wins over plan.md.
- **Size discipline:** any file that would cross 500 lines is split in the same task.
  `graph.rs`, `WorkspaceGraphPane.tsx` and `RepoWorkspace.tsx` must not grow.
- **No part b work here.** `src/obs/types.ts`, `src/ipc/types/obs.ts` and `src-tauri/src/obs/**`
  stay untouched until the part b task list exists.
- **Browser channel:** on Windows, local runs use `msedge`. The baseline is keyed by channel, so
  the first run on a new channel seeds that channel rather than failing.
- **Intermediate rounds** run targeted checks only (`vitest` on the touched files,
  `pnpm test:perf`, `cargo nextest -p bonsai-core`, `tsc`). The bare `pnpm gate` is task 12 only.
- **No USER CHECKPOINT in parts a + c** beyond trying `/perf-review` on a real session log; the
  native-window checkpoint belongs to part b.
