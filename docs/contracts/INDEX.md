# Contract index

One line per file in `docs/contracts/`, so no session has to grep the directory to find where
something was specced. **Curated by `docs-curator`; the contracts themselves belong to `architect`
and `ui-designer`.**

## Contract hygiene (convention going forward)

- A contract states **interfaces, types, the IPC surface, algorithm pseudocode, and acceptance
  criteria** — not prose narration or a build diary.
- Target **well under ~15k tokens** (~60 KB) per contract; split when it crosses the ~500-line
  house limit rather than letting it grow.
- **Archive a milestone's contract once its native USER CHECKPOINT is confirmed (or explicitly
  waived).** Move it with `git mv` (history preserved) into `docs/contracts/archive/`, so every
  session that greps `docs/contracts/` pays only for in-flight work. A pending USER CHECKPOINT
  means the contract stays active.
- **Status here is a pointer, not an independent verification.** It mirrors the board (`TODO.md`)
  and the archives (`docs/history/todo-archive-2026-09.md`, `docs/history/todo-archive-2026-08.md`, `docs/history/todo-archive.md`). A
  `done` status means the milestone shipped, **not** that the file is free of drift; known drift is
  tracked under the follow-ups in `TODO.md`.
- `ui-reference.md` is the canonical design system and is **owned by `ui-designer`** — no other
  agent edits it.

## Active contracts

The only specs still in the live read path. Everything shipped-and-confirmed is in `archive/`
(see below). Vocabulary: `living` · `deferred` · `HALTED` · `done` (kept active only when it still
carries open, tracked work).

| File | Milestone | Scope | Status |
|---|---|---|---|
| `ui-reference.md` | — | Canonical design system: tokens, geometry, graph metrics, ref pills, states, both themes. | living |
| `P65a-lazy-topo-spike.md` | P66 | Feasibility spike for lazy generation-number topo order (VERDICT: tractable, effort L). | deferred — approved future work, not scheduled (user 2026-08-10) |
| `P68-ai-conflict-streaming.md` | P68 | Streaming / interactive / bulk AI conflict resolution; invariants D1–D16 (canonical). | done — kept active: open P68 contract debt (TODO.md) |
| `P68e-ai-activity-dock.md` | P68e | AI activity dock UI (bottom dock, live log, cancel, ask block). | done — kept active: 1064 lines, stale vs shipped code, pending splice + split |
| `P68g-ui.md` | P68g-2 | Eight AI-run settings, honest consent copy, ask-block hardening; holds the §3.1–3.5 splice blocks for P68e. | done — kept active: source of the pending P68e splice |
| `P68-security-audit.md` | P68g | Security audit of the AI conflict surface (1 HIGH, 5 MEDIUM, 7 LOW/INFO). | done — kept active: follow-ups 7–11 OPEN |
| `P68-user-checklist.md` | P68 | Native checklist (real CLI past 90 s, cancel, mid-run question, read-only tools, bulk, settings, consent copy). | done — kept with the P68 cluster |
| `P75-ipc-codegen.md` | P75 | Generate the IPC boundary from Rust with tauri-specta v2 (all 173 commands, no call-site churn). | HALTED 2026-08-21 — tauri-specta breaks Win10 app launch (`kernel32!WaitOnAddress`); reverted, findings + pins kept |
| `P76-native-checkpoint-automation.md` | P76 | tauri-driver + WebdriverIO harness to automate ~60–70% of the native USER CHECKPOINT backlog. | deferred — HELD as contract-only per user (2026-08-20) |
| `P84-sidebar-reveal-and-tag-autosync.md` | P84 | Sidebar click-to-reveal-in-graph (frontend) + automatic tag sync on fetch (one core fn + one command). | done (code shipped: `cce9eb9`/`90b315c`/`1803391`) — kept active: P84 has no board section and no `docs/history/` entry, so its USER CHECKPOINT is unverifiable (2026-09-01) |
| `P84-reveal-in-graph-ui.md` | P84 | UI contract for reveal-in-graph: single-click sidebar → scroll + flash. | as above — kept active pending a verifiable P84 checkpoint record |
| `P95-a11y-ui.md` | P95 | Graph scroller semantics (live-region-only ARIA), keyboard reachability, toolbar/control contrast; AC1–AC17. | awaiting USER CHECKPOINT (AC8/AC14/AC15/AC16) — implemented `f9a9209` |
| `graph-design-review-2026-08-22.md` | — (dated review) | Commit-graph design review: M1–M4 MUST-FIX, S1–S3, N1–N2, with a resolution plan. | done in part — kept active: M1's `role="grid"`/`aria-activedescendant` prescription was superseded by P95, and S2/S3/N1/N2 resolution is unverified |
| `review-2026-08-22-ui.md` | — (dated review) | Whole-app UI/UX review: prioritized findings, onboarding/token redesign specs, icon-system verdict, sidebar keyboard-a11y contract. | done in part — kept active: per-finding resolution unverified |
| `checkout-commit-backend.md` | — | Dirty-safe "checkout an arbitrary commit → detached HEAD" command + IPC surface + frontend handler. | implementation appears shipped (`7036fef` covers detached-HEAD checkout) — kept active: no checkpoint record mapped |
| `checkout-commit-ui.md` | — | Commit & branch menu structure for checkout-commit across graph rows, ref pills, sidebar rows. | implementation appears shipped (`7036fef` covers detached-HEAD checkout) — kept active: no checkpoint record mapped |
| `hook-disclosure.md` | — | First-time per-repo git-hook execution disclosure (`hooks_enabled` defaults true). | spec — implementation status unverified |
| `icon-system-ui.md` | — | Replace Unicode/emoji glyphs used as icons with the inline-SVG idiom; verdict + tiers. | superseded in part by `lucide-icons-ui.md` |
| `lucide-icons-ui.md` | — | Full migration of chrome icons to `lucide-react` (decision LOCKED). | ready for senior-dev — implementation status unverified |
| `novel-content-gate.md` | P68 #7 / H1 | Novel-content gate: demote auto-resolved files containing lines absent from base/ours/theirs. | open — P68 security follow-up 7 (TODO.md) |
| `pr-badge-placement-ui.md` | — | Move the forge PR badge + CI dot out of the ref-column band into a right-aligned forge column. | spec — implementation status unverified |
| `settings-ai-autonomy-disabled-ui.md` | — | "Why is the autonomy choice disabled?" single-row variant of the disabled-group pattern. | spec, not yet implemented |

