# Tasks: Author Coloring & Parent-Highlight on Hover

**Plan:** ./plan.md
**UI contract:** ../../contracts/spec-006-ui.md (+ ui-reference §5.2 author palette)
**Status:** ready

- [ ] 1. Frontend unit (single senior-dev pass): `authorHue()` export in geometry.ts; new `src/graph/authorColor.ts` (per-theme constants — use the CONTRACT values: dark hsl(h,60%,65%), light hsl(h,60%,33%), NOT the plan's placeholders); new `src/graph/highlight.ts` (pure display-space `highlightTargets` from projected visibleEdges); drawGraph pass 3.5 edge re-stroke (+1.5 width, mode-resolved color) + pass-4 parent ring at avatarSelRingRadius **+3.5** (contract deviation from plan); `GraphDisplayOptions.colorMode`; hover via existing Interaction.hoverRow (keyboard = hoverRow ?? selectedIndex; no highlight on fold pills; no dim); pref `graphColorMode: 'lane'|'author'` full chain (graphStyle precedent incl. small Rust settings plumbing) + Appearance segmented row `appearance.graph-colors`; extract `drawEdges.ts` if draw.ts (~449) crosses 500; vitest for authorColor/highlight math + pref chain — owner: senior-dev
- [ ] 2. Review + design review (concurrent; contrast + ring stacking per §2.3) — owner: reviewer + ui-designer
- [ ] 3. MUST-FIX round(s) — owner: senior-dev
- [ ] 4. Tests + e2e smoke (mode toggle persists + canvas repaints; hover no-error; no idle repaints assertion where possible) — owner: tester
- [ ] 5. Orchestrator: gate, commit `wip(spec-006)`, TODO ledger — owner: orchestrator

## Notes
- Contract values override plan on light S/L constants and parent-ring radius.
- Land AFTER spec-005 commits (GraphCanvas contention); spec-006 GraphCanvas delta is ~6 ln.
- USER CHECKPOINT ledger: light-mode author contrast vs Bonsai paper (≈3.4:1 worst), hover feel/idle CPU.
- Windows: TMP/TEMP=D:\Temp; no prettier.
