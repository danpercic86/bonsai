# Plan: P120 — Performance & render-stability measurement loop

**Spec:** ./spec.md
**Status:** draft

## Approach
- **One capture module, used in two places.** A single frontend "stability" module does the
  detection:
  - watches layout shifts through the browser's Layout Instability API
  - detects focus loss from `focusout` events
  - attributes both to a named region via `data-obs-region` attributes on the five pane roots.

  The module feeds two consumers:
  1. **Field (b):** the P91 dev-mode log, as two new additive record kinds with two new sink
     anomaly rules.
  2. **Lab (a):** a mock-mode-only test hook on `window.__bonsai`, which Playwright reads.

  One detector means the lab and field signals can't disagree.
- **Lab counts.** A dedicated Playwright project runs the seven scripted interactions serially. It
  collects:
  - shifts and focus losses from the hook
  - React commits from a mock-mode `<Profiler>` at the app root
  - IPC calls from the mock dispatcher
  - `RecalcStyleCount` / `LayoutCount` from a CDP session.

  It writes a counts JSON. A checker script, modelled on `scripts/check-file-size.mjs`, compares
  that JSON to a committed ceiling baseline.
- **Rust.** Graph layout gains plain `u64` operation counters. A counts test compares them to a
  committed baseline.
- **Reviewer (c).** A read-only project agent does the reviewing, and a slash command locates the
  log and invokes the agent.
- **Rejected alternatives:**
  - Wall-clock thresholds: noisy; the spec rules them out.
  - Valgrind instruction counts: Linux-only.
  - A separate "stability mode" setting: resolved decision 6 rules it out.
  - Reusing `render.tally` for lab commit counts: it's windowed at 500 ms and gated on dev-mode
    capture, so the counts would depend on timing.

## Rust/TS boundary
- No Git logic moves anywhere, and there is no new IPC command, event or channel.
- The frontend produces the two new record kinds and ships them through the existing `log_append`
  batching path (`src/obs/log.ts` → `batcher.ts`).
- Rust owns the new anomaly rules. Cross-record correlation happens in the sink, which is the only
  place that sees both frontend and backend records (P91 §5).
- The Rust layout counters stay inside `bonsai-core`. They are **not** exported over IPC; only the
  test harness and the counts test read them.

## Files touched

### Part a: lab detection and ratchet
- `src/obs/stability/*` (new; see below): the shared detector.
- `src/graph/useMockDevHooks.ts` (63 lines): add `window.__bonsai.perf` = `{ reset(),
  snapshot() }`, which returns counts since `reset()`. It stays inert outside `VITE_MOCK_IPC=1`.
  About +15 lines.
- `src/ipc/mock.ts`: a per-command call counter, readable via `__bonsai.perf.snapshot()`, in mock
  mode only. About +10 lines.
- App root (`src/main.tsx` or `App.tsx`; senior-dev locates it): wrap the tree in `<Profiler
  id="app" onRender={countCommit}>` **only when `VITE_MOCK_IPC=1`**, so there's no production
  cost. About +8 lines.
- Pane roots get `data-obs-region="<name>"` added to an existing element, with no net new lines:

  | Region | Where |
  |---|---|
  | `sidebar` | `src/components/Sidebar.tsx` (482) |
  | `graph` | `src/components/WorkspaceGraphPane.tsx` (505, **in the size baseline, must not grow**: put the attribute on an existing line) |
  | `details` | `src/components/StatusPanel.tsx` (217) and `src/components/CommitPanel.tsx` (231) |
  | `diff` | `src/components/DiffView.tsx` (487) |
  | `dock` | `src/components/GitActivityDock.tsx` (246) and the AI dock root (senior-dev locates it via `src/components/repoWorkspace/useAiDock.ts`) |

  `RepoWorkspace.tsx` (2262, baselined) is **not** touched; the attributes go on the child
  components.
- `playwright.config.ts`: add a second project `perf` with `testDir: 'e2e/perf'`, `workers: 1` and
  `fullyParallel: false`. Exclude `e2e/perf/**` from the existing project with `testIgnore`. About
  +10 lines.