> **Why the P68 cluster stays active despite `done`.** `TODO.md` §"P68 contract debt" schedules edits
> *to these files* (apply the `P68g-ui.md` §3.1–3.5 splice into `P68e-ai-activity-dock.md`, then
> split it; one stale module-path line at `P68-ai-conflict-streaming.md:304`) and holds
> `P68-security-audit.md` canonical for OPEN security follow-ups 7–11. Archive them once that debt
> clears.

## Archived contracts — `docs/contracts/archive/`

**173 files** (contracts + `*-user-checklist` scripts) for milestones that shipped **and** had their
native USER CHECKPOINT confirmed or explicitly waived. 161 were moved out of the live path on
**2026-08-21**; a further 12 on **2026-09-01** (see the sweep note below). All moved with `git mv`
(history preserved). Board history for these milestones is in
`docs/history/todo-archive.md` and `docs/history/todo-archive-2026-08.md`; the MVP AI-gate vs
USER-CHECKPOINT split is in `docs/history/milestones-mvp.md`. Coverage:

- **MVP** M0–M6 · **Polish / feature** P1–P27 · **repo-management + git-completeness** P28–P48
- **Phase 1** P49–P52 · **Phase 2** P53–P57 (+ `phase2-ai-native-overview.md`) · **Phase 3** P58–P61
- **Phase 4** P62–P65 (+ `phase4-forge-overview.md`; native halves waived 2026-08-20) · **P67**
- **Settings redesign** P69 (incl. the `P69c-draft-feedback-ui.md` and
  `P69-settings-shell-amendment-A.md` superseded pointer stubs)
- **Post-1.0.0** P70–P74 · P77–P83 (tag-sync, forge account-mgmt / multi-account / PR-actions,
  color-coded identity profiles, submodule-force, refetch-coalescing, commit-panel UX) +
  `design-review-2026-08-19-p73-submodules.md`
- **Testing campaign** T1, T2, T4, T5
- **2026-09-01 sweep** (12 files, all checkpoint-confirmed per `docs/history/todo-archive-2026-09.md`):
  `P85-refresh-perf.md` · `P86-refresh-caching.md` · `P87-git-observability.md` · `P87-ui.md`
  (checkpoints verified 2026-08-25, archive Part 27) · `P88-git-action-perf.md` (2026-08-25,
  Part 26) · `P89-pr-local-diff.md` · `P89-ui.md` · `P90-ci-checks.md` · `P90-ci-checks-ui.md`
  (2026-08-25, Part 25) · `P92-multi-ref-commit-ui.md` · `P92-review-2026-08-31-addendum.md`
  (its owed `ui-reference.md` §6.2 edit is applied — verified at `ui-reference.md:394-406`) ·
  `P93-pr-diff-center-overlay-ui.md` (both 2026-08-31, Part 23).
- **No contract file** exists for **P94** (e2e parallel-worker isolation) — board-only.

> **Known label collisions in the archive** (kept as-is, resolve on next touch): **P82** names two
> milestones — color-coded identity profiles (`P82-color-profiles.md`, `P82-ui.md`) and
> submodule-force (`P82-submodule-force.md`, `P82-submodule-force-ui.md`); **P69** names both the
> Settings redesign (contract files) and the 1.0.0 release-readiness milestone (no contract file).
> **No contract file** was ever written for P18, P41 (Git LFS — deferred), P48, T3, T6, the 1.0.0
> release-readiness batch, or the DX dev-loop acceleration initiative — those are board-only.
