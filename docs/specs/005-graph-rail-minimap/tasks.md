# Tasks: Graph Overview Rail — Match Ticks & On-Demand Minimap

**Plan:** ./plan.md
**UI contract:** ../../contracts/spec-005-ui.md (+ ui-reference §4.2 overlay stack)
**Status:** ready

- [ ] 1. Frontend unit (single senior-dev pass — frontend-only feature): `src/graph/rail/*` (OverviewRail component, bucket build/resample math as pure modules, RailRowModel seam), `src/graph/drawRail.ts` painter, `src/styles/graph-rail.css`, GraphCanvas integration (≤~25 ln, hover zone in existing mousemove, unmount-when-hidden), `currentMatchRow` derivation, `graphMinimapAlwaysShow` pref chain + `SettingsGraphOverviewSection.tsx` (Commit-graph "Overview" group), mock persistence; extract `railProps.ts` if WorkspaceGraphPane nears 500 ln; vitest for bucket/coalesce/thumb/rowmodel math — owner: senior-dev
- [ ] 2. Review diff (perf: zero idle cost hidden, no per-paint allocs; correctness of mapping math) + design review (contract fidelity, both themes/styles) — owner: reviewer + ui-designer (concurrent)
- [ ] 3. MUST-FIX round(s) — owner: senior-dev
- [ ] 4. Tests + e2e smoke (hidden by default + no .graph-rail in DOM, search-open reveal + tick click-to-jump + current-match emphasis, always-show persistence, click/drag thumb jump) — owner: tester
- [ ] 5. Orchestrator: gate, harness proof, commit `wip(spec-005)`, TODO ledger — owner: orchestrator

## Notes
- Pass: spec.md, plan.md, docs/contracts/spec-005-ui.md, ui-reference §4.2.
- Locked: ticks mirror whichever rings channel is live (search or Ask-history; no current tick on
  history channel); rail aria-hidden with keyboard parity via existing next/prev; pref is
  plain-bool graphFirstParent precedent.
- Depends on spec-004's foldModel for the RailRowModel backing when fold is on — identity mapping
  when fold is off; build against the seam, wire foldModel when spec-004 lands.
- Windows: TMP/TEMP=D:\Temp; no prettier.