- `scripts/gate.mjs`: two new steps in the `e2e` group (default and `--full` tiers only, per
  resolved decision 3), placed after `playwright e2e`:
  - `{ name: 'perf counts', cmd: 'pnpm', args: ['test:perf'], group: 'e2e' }`
  - `{ name: 'perf ratchet', cmd: 'pnpm', args: ['lint:perf'], group: 'e2e' }`

  About +4 lines.
- `package.json`: add `"test:perf": "playwright test --project=perf"` and
  `"lint:perf": "node scripts/check-perf-baseline.mjs"`.

### Part b: dev-mode field signals
- `src-tauri/src/obs/record.rs` (503, **at its baseline ceiling**): add two newtype variants,
  `#[serde(rename = "layout.shift")] LayoutShift(LayoutShiftPayload)` and
  `#[serde(rename = "focus.lost")] FocusLost(FocusLostPayload)`. The payload structs go in the new
  `record_stability.rs`.
  - **Split in the same increment:** move a cohesive block of supporting enums (e.g. `FrameDim`
    and the other small `#[derive]` enums near the bottom) into `record_enums.rs` and re-export
    them. The goal is to bring `record.rs` under 500 and lower its baseline entry. This is a
    refactor only, with a `pub use` so no public path breaks.
- `src-tauri/src/obs/record_kind.rs` (42): two match arms.
- `src-tauri/src/obs/anomaly.rs` (399): two dispatch arms in `observe()` delegating to the new
  `anomaly/stability.rs`, plus the field on the detector struct. About +12 lines.
- `src/obs/types.ts` (272): `LayoutShiftPayload`, `FocusLostPayload`, the two `UiPayload` union
  members (`types.ts:222`), and both kinds added to that union's `Exclude<LogKind, …>` catch-all
  list. About +30 lines.
- `src/ipc/types/obs.ts` (234): add `'layout.shift'` and `'focus.lost'` to the `LogKind` union
  (`obs.ts:32`). **No `OBS_SCHEMA_VERSION` bump:** the change is additive, per the `record.rs:18`
  policy.
- Frontend boot (wherever `configureObs` is first called, found via `src/obs/enabled.ts`
  consumers): start the stability observers once. They check `obsEnabled()` at emit time and
  subscribe to `onObsConfigChange` to detach when dev mode turns off.
- The app's "interactive" moment: call `markInteractive()` from the stability module once the
  first graph `meta` **and** the first status round have both landed. This is the same condition
  e2e's `waitForGraphSettled` polls for. Senior-dev locates the existing site that knows both.
- Dev-mode disclosure copy: `src/components/settings/catalog/dev.ts` (121) help text for
  `dev.capture-react` ("Screen updates"), extended to mention layout movement and focus changes.
  Also update the "What is captured" copy in `SettingsDevCaptureSection.tsx` (145) if it lists
  kinds. **This is copy only**; ui-designer reviews the wording.

### Part c: reviewer
- `docs/specs/README.md`: no change.
- `CLAUDE.md`: add `perf-reviewer` to the "On-demand specialists" list (one bullet). Edit it with
  the Edit tool, not a script.

## New files (if any)

### Part a (lab)
- `src/obs/stability/regions.ts`: `REGIONS` constant (`sidebar | graph | details | diff | dock`)
  and `regionOf(node): Region | 'other'` via `closest('[data-obs-region]')`. About 40 lines.
- `src/obs/stability/layoutShift.ts`: a `PerformanceObserver({type: 'layout-shift', buffered:
  true})`. For each entry, per source, it records `{region, phase, value, hadRecentInput,
  resizing}`. `resizing` is true when a `resize` event fired within 500 ms. It emits to registered
  sinks. About 110 lines.
- `src/obs/stability/focusLoss.ts`: a `focusout` listener with a microtask re-check. It classifies
  the event as a loss when **all** of these hold:
  - `document.activeElement` is `body` or `null`
  - `document.hasFocus()` is true (so the window itself still has focus)
  - there was no pointer or key input in the last 500 ms
  - the new focus target isn't inside a `dialog` / `[role=dialog]` / toast / palette that just
    opened.

  It emits `{fromRegion, to: 'body' | 'none'}`. About 110 lines.
