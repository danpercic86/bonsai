# `docs/history/` — the archive index

Everything ever removed from `TODO.md` lives here. Compaction is **lossless**: a board section is
moved, never summarized away. If you are looking for a milestone that is not on the board, it is in
one of the files below.

Curated by `docs-curator`. Last updated **2026-09-03**.

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
| P102 + P105 hue audit — full narrative | `todo-archive-2026-09.md` Part 37 | **Milestone still LIVE**: AC18/19/20 pending USER CHECKPOINT. |
| P106 status-badge ink — full narrative | `todo-archive-2026-09.md` Part 38 | **Milestone still LIVE**: AC14/AC15 + the real-repo half of AC9 pending. |
| P108 hue-as-text on neutral — full narrative | `todo-archive-2026-09.md` Part 39 | **Milestone still LIVE**: AC12/13/14 pending; **AC11 OWED/unverified**. |
| P107 hue-over-own-tint — full narrative | `todo-archive-2026-09.md` Part 40 | Header archived as it stood ("IMPL PENDING") although `2168057` shipped it. |
| Superseded pre-ship filings (P102/P105/P106/P108), the dead-CSS decision block, the resolved `lint:size` blocker | `todo-archive-2026-09.md` Part 41 | |
| P91 security arc (code + contract complete) | `todo-archive-2026-09.md` Part 42 | **P91 still LIVE**: its USER CHECKPOINT was never presented. |
| P91 security audit F1–F9 | `todo-archive-2026-09.md` Part 43 | **F6 and the home-directory masking question stay live** as user decisions. |
| P91 observability build diary + planning record | `todo-archive-2026-09.md` Part 44 | Includes the "WIP on branch, do not merge" note. |
| P91 increment-4-7 SHOULD-FIX follow-ups (full text) | `todo-archive-2026-09.md` Part 45 | **Still open**; one line per item stays on the board. |
| Velocity — workspace test wall cut 14% (2026-09-03) | `todo-archive-2026-09.md` Part 46 | `737cc4b`, `5731d37`. |
| P99, P100, P101, P98, P95, P96, P97 + the P100+P101+DX-e2e banner | `todo-archive-2026-09.md` Part 47 | All done with USER CHECKPOINT confirmed. |
| Built-bundle e2e, P103, P104, the dev/prod gap inventory | `todo-archive-2026-09.md` Part 48 | **The bundle-default flip stays live** as a user decision. |
| DX dev-loop + velocity/gate-cost stubs | `todo-archive-2026-09.md` Part 49 | P75 HALTED and P76 held stay live on the board. |
| OPEN follow-ups as they stood before the 2026-09-03 condensation | `todo-archive-2026-09.md` Part 50 | Nothing here was closed; the board carries one line per item. |

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
stays on the board. Open follow-ups stay on the board however old they are.
