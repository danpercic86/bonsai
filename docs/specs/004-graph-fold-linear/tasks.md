# Tasks: Fold Linear Runs

**Plan:** ./plan.md
**UI contract:** ../../contracts/spec-004-ui.md
**Status:** done (AI gate green 2026-08-27; 20k scroll feel = USER CHECKPOINT pending)

- [x] 1. Rust unit: `GraphFilter.fold_linear` (serde-default); `FoldScan`/`FoldSpan` computation (MIN_FOLD_RUN=5, conservative lane rule, post-redecorate, never cached; `merge_rows` in `CachedGraph`); spans on `Done` chunk / `GraphLayout.foldSpans`; cache `walk_eq` classification with fold cleared; `graph_fold_linear: bool` pref; Rust tests per plan Testing — owner: senior-dev
- [x] 2. Frontend unit: TS types (`FoldSpan`, `foldSpans`); `src/graph/foldModel.ts` (display↔model row mapping, transient expansion state, selection-run pinning, reveal auto-expand); `src/graph/drawFold.ts` (fold pill + collapse pill + keyboard-active paint per contract); GraphCanvas display-row integration (virtualization, hit-testing, aria-rowcount per amended ui-reference §4.1, keyboard semantics); mock `graphFold.ts`; settings toggle + popover switch + chip `Folded` segment; `graphFoldLinear` pref chain; vitest for foldModel/mocks — owner: senior-dev
- [x] 3. Review diff (correctness, boundary, cache walk_eq, display-row model) + design review — owner: reviewer + ui-designer (concurrent)
- [x] 4. MUST-FIX round(s) — owner: senior-dev
- [x] 5. Tests + e2e smoke (fold/expand/collapse, compose with first-parent/solo, reveal auto-expand, persistence) — owner: tester
- [x] 6. Orchestrator: gate, harness proof, commit `wip(spec-004)`, TODO ledger — owner: orchestrator

## Notes
- Pass: spec.md, plan.md, docs/contracts/spec-004-ui.md, and (frontend) the amended
  docs/contracts/ui-reference.md §4.1 display-row canon.
- Task 1 and 2 share `src/ipc/types/graph.ts` at the seam — run task 1 first OR give task 2 the
  exact TS shapes; prefer sequential (1 → 2) since the frontend consumes real span semantics.
- Windows: TMP/TEMP=D:\Temp; sequential cargo; no prettier.