- `src/obs/stability/phase.ts`: `markInteractive()` and `currentPhase(): 'boot' | 'interactive'`.
  About 30 lines.
- `src/obs/stability/sinks.ts`:
  - a tiny fan-out: `addStabilitySink(fn)`
  - the dev-mode sink that maps to `logRecord({kind: 'layout.shift' | 'focus.lost', …})`
  - the mock-mode counter sink used by `__bonsai.perf`.

  About 80 lines.
- `src/obs/stability/*.test.ts`: vitest units, with the observer and `document` faked.
- `e2e/perf/perfHelpers.ts`: helpers for:
  - `resetPerf(page)` and `snapshotPerf(page)`
  - `withCdpMetrics(page, fn)`, which reads CDP `Performance.getMetrics` before and after and
    returns the deltas of `RecalcStyleCount` and `LayoutCount`
  - `writeCounts(results)` → `test-results/perf/perf-counts.json` (already gitignored; the perf
    project has its own `outputDir`).

  About 120 lines.
- `e2e/perf/interactions.spec.ts`: the seven scripted interactions from resolved decision 2, one
  `test` each. Each test runs its interaction **three times in fresh pages** and records a count
  only if all three runs agree. Disagreeing counts are written as `nondeterministic` and excluded
  from the ratchet (spec edge case). It reuses selectors from `e2e/helpers.ts`, for example:
  - `openRepo`, `clickGraphRow`
  - `openPalette`, `openBranchContextMenu`
  - the "Stage README.md" / "Unstage README.md" buttons
  - the Refresh button
  - the 20k fixture flag.

  About 250 lines; split per interaction group if it crosses 500.
- `e2e/perf/detector.spec.ts`: the AC 2 proof. A mock-only query flag
  (`?perfInjectShift=<region>`, handled in `useMockDevHooks.ts`) injects a deliberate
  post-interactive shift. The test repeats the check 20 times with the flag (all 20 must detect the
  shift) and 20 times without (none may). The same pattern with `?perfInjectFocusSteal` covers AC 3.
- `scripts/check-perf-baseline.mjs`: modelled on `check-file-size.mjs`.
  - It reads `test-results/perf/perf-counts.json` and `scripts/perf-baseline.json`.
  - It fails (exit 1) if any `interaction.metric` count exceeds its ceiling, printing the
    interaction, metric, baseline and new value.
  - It passes when counts drop, and prints a hint that the baseline can be lowered.
  - `--update-baseline` lowers ceilings **only**; it never raises one. Raising requires a
    hand-edit of the JSON, which then shows in the diff (AC 6).
  - If there's no counts file, it fails with instructions (`run pnpm test:perf first`).

  About 150 lines.
- `scripts/perf-baseline.json`: the first commit records today's counts (resolved decision 1).
  That includes non-zero `layoutShiftsUnprompted` / `focusLosses`, which become the work queue.

### Part a (Rust)
- `crates/bonsai-core/src/graph/counters.rs`: `pub struct LayoutCounters { rows_visited,
  lane_steps, lane_scans, lanes_allocated, edges_emitted, parent_lookups: u64 }`. It also holds
  `pub fn compute_graph_counted(workdir, filter) -> Result<(GraphLayout, LayoutCounters),
  AppError>`, which is `#[doc(hidden)]` for tests and benches. About 80 lines.
