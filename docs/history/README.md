# `docs/history/` — the archive index

Everything ever removed from `TODO.md` lives here. Compaction is **lossless**: a board section is
moved, never summarized away. If you are looking for a milestone that is not on the board, it is in
one of the files below.

Curated by `docs-curator`. Last updated **2026-09-23**.

## ⚠️ One live reference used to be on the board and is NOT here

**`docs/durable-rules.md`** — the "durable lessons" block, moved off `TODO.md` on **2026-09-23** by
explicit user decision (211 lines, byte-identical, from `TODO.md:1474-1684` at `473d9fa`). It is
**live reference content, deliberately not filed under `docs/history/`**: read it before asserting
that anything is tested, measured, covered, closed or green. It holds the six failed app-wide
claims, the aliasing rule, the grep-counting rules, the BASE rule, the three P91 testing rules, the
measurement rules, **the gate-running rules**, the coverage and evidence rules, the two durable
constraints (no `tracing` in this workspace; the settings-load path cannot use the `obs` sink), and
the rules earned 2026-09-16/17. **The stories and worked numbers behind them are Part 53** — cite
the rule there, read the story here.

## How to find a milestone

| Milestones / topic | File | Notes |
|---|---|---|
| M0–M6 (MVP) AI-gate vs USER CHECKPOINT split | `milestones-mvp.md` | The gate breakdown only, not the build diary. |
| M0–M6, P2 → P27 | `todo-archive.md` | The original archive; oldest history. |
| P28 → P65 build detail, Phase 1–4 banners, resolved FOR-USER decisions, P67/P68/P69(1.0.0) detail | `todo-archive-2026-08.md` Parts 1–9 | |
| P62–P74 checkpoint waiver, P71–P74, the P69 Settings redesign, the Audit #2 fix batch | `todo-archive-2026-08.md` Parts 10–16 | Moved 2026-08-20; condensed. |
| P70, P77 (both checkpoints verified) | `todo-archive-2026-08.md` Parts 17–18 | Moved 2026-08-21. |
| Follow-ups resolved in the 2026-08-21 fix batch (verbatim) | `todo-archive-2026-08.md` Part 19 | read_status / palette / refetch / stash / submodule / STDERR / cred-split. |
| P78/P79/P80 forge milestones | `todo-archive-2026-08.md` Part 20 | Condensed. |
| P80b/P81/P82 (done + checkpoints confirmed) | `todo-archive-2026-08.md` Part 21 | Condensed. |
| P94 · P93 + P92 · DEP REFRESH 2026-08-28 · P90 + P89 · P88 · the P85–P87 perf+observability batch (incl. P87c/P87d) · P82 + P83 · divergence reconcile + Release 1.1.0 + P80b/P81/P82 | `todo-archive-2026-09.md` Parts 22–29 | Moved 2026-09-01, verbatim. |
| The full DX dev-loop text (a condensed stub stays on the board) | `todo-archive-2026-09.md` Part 30 | |
| The full confirmed-checkpoints + accepted-decisions block | `todo-archive-2026-09.md` Part 31 | The accepted defaults and the two FOR USER items stay live on the board. |
| OPEN follow-ups resolved in the 2026-08-21 session, as they stood on the board | `todo-archive-2026-09.md` Part 32 | |
| **P84** (sidebar reveal-in-graph + tag auto-sync) — record gap | `todo-archive-2026-09.md` Part 33 | Code shipped (`cce9eb9`, `90b315c`, `1803391`, `6868be6`); **USER CHECKPOINT never recorded**; contracts archived on user instruction 2026-09-01. |
| macOS ad-hoc code signing — config done 2026-08-30, **release still pending** | `todo-archive-2026-09.md` Part 34 | A one-line live pointer stays in `TODO.md`. |
| The two dated 2026-08-22 design reviews — per-finding dispositions | `todo-archive-2026-09.md` Part 35 | Includes which findings are still open. |
| File-size refactor pass (2026-09-02) | `todo-archive-2026-09.md` Part 36 | Its still-open follow-ups stay live in `TODO.md`. |
| P102 + P105 hue audit — full narrative | `todo-archive-2026-09.md` Part 37 | Milestone entry archived 2026-09-10 as **Part 54.2**; AC18/19/20 confirmed by USER 2026-09-10. |
| P106 status-badge ink — full narrative | `todo-archive-2026-09.md` Part 38 | Milestone entry archived 2026-09-10 as **Part 54.3**; AC14/AC15 + the real-repo half of AC9 confirmed by USER 2026-09-10. |
| P108 hue-as-text on neutral — full narrative | `todo-archive-2026-09.md` Part 39 | Milestone entry archived 2026-09-10 as **Part 54.5**; AC12/13/14 confirmed by USER 2026-09-10. `AC11` was an owed AI-gate contrast measurement; **CLOSED 2026-09-11 by user ruling #13** (Part 67). |
| P107 hue-over-own-tint — full narrative | `todo-archive-2026-09.md` Part 40 | Header archived as it stood ("IMPL PENDING") although `2168057` shipped it. Milestone entry archived 2026-09-10 as **Part 54.4**; AC11/12/13 confirmed by USER 2026-09-10. |
| Superseded pre-ship filings (P102/P105/P106/P108), the dead-CSS decision block, the resolved `lint:size` blocker | `todo-archive-2026-09.md` Part 41 | |
| P91 security arc (code + contract complete) | `todo-archive-2026-09.md` Part 42 | Milestone entry archived 2026-09-10 as **Part 54.6**; USER CHECKPOINT confirmed 2026-09-10. **Its last live item is now CLOSED:** the owed real `logs/*.jsonl` parse was done 2026-09-16 (ruling #16) and is **Part 81.1**. The do-not-merge instruction is **void** — the user ruled MERGE on 2026-09-11 and the branch was fast-forwarded onto `dev` (165 commits) and pushed. |
| P91 security audit F1–F9 | `todo-archive-2026-09.md` Part 43 | **F6 and home-directory masking were RULED by the user 2026-09-11** — masking shipped fail-closed in `dc295c5`; F6 is contracted (`P91-F6-usage-retention.md`) and **shipped 2026-09-14** (`d46c98e` + `b53618a`; Parts 72.2 / 77.1). **F7 and F9 stay live** as open findings. |
| P91 observability build diary + planning record | `todo-archive-2026-09.md` Part 44 | Includes the "WIP on branch, do not merge" note, which was live until **2026-09-11, when the user ruled MERGE** (ruling #1). It is history now, not instruction. |
| P91 increment-4-7 SHOULD-FIX follow-ups (full text) | `todo-archive-2026-09.md` Part 45 | **Still open**; one line per item stays on the board. |
| Velocity — workspace test wall cut 14% (2026-09-03) | `todo-archive-2026-09.md` Part 46 | `737cc4b`, `5731d37`. |
| P99, P100, P101, P98, P95, P96, P97 + the P100+P101+DX-e2e banner | `todo-archive-2026-09.md` Part 47 | All done with USER CHECKPOINT confirmed. |
| Built-bundle e2e, P103, P104, the dev/prod gap inventory | `todo-archive-2026-09.md` Part 48 | **The bundle-default flip stays live** as a user decision. |
| DX dev-loop + velocity/gate-cost stubs | `todo-archive-2026-09.md` Part 49 | P75 HALTED and P76 held stay live on the board. |
| OPEN follow-ups as they stood before the 2026-09-03 condensation | `todo-archive-2026-09.md` Part 50 | Nothing here was closed; the board carries one line per item. |
| Gate states `5c2dcd2` + `c6cd7dd` and the e2e-contention mis-diagnosis story | `todo-archive-2026-09.md` Part 51 | Moved 2026-09-03. The operational rules they earned stay live in `TODO.md`. |
| Full narratives of everything closed on 2026-09-03 (P107 F2, the four `112800c` ticks, the `4002ad2` struck entries) | `todo-archive-2026-09.md` Part 52 | The `ai::session*` clock-seam item is **not** closed — only its evidence paragraph moved. |
| "Durable lessons" — the stories, worked numbers and measurement narrative | `todo-archive-2026-09.md` Part 53 | Moved 2026-09-03. **The rules themselves are live in `docs/durable-rules.md`** (moved off `TODO.md` 2026-09-23, byte-identical) — *not* in `TODO.md` any more, and *not* in this archive. |
| **The whole USER-CHECKPOINT block: P102+P105, P106, P107, P108, P91** | `todo-archive-2026-09.md` Part 54 (54.1 banner · 54.2 P102+P105 · 54.3 P106 · 54.4 P107 · 54.5 P108 · 54.6 P91) | Moved **2026-09-10**, after the user confirmed all eight native checkpoints (`548cc0a`). **Still live in `TODO.md`:** P91's user decisions / architectural rulings / F7 / F9 / SHOULD-FIX list. **The owed logs parse is CLOSED** (2026-09-16, Part 81.1). **Superseded 2026-09-11:** P108 `AC11` was closed by user ruling (Part 67) and the do-not-merge note is void (the branch was merged). |
| **P110** (selection flicker + watcher burst scoping) and **P109** (status-badge semantics) | `todo-archive-2026-09.md` Part 55 | Moved 2026-09-10; both checkpoints confirmed. P110's op-state-file gap stays live as a candidate follow-up. |
| The 2026-09-03 closure one-liners and the full **SEC-2026-09-03** external-launch section | `todo-archive-2026-09.md` Part 56 | Moved 2026-09-10. SEC's residual symlink case, its test gap and MEDIUM-2 / LOW-1 stay live. |
| The `Queued housekeeping` section, incl. the `e149382` forge-pr.css split proof | `todo-archive-2026-09.md` Part 57 | Moved 2026-09-10. Six still-open items were condensed onto the board. |
| Gate states `c218258` and the `Earlier gate states` block | `todo-archive-2026-09.md` Part 58 | Moved 2026-09-10, superseded by `1d8c6f9` and by the later 542.4s run over `e9d025d` + `7f9f16b`. **The gate-running rules stay live** in `TODO.md`. |
| The 2026-09-10 session: **P111**, **P87b FU-1**, and the six reviewer-follow-up closures | `todo-archive-2026-09.md` Part 59 (59.1 P111 · 59.2 FU-1 · 59.3 the follow-ups) | All three contracts declare **no USER CHECKPOINT item**, so no native half was archived. `1b`/`1c`/`1d`/`2b`, P111's `.asset-chip` residue and the `whichAll` PATH_EXTS trap stay live. |
| The board's own record of the 2026-09-10 confirmation | `todo-archive-2026-09.md` Part 60 | The only place stating why the `8dd5b24` CSP change needed a native checkpoint. |
| Superseded curator bookkeeping (pre-pass navigation text, `Verification state`, the 2026-09-03 curator note + its line-count composition) | `todo-archive-2026-09.md` Part 61 | Kept so the 2026-09-10 pass is lossless down to the meta-lines it rewrote. |
| The stale `RESUME HERE — 2026-09-10` block, its `Next, in order` items and `Verification state` | `todo-archive-2026-09.md` Part 62 | Moved 2026-09-14. The `whichAll` PATH_EXTS warning, the `src/obs/types.ts:68` decision and the P87b hygiene residue stay live. |
| **The FOR-USER decision evidence for items 0-6** (happy-dom measurements, the e2e bundle figures, F6, masking, D3, the two 1.0.0 items, the record contradictions) | `todo-archive-2026-09.md` Part 63 | Moved 2026-09-14 **because all 22 items were RULED by the user 2026-09-11**. The rulings themselves stay live in `TODO.md` and are authoritative. |
| The `IN FLIGHT` ruling queue, the 2026-09-11 orchestrator closures, and the unreviewed-MCP-merge warning | `todo-archive-2026-09.md` Part 64 | Moved 2026-09-14. The two owed code items (D3 CSS swap, A3 comments) stay live as **in-progress**. |
| **`SEC-2026-09-11`** — MCP tool-contract audit of `2a0b8f1` (HIGH `stage_paths` symlink escape, MEDIUM, 4 LOWs, PROCESS) | `todo-archive-2026-09.md` Part 65 | Implemented `216ca45`. **Holds a verified-CLEAN register — read it before re-auditing MCP.** Report: `docs/audit-2026-09-11-mcp-tool-contracts.md`. |
| **`SEC-2026-09-11b`** — review of that implementation | `todo-archive-2026-09.md` Part 66 | **Holds a second verified-CLEAN register.** Its open item — the UNC/`\\wsl$` `canonicalize` ship-blocker — was **CLEARED 2026-09-14 by a real UNC probe** (Part 77.4); only the untested `\wsl$` and OneDrive-placeholder cases stay live in `TODO.md`. |
| **P108 `AC11`** — the owed contrast measurement | `todo-archive-2026-09.md` Part 67 | **CLOSED 2026-09-11 by user ruling #13**: the two source-derived 3.05 figures are ACCEPTED, limitation recorded. |
| The happy-dom narrative and the `Known load-flakes` section | `todo-archive-2026-09.md` Part 68 | Moved 2026-09-14. happy-dom ADOPTED (`1953c0a`), two real test defects fixed (`9422e8b`). The 5 s test-budget fragility stays live. **The `h_ai` parallel defect's env-race half is CLOSED** (`80a852e` + `105131a`); the process-concurrency half is handled by the `h-ai-stub` nextest group. |
| Open follow-ups as they stood before the 2026-09-14 condensation | `todo-archive-2026-09.md` Part 69 (69.1 P91 · 69.2 SEC-2026-09-03 → the 2026-09-01 hoisted items · 69.3 P69 Settings · 69.4 the `terminalCommand`/`editorCommand` removal entry) | **Nothing here was closed.** Every item keeps at least one line on the board. |
| Superseded curator bookkeeping (the pre-pass navigation body, the `Archive` table, the 2026-09-10 "~915 lines" note) | `todo-archive-2026-09.md` Part 70 | Kept so the 2026-09-14 pass is lossless down to the meta-lines it rewrote. |
| **P112 sub-increments 3 and 4** — build + review transcript | `todo-archive-2026-09.md` Part 71 (71.1 the resolved §1.3 citation · 71.2 the orphaned dev server, the `P112-ui.md` §17 rulings, the four bad citations, the §16/§17 refresh · 71.3 the sub-inc-4 refresh brief, sub-inc 3 `d0e6cf0`, the coalescing lesson, the host-bound-test finding, the security audit and its LOWs, the three permanently-kept results) | Moved **2026-09-16**. **SUPERSEDED 2026-09-22: P112 is now archived in full as Part 76** — the user confirmed its native checkpoint on 2026-09-22, so the milestone entry, the five checkpoint items and the two decisions that were owed moved there. Its ranked follow-ups stay live in `TODO.md`. Rules relocated to the board: the signed-error-string rule, "cite from the file", "a green `pnpm gate` is Windows-only evidence", the port-1420 check, the `PickedTool`-literal invariant. |
| Superseded gate states (`d0e6cf0` 457.5s · `dcff54b` 454.0s · the 427.4s confirming run · the `e9ed93d` Rust tier 386.0s) and the completed 2026-09-14 queue — **F6**, **P77**, the e2e cold-timing measurement, the UNC clearance | `todo-archive-2026-09.md` Part 72 (72.1-72.4) | Moved 2026-09-16. **Superseded again 2026-09-22:** the live gate state is now the 11-step `--full` green (510.6s) in the release block, and the rest of this queue plus the `9fca997` evidence moved to **Part 77.1**. Queue item 6 is closed (`80a852e` + `105131a`) bar its `generate.rs` duplicate; **two of the four USER ACTIONS were cleared by the user** (the Dev-mode log parse 2026-09-16, the `updater-prod.key` backup 2026-09-22). |
| **P112 sub-increment 2** (`a2eb091`) and **P113 phase 1** (`0c86376`) — review transcripts | `todo-archive-2026-09.md` Part 73 (73.1 sub-inc 2 + its audit · 73.2 three smaller items · 73.3 the two measured contract corrections · 73.4 P113 phase 1) | Moved 2026-09-16. **SUPERSEDED 2026-09-22: P113 phases 1+2 both landed and are archived as Part 78** (`0c86376`, `dcff54b`, `2b3dfd6`); its residuals stay live in `TODO.md`. Still live: the unrepresentability property, the `capabilities/default.json` `fs:` dependency, the observability-capture privacy decision, and the two durable constraints (no `tracing` in this workspace; the settings-load path cannot use the `obs` sink). |
| **The two items CLOSED 2026-09-16**, with their evidence, plus the `.cmd` launch-path security audit | `todo-archive-2026-09.md` Part 74 (74.1 the subtitle ruling · 74.2 the `.cmd` audit · 74.3 the follow-up pass + `fd93616` · 74.4 the "Open in editor" entry and its measurement table) | (1) **"Open in editor" was broken on Windows — FIXED in `fd93616`**: `PATHEXT` before the bare name, empty `PATH` components skipped, `is_absolute()` required; AI resolution measured unmoved (4375 vs 4376 stats). (2) **The false General subtitle — resolved by implementation**, `settingsCatalog.ts:42-43`. **The security consequence stays live on the board:** the launch surface flipped `Registry → Path` for `vscode`, so the app launches `code.CMD` rather than a PE, through the mitigated CVE-2024-24576 / "BatBadBut" path. |
| Superseded curator bookkeeping, the duplicated `cargo fmt` measurement, and the pre-consolidation `cargo fmt` section | `todo-archive-2026-09.md` Part 75 (75.1 navigation body · 75.2 the stale `RESUME HERE` header · 75.3 the 2290-hunk duplicate · 75.4 the "~988 lines" note · 75.5 the canonical `cargo fmt` section) | Moved 2026-09-16. Both `cargo fmt` figures (1773/221 and 2290 hunks) now live in one place on the board. |
| **P112 in full** — the milestone entry, its AI gate (`9fca997`) and the five USER CHECKPOINT items as confirmed | `todo-archive-2026-09.md` Part 76 (76.1 the `RESUME HERE` block + the 41-file process-failure narrative · 76.2 the milestone entry, the five checkpoint items, the still-unverifiable note, the two decisions owed) | Moved **2026-09-22**, after the user **confirmed and verified the native checkpoint on 2026-09-22** (`ba4b9d3`) — the only thing that could clear it. Kept in full because it is the record of *what the confirmation covered*, incl. **item 4's `set_parent` sub-question**. **Still live in `TODO.md`:** P112's ranked follow-ups, the AMEND-8 reasoned-not-executed gap, and the contract deltas owed to `architect`. |
| The completed **2026-09-14/15 queue**, the superseded `5654eaa`/`934a280` gate states, ruling #24's scope facts, the second-round rulings' evidence blocks, and the whole **2026-09-11 ruling queue** | `todo-archive-2026-09.md` Part 77 (77.1 the queue + item 6 + the four USER ACTIONS + `Verification state` · 77.2 ruling #24's scope facts · 77.3 `Why #21` / LOW-1 / home masking · 77.4 the ruling queue) | Moved 2026-09-22. **The ruling *tables* stay live and verbatim** — only the evidence prose moved (the Part 63 rule). Still live: the `generate.rs` duplicate-stub follow-up, the two `--test-threads=1` / `h-ai-stub` rules, the `\wsl$` + OneDrive UNC gap, the Dependabot user action, and the bundle default remaining the user's call. |
| The **2026-09-16 session**: the real-log investigation, the nine-file second review pass and its closure, the Settings-scrim finding, **P113 phases 1+2** | `todo-archive-2026-09.md` Part 78 (78.1) | Moved 2026-09-22. Landed in `88a4004`, `7f186b3`, `2fc03cc`, `5654eaa`, `2b3dfd6`, `9aa25f4`, `934a280`, `0c86376`, `dcff54b`. **Refused for archiving and still live: A4 finding 6** (a diagnostic regression with no commit that closes it), the open `render-storm` threshold **user decision**, the observability-capture privacy decision, and ~20 filed items condensed one line each. |
| The **2026-09-17 session**: the orphaned-credential arc, **P114**, contract hygiene, the rustfmt pass, three security audits, and the superseded `ea6d323`/`5f015be`/`3948478` greens | `todo-archive-2026-09.md` Part 79 (79.1 the credential defect → the five filed items · 79.2 P114 + contract hygiene + the rustfmt pass · 79.3 the four user decisions + the queued list · 79.4 the full credential-honesty arc and both audits) | Moved 2026-09-22. **Holds TWO verified-CLEAN registers — read them before re-auditing the forge credential paths.** All four 2026-09-17 rulings were **relocated verbatim** to the board's ledgers. Still live: the three AWAITING-USER credential items, the live orphan in the user's own keychain, and the **20-file split queue, which is 5 of 20 done** (`fbf81d0`). |
| The **`RELEASE v1.6.0` block**, verbatim as it stood before the 2026-09-22 tightening | `todo-archive-2026-09.md` Part 80 | **NOT an archived section — the release block is LIVE at the top of `TODO.md`** and v1.6.0 is prepared, verified and **not published**. This part exists only so the 269 → 229-line tightening is lossless. |
| **P91's closed `logs/*.jsonl` parse** (ruling #16), the `mono` defect it found, and the superseded `cargo fmt` section | `todo-archive-2026-09.md` Part 81 (81.1 the parse + the `mono` defect · 81.2 the `cargo fmt` section) | Moved 2026-09-22. The parse closed P91's last owed AI-gate item; the `mono` defect was **fixed in `88a4004`** with `OBS_SCHEMA_VERSION` 1 → 2. The `cargo fmt` section is **void in full** — `8ad3c72` rustfmt'd the tree and `cargo fmt --all --check` is gate step 3. Still live: the 431-anomalies-in-6-minutes product signal. |
| Superseded curator bookkeeping (the pre-pass navigation body, the `Archive` table, the "~1800 lines" note) and the pre-pass P87b-residue bullet | `todo-archive-2026-09.md` Part 82 (82.1 navigation · 82.2 the Archive table + curator note · 82.3 the P87b bullet) | Moved 2026-09-22. 82.3 is the one range this pass replaced with a *different* statement rather than a condensation — the architect's 2026-09-17 verification answered its "which ones is unverified". |
| **P116** (`OBS_SCHEMA_VERSION` TS/Rust drift + parity test) — in full | `todo-archive-2026-09.md` Part 83 | Moved **2026-09-23**. `done`, `e49cf20`, and its own text states **"No USER CHECKPOINT applies"** — the only cleanly archivable milestone on that board. **Still live in `TODO.md`:** the `P91-observability.md` schema drift it filed "for `docs-curator`" is contract substance owned by `architect`, folded into the one canonical contract-drift entry and re-measured (code is at **3**, the contract still says **1** at six sites). |
| **The `P117` section, verbatim as it stood before the 2026-09-23 tightening** | `todo-archive-2026-09.md` Part 84 | **NOT an archived milestone — P117 is LIVE on the board and `awaiting USER CHECKPOINT`.** `d63a571` + `83bbdf3`, AI gate green, but both checkpoint items need the native app and real repos. This part exists only so the **380 → 352** tightening is lossless. What came out: the duplicated `CLAUDE.md` gate-rule correction (canonical copy now in P115), the spent in-flight states, two mid-paragraph splices. |
| **The `P115` section, verbatim as it stood before the 2026-09-23 tightening** | `todo-archive-2026-09.md` Part 85 | **NOT an archived milestone — P115 is LIVE on the board and `in-progress`.** `bbd8993`, full gate green (9/9, 574.5s), but **a Linux/macOS run is still owed** — the `#[cfg(unix)]` legs have never compiled on this machine. **134 → 134, net zero**: nine lines out, nine back in, because the `CLAUDE.md` gate-rule correction was promoted here as canonical. |
| **The open follow-up entries rewritten by the 2026-09-23 staleness sweep** | `todo-archive-2026-09.md` Part 86 (86.1 the real-log residue · 86.2 A4 finding 6's citation · 86.3 the U+200B closure · 86.4 the 20-file split queue · 86.5 the `gitbin.rs` count · 86.6 the gate `DEP0190` bullet · 86.7 the velocity follow-ups · 86.8 the struck `no_proxy_client` · 86.9 P80 forge follow-up (a)) | **Three items were CLOSED, each verified against the current tree and never from a commit subject:** cross-repo detector keying (`obs/anomaly/window.rs:160-178` + `src/obs/types.ts:266-269`; P117 inc 2, `83bbdf3`) · `RemotesSection` re-rendering on a local-branch change (`RemotesSection.tsx:13-17`, `:156`; P118b, `c6ae304`) · the gate's `DEP0190` / `shell: true` argv concatenation (zero live uses in `scripts/`). **Everything else was re-measured and left open**, including the 20-file split queue (**5 of 20 done, 15 open**) and the `P91-observability.md` schema drift, which got worse. |
| Superseded curator bookkeeping (the pre-pass navigation body, the `Archive` table, the 2026-09-22 "~1810 lines" note) | `todo-archive-2026-09.md` Part 87 (87.1 navigation · 87.2 the Archive table + curator note) | Moved 2026-09-23. 87.2's final paragraph is the one this pass acted on: it escalated moving the durable rules to their own file for the third time, and the user decided in favour. |

## Non-milestone records

| File | Covers |
|---|---|
| `velocity-2026-09-01.md` | Gate wall-clock numbers, test-suite hotspots, inner-loop rebuild cost, ceremony-vs-machine-time split (measured 2026-09-01). |
| `context-pollution-audit.md` | The context/token-cost audit. |

## Related indexes (not history)

- `TODO.md` (repo root) — the live board: in-progress + queued milestones and the open follow-ups.
- `docs/contracts/INDEX.md` — one line per contract file, active and archived.
- `docs/contracts/archive/` — contracts whose milestone is closed.

## The rule

Move a milestone's section into the current dated archive file only once **both** halves of its gate
have passed (or the native half is explicitly waived). A milestone with a pending USER CHECKPOINT
stays on the board. **A milestone with an owed AI-gate item keeps that item on the board too** — a
native confirmation cannot close a measurement. **Both live examples are now closed** — P91's
`logs/*.jsonl` parse on 2026-09-16 (ruling #16; Part 81.1) and P108's `AC11` on 2026-09-11 by an
explicit user ruling (Part 67) — in each case by the user or by a measurement, never by the curator,
which is why P91 and P112 could finally leave the board. Open follow-ups stay on the board however
old they are.

**And the order is mandatory, since `c5b3ea5` truncated 1950 lines into nowhere:** extract → diff
byte-identical against `git show HEAD:TODO.md` → *then* remove → leave a Part pointer where the text
stood. The 2026-09-23 pass moved **956 lines across 15 ranges, all 15 verified byte-identical in
the destination after the append** (14 here as Parts 83-87, 1 in `docs/durable-rules.md`), plus a
whole-file check over all 2305 unique non-blank, non-`---` lines of `git show HEAD:TODO.md`:
**zero unaccounted for.** The 2026-09-22 pass moved **2425 lines across 17 ranges, all 17 verified
byte-identical in place after the append**, plus the same whole-file check.

**Two rules the 2026-09-23 pass adds, both earned:**
1. **A staleness sweep closes an item against the tree, or not at all.** Three items closed this
   pass; seven more were re-measured and deliberately **left open** because the tree still shows
   them live. Two of the seven had *drifted line numbers* and would have read as closed to anyone
   checking the citation instead of the behaviour.
2. **`.claude/worktrees/` holds stale copies of repo paths.** A `find`-based measurement of the
   20-file split queue returned counts from a worktree for two files. Measure the repo path.
