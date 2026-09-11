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
| `P68e-ai-activity-dock.md` | P68e | AI activity dock UI (bottom dock, live log, cancel, ask block). | done — kept active: **1123 lines** (re-measured 2026-09-03; said 1064), stale vs shipped code, pending splice + split |
| `P68g-ui.md` | P68g-2 | Eight AI-run settings, honest consent copy, ask-block hardening; holds the §3.1–3.5 splice blocks for P68e. | done — kept active: source of the pending P68e splice |
| `P68-security-audit.md` | P68g | Security audit of the AI conflict surface (1 HIGH, 5 MEDIUM, 7 LOW/INFO). | done — kept active: follow-ups 7–11 OPEN |
| `P68-user-checklist.md` | P68 | Native checklist (real CLI past 90 s, cancel, mid-run question, read-only tools, bulk, settings, consent copy). | done — kept with the P68 cluster |
| `P75-ipc-codegen.md` | P75 | Generate the IPC boundary from Rust with tauri-specta v2 (all 173 commands, no call-site churn). | HALTED 2026-08-21 — tauri-specta breaks Win10 app launch (`kernel32!WaitOnAddress`); reverted, findings + pins kept |
| `P76-native-checkpoint-automation.md` | P76 | tauri-driver + WebdriverIO harness to automate ~60–70% of the native USER CHECKPOINT backlog. | deferred — HELD as contract-only per user (2026-08-20) |
| `P95-a11y-ui.md` | P95 | Graph scroller semantics (live-region-only ARIA), keyboard reachability, toolbar/control contrast; AC1–AC17. | done — implemented `f9a9209`; AC8/14/15/16 confirmed by USER 2026-09-01 (archive Part 47). Archive-eligible; kept active because §4.1 forbids the `role="grid"` model. |
| `checkout-commit-backend.md` | — | Dirty-safe "checkout an arbitrary commit → detached HEAD" command + IPC surface + frontend handler. | implementation appears shipped (`7036fef` covers detached-HEAD checkout) — kept active: no checkpoint record mapped |
| `checkout-commit-ui.md` | — | Commit & branch menu structure for checkout-commit across graph rows, ref pills, sidebar rows. | implementation appears shipped (`7036fef` covers detached-HEAD checkout) — kept active: no checkpoint record mapped |
| `hook-disclosure.md` | — | First-time per-repo git-hook execution disclosure (`hooks_enabled` defaults true). | spec — implementation status unverified |
| `icon-system-ui.md` | — | Replace Unicode/emoji glyphs used as icons with the inline-SVG idiom; verdict + tiers. | superseded in part by `lucide-icons-ui.md` |
| `lucide-icons-ui.md` | — | Full migration of chrome icons to `lucide-react` (decision LOCKED). | ready for senior-dev — implementation status unverified |
| `novel-content-gate.md` | P68 #7 / H1 | Novel-content gate: demote auto-resolved files containing lines absent from base/ours/theirs. | open — P68 security follow-up 7 (TODO.md) |
| `pr-badge-placement-ui.md` | — | Move the forge PR badge + CI dot out of the ref-column band into a right-aligned forge column. | spec — implementation status unverified |
| `settings-ai-autonomy-disabled-ui.md` | — | "Why is the autonomy choice disabled?" single-row variant of the disabled-group pattern. | spec, not yet implemented |
| `002-bonsai-graph-theme-ui.md` | spec-002 | Commit-graph theme: lane palette, dots, ref pills, canvas metrics. | spec — own header says "ready for implementation"; implementation status unverified |
| `P110-watcher-burst-scoping.md` | P110 | Watcher path classification (`Worktree`/`Refs`) carried into `repo-changed` so a worktree-only burst refreshes narrowly instead of re-streaming the graph. | done — shipped `84bbf85` + `1be3a85`; checkpoint confirmed USER **2026-09-10** (archive Part 55); **archive-eligible** |
| `P111-pill-truncation-ui.md` | P111 | Ref-pill / chip truncation policy: long refs stay one line, the leaf ellipsizes instead of hard-clipping (head `flex-shrink: 999` vs leaf `1`). | done — implemented `1192f2a`, e2e `e2e/33-pill-truncation.spec.ts`. **`:326` states "Not a USER CHECKPOINT — every surface here is reachable in the browser harness"**, so no native half is owed. Open: `.asset-chip` has no R2 `max-width` (`TODO.md`). **archive-eligible** |
| `P87b-FU1-run-target.md` | P87b FU-1 | Git-activity runs carry their target ref: backend resolution, the `?gitNoTarget` / `?gitLongTarget` / `?gitBidiTarget` harness seams, AC §9.1-13. | done — implemented `1d8c6f9`; **§9 header states "AI gate — no USER CHECKPOINT item"** (`:425`). **Kept active: five contract-hygiene corrections are owed** (`TODO.md` items `1b` / `1d`), the highest-value being that §8's own `?gitBidiTarget` row embeds literal U+202E / zero-width characters while the same section forbids exactly that |
| `P87b-FU1-FU4-git-dock-ui.md` | P87b FU-1 / FU-4 | Git-dock UI for the run target: row copy, `.git-run-noun` / `.git-run-summary` geometry, the FU-4 clickable-dock-bar rejection, and the §5 F-A…F-G findings. | done — implemented `1d8c6f9`; **`:385` states "No USER CHECKPOINT item in this contract"**. **F-E is FIXED — do not re-report it.** It was corrected in place 2026-09-10 (`:461-478`) and independently re-verified 2026-09-11 against source (`staging.rs:178` resolves `activity_target(state, repo_id, Amend)`, `:179` wraps in `with_activity(…, Amend, target, …)`). The contract now carries the refutation, not the claim: **there is no FU-2 amend-wrapping gap.** This INDEX line was itself the propagation vector for a third false report. F-G was resolved 2026-09-10; F-F(a) mock-target fidelity stays open (`TODO.md` `1b`/`1c`) |
| `P91-observability.md` | P91 | Architecture of record: Dev mode, JSONL logs, trace ids, spans, anomaly rules, durable metrics, redaction. | living — checkpoint confirmed USER **2026-09-10**, but **kept active**: the branch was **MERGED to `dev` and pushed 2026-09-11** by user ruling (fast-forward, 165 commits — the board's "30" was wrong); the real `logs/*.jsonl` parse is still owed, and the §6/§8/§8.1/§10 contract follow-ups are open (`TODO.md`) |
| `P91-observability-ui.md` | P91 | Dev-mode Settings surface + React causality instrumentation on the six surfaces. | living — awaiting USER CHECKPOINT. The `:496`/`:951` save-dialog staleness was **fixed in `fc9c36e`** (§ now states `log_export_session()` takes no destination and always writes to `<app_config_dir>/exports`); the old note here was itself stale as of 2026-09-03 |
| `P91-privacy-copy-ui.md` | P91 | Dev-mode privacy consent copy across all three surfaces (satisfies raw-args AC12). | spec — §2-§5 implemented (`b26833f`); the rest is held pending the F6 user decision, which the 2026-09-10 checkpoint confirmation did not resolve |
| `P98-text3-readtext-ui.md` | P98 | `--text-3` read-text sweep; §8.8 is the canonical enumerate/bucket/verdict audit method. | done — checkpoint confirmed USER 2026-09-01; kept active because §8.8 is still the method of record |
| `P100-accent-fill-ui.md` | P100 | Accent-fill contrast: recipe A (a state demotes to `--selection`) vs recipe B (an action keeps the fill, flips the ink). | done — checkpoint confirmed USER 2026-09-02 (archive Part 47); archive-eligible |
| `design-review-2026-09-01-P100.md` | P100 | Design review + contract amendments. Verdict: APPROVE with amendments, no MUST-FIX. | done — archive-eligible with P100 |
| `P101-text3-audit-ui.md` | P101 | The full `--text-3` audit: 124 declarations, each with a recorded bucket and verdict (§3). | done — checkpoint confirmed USER 2026-09-02 (archive Part 47); archive-eligible |
| `P102-P105-hue-audit-ui.md` | P102 + P105 | Two defects of one shape: `--accent` as text, and hardcoded `#ffffff` as ink on a `--danger` fill. Introduces `--accent-strong`, `--danger-text`, `--success-text`, `--merged`. | done — implemented `0e5dcab`, fixes `185c352`; AC18/AC19/AC20 confirmed USER **2026-09-10** (archive Part 54.2); **archive-eligible** |
| `P106-status-badge-ink-ui.md` | P106 | Status-badge ink (the A/M/D/U/R/T/C letter family): 8 render sites, 3 new ink-only `-strong` tokens. | done — implemented `10ce967`; AC14/AC15 + the real-repo half of AC9 confirmed USER **2026-09-10** (archive Part 54.3); **archive-eligible** |
| `P107-hue-over-own-tint-ui.md` | P107 | Hue text over its own tint: 38 call sites (§2 had claimed 6); the three-pass search incl. `--h` indirection. | done — implemented `2168057`, errata `ef06e6b`; AC11/AC12/AC13 confirmed USER **2026-09-10** (archive Part 54.4); **archive-eligible**. Must not be re-opened against the original AC2 wording — see the errata. |
| `P107-F2-copy-chip-ui.md` | P107 F2 | The copy-candidate chip's `unknown` verdict gets its own neutral variant; resolves P107 §10/§12 F2. Touches `WorktreeCopyCandidates.tsx`, `dialogs-forms.css`, one mock knob. | implemented `8337d9b` — **contract's own header still says "spec complete, awaiting implementation"; that header is stale, the board is right** |
| `P109-status-badge-semantics-ui.md` | P109 | The status letter's *meaning*, not its ink: one `FileStatusBadge.tsx` replaces 8 drifted render sites; badges get accessible names. Zero CSS/token/geometry diff. | done — implemented `5a254ba`, recorded `5c2dcd2`; AC13/AC14 confirmed USER **2026-09-10** (archive Part 55); **archive-eligible** |
| `P108-hue-as-text-on-neutral-ui.md` | P108 | Hue used as text over a NEUTRAL `--bg-*` surface: 62 call sites, 28 fixes, no new tokens. | in-progress — `AC11` owed. Implemented `42206fd`; AC12/AC13/AC14 confirmed USER **2026-09-10** (archive Part 54.5) — **kept active: `AC11` is still OWED**, an AI-gate contrast measurement a native confirmation cannot close (`.file-count-del` selected and `.context-menu-item[data-tone='danger']` hovered, both source-derived at 3.05) |
| `spec-003-ui.md` | spec-003 | Graph declutter modes (first-parent, seed-ref filtering) — UI contract. | implemented; e2e `e2e/28-graph-filter.spec.ts`. No USER CHECKPOINT record mapped |
| `spec-004-ui.md` | spec-004 | Fold linear runs: the fold pill as a frontend display row over `FoldSpan` metadata. | implemented; e2e `e2e/29-graph-fold.spec.ts`. Open bug: the fold-pill cursor is dead (`TODO.md`) |
| `spec-005-ui.md` | spec-005 | Graph overview rail: match ticks + on-demand minimap. | implemented; e2e `e2e/30-graph-rail.spec.ts`. No USER CHECKPOINT record mapped |
| `spec-006-ui.md` | spec-006 | Author colouring + parent-highlight on hover (canvas paint + one settings row). | implemented; e2e `e2e/31-graph-author-color.spec.ts`. No USER CHECKPOINT record mapped |
| `spec-007-ui.md` | spec-007 | Replay mode: animated history playback. | implemented; e2e `e2e/32-graph-replay.spec.ts`. No USER CHECKPOINT record mapped |

> **2026-09-03 index sweep.** The index had drifted: **18 active contract files had no row** — the
> whole P91 cluster (4 files), the hue-audit programme (P98, P100 + its design review, P101,
> P102/P105, P106, P107, P108), the five `spec-00N-ui.md` companions and `002-bonsai-graph-theme-ui.md`.
> All are added above. **No file was moved in this pass.** `P95-a11y-ui.md`, `P98-text3-readtext-ui.md`,
> `P100-accent-fill-ui.md`, `design-review-2026-09-01-P100.md` and `P101-text3-audit-ui.md` are now
> **archive-eligible** (their checkpoints are confirmed) and should go to `archive/` with `git mv` on
> the next touch; the P102/P105/P106/P107/P108 and P91 contracts stay active because their USER
> CHECKPOINTs are pending.

> **2026-09-03 staleness re-sweep (second pass, same day).** Index diffed against the directory:
> **2 files had no row** — `P107-F2-copy-chip-ui.md` and `P109-status-badge-semantics-ui.md`, both
> added above. **0 dangling rows**: every row that is not in `docs/contracts/` resolves to
> `docs/contracts/archive/`, and there is no row for the deleted `P91-raw-args-privacy.md` (folded
> into `P91-observability.md` §7.4 by `12b0ab6`) — that one was already clean. **4 stale statuses
> corrected**: `P110` (said "working tree, awaiting review sign-off"; shipped `84bbf85`+`1be3a85`),
> `P91-observability-ui.md` (its staleness note was fixed by `fc9c36e`), `P107-hue-over-own-tint-ui.md`
> (pointed at a `TODO.md` contradiction that item 6 has since resolved), `P68e-ai-activity-dock.md`
> (1064 → 1123 lines). No file was moved in this pass either.

> **2026-09-10 sweep (all eight USER CHECKPOINTs confirmed).** The user confirmed every outstanding
> native checkpoint on 2026-09-10 (`548cc0a`), so **eight statuses moved**: `P102-P105`, `P106`,
> `P107`, `P109` and `P110` are now `done — checkpoint confirmed` and **archive-eligible**;
> `P108` is confirmed but **stays active because `AC11` is still OWED** (an AI-gate contrast
> measurement, which a native confirmation cannot close); the three `P91-*` files stay active
> because the branch is unmerged by user instruction, the real `logs/*.jsonl` parse is owed, and
> the F6 copy decision is still with the user. **Three files had no row and are added:**
> `P111-pill-truncation-ui.md`, `P87b-FU1-run-target.md`, `P87b-FU1-FU4-git-dock-ui.md` — all three
> **explicitly declare no USER CHECKPOINT item**, so their milestones are fully closed; two are kept
> active only for owed contract corrections. **No file was moved in this pass** (`git mv` is outside
> the curator's file allowlist). The archive-eligible set is now `P95-a11y-ui.md` (with its stated
> §4.1 caveat), `P98`, `P100`, `design-review-2026-09-01-P100`, `P101`, `P102-P105`, `P106`, `P107`,
> `P109`, `P110` and `P111` — eleven files still sitting in the live read path.
>
> **One contradiction recorded, not resolved:** this index says `P91-observability-ui.md`'s
> `:496`/`:951` save-dialog staleness was fixed in `fc9c36e`; `TODO.md` still carries it as an open
> `ui-designer` follow-up. Verify before acting on either.

> **Why the P68 cluster stays active despite `done`.** `TODO.md` §"P68 contract debt" schedules edits
> *to these files* (apply the `P68g-ui.md` §3.1–3.5 splice into `P68e-ai-activity-dock.md`, then
> split it; one stale module-path line at `P68-ai-conflict-streaming.md:304`) and holds
> `P68-security-audit.md` canonical for OPEN security follow-ups 7–11. Archive them once that debt
> clears.

## Archived contracts — `docs/contracts/archive/`

**177 files** (contracts + `*-user-checklist` scripts) for milestones that shipped **and** had their
native USER CHECKPOINT confirmed or explicitly waived — **with the four documented exceptions in the
second 2026-09-01 sweep note below, which were archived on explicit user instruction and NOT because
a checkpoint passed**. 161 were moved out of the live path on
**2026-08-21**; a further 12 + 4 on **2026-09-01** (see the sweep notes below). All moved with `git mv`
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
- **2026-09-01 second sweep** (4 files, moved on the user's explicit "archive history" instruction —
  **not** on a confirmed checkpoint; dispositions in `docs/history/todo-archive-2026-09.md`
  Parts 33 and 35):
  - `P84-sidebar-reveal-and-tag-autosync.md` and `P84-reveal-in-graph-ui.md` — P84's code shipped
    (`cce9eb9`/`90b315c`/`1803391`/`6868be6`). Its USER CHECKPOINT was never recorded at the time,
    but **the user confirmed on 2026-09-01 that it DID pass**. Status: `done + verified`
    (checkpoint confirmed by USER 2026-09-01, not from a contemporaneous 2026-08 record).
  - `graph-design-review-2026-08-22.md` — **M1 (`role="grid"` + `aria-rowcount` +
    `aria-activedescendant`) is SUPERSEDED by P95 and forbidden by `ui-reference.md` §4.1
    (`:250-252`) — do not implement.** M2/M3/M4/S2/S3/N1/N2 resolution unverified (live line in
    `TODO.md`).
  - `review-2026-08-22-ui.md` — MUST-1/2/3, SHOULD-1/2/4 and NIT-3 verified resolved at HEAD
    `ed5bb11`; SHOULD-3 (= P69 A9), NIT-1 and NIT-2 remain open as live lines in `TODO.md`.

> **Known label collisions in the archive** (kept as-is, resolve on next touch): **P82** names two
> milestones — color-coded identity profiles (`P82-color-profiles.md`, `P82-ui.md`) and
> submodule-force (`P82-submodule-force.md`, `P82-submodule-force-ui.md`); **P69** names both the
> Settings redesign (contract files) and the 1.0.0 release-readiness milestone (no contract file).
> **No contract file** was ever written for P18, P41 (Git LFS — deferred), P48, T3, T6, the 1.0.0
> release-readiness batch, or the DX dev-loop acceleration initiative — those are board-only.