- `crates/bonsai-core/src/graph/walk.rs`: `layout_walk` moved out of `graph.rs` (499 lines, so it
  **can't absorb the counter parameter without crossing 500**). This is a behaviour-preserving
  move. `layout_walk` and `LaneWalker::step` take `&mut LayoutCounters`; the uncounted path passes
  a throwaway default.
- `crates/bonsai-core/tests/graph_layout_counts.rs`: builds a **small deterministic fixture** with
  the existing fixture builder and fixed timestamps; it doesn't use the 31k cached fixture, which
  would be slow. It runs `compute_graph_counted` twice and asserts identical counters (AC 7), then
  compares against `crates/bonsai-core/tests/fixtures/layout-counts-baseline.json`. If
  `BONSAI_UPDATE_PERF_BASELINE=1`, it rewrites the baseline, but only with lower values. It runs
  under `cargo nextest` in the rust tier.
- `crates/bonsai-core/tests/fixtures/layout-counts-baseline.json`

### Part b
- `src-tauri/src/obs/record_stability.rs`: `LayoutShiftPayload` and `FocusLostPayload`.
- `src-tauri/src/obs/record_enums.rs`: the split described above.
- `src-tauri/src/obs/anomaly/stability.rs`: the two rules. About 200 lines.
- `src-tauri/src/obs/tests_anomaly_stability.rs`: unit tests for the rules.
- `src-tauri/src/obs/tests_kind_parity.rs`: AC 14. It uses `include_str!` on
  `src/ipc/types/obs.ts`, following the same comment-stripping pattern as
  `tests_schema_parity.rs`. It parses the `export type LogKind = | '…' | …;` block and asserts
  that the set equals Rust's `LogPayload::ALL_KINDS` in both directions (a closed literal union,
  so both directions are checkable). `ALL_KINDS: &[&str]` goes in `record_kind.rs`, kept
  exhaustive by a companion test that builds one value of every variant and checks that its
  `kind()` is in the list.

### Part c
- `.claude/agents/perf-reviewer.md`:
  - Frontmatter: `tools: Read, Grep, Glob, Bash`, `model: inherit`.
  - Body rules:
    - read-only; Bash only for `jq`/`wc`/`grep`-style reading, never state-mutating
    - input is a session JSONL plus `usage.json`
    - check the `session` header's `truncated`/`droppedParts` first (AC 16)
    - group `anomaly` records by `rule`, follow the `refs` sequence numbers to the evidence, and
      rank by severity × count × user-facing impact (layout/focus first)
    - locate the suspected source with Grep/Glob by the component/cmd names in the evidence.
  - Output per finding:
    - rule/count
    - `refs`
    - evidence summary
    - suspected `file:line` marked as a hypothesis
    - a proposed deterministic check (which perf interaction or metric, or which new e2e
      assertion, would fail today).
  - An explicit "no findings" result is allowed (AC 18). It never edits files (AC 17).
- `.claude/commands/perf-review.md`:
  - It resolves the log directory from `<app_config_dir>/logs`, per `src-tauri/src/obs/mod.rs:139`:

    | OS | Path |
    |---|---|
    | Windows | `%APPDATA%\com.bonsai.app\logs` |
    | macOS | `~/Library/Application Support/com.bonsai.app/logs` |
    | Linux | `~/.config/com.bonsai.app/logs` |

  - It takes the newest `*.jsonl` by default or an explicit path argument (resolved decision 5),
    plus `../metrics/usage.json`.
  - If there's no log, it prints how to enable dev mode and stops.
  - It spawns `perf-reviewer` with the paths (not the contents), and writes the report to
    `D:\Data\Temp\bonsai-perf-review\<session-stem>.md` (global temp rule; on macOS/Linux, the
    session scratchpad).
  - It presents the ranked summary to the user. **It never files board items itself**; the
    orchestrator does that after user triage (resolved decision 4).

## Data model / types
```rust
// src-tauri/src/obs/record_stability.rs
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutShiftPayload {
    pub region: String,        // "sidebar"|"graph"|"details"|"diff"|"dock"|"other" — closed vocabulary, never a path
    pub phase: StabilityPhase, // Boot | Interactive
    pub value: f64,            // summed shift score for this region in this entry
    pub had_recent_input: bool,
    pub resizing: bool,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusLostPayload {
    pub from_region: String,   // same vocabulary
    pub to: FocusSink,         // Body | None
    pub phase: StabilityPhase,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StabilityPhase { Boot, Interactive }
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FocusSink { Body, None }
```
TS mirror in `src/obs/types.ts`:
```ts
export type StabilityRegion = 'sidebar' | 'graph' | 'details' | 'diff' | 'dock' | 'other';
export interface LayoutShiftPayload { region: StabilityRegion; phase: 'boot' | 'interactive'; value: number; hadRecentInput: boolean; resizing: boolean; }
export interface FocusLostPayload { fromRegion: StabilityRegion; to: 'body' | 'none'; phase: 'boot' | 'interactive'; }
```

