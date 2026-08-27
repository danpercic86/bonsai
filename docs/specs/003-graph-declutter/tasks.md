# Tasks: Graph Declutter Modes — First-Parent Toggle & Branch Solo/Hide

**Plan:** ./plan.md
**Status:** done (AI gate green 2026-08-26; native scroll feel = USER CHECKPOINT pending)

- [x] 1. Rust core: new `crates/bonsai-core/src/graph/filter.rs` (GraphFilter, seed filtering, staleness/hide-all rule, HEAD injection + synthesized Head label, stash exclusion) + thread filter through `graph.rs` (wrapper `compute_graph_with`), `graph/seed.rs` (`GraphSeed.seed_refs_applied`), `graph/lane.rs` (first-parent truncate), `graph/stream.rs` (`Meta.filtered`/`seed_refs_applied`), incl. `simplify_first_parent` — owner: senior-dev
- [x] 2. Tauri layer: `src-tauri/src/graph_cache.rs` (filter in `CachedGraph`, all classify/redecorate paths filtered), `src-tauri/src/commands/status.rs` (`Option<GraphFilter>` params), `src-tauri/src/settings.rs` + `commands/ui_settings.rs` (`graph_first_parent`, `graph_ref_filter` prefs per graphStyle precedent) — owner: senior-dev
- [x] 3. IPC/TS + mock: `src/ipc/types/graph.ts`, `ipc-api.ts`, `tauri/repo.ts` signatures; `src/ipc/mock/handlers/graphFilter.ts` (new, BFS fixture filter), `mock/handlers/graphStream.ts` (honor filter + meta flags), `mock/persistence.ts` (validate prefs); `src/components/repoWorkspace/graphStreamApply.ts` (surface flags); `src/hooks/useUiSettings.ts` + `src/settings/uiSettingsDefaults.json` + `src/settings/defaults.ts` — owner: senior-dev
- [x] 4. UI per docs/contracts/spec-003-ui.md: new `src/hooks/useGraphFilter.ts`, `src/components/GraphFilterChip.tsx`, `GraphFilterPopover.tsx`, `settings/SettingsGraphDeclutterSection.tsx` (+ register in Commit-graph category); sidebar context-menu items + marker glyphs in Branches/Remotes/TagsSection; wiring in `WorkspaceGraphPane.tsx` + `RepoWorkspace.tsx` (refetch on filter change, selection remap + reveal) — owner: senior-dev
- [x] 5. Review tasks 1–4 diff (correctness, boundary, cache paths, contract fidelity) — owner: reviewer + ui-designer (concurrent)
- [x] 6. MUST-FIX round(s) — owner: senior-dev
- [x] 7. Tests per plan.md Testing section (Rust fixture tests incl. default-regression guard, meta-flag matrix, cache; vitest for derivation/persistence/mock filter; harness e2e smoke) — owner: tester
- [x] 8. Orchestrator: `pnpm gate`, harness verification, commit `wip(spec-003)`, update TODO.md ledger — owner: orchestrator

## Notes

- Pass subagents these paths: `docs/specs/003-graph-declutter/spec.md`, `plan.md`,
  `docs/contracts/spec-003-ui.md`. Use plan.md's Data model / types **verbatim** — do not
  re-derive shapes.
- Locked semantics live in plan.md "Approach": `Some([])` = hide-all (HEAD-only, applied=true);
  `Some(non-empty)` all-stale = fallback to full (applied=false); stashes excluded under `Some`.
- Windows: any test-running subagent must set TMP/TEMP to `D:\Temp`; never run cargo test and
  clippy concurrently on a shared target dir.
- Tasks 1–2 are one coherent Rust unit and may be batched into a single senior-dev spawn;
  tasks 3–4 likewise form the frontend unit.
