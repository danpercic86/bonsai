# P120 — Performance & render-stability measurement loop

**Status:** draft
**Created:** 2026-09-24

## Problem
Bonsai users run into UI elements that jump after the screen already looks ready, and keyboard
focus that disappears without them doing anything (for example after a background refresh). None of
Bonsai's current measurements can see either problem. Dev-mode logging records IPC calls, renders,
frames and anomalies, but not layout movement or focus loss. The automated gate also can't stop a
performance regression: the only timing it could check is wall-clock time, which is too noisy in
headless runs, so the 20k-commit e2e deliberately makes no frame-budget assertion. The result is
that fixes don't stick, and the problems the user complains about most can't be seen.

The approach follows Anthropic's "How we made claude.ai 3x faster" write-up: turn each problem into
a **deterministic count**, prove the count tracks what the user feels, then **ratchet** it so it can
only go down.

## Goals
- **Lab detection (a):** the browser harness can detect, for a fixed set of scripted interactions:
  - layout shifts, each attributed to a named UI region (sidebar, commit graph, right-hand
    status/details panel, diff view, activity/AI dock)
  - focus loss that no user input caused.
- **Deterministic per-interaction counts (a):** for the same scripted interactions, record numbers
  that are identical run to run: React commits, IPC calls, and browser style-recalculation and
  layout passes. Unexpected layout shifts and focus losses count too.
- **Ratchet (a):** the canonical gate fails when any recorded count goes above its committed
  baseline. A count that drops can be locked in as the new baseline.
- **Rust layout counter (a):** graph-layout work on a fixed fixture is measured by deterministic
  operation counts (not wall-clock and not an instruction counter), and those counts are ratcheted
  the same way.
- **Field signals (b):** when dev mode is on, the existing observability log also records layout
  shifts (region + phase) and unprompted focus loss. This extends the existing log; it is not a
  parallel system.
- **Anomaly rules (b):** new rules flag:
  - a shift after the app became interactive that no user input caused
  - focus loss that lines up with a refresh or file-watcher round in the same trace.
- **Reviewer (c):** a repeatable review of an exported dev-mode session log plus the usage-metrics
  file. It produces ranked findings, each with:
  - the log evidence (record references)
  - a suspected source location
  - a proposed deterministic test that fails today.
- The reviewer is read-only: it reports and never fixes.

## Non-goals
- Fixing the shifts or focus losses this surfaces. Each becomes its own follow-up item; this spec
  delivers the instruments.
- Wall-clock / millisecond thresholds as gate failures.
- Instruction-count profiling (Valgrind or similar). It isn't available on the primary Windows dev
  machine.
- Any capture when dev mode is off, and any upload or telemetry leaving the machine. Everything
  stays local, like the rest of dev-mode logging.
- Loosening any existing redaction rule. New records carry region names, phases, counts and timings
  only, never repo content, paths or file names.
- A new "measure everything" toggle that duplicates dev mode's existing verbosity levels.
- Mass-parallel automated investigation. Findings are worked through the normal milestone loop.
- Hunk-level or per-element attribution beyond the named regions.

## User-facing behavior
- **Normal use (dev mode off):** no visible change and no measurable overhead.
- **Dev mode on:**
  - The session log gains layout-shift and focus-loss records.
  - The anomalies those records trigger appear alongside the existing ones, wherever dev-mode
    anomalies are surfaced today.
  - The dev-mode page's capture disclosure mentions the new record types, per the existing
    disclosure conventions in `docs/contracts/ui-reference.md`. There is no new switch; see
    Resolved decision 6.
- **Developer / AI workflow:**
  - Running the gate prints a perf-ratchet result: each tracked count, its baseline, and pass or
    fail.
  - A failing ratchet names the interaction, the count that rose, and by how much.
  - Running the perf review on a session log produces a ranked findings report.

## Acceptance criteria

### Lab detection and ratchet (a)
1. Given the mock-IPC browser harness, when a scripted interaction causes a layout shift inside a
   named region, then the harness reports that shift with its region and whether it happened
   before or after the app became interactive.
2. Given a deliberately introduced post-interactive shift in one region, when the shift check runs
   20 times, then it fails 20/20. With the shift removed, it passes 20/20. This proves the detector
   is deterministic, not flaky.
3. Given a focused input or list, when a background refresh, watcher round, or window-focus rescan
   runs without user input, then any resulting focus loss is detected and reported.
4. Given the fixed scripted-interaction set, when the count collection runs twice in a row on
   unchanged code, then every recorded count is identical between the two runs.
5. Given committed baselines, when a change raises any tracked count, then the canonical gate fails
   and names the interaction, the count, the baseline and the new value.