**Emission rules (frontend):**
- **Layout shifts:** only when `phase === 'interactive'` or `value > 0` during boot. Emission is
  coalesced per region per animation frame, so a streaming list can't flood the log.
- **Focus losses:** only unprompted ones (the classifier above).
- **Level:** both kinds default to `info`.
- **Redaction:** region names are a closed enum, so no redaction is needed and the records are
  identical under strict and raw (AC 13).

**Anomaly rules (`anomaly/stability.rs`):**

| Rule id | Window | Condition | Severity | refs |
|---|---|---|---|---|
| `unprompted-shift` | 1 s per region | `layout.shift` with `phase: interactive`, `!hadRecentInput`, `!resizing`; one anomaly per region per window, `detail` = count + summed value | warn | all shift seqs in the window |
| `focus-steal` | look-back 1000 ms | `focus.lost` with a `refresh` or `watcher` record in the look-back window | warn | the focus.lost seq + the refresh/watcher seq(s) |
| `focus-steal` (no cause) | same | `focus.lost` with nothing in the look-back window | info | the focus.lost seq (still surfaced, cause unknown) |

- `focus-steal` records carry `traces` from the refresh's `contributing_traces`.
- Both rules key only on kinds, regions and timing, never on ordinals (P91 §5 invariant).

**Lab counts file (`test-results/perf/perf-counts.json`) and baseline (`scripts/perf-baseline.json`):**
```json
{ "_comment": "...", "interactions": {
    "select-commit": { "reactCommits": 7, "ipcCalls": 3, "recalcStyle": 12, "layout": 4,
                       "layoutShiftsUnprompted": 0, "focusLosses": 0 } } }
```
In the counts file, a metric may be the string `"nondeterministic"`. The checker ignores it and
prints a warning; the baseline never stores it.

**Rust `layout-counts-baseline.json`:** `{ "rows_visited": N, "lane_steps": N, ... }`.

## Testing
- **vitest (`src/obs/stability/*.test.ts`):**
  - region attribution (nested, missing attribute → `other`)
  - phase transition
  - focus classifier truth table: window blur, dialog open, recent input, body focus
  - shift coalescing
  - the dev-mode sink is a no-op when `obsEnabled()` is false (AC 12)
  - the mock counter sink resets.
- **Rust:**
  - `tests_anomaly_stability.rs`:
    - shift with `hadRecentInput` → no anomaly (AC 11)
    - resizing → none
    - boot phase → none
    - three shifts in one region in 1 s → one anomaly with three refs
    - focus.lost after a refresh → warn with both refs and the refresh's traces (AC 10)
    - focus.lost with no cause → info
    - strict and raw fixtures produce identical anomalies (AC 13).
  - `tests_kind_parity.rs` (AC 14).
  - `graph_layout_counts.rs` (AC 7).
  - Existing `tests_record.rs` / `tests_schema_parity.rs` stay green after the `record.rs` split,
    with **identical test counts before and after the split** (refactorer rule).
- **e2e perf project:**
  - `detector.spec.ts`: the 20/20 proofs (AC 2, AC 3)
  - `interactions.spec.ts`: three-run agreement (AC 4)
  - the checker fails on a hand-raised count in a temp copy of the baseline (AC 5), a unit-style
    test in `scripts/`, like `e2e-teardown-reporter.test.mjs`
  - `--update-baseline` refuses to raise a ceiling (AC 6).
- **AC 8 (count tracks clock):** a one-time evidence step, not a gate step.
  - Pick the Rust `lane_scans` counter.
  - Make one targeted reduction in `LaneWalker::step`, if the reviewer agrees a safe one exists.
  - Show the counter drop plus the Criterion `compute_graph_31k` before/after.
  - Record the numbers in the P120 board entry.
  - If no reduction is available in P120's scope, AC 8 moves to the first follow-up that lowers a
    count, and the board says so explicitly.
