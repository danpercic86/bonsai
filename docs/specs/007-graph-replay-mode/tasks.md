# Tasks: Replay Mode — Animated History Playback

**Plan:** ./plan.md
**UI contract:** ../../contracts/spec-007-ui.md (+ ui-reference §4.2 replay additions)
**Status:** ready

- [ ] 1. Frontend unit (single senior-dev pass — frontend-only, no Rust/IPC/pref): `src/graph/replay/*` (pure `replayModel.ts` state machine: narrative-time from clamped committerTs deltas, binary-search scrub, speeds 1×/2×/4×; single self-terminating rAF driver; frontier pulse per contract §4 — 600ms revealFlash-derived, alphas 0.30 dark/0.24 light), `ReplayMode.tsx` overlay (z-6, whole pane column, working GraphCanvas stays mounted inactive; reveal = [c, n) newest→oldest, frontier pinned at viewport top on auto-follow; paints via drawGraph with clamped Viewport + pre-filtered edges — ZERO draw.ts edits), transport bar per contract §3 (40px bottom bar, .commit-search idiom, .settings-segment speed, div-slider role="slider" + aria-valuetext, Space/←→/Home/End/Esc; scrub-during-play pauses), replay fab (third in §4.2 cluster, leftmost; disabled+tooltip on empty repo) + palette action "Replay history"; reduced-motion = scrubber-only (play/speed hidden, hint line); exit restores selection+scroll (nothing was mutated); `replayProps.ts` seam (WorkspaceGraphPane at 500 cap — keep seam ~8 ln or extract); vitest for replayModel (cutoff/scrub/clock/speeds/reduced-motion no-op play, single/empty repo) — owner: senior-dev
- [ ] 2. Review (rAF discipline: none when paused/ended/exited; no working-state mutation; perf) + design review — owner: reviewer + ui-designer (concurrent)
- [ ] 3. MUST-FIX round(s) — owner: senior-dev
- [ ] 4. Tests + e2e (enter/exit restore, scrub-while-paused maps correctly, reduced-motion emulation → scrubber-only, no-rAF-when-paused via counted scheduling wraps, empty-repo entry disabled) — owner: tester
- [ ] 5. Orchestrator: gate, commit `wip(spec-007)`, TODO ledger + AC5 downscope note — owner: orchestrator

## Notes
- Locked decisions: AC5 satisfied in reduced form (frontier pulse; bespoke Bonsai sprout = recorded
  follow-up); scrub-during-play pauses; no global entry shortcut (palette + fab only); no persisted
  pref; replays the unfolded model-space layout with active filters baked in.
- Headless harness pauses rAF — playback feel/smoothness is USER CHECKPOINT; state machine +
  scrub mapping are harness-verifiable.
- Windows: TMP/TEMP=D:\Temp; no prettier.