6. Given a change that lowers a tracked count, when the baseline is updated through the documented
   procedure, then the new, lower value becomes the ceiling. Raising a baseline requires an
   explicit, visible edit in the diff; it never happens silently.
7. Given the graph-layout fixture, when layout runs twice on unchanged code, then the operation
   counts are identical, and a change that increases them fails the ratchet.
8. For at least one existing hot path, a change that lowers its count is shown to also lower
   measured wall-clock time. This follows the article's "prove the count tracks the clock"
   requirement. If P120 itself contains no such reduction, the proof moves explicitly to the first
   follow-up that lowers a count. A deterministic count stays in the ratchet meanwhile, because it
   is cheap and can't cause flaky failures. Only counts that prove *nondeterministic* are dropped
   (see Edge cases).

### Field signals (b)
9. Given dev mode on, when an element in a named region moves after the app is interactive and no
   user input preceded it, then the session log contains a layout-shift record (region + phase)
   and a matching anomaly referencing it.
10. (User ruling 2026-09-24: "same trace" is satisfied by time correlation, with a trace join only
    when a trace exists, because refresh records carry no trace. See contract A2.)
    Given dev mode on, when focus falls to no element (or the document body) within the same trace
    as a refresh or watcher round and without preceding user input, then the log contains a
    focus-loss record and a focus-steal anomaly referencing both it and the refresh/watcher record.
11. Given dev mode on, when a shift or focus change follows a user gesture (click, key, scroll),
    then no anomaly is raised for it.
12. Given dev mode off, then none of the new records are produced.
13. Given any redaction setting, then the new records contain no repo content, path or file name,
    and anomaly detection behaves identically under strict and raw redaction.
14. The log's list of record kinds is guarded by a frontend/backend parity check, so the new kinds
    cannot drift between the two sides. The schema version stays unchanged, because adding kinds
    is an additive change under the existing versioning policy.

### Reviewer (c)
15. Given an exported session log and the usage-metrics file, when the perf review runs, then it
    produces findings ranked by severity. Each finding includes:
    - the implicated record references
    - the anomaly rule or count that triggered it
    - a suspected source location
    - a proposed deterministic test that would fail today.
16. Given a truncated session log, then the review states that early evidence may be missing, and
    treats absence as inconclusive rather than as proof.
17. The review makes no code, test or ledger edits. Its output is a report, and any follow-up items
    are filed by the orchestrator.
18. Given a log with no findings, then the review says so explicitly rather than inventing issues.

## Edge cases & error states
- **No session log available or dev mode never enabled:** the review says clearly that there is
  nothing to review and how to produce a log. It doesn't fail silently.
- **Truncated or partially dropped log:** reported as incomplete evidence (see AC 16).
- **Shifts caused by the browser or OS rather than Bonsai** (window resize, DPI change, native
  scrollbar appearing): these are attributed to the right phase or cause where possible, and not
  counted as unprompted app shifts when a resize is in progress.
- **Intentional motion** (expand/collapse animations, the graph streaming in more rows,
  virtualized scrolling): user-initiated or explicitly expected movement doesn't trip the shift
  rule.
- **Focus moving to a dialog, toast or palette the user opened:** not a focus loss.
- **Focus lost because the window itself lost focus** (user switched apps): not a focus loss.
- **A count that turns out to be non-deterministic across machines:** it is removed from the
  ratchet rather than loosened.
- **First introduction on existing code:** today's app likely already has some shifts and elevated
  counts. See the open question on baselining.
- **Platform without layout-shift support:** macOS and Linux webviews can't report layout shifts.
  There, layout-shift capture is silently absent: no error, and no claim in the dev-mode disclosure
  that it's captured. Focus-loss capture still works on every platform.
- **Instrumentation overhead:** capture must not itself cause renders or layout passes that inflate
  the counts it measures.

## Resolved decisions (user, 2026-09-24 — "go with your defaults for all six")
1. **Baseline at introduction:** today's counts, including existing shifts and focus losses,
   become the starting ceiling. Existing violations are a work queue to drive down, not a red
   gate.
2. **Scripted-interaction set:**
   - cold open of a repo
   - select a commit
   - stage and unstage a file
   - switch branch
   - background refresh with the status panel focused
   - scroll the 20k-commit graph
   - open and close the command palette
3. **Gate tier:** the lab checks run in the full gate (with e2e) only, not in `--quick` /
   `--frontend`. The Rust layout-counter ratchet runs wherever the Rust tests already run.
4. **Review output destination:** the perf review writes a report only. The orchestrator files
   board items after the user triages it.
5. **Log source:** defaults to the most recent session log in the app's log folder. An explicit
   path is optional.
6. **Dev-mode switch:** there is no new switch. The new signals are captured whenever dev-mode
   capture is on, and only the capture disclosure copy changes.

## Open questions
- None.