- **Browser harness (orchestrator, AI gate):**
  - `pnpm dev:mock`, run `__bonsai.perf.snapshot()` after a scripted interaction, and read the
    console for errors
  - enable dev mode in the mock and confirm that `layout.shift` / `focus.lost` records reach the
    mock `log_append` handler.
- **USER CHECKPOINT:**
  - In `pnpm tauri dev` with dev mode on, work normally for a while.
  - Confirm the log contains `layout.shift` records attributed to real regions.
  - Run `/perf-review` and judge whether its top findings match jumps the user actually saw.

## Risks / open questions
- **The Layout Instability API is Chromium-only.**
  - It works in WebView2 (Windows) and in the Edge/Chromium e2e project.
  - It does **not** exist in macOS WKWebView or Linux webkit2gtk.
  - `layoutShift.ts` must guard with
    `PerformanceObserver.supportedEntryTypes?.includes('layout-shift')` and install nothing
    otherwise, so it never throws on WebKit. A vitest case covers the unsupported path.
  - Consequence: in v1, the `layout.shift` field signal and the `unprompted-shift` rule are
    **Windows-only**. `focus.lost` works on all three OSes, because `focusout` is universal.
  - The lab ratchet is unaffected, since it always runs on Chromium.
  - The Dev-page disclosure copy must not promise layout capture on macOS/Linux (ui-designer
    wording).
- **Determinism of browser counts (highest risk).** React commit, recalc-style and layout counts
  can vary with timer interleaving:
  - mock stream batches are 30 ms apart
  - there's a 300 ms watcher debounce and a 500 ms render-tally window.

  Mitigations:
  - `workers: 1` in the perf project
  - wait for `waitForGraphSettled` plus Refresh-enabled before `reset()`
  - three-run agreement; disagreeing metrics are dropped, not loosened (spec edge case).

  If most metrics turn out nondeterministic, fall back to Playwright's `page.clock` to freeze
  timers during the measured window. That's a senior-dev call, reported back.
- **Edge vs Chromium locally.** On Windows the project uses the `msedge` channel. CDP metrics work
  on both, but absolute counts may differ between Edge and bundled Chromium, and between CI and
  local runs. The baseline is recorded on the gate's canonical browser. If CI uses Chromium and
  local runs use Edge, the checker keys baselines per browser channel (`interactions[channel]`).
  The first run on each channel seeds that channel.
- **Gate time.** This adds a serial Playwright project, about 7 interactions × 3 runs plus 2 × 20
  detector runs. Estimated at 1–3 minutes on top of the default gate; measure it in the first
  increment. If the detector's 20× runs are too slow for the gate, they move to a manual
  `pnpm test:perf --grep detector` proof. They're one-time evidence per AC 2, not a per-commit
  check.
- **Spec correction (AC 14):** the spec says the schema version is "updated". Per the existing
  policy at `record.rs:18`, additive kinds do **not** bump `OBS_SCHEMA_VERSION`. The drift guard
  is the new kind-parity test instead. The version stays 3.
- **Size-limit files.** Four files are affected:
  - `record.rs` (503) needs a split in part b.
  - `graph.rs` (499) needs `layout_walk` moved out before the counters go in.
  - `WorkspaceGraphPane.tsx` (505) takes only an attribute on an existing line.
  - `RepoWorkspace.tsx` is not touched.

  The two splits are behaviour-preserving moves, suited to `refactorer` or done in-increment with
  identical test counts.
- **Security trigger check.** No change touches `crates/bonsai-mcp/src/server/tools_*.rs` or the
  external-tool launch surface, so no mandatory `security-auditor` pass. The new records are
  covered by P91's redaction invariants (closed-vocabulary strings only), and `reviewer` should
  confirm that.
- **Milestone-sized.** Part b adds wire record kinds and anomaly rules across the Rust/TS boundary.
  Per `CLAUDE.md`, the plan should be confirmed as an architect contract (`docs/contracts/P120-…`),
  with the ui-designer copy review for the disclosure text, before senior-dev starts part b. Parts
  a and c can go straight from `/tasks`.
