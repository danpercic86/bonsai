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
| `P112-external-tool-detection.md` | P112 | Removes free-text `terminalCommand` / `editorCommand`: detected-list picker + native Browse, `procutil`/`safe_cwd` move, the launch invariant (no renderer/`settings.json` byte becomes a program path). | **done — BOTH halves.** All 4 sub-increments in; AI gate `9fca997`; **native USER CHECKPOINT confirmed and verified by the user 2026-09-22** (`ba4b9d3`; board narrative archive Part 76). **Archive-eligible.** **1159 lines.** Carries AMEND-4 (resolve the catalog by the `os` parameter, never `find`) through AMEND-8. Contract deltas still owed to `architect` — see `TODO.md` |
| `P112-tool-catalog.md` | P112 | Data half of the P112 spec: the compile-time candidate table, auto ladders and legacy aliases for `crates/bonsai-core/src/tools/catalog.rs`. | **done — both halves** (checkpoint confirmed 2026-09-22); archive-eligible. 36 rows 1:1 with `catalog_table.rs`. 111 lines; only the `catalog.rs` implementer needs it |
| `P112-ui.md` | P112 | UI contract for the two external-tool pickers: `Combobox` + `btn-secondary` Browse, state table (§5), signed copy (§8); replaces the free-text rows in `SettingsExternalToolsSection.tsx`. | **done — both halves** (checkpoint confirmed 2026-09-22); archive-eligible. **PRECEDENCE IS §17 > §16 > §§0-15, and 12 passages carry an in-place `SUPERSEDED 2026-09-15 (§16.n)` marker (counted 2026-09-16; the board says 15). Read §17 first and check a passage's status before reviewing against it.** Refreshed 2026-09-15 (`2add6ee`, `0e50fa5`); **1779 lines**, far over the house limit — split candidate. Milestone archived to `docs/history/` as Part 76 |
| `P113-settings-inline-notes.md` | P113 | Settings inline outcome notes replace toasts raised from the Settings surface (ruling #24): `SettingsOutcomeNote` / `useOutcomeNotes`, the `pushToast` DEV assertion, the save-failure banner, AC1's enumerate-every-call-site method. | **implemented — phases 1+2 both landed and reviewed**: phase 1 `0c86376`, phase 2 `dcff54b` + both MUST-FIX fixed `2b3dfd6`, 4 SHOULD-FIX `9aa25f4` (board narrative archive Part 78). Contract updated by `2233cc0` with the new `?forgeRemoveFail` seam values. **Residuals open** (`P113-F1-accounts-error-copy`) — see `TODO.md`. **1300 lines**; §8.2's key-scoped announcer rule was WITHDRAWN in `2aee970` |
| `P113-FU-forge-mock-seams.md` | P113 FU | Follow-up spec for the forge mock seams the credential work added: both seam tables (`?forgeRemoveFail=`, `?forgeClearHostFail=`) with per-line citations, written because the P113 contract documented only the old knob values. | implemented — written `2233cc0` by `architect` as a companion file (it had `Write` only, so it could not safely edit the 1300-line P113 contract). **229 lines.** Its guard-asymmetry finding is live in `TODO.md` |
| `P114-forge-failure-copy-ui.md` | P114 | Forge failure copy: seven ruled outcome variants plus two non-outcome wrappers; the **STATE, NOT ACT** rule (copy says "is no longer in the OS keychain", never "was removed", because the `NoEntry` fold makes an act-verb false on exactly the retry the copy recommends); outcome-owns-the-sentence; cause last behind `Details: `. | implemented — contract signed `2233cc0`, extended `ea6d323`, code `61af79b` ("the failure copy now describes state, not the act"). **479 lines.** **One stale option-2 line in §A.5** ("then *reject* under the new kind") is `ui-designer`-owned — live in `TODO.md` |
| `P87b-FU1-hygiene-2026-09-17.md` | P87b FU-1 | Companion hygiene file naming the stale ranges in `P87b-FU1-run-target.md` for an `Edit`-capable pass, rather than retyping a 1301-line contract with `Write`. | implemented — written `2233cc0`; the owed edits were applied by the orchestrator (narrative archive Part 79.2). **124 lines.** **§1's counts are still drifted in 2 of 4** — live in `TODO.md` |
| `P87b-FU1-run-target.md` | P87b FU-1 | Git-activity runs carry their target ref: backend resolution, the `?gitNoTarget` / `?gitLongTarget` / `?gitBidiTarget` harness seams, AC §9.1-13. | done — implemented `1d8c6f9`; **§9 header states "AI gate — no USER CHECKPOINT item"** (`:425`). **Kept active: five contract-hygiene corrections are owed** (`TODO.md` → `OPEN follow-ups` → `Still open, short form` → the P87b hygiene bullet; the old `1b`/`1d` item labels went with the 2026-09-14 compaction, archive Part 62), the highest-value being that §8's own `?gitBidiTarget` row embeds literal U+202E / zero-width characters while the same section forbids exactly that |
| `P87b-FU1-FU4-git-dock-ui.md` | P87b FU-1 / FU-4 | Git-dock UI for the run target: row copy, `.git-run-noun` / `.git-run-summary` geometry, the FU-4 clickable-dock-bar rejection, and the §5 F-A…F-G findings. | done — implemented `1d8c6f9`; **`:385` states "No USER CHECKPOINT item in this contract"**. **F-E is FIXED — do not re-report it.** It was corrected in place 2026-09-10 (`:461-478`) and independently re-verified 2026-09-11 against source (`staging.rs:178` resolves `activity_target(state, repo_id, Amend)`, `:179` wraps in `with_activity(…, Amend, target, …)`). The contract now carries the refutation, not the claim: **there is no FU-2 amend-wrapping gap.** This INDEX line was itself the propagation vector for a third false report. F-G was resolved 2026-09-10; F-F(a) mock-target fidelity stays open (`TODO.md` → `OPEN follow-ups` → `Still open, short form`; the old `1b`/`1c` labels went with the 2026-09-14 compaction, archive Part 62) |
| `P91-observability.md` | P91 | Architecture of record: Dev mode, JSONL logs, trace ids, spans, anomaly rules, durable metrics, redaction. | living — checkpoint confirmed USER **2026-09-10**, but **kept active**: the branch was **MERGED to `dev` and pushed 2026-09-11** by user ruling (fast-forward, 165 commits — the board's "30" was wrong); the real `logs/*.jsonl` parse is still owed, and the §6/§8/§8.1/§10 contract follow-ups are open (`TODO.md`) |
| `P91-observability-ui.md` | P91 | Dev-mode Settings surface + React causality instrumentation on the six surfaces. | living — awaiting USER CHECKPOINT. The `:496`/`:951` save-dialog staleness was **fixed in `fc9c36e`** (§ now states `log_export_session()` takes no destination and always writes to `<app_config_dir>/exports`); the old note here was itself stale as of 2026-09-03 |
| `P91-F6-usage-retention.md` | P91 F6 | **Normative amendment** to `P91-observability.md`: `usage.json` stays always-on, gains a 90-day window and becomes deletable via `logs_delete_all`. Lists the ten superseded passages (§2) and three new §13 decision rows (§11). | spec — not yet implemented. Its §2 pointers and §11 rows were **spliced into `P91-observability.md` by `docs-curator` 2026-09-14** |
| `P91-privacy-copy-ui.md` | P91 | Dev-mode privacy consent copy across all three surfaces (satisfies raw-args AC12). | spec — §2-§5 implemented (`b26833f`); the rest was held pending the F6 user decision, **which the user ruled 2026-09-11** (ledger row 3 + the `lifetime` follow-up ruling; design of record `P91-F6-usage-retention.md`). Copy §6 is unimplemented work, no longer a blocked decision |
| `P98-text3-readtext-ui.md` | P98 | `--text-3` read-text sweep; §8.8 is the canonical enumerate/bucket/verdict audit method. | done — checkpoint confirmed USER 2026-09-01; kept active because §8.8 is still the method of record |
| `P100-accent-fill-ui.md` | P100 | Accent-fill contrast: recipe A (a state demotes to `--selection`) vs recipe B (an action keeps the fill, flips the ink). | done — checkpoint confirmed USER 2026-09-02 (archive Part 47); archive-eligible |
| `design-review-2026-09-01-P100.md` | P100 | Design review + contract amendments. Verdict: APPROVE with amendments, no MUST-FIX. | done — archive-eligible with P100 |
| `P101-text3-audit-ui.md` | P101 | The full `--text-3` audit: 124 declarations, each with a recorded bucket and verdict (§3). | done — checkpoint confirmed USER 2026-09-02 (archive Part 47); archive-eligible |
| `P102-P105-hue-audit-ui.md` | P102 + P105 | Two defects of one shape: `--accent` as text, and hardcoded `#ffffff` as ink on a `--danger` fill. Introduces `--accent-strong`, `--danger-text`, `--success-text`, `--merged`. | done — implemented `0e5dcab`, fixes `185c352`; AC18/AC19/AC20 confirmed USER **2026-09-10** (archive Part 54.2); **archive-eligible** |
| `P106-status-badge-ink-ui.md` | P106 | Status-badge ink (the A/M/D/U/R/T/C letter family): 8 render sites, 3 new ink-only `-strong` tokens. | done — implemented `10ce967`; AC14/AC15 + the real-repo half of AC9 confirmed USER **2026-09-10** (archive Part 54.3); **archive-eligible** |
| `P107-hue-over-own-tint-ui.md` | P107 | Hue text over its own tint: 38 call sites (§2 had claimed 6); the three-pass search incl. `--h` indirection. | done — implemented `2168057`, errata `ef06e6b`; AC11/AC12/AC13 confirmed USER **2026-09-10** (archive Part 54.4); **archive-eligible**. Must not be re-opened against the original AC2 wording — see the errata. |
| `P107-F2-copy-chip-ui.md` | P107 F2 | The copy-candidate chip's `unknown` verdict gets its own neutral variant; resolves P107 §10/§12 F2. Touches `WorktreeCopyCandidates.tsx`, `dialogs-forms.css`, one mock knob. | implemented `8337d9b` — **contract's own header still says "spec complete, awaiting implementation"; that header is stale, the board is right** |
| `P109-status-badge-semantics-ui.md` | P109 | The status letter's *meaning*, not its ink: one `FileStatusBadge.tsx` replaces 8 drifted render sites; badges get accessible names. Zero CSS/token/geometry diff. | done — implemented `5a254ba`, recorded `5c2dcd2`; AC13/AC14 confirmed USER **2026-09-10** (archive Part 55); **archive-eligible** |
| `P108-hue-as-text-on-neutral-ui.md` | P108 | Hue used as text over a NEUTRAL `--bg-*` surface: 62 call sites, 28 fixes, no new tokens. | done — Implemented `42206fd`; AC12/AC13/AC14 confirmed USER **2026-09-10** (archive Part 54.5); **`AC11` CLOSED 2026-09-11 by user ruling** (ledger #13 — the two source-derived 3.05 figures for the unreachable states are ACCEPTED, limitation recorded; full text archive Part 67). **archive-eligible** |
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
> `ui-designer` follow-up. Verify before acting on either. — **RESOLVED 2026-09-11 by the
> orchestrator: this index was right and the board was stale.** The board line is gone; the
> architect's own `P91-observability.md` §6/§10 are the copies that are still stale.
>
> **Correction to the note above:** its reason for keeping the three `P91-*` files active — "the
> branch is unmerged by user instruction" — is **void**. The user ruled MERGE on 2026-09-11 and the
> branch was fast-forwarded onto `dev` and pushed. They stay active for the other two reasons (the
> owed `logs/*.jsonl` parse, and F6 — now ruled and contracted, not decided-pending).

> **2026-09-14 sweep (post-ruling).** The user ruled **all 22** open FOR-USER items on 2026-09-11;
> `TODO.md` was compacted 1537 → 988 the same week (`docs/history/todo-archive-2026-09.md`
> Parts 62-70). **Four files had no row and are added:** `P112-external-tool-detection.md`,
> `P112-tool-catalog.md`, `P112-ui.md` and `P91-F6-usage-retention.md`. **Two statuses moved on the
> user's ruling, not on the curator's judgement:** `P108-hue-as-text-on-neutral-ui.md` is now `done`
> and archive-eligible (`AC11` closed by ruling #13), and `P91-privacy-copy-ui.md` is no longer
> "held pending a user decision" — F6 was ruled, so its unimplemented §6 copy is **work**, not a
> blocked decision. **No file was moved** (`git mv` is outside the curator's file allowlist); the
> archive-eligible set is unchanged apart from gaining `P108`.
>
> **Audit reports are deliberately NOT indexed here.** `docs/audit-2026-08-07.md`,
> `audit-2026-08-18.md`, `audit-2026-09-03-external-launch.md` and
> `audit-2026-09-11-mcp-tool-contracts.md` are not contracts and have never had rows; they are
> reachable from `TODO.md`'s security section and from `docs/history/` Parts 56, 65 and 66. Raise it
> if that ever costs someone a search.
>
> **One contract task executed by the curator, 2026-09-14** (the architect has `Write` only and
> could not safely edit a 2029-line file): the ten `SUPERSEDED` pointers listed in
> `P91-F6-usage-retention.md` §2 were spliced into `P91-observability.md`, and §11's three decision
> rows were appended to its §13 as rows 34-36.

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

> **2026-09-16 sweep.** `TODO.md` was compacted **2438 → 1810** (`docs/history/todo-archive-2026-09.md`
> Parts 71-75). **One file had no row and is added:** `P113-settings-inline-notes.md`. **Three P112
> rows were stale and are corrected** — the detection contract still read "**not started**" although
> all four sub-increments have landed, and both it and `P112-ui.md` carried line counts from before
> their refreshes (`P112-ui.md` is **1779** lines, not "~520"). **The important one:**
> `P112-ui.md`'s **precedence is §17 > §16 > §§0-15** — the file's own banner reads "READ §17, THEN
> §16" and "§16 is the authoritative amendment record" (`:10-15`) — **with 12 passages carrying an
> in-place `SUPERSEDED 2026-09-15 (§16.n)` marker** (curator-counted 2026-09-16; the board says 15), so a
> reviewer who opens it at §6 or §9 can review against a dead passage — two of its own P113-era
> passages (§9's "two live regions", §6's `<p aria-live>` for `BROWSE_ERR`) are defects if
> implemented, and §16 supersedes both. **No status was upgraded on the curator's judgement**, and
> **no file was moved** (`git mv` is outside the curator's file allowlist). Still archive-eligible
> and still sitting in the live read path: `P95-a11y-ui.md`, `P98`, `P100`,
> `design-review-2026-09-01-P100`, `P101`, `P102-P105`, `P106`, `P107`, `P108`, `P109`, `P110`,
> `P111`. Directory size for the record: **46 active contract files + 177 in `archive/`**.

> **2026-09-22 sweep.** `TODO.md` was compacted **3361 → 1804** (`docs/history/todo-archive-2026-09.md` Parts 76-82; 2425 lines moved across 17 ranges, all 17
> verified byte-identical in place). **Three files had no row and are added:**
> `P113-FU-forge-mock-seams.md`, `P114-forge-failure-copy-ui.md` and
> `P87b-FU1-hygiene-2026-09-17.md` — all three written by `architect` in `2233cc0`, which is also
> why they were missed: they are companion files, not milestone contracts.
> **Four statuses moved, none on the curator's judgement.** All **three P112 rows** go
> `awaiting USER CHECKPOINT` → **`done`, both halves**, because **the user confirmed and verified
> the native checkpoint on 2026-09-22** (`ba4b9d3`) — the user's attestation, not a measurement of
> the orchestrator's. That makes all three **archive-eligible**. `P113-settings-inline-notes.md`
> goes *phase 2 in review* → **implemented, phases 1+2**, mirroring the board's own
> `✅ P113 PHASE 2 — LANDED dcff54b, REVIEWED + BOTH MUST-FIX FIXED 2b3dfd6` entry and the commits
> behind it. **No file was moved** — `git mv` is outside the curator's file allowlist — so the
> archive-eligible backlog now reads: `P95-a11y-ui.md`, `P98`, `P100`,
> `design-review-2026-09-01-P100`, `P101`, `P102-P105`, `P106`, `P107`, `P108`, `P109`, `P110`,
> `P111`, **plus the three P112 files**. Directory size for the record: **49 active contract files +
> 177 in `archive/`**.
> **`P112-ui.md` is still 1779 lines and still the worst split candidate in the live path**; its
> precedence warning (§17 > §16 > §§0-15) stands and is the single most important line in this index
> for anyone reviewing against it.

> **Known label collisions in the archive** (kept as-is, resolve on next touch): **P82** names two
> milestones — color-coded identity profiles (`P82-color-profiles.md`, `P82-ui.md`) and
> submodule-force (`P82-submodule-force.md`, `P82-submodule-force-ui.md`); **P69** names both the
> Settings redesign (contract files) and the 1.0.0 release-readiness milestone (no contract file).
> **No contract file** was ever written for P18, P41 (Git LFS — deferred), P48, T3, T6, the 1.0.0
> release-readiness batch, or the DX dev-loop acceleration initiative — those are board-only.
