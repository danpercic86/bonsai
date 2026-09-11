# Bonsai — Milestone TODO

> Single source of truth for session resume. Keep the "Current step:" line of the
> in-progress milestone updated at every workflow transition.

Environment: Rust 1.97.1 stable-msvc, VS Build Tools 2022 17.14, pnpm 11.17.0, Node 24, WebView2.
Cargo not on default PATH — `$HOME/.cargo/bin`. Browser harness: `pnpm dev:mock` (port 1420).
Avoid tauri "test" feature on this machine (STATUS_ENTRYPOINT_NOT_FOUND); use runtime-free
inner functions for command tests.
Harness traps (cost a session each): the hidden Browser pane reports `innerWidth/innerHeight = 0`, so
every `vh`/`vw` rule evaluates to 0 — call `resize_window` (1440×900) before any layout measurement;
`setTimeout` is throttled to ~1 s in a hidden page, so batch tool dispatches instead of many `await`s;
To drive the git dock in the harness you must seed `bonsai.mockUiSettings` (`{onboardingSeen:true}`)
and `bonsai.mockSession` (`{openRepos:['C:\mock\bonsai-fixture']}`) into localStorage before load —
otherwise you land on the no-repo empty state; and select the toolbar Push button by its `title`, not
its `aria-label` (the label changes with the ahead-count once a push has landed).
never remove React-owned DOM nodes to "reset" a menu (throws `removeChild` on the next render) —
dismiss with Escape; headless preview pauses `requestAnimationFrame`, so canvas repaint/scroll-feel
ACs can only be checked in the native window.
**USER MANDATE (2026-07-28, updated 2026-08-04 for cross-platform support): on Windows, never use
C: for temp/scratch/mock repos — C: is critically full. Use `D:\Data\Temp\bonsai-scratch`; when running
cargo tests set TMP/TEMP to `D:\Data\Temp` (tempfile honors them). On macOS/Linux, `scratch_dir()` now
falls back to the OS temp dir (`std::env::temp_dir()/bonsai-scratch`) automatically — no special
handling needed there. Include the Windows-specific guidance in every subagent prompt that runs
tests or creates repos only when running on a Windows machine.**

## Board conventions

Status vocabulary: `pending` · `in-progress` · `done` · `awaiting USER CHECKPOINT` · `deferred`
(deferred always carries a one-line reason). A milestone is `done` only when the AI gate **and** the
native USER CHECKPOINT have both passed — the orchestrator never self-declares the second half.

**History is archived, not deleted.**

---

## Where the rest of the board went

Full detail for everything compacted out of this file is in `docs/history/` — start at
`docs/history/README.md`. The **Archive** table at the bottom of this file is the short form.
Nothing below was closed by the curator: a milestone with a pending USER CHECKPOINT stays here, an
owed AI-gate item stays here, and open follow-ups stay here however old they are.

**2026-09-10 compaction pass — Parts 54-60.** The event the board was waiting on happened: the user
confirmed **all eight** native USER CHECKPOINTs on 2026-09-10 (`548cc0a`). Moved off the board: the
seven-milestone checkpoint block — P102+P105, P106, P107, P108, P91 (Part 54) and P110 + P109
(Part 55); the 2026-09-03 closures plus the SEC-2026-09-03 remediation (Part 56); the `Queued
housekeeping` section (Part 57); the superseded `c218258` and earlier gate states (Part 58); the
2026-09-10 session — P111, FU-1 and the six reviewer-follow-up closures (Part 59); and the board's
own record of the confirmation (Part 60). **What did NOT move:** P108's `AC11` (an owed *AI-gate*
measurement, so a native confirmation does not reach it), P91's owed `logs/*.jsonl` parse and its
do-not-merge instruction, all seven FOR USER decisions, every open follow-up, every accepted
decision and every durable rule.

**2026-09-03 passes.** Compaction Parts 51-53 moved the gate states, the narratives of everything
closed that day, and the stories behind the durable lessons; the **rules** those stories taught
stayed here. A same-day staleness sweep (no archiving) cross-checked **35 open entries against the
tree and found 11 had drifted** — corrections are inline below, each marked `re-verified 2026-09-03`
or struck through with its SHA. The board's own record of being wrong is kept deliberately. Two
findings are worth reading before trusting anything nearby: the `ai::session*` clock seam **is in
the tree** (`734b310`), and the M1 design-review residue was telling sessions to delete
`aria-activedescendant`, which `ui-reference.md:865` explicitly sanctions.

---

## ⏸ RESUME HERE — updated 2026-09-10

**Branch `feat/p91-observability`. Nothing pushed, by instruction.** 30 commits ahead of `5c2dcd2`
(one, `2a0b8f1`, is a PEER session's MCP work — not this session's).
**DO NOT MERGE this branch to `dev` without the user.** The user confirmed 2026-08-31 that it is
WIP; the 2026-09-10 checkpoint confirmation is **not** authorisation to merge. The branch's own
commit messages must not be read as an authoritative status.
**Tree is NOT clean, and deliberately so:** `CLAUDE.md` and `.claude/agents/context-explorer.md`
carry uncommitted edits that predate the 2026-09-10 session and are **not** FU-1's — they were left
unstaged on purpose. Don't spend time reconciling them; find out whose they are first.

**Current step: see `## 🔄 IN FLIGHT` — the 2026-09-11 ruling queue. Previously:** FU-1 (`1d8c6f9`), P111 (`1192f2a`) and every reviewer
follow-up (`e9d025d` / `7f9f16b` / `c03d11d`) are done and committed; all eight USER CHECKPOINTs
were confirmed 2026-09-10 (`548cc0a`). Narratives: archive Parts 54-60.

**SUPERSEDED 2026-09-11 — all 17 FOR-USER items are now RULED; see `## USER DECISION LEDGER`.**
The implementation queue those rulings created is the current work. Previously this read:
**There is no unblocked implementation work in this block.** What remains here is FOR-USER
decisions (the seven in the next section, plus item 3's two security calls) and **two owed AI-gate
items** — `### P108 — AC11 is still OWED` and `### P91 — open items`, both under
`## OPEN follow-ups`. Neither is closed by a native confirmation. For code work, pick from
`1b` / `1c` / `1d` below or from `## OPEN follow-ups`.

### Next, in order

1. **✅ FU-1 is DONE — `1d8c6f9`.** Both halves shipped, both reviews approved with **no MUST-FIX**,
   all 5 SHOULD-FIX + 7 NITs applied, **full 8-step gate green (452.5s, 2344 Rust tests, 185 e2e)**.
   Contracts: `docs/contracts/P87b-FU1-run-target.md` and
   `docs/contracts/P87b-FU1-FU4-git-dock-ui.md`. Both declare **no USER CHECKPOINT item**, so the
   milestone is fully closed. Full narrative: archive Part 59.2.

1d. **Contract-hygiene residue surfaced 2026-09-10 (architect's own list, filed not fixed).**
   All in `docs/contracts/P87b-FU1-run-target.md`. Hand these to the **next** architect spawn that
   touches the file — none is worth a spawn of its own:
   - The header blockquote still says "as of HEAD (`1be3a85`)" and "a senior-dev is mid-change in
     `GitActivity*` / `stash.ts` / `styles/`" — a pre-implementation snapshot. F-4 says the same.
   - §3's `remote_push_activity.rs:67-94` / `:226-256` line ranges have drifted (upstream
     resolution now starts ~`:56` and ~`:215`).
   - §8 understates the shipped seams: `?gitLongTarget` and `?gitBidiTarget` apply to
     `push || forcePush`, not push alone; the exported `MOCK_LONG_TARGET` / `MOCK_BIDI_TARGET`
     consts are never named; the `query()` idiom is at `:70-79`, not `:58-63`.
   - **§8's `?gitBidiTarget` row and §9 items 8-9 embed literal U+202E / zero-width characters
     while the same section instructs "write the escape, not the literal char"** — the contract
     violates its own rule. Same defect class as the one fixed in `gitActivityFormat.test.ts`
     during FU-1. Highest-value of this group.
   - §1's line-count estimates and §9's "`activity.rs` lands ~430" were never verified (it shipped
     at 437).

   **Closed 2026-09-10:** the designer's **F-G** (the two contracts disagreeing on the
   `MOCK_LONG_TARGET` literal) — the architect corrected §8 to the shipped 95-char string, and the
   orchestrator marked F-G resolved in `P87b-FU1-FU4-git-dock-ui.md` §5, since that section is
   addressed to the orchestrator. Both contracts now agree with the code.

1c. **Small follow-ups filed from the FU-1 pass (velocity mode — none blocking).**
   - `.git-run-noun` lacks the `white-space: nowrap` that `.git-dock-noun` has — the second
     bar-vs-row asymmetry after the `font-weight: 600` one FU-1 fixed. Found by the fix pass.
   - `.git-run-summary`'s gap is a hardcoded `8px` while `.git-dock-header` uses `--git-dock-gap`
     (6px compact). **Pre-existing**, not from FU-1 (ui-designer NIT 5).
   - Size-ratchet baseline drift: it reports `RepoWorkspace.tsx 2265 → 2264 (1 reclaimed)`, which
     predates FU-1 and wants `pnpm lint:size -- --update-baseline`.
   - Mock target fidelity is on disk as **F-F(a)** in `P87b-FU1-FU4-git-dock-ui.md` §5 (call sites
     pass literal `'main'`/`'origin/main'` regardless of the fixture's HEAD, so a `detached`/
     `unborn` mock fixture would render `Commit main`). Per spec; only matters if those seams are
     used with the dock open.
   - Code-reviewer NIT 7 (the running row's `aria-label` re-renders each tick because
     `durationWords` uses `.toFixed(1)`) was **judged conformant** with §3.7, which includes the
     duration in its own examples. Deliberately not filed as a defect.

1b. **Doc corrections owed from the FU-1 pass (three one-liners, none blocking).** Filed rather than
   fixed, to respect contract ownership:
   - **`docs/contracts/P87b-FU1-run-target.md` §4** — its unborn-HEAD rationale is **factually
     wrong**: it says `read_head_info` returns `branch_name: None` for unborn HEAD "so this is
     free", but it returns `Some` (it reads HEAD's symbolic target). The `null` is enforced by an
     explicit `if head.unborn || head.detached` guard, which is **load-bearing** — a future reader
     must not delete it as redundant. (architect's file)
   - **`docs/contracts/P87b-FU1-run-target.md` §8** — still carries the **83-char**
     `MOCK_LONG_TARGET` literal, which fails the same section's own "≥90 chars" requirement.
     Shipped as 95. §9.2 and §177 also still name the old test name. (architect's file)
   - **`docs/contracts/P87b-FU1-FU4-git-dock-ui.md` F-E** — claims `commitAmend` is not
     activity-wrapped. **It is** (`src-tauri/src/commands/staging.rs:179`, `with_activity(…,
     Amend, target, …)`). This stale line has now caused **two separate agents** to report a
     nonexistent FU-2 gap (architect's F-4 refuted it; the refutation never made it into the UI
     contract, so the next reader picked the wrong one up again). Highest-value of the three.

   **Batch these into the next `architect` / `ui-designer` spawn rather than spawning for them.**
   Lesson from this pass: F-E was known stale *before* ui-designer was invoked (both senior-devs were
   told so in their prompts) and the designer was editing §5 of that very file — the correction should
   have been in its prompt. When an agent that OWNS a contract is spawned for any reason, hand it the
   known corrections to that contract.

2. **✅ All reviewer follow-ups from the `a82740ff` pass are CLOSED — `e9d025d` / `7f9f16b` /
   `c03d11d`** (full 8-step gate green, 542.4s). Three of the six were not what the board said; the
   corrections are archive Part 59.3.
   **One standing warning from that pass, kept live because it is exactly the kind of fix a later
   session would "simplify" back:** in `whichAll` (`scripts/lib/spawn-tool.mjs`), prepending `''` to the Windows
   `PATH_EXTS` unconditionally would *regress* tool resolution — npm/corepack install
   **extensionless POSIX shell scripts** beside every shim, and `resolveTool` picks
   `hits.find(p => !isBatch(p))` as the real executable, so `''` first selects an unrunnable script.
   `''` is gated behind `hasExecExt` **deliberately**; 3 regression tests guard it. Do not ungate it.

2b. **One follow-up NOT taken (deliberate).** `src/obs/types.ts:68` still cites `A26 §D`, the same
   dead lettered scheme retired inside `raw_args.rs`. A repo-wide letter→section migration is a
   decision, not a drive-by; flagged rather than half-migrated.

3. **Security, still open** (`docs/audit-2026-09-03-external-launch.md`): MEDIUM-2
   (`terminalCommand`/`editorCommand` unvalidated — renderer compromise still converts to local
   execution) and LOW-1 (cwd DLL search order). **Both deliberately left for the user:** MEDIUM-2's
   suggested remedy breaks a legitimate absolute path to a portable editor, and LOW-1's mitigation
   changes launch behaviour. Product calls, not patches.

### Verification state

- **Full 8-step gate green at `1d8c6f9` (2026-09-10, 452.5s)** — 2344 Rust tests, 185 e2e passed /
  1 skipped. Per step: nextest 133.2s · doctests 3.7s · clippy 22.8s · eslint 12.7s · size ratchet
  0.9s · vitest 82.9s · tsc+build 15.3s · e2e 181.1s. A **later** full 8-step green run (542.4s)
  closed the reviewer follow-ups across the pair `e9d025d` (the code) + `7f9f16b` (docs/contracts
  only — verified `--stat`: `TODO.md` + the two `P87b-FU1-*` contracts, so the gate cannot have run
  *at* it). Both runs supersede the `c218258` state (archive Part 58).
- Exit code 0 is not sufficient evidence on its own: **grep the log** for failures. Many test NAMES
  contain `error`/`failed`, so a naive scan returns false positives — filter them.
- Verify the machine is idle first, and **redirect the whole log to a file** — piping `pnpm gate`
  through `tail` lost a failure detail and cost a re-run.
- Port **1420 is free**. Keep it so: `strictPort: true` means a held port breaks `pnpm tauri dev`.

---

## ✅ USER DECISION LEDGER — 2026-09-11 (all 17 open items ruled)

**Every FOR-USER decision below this section is now RULED.** The evidence that justified each
ruling is kept in place in the sections that follow; this ledger is the authoritative record of
*what was decided*. Do not re-open any of these without the user.

| # | Item | Ruling (user, 2026-09-11) |
|---|---|---|
| 1 | Branch `feat/p91-observability` | **MERGE to `dev`.** See the correction note below — the merge is a **fast-forward of 164 commits**, not 30. |
| 2 | Uncommitted `CLAUDE.md` + `context-explorer.md` | **COMMIT them** (jbcontext CLI→MCP migration), fixing the tool-list line that contradicts the agent's own frontmatter. |
| 3 | F6 `usage.json` | **Stay always-on, but 90-day window + deletable.** Statistics page stays viable. Disclosure copy still required (naming the `metrics` folder, not the file). |
| 4 | Security MEDIUM-2 `terminalCommand`/`editorCommand` | **Validate the shape now** (absolute path to an existing executable, no shell metachars, no arg injection) **and put removal of user-supplied commands on the roadmap.** |
| 5 | Username / home masking in raw log paths | **Mask the home prefix — and it MUST be cross-platform** (user's explicit addition): resolve the actual home dir per-OS so Windows `C:\Users\x`, macOS `/Users/x` and Linux `/home/x` all collapse. Do NOT pattern-match `C:\Users`. |
| 6 | Security LOW-1 cwd DLL search order | **Fix it in the same increment as #4's validation** (one call site, near-free while there). |
| 7 | D3 `.op-worktree-warning` | **Repaint as the warning hue.** Danger stays reserved for destructive/irreversible actions. |
| 8 | happy-dom | **ADOPT** (`docs/proposals/happy-dom.patch`) **and also do the lazy `window.location` fix** in `src/ipc/mock/repoState.ts:160`. jsdom stays installed so the shim self-guard stays meaningful. |
| 9 | e2e bundle default | **MEASURE and REPORT, do NOT flip.** One cold `E2E_BUNDLE=1` run vs the 162s dev figure *including build*. The number goes on the record; the default does not change without the user seeing it. |
| 10 | P69 A3 AI gate-note copy | **ui-designer finalises it** with the surrounding copy in view; its signed string ships. |
| 11 | P77 tag-sync check | **Fold into the existing auto-fetch cycle** (on, 5-min). No new trigger, no repo-open network call. |
| 12 | Process changes | **ALL THREE ADOPTED** — see the new rules block below. |
| 13 | P108 `AC11` | **ACCEPT the source-derived figures** for the two unreachable states; record the limitation and **close AC11**. |
| 14 | Back up `.tauri/updater-prod.key` | **Not now** — stays on the board as a user action. Still single-copy; still breaks auto-update for every installed client if lost. |
| 15 | Dependabot moderate alert | **Not now**, and **no `gh` install authorised** — so the orchestrator cannot read the page. Stays open as a user action. |
| 16 | P91 owed AI-gate item (real `logs/*.jsonl`) | **User will boot `pnpm tauri dev` with Dev mode ON.** Orchestrator parses the files once they exist. Verified 2026-09-11: no Dev-mode key in persisted `settings.json`, so it cannot be pre-set from disk. |
| 17 | macOS ad-hoc signing | **PARK as blocked-on-release.** Re-raise when a tag is next cut. Not open work. |

### Correction to the board's own commit count (verified 2026-09-11)

The RESUME block said "30 commits ahead of `5c2dcd2`". Measured:

- `dev` is at `cb70f4a` and is a **strict ancestor** of this branch → the merge is a **pure
  fast-forward**, no merge commit, nothing to resolve.
- `cb70f4a..5c2dcd2` = **126 commits**; `5c2dcd2..HEAD` = **38** (the board's "30" is stale by 8).
- Total landing on `dev`: **164 commits**, of which 126 predate the board's own reference point.
  The "30 commits" framing described only the recent P91 window, not what `dev` has never seen.
- Also on the remote: `origin/Dev` (capital D) at `691f48b`, a separate ref from `origin/dev`
  (`cb70f4a`). A case-collision artefact — not touched, but do not confuse the two.

### The three process rules adopted 2026-09-11 (user)

1. **Batch small P-tasks through ONE senior-dev spawn.** Every fresh subagent re-pays this
   CLAUDE.md + its agent def (~6-8k tokens) before doing anything.
2. **Skip the architect contract for single-component fixes.** Accepted cost: nothing survives on
   disk if the session dies mid-increment, so keep such increments short.
3. **Fold the board update into the feat commit.** Accepted cost: a docs-only commit can no longer
   be identified as such from the log.

---

## FOR USER — decisions (ALL RULED 2026-09-11 — evidence kept, rulings in the ledger above)

### 0. happy-dom — ✅ RULED 2026-09-11: **ADOPT, plus the lazy `window.location` fix**

> Evidence below is the measurement that justified it. Apply `docs/proposals/happy-dom.patch`, then
> `pnpm install`. Also make `src/ipc/mock/repoState.ts:160` lazy (the board already called this worth
> doing regardless). The shim's quiet a11y-naming failure mode is an ACCEPTED risk — if a future
> test uses an inline tag outside the shim's set it silently gets `display: block`.

**Patch is saved at `docs/proposals/happy-dom.patch`** — apply with
`git apply docs/proposals/happy-dom.patch` then `pnpm install`. The working tree was **restored to
jsdom** so nothing downstream is measured against an unapproved toolchain.

**The win is real and measured**, five jsdom runs against four happy-dom runs, machine-load sampled
before each, plus a back-to-back control 45 seconds apart so it is not a stale-baseline artifact:

| | wall (median) | environment CPU | tests CPU |
|---|---|---|---|
| jsdom 30.0.1 | 60.3 s | 574 s | ~118 s |
| happy-dom + shim | **44.6 s** | **321 s** | **~64 s** |

**−26% wall, −44% environment CPU, −46% test CPU**, and variance tightens from ±6 s to ±0.4 s.

**Two things make this your call, not mine.**

1. **A dependency add:** `happy-dom ^20.12.2` as a devDependency (+13 transitive, −4). `jsdom` is
   deliberately **left installed** so the shim's self-guard stays meaningful and rollback is one line.
2. **It needs a hand-maintained shim, and that is the part I would weigh hardest.** happy-dom's
   `getComputedStyle` omits the UA default stylesheet — `display` on every inline element and
   `visibility` on all elements come back `""`. `dom-accessibility-api` branches on `display` to
   decide whether to insert a space between child text alternatives, so three tests computed
   `"Added src/ app.rs"` instead of `"Addedsrc/app.rs"` — **a space injected mid-path**. The shim
   supplies the missing UA values and is self-guarding (inert under jsdom, verified by flipping the
   config back). But it is **a hand-maintained subset of the UA stylesheet**, and *if a future test
   uses an inline tag outside that set it silently gets `display: block`.*

   **Exposure:** 715 `ByRole(…, { name })` call sites across 77 files depend on that computation;
   712 are unaffected today only because they target leaf elements with flat text. So the
   silent-failure mode sits precisely in accessibility-name computation — the area where this
   session found several real defects. That is the trade: **26% faster tests against a maintained
   shim with a quiet failure mode in a11y naming.**

**Option 2 (`environmentMatchGlobs`, so only DOM-touching files pay) is DEAD** — and the reason is
worth keeping. Of 156 DOM-project files, 137 use the DOM directly; the other 19 *look* DOM-free but
**all 19 fail in a node environment for one root cause**: `src/ipc/mock/repoState.ts:160` calls
`new URLSearchParams(window.location.search)` at **module init**, so anything that transitively
imports the mock IPC layer needs `window`. Measured anyway: 57.0 s with 18 files failing, and its
theoretical ceiling was only ~6.7% of total CPU.

**Filed follow-up (unlocks option 2, ~3-line app change, not made):** make that `window.location`
read **lazy** in `repoState.ts`. It would move ~19 files to the node environment, worth roughly a
further 70 s of CPU, and is worth doing *regardless* of the happy-dom decision.

**Verified clean, no shim needed:** `Range`, `Selection`, `createRange`, `MutationObserver`,
`IntersectionObserver` (happy-dom *has* it and jsdom does not — a gain), canvas stubs, `DOMRect`,
`requestAnimationFrame`, `structuredClone`. `toBeVisible()` was checked against jsdom on all six
hiding mechanisms — **no silent-pass hazard**.

---

### 1. e2e bundle default — ✅ RULED 2026-09-11: **MEASURE and REPORT, do NOT flip**

- P104 is cleared, so nothing blocks the flip; the orchestrator deliberately did **not** make it.
- The blocker is a measurement gap: the figures are **162 s dev server vs 122 s bundle**, and it is
  not established that the 122 s includes the **bundle build step**. If it does not, flipping could
  make the default gate *slower* — the opposite of the purpose.
- In favour: bundle mode is equally green (181 passed / 1 skipped / 0 failed in both modes) and has
  **higher fidelity** — P103 was a real product bug only the bundle-vs-dev check exposed, because
  `import.meta.env.DEV` code is absent from a production bundle.
- One line each, trivially reversible: `playwright.config.ts:38` →
  `const BUNDLE = process.env.E2E_BUNDLE !== '0'`, plus inverting `--e2e-bundle` in `gate.mjs`.
- **To decide it: time one `E2E_BUNDLE=1` run from a cold build** and compare with the 162 s dev
  figure including build. Full context: archive Part 48.

### 2. F6 `usage.json` — ✅ RULED 2026-09-11: **always-on stays; 90-day window + deletable**

> Not option A/B/C as specced: the user kept collection always-on (so a future Statistics page stays
> viable) but cut 400 days → **90** and required it be **deletable**. The disclosure copy is still
> owed and must name the **`metrics` folder**, not the file.

- `usage.json` is **always-on durable local telemetry**, independent of Dev mode, from first launch:
  `firstSeen`, launch count, and a **400-day** per-day profile of which operations ran and how long.
- It survives `logs_delete_all` **by design**, `metrics_reset` has **no UI**, and the privacy panel
  never mentions the file exists.
- Content is non-identifying by construction, so this is a **disclosure** question, not a leak. But
  the panel is headed "What a log file contains" and ends with "removes every one of them", so **by
  omission it reads as though Dev mode is the only thing recorded and the delete button clears it**.
- `ui-designer` recommends **option A: disclose, no reset button**. Option B (a reset row) reverses
  ratified §10 and belongs on a future Statistics page.
- The remedy must name the **`metrics` folder**, not the file: deleting `usage.json` alone lets
  `usage.json.bak` restore it.
- **Until the user rules, only §2-§5 of the copy contract get implemented.**
- P91 has never shipped, so now is the moment to decide whether a local Git client should keep an
  undeletable 400-day usage profile with no disclosure. Full text: archive Part 43.

### 3. Home/username masking — ✅ RULED 2026-09-11: **mask the home prefix, CROSS-PLATFORM**

- `scrub.rs` has **no username rule** (verified independently by grep), so a **raw absolute repo path
  carries the OS account name** into a mailed export zip.
- Not a leak of repo content, but it is identifying, and the export workflow mails it to a third party.
- Now **disclosed** in the consent copy (`0a785b3`); **whether to also mask it is unresolved** —
  masking would undercut raw mode's stated purpose of showing real paths.

### 4. D3 `.op-worktree-warning` — ✅ RULED 2026-09-11: **repaint as the warning hue**

- It is a **warning painted danger**: a *tone/semantics* question, not a contrast one.
- Deliberately **not** folded into P108 — changing it alters what the UI means, not whether it can be
  read. P108's fix kept the danger hue and changed only legibility, so the tone question is untouched.

### 5. Two 1.0.0 items — ⏳ 2026-09-11: both DEFERRED by the user, still user actions

1. **Back up `.tauri/updater-prod.key`.** Correctly gitignored and untracked, so it exists in exactly
   ONE place: this working copy. Losing it permanently breaks auto-update for every installed client.
   (The committed `tauri.conf.json` pubkey was verified to match it.) **Also: P71 must not touch it.**
2. **GitHub reported 2 Dependabot alerts (1 high, 1 moderate)** on push. The high is the known
   `nanoid` GHSA-2v37-7h3g-55p8 — build/test tooling only, deliberately ignored in
   `pnpm-workspace.yaml`. **The moderate is unidentified** — `gh` is not installed here; both project
   gates are green. Check the Dependabot page.

### 6. Record contradictions — 🔧 2026-09-11: **orchestrator to verify and close these** (user assented)

The curator refuses to resolve these; resolving any would upgrade a status.

- ~~**P107's board heading says "CONTRACT DONE, IMPL PENDING"**~~ — **RESOLVED by the orchestrator
  2026-09-03.** The heading predated `2168057` and was stale; it was restated in P107's own section
  — **which is now archive Part 54.4** (the section left the board 2026-09-10). The clause "AC11 /
  AC12 / AC13 remain pending USER CHECKPOINTs" was true when written and is **no longer true**: the
  user confirmed all three on 2026-09-10 (`548cc0a`). Kept, corrected in place, because this bullet
  is the board's record of its own staleness.
- ~~**`no_proxy_client()` is claimed both open and closed.**~~ — **RESOLVED: CLOSED, verified by the
  orchestrator 2026-09-03.** Checked directly rather than taken from the audit report:
  `src-tauri/src/mcp.rs:411` declares `#[cfg(test)] mod http_support;`, so the module carrying the
  raw `.expect("build reqwest client")` (`http_support.rs:221`) **is never compiled into a shipped
  binary**. The DEP-REFRESH follow-up filed it as a panic path reachable in production; that premise
  is false, so the item is closed on evidence rather than on a report's say-so. Closing a follow-up
  on a verified fact is not a status upgrade — no checkpoint is involved.
- **Four follow-ups from the 2026-09-02 file-size refactor look addressed by later commits on this
  branch, but nothing records them closed:** `52c815e`/`6092eb3`/`338d71f` (reflog + overlay
  teardown), `1d9d9bf` (armed dialogs during confirm dialogs), `734b310` (the `ai::session*` clock
  seam). Kept open below.
- **Two 2026-09-01 velocity follow-ups read as superseded** by the 2026-09-03 pass (`737cc4b`,
  `5731d37`), which banded `prop_status` (3.25x) and `prop_stash_roundtrip` (3.2x).
- **No contract file was ever written for P94.**

---

## Durable lessons — the rules

The reusable rules. They are on the board, not in the archive, because every one was learned by a
claim that was green the whole time it was wrong. **The stories, worked numbers and measurement
narrative behind them are archive Part 53** — cite the rule here, read the story there.

### The six failed app-wide claims

Every one had the same shape: **a sentence claiming an app-wide property, with a call-site count that
nobody enumerated.** The first five failed for want of an enumeration. The sixth is worse — the
enumeration **existed** and was still blind, because it was **scoped by token name**.

1. **P95** — the enabled-control class; 3 escapes found by P101.
2. **P98** — "`--text-3` family closed"; 122 declarations were never classified.
3. **P74** — the hue-as-text sweep; became P105.
4. **`ui-reference.md` §2** — "6 live hue-over-own-tint instances"; the real population is **38**.
5. **P106's hand-over count of 48** for P108; the real inventory was **62**.
6. **P101** — an *exhaustive* `--text-3` audit recorded as CLOSED, which still missed a **2.96 light**
   glyph, because **`--badge-unknown` is byte-identical to `--text-3`**.

**The rule:** a bucket + verdict per call site, P101 §3 style, or it is not closed. Enumerate,
bucket, record a verdict per site, predict the post-fix residue, then verify the prediction.
**Do not accept a "~N call sites and it's fine" sentence as evidence.**

### The aliasing rule

- **An audit scoped by token NAME cannot see an alias. Scope by resolved VALUE.**
- Token aliasing has hidden instances three times: `--badge-good`/`--badge-warn` are byte-identical
  to `--success`/`--danger` (the first pair), then `--badge-unknown` to `--text-3`.
- A `var()` **fallback masking a missing token is invisible to any hue-name search**, because the hue
  name appears only in the fallback (`--warn`, which is defined nowhere).
- A naive probe of an **undefined** custom property returns the *inherited* value, not the value the
  contract cites — same trap class.

### The grep-counting rules

- **An acceptance grep counts *text*.** Prose comments and `var()` fallbacks inflate it. P106's R10
  read 0 predicted vs **12** measured: 7 hex literals quoted inside comments, 5 `var()` fallbacks.
- A residue prediction must state whether it counts **declarations or raw matches**.
- **A baseline must be measured against the real pre-fix tree**, never inherited from a prior
  contract. Both of P106's misses were the contract's *baselines* being wrong, not the fix.
- **When a grep and a prediction disagree, re-measure the baseline BEFORE touching code.**
- Where a prediction names `file:line`, **the count is the criterion** — comments added by the fix
  itself shift the lines (216/227 → 220/231 in P106; count unchanged).
- An implementer must deliberately avoid literal strings in its own comments: without that care two
  P106 counts would have read 11 and 2 instead of 8 and 1.

### The BASE rule

- **A contrast figure is meaningless without its composited base.** Every ratio must name the ink,
  the tint, **AND** the base.
- A figure naming only the tint is **incomplete evidence and may not be used to close an AC**.
- Proof: `--accent-strong` on a 14% accent tint measures **6.42/5.19 over `--bg-0`**, **5.85/4.87
  over `--bg-1`** (P107's figure) and **5.16/4.52 over `--bg-2`** (P106's) — one method, three bases.
  The two contracts never disagreed; **neither stated its base**, and the base alone accounts for
  **1.26** of dark-theme spread.

### The three P91 testing rules — now in the contract, not only in session memory

1. **A writer rule is covered only by a `LogRecord` → `append_record` → read-back round-trip.**
   Synthetic-`Value` tests are additive, never substitutive. This is what let W6 ship dead in
   production while its test passed — the one rule of six that skipped the round-trip.
2. **A validator's tests must use inputs the REAL producer emits**, and a cross-boundary vocabulary
   must be pinned by a drift test that re-derives it from the producer's own source. *A predicate
   that rejects everything is indistinguishable from one that works, unless something asserts a real
   input is ACCEPTED.* This is the `cmd.*` camelCase bug — the **entire `cmd.*` histogram family
   recorded NOTHING in production** while every test was green.
3. **A negative test must be PROVEN to fail on the unfixed code.** Saying "this must fail on today's
   code" is not proof. AC6 as originally written specified a payload the scrubber **already caught**,
   so it would have been green on the buggy code — a negative test that could not go red.

### Two more failure modes, each named after it cost a session

- **Evidence lost in transcription** (named at `ef06e6b`) — a figure or a qualifier that survives the
  measurement and dies in the summary. P108's AC11 is the live example: **source-derived and
  unverified** must travel with the number.
- **The specificity trap** — **a contrast fix that is out-specified by an existing rule is a no-op
  that still passes a grep.** `.diff-stage-float button` (0,1,1) out-specified `.diff-float-discard`
  (0,1,0), so the destructive discard button rendered in accent blue with no danger hue at all while
  passing every AC grep. Sibling to the child-rule trap; both are in `ui-reference.md` §2.

### The measurement rules (numbers: archive Part 46; the worked narrative: archive Part 53)

- **Optimising the measured-slowest test may not move wall clock, because a different test becomes
  the floor.** Net for the 2026-09-03 pass: `cargo nextest --workspace` **106.5s → 92.5s** with tests
  *increasing* 2298 → 2316.
- **Concurrent agents on this box produce outliers** — single-run numbers are worthless; use paired
  or repeated runs.
- **proptest regression seeds can be worse than useless**: proptest keys its persistence file **per
  source file, not per test fn**, and a `cc` seed regenerates values through the *current* strategy.
  Random cases wearing a regression label.
- **Do not run the e2e suite concurrently with other heavy jobs.** `gate.mjs` is strictly serial, so
  the gate itself is safe.
- **A flake that reproduces deterministically in a production bundle is not a flake** (P103).

### The gate-running rules (the gate states that earned them: archive Parts 51 and 58)

- **Before trusting any timing-sensitive failure, verify machine state AT THE TIME IT RAN** — sample
  CPU repeatedly, look for scratch/load processes, confirm no agent is mid-run.
- A slow timing number is **evidence about the machine** until proven otherwise. Timing analogue of
  the grep-counting rule: *measure the baseline, do not infer it.*
- **Verifying machine state before running the gate is a standing pre-gate step**, not a nicety.
- The e2e leg **will** fail intermittently on a loaded machine — expected, documented at P104 (Edge
  misses a hardcoded 30 s CDP close window, then a blocking `taskkill` runs).
  `FIRST_PAINT_TIMEOUT = 15 s` (`c6cd7dd`) removed the largest source, measured at 7.6× p99.
- **Run the gate on an otherwise-idle machine, or run e2e with `--workers=1`.**
- **Give subagents `--workspace`, not `-p <crate>`, whenever a change crosses a crate boundary.**
  A remediation brief scoped to `cargo nextest -p bonsai-core` never ran the `bonsai` crate's tests
  under `src-tauri/`, and one of them asserted the exact behaviour the change removed — the gate's
  first run failed on it.
- **Redirect the whole gate log to a file**; read the summary from there. Piping through `tail -60`
  lost the failure detail and cost a re-run of the Rust leg.
- **Read the `gate summary` block, never the exit status alone** — a background wrapper reported
  exit 0 while the gate had failed (that was the pipeline's exit code, not the gate's).

---

## Accepted decisions that must survive compaction

- **Accepted defaults (2026-08-08, "ACCEPTED AS-IS"; changeable any time):** P55 `undoLastMerge` =
  reset-to-first-parent (Mixed, rewrites history, confirm-gated) · P57 retriever = BM25 lexical, no
  embeddings · P61 image-diff base64 = hand-rolled, no new crate.
- **OD1 (confirmed):** AI stays **local-`claude`-CLI-only**; model tiers deferred.
- **Forge defaults (2026-08-08, accepted):** new Rust deps `reqwest{blocking,json,rustls-tls}` +
  `keyring` · auth = **PAT-only** v1 (OAuth device-flow deferred) · provider order GitLab →
  Bitbucket → Azure DevOps.
- **v1.0.0 shipped** 2026-08-18 (tag `bd52483`), unsigned; forge/PR flagged beta.
- **P62-P74 native USER CHECKPOINTs were WAIVED and marked `done` 2026-08-20** (user decision).
- All native USER CHECKPOINTs for **P2 → P61** are confirmed; P70, P77, P78/P79/P80, P80b/P81/P82,
  P83, P84, P85-P90, P92, P93, P95, P96, P97, P98, P99, P100, P101 and DEP REFRESH are confirmed too.
  Full per-milestone text → `docs/history/todo-archive-2026-08.md` Parts 17-21 and
  `todo-archive-2026-09.md` Parts 22-31, 47.
- **P100 + P101 checkpoints were recorded 2026-09-02 on the user's direct instruction**, not on a
  contemporaneous native run — the user was going away for an unattended session and scoped that
  authority **to those two milestones only**. It does **not** reach anything created that session.
- **P75 (IPC codegen) — HALTED 2026-08-21 (user decision).** Linking `tauri-specta` breaks app launch
  on Windows 10 (`kernel32!WaitOnAddress` not exported → `STATUS_ENTRYPOINT_NOT_FOUND`). Spike
  reverted; findings + crate pins kept in `docs/contracts/P75-ipc-codegen.md`. Revisit only if
  validated on Windows 11 or with a link-order fix.
- **P76 (native-checkpoint automation) — HELD as contract-only per user (2026-08-20).**
  `docs/contracts/P76-native-checkpoint-automation.md`.
- **Process change adopted (P100 §6-D).** `ui-reference.md` is ~1322 lines / ~40k tokens and
  `ui-designer` has **no `Edit` tool** — only whole-file `Write`, which **truncates mid-file** at that
  size. That is the structural cause of the P95 "silently unapplied patch". From P100 on: the designer
  supplies **verbatim line-anchored hunks**, the orchestrator applies them with `Edit`, and verifies
  line count + section count + tail sentinel + hunk confinement. **This deviates from CLAUDE.md's
  "no other agent edits `ui-reference.md`" — raise with the user whether to give the designer `Edit`
  or split the file.**
- **Do NOT run prettier in this repo** until a config exists — there is no `.prettierrc*`, no
  `prettier.config.*`, no `package.json` key and no `.editorconfig`, so prettier falls back to its
  defaults and rewrites whole files (74 insertions for a ~20-line edit; double quotes at 80 columns
  against the repo's ~100).
- **Do NOT run `cargo fmt`** as part of another change — see the open item below.
- Gate child processes use `D:\Data\Temp\bonsai-build`, not Defender-scanned `C:\Temp`.
- Ports: browser harness **1420** · e2e dev server **1430** · e2e built bundle **1440**.
- Velocity/gate-cost numbers: `docs/history/velocity-2026-09-01.md`. Ceremony — not machine time — is
  **~75-85%** of per-task wall clock, which is what CLAUDE.md's velocity-mode and batching rules
  exist to cut.

- **All eight native USER CHECKPOINTs were confirmed by the user on 2026-09-10** (`548cc0a`):
  P102+P105, P106, P107, P108, P91, P110, P109, and the `8dd5b24` CSP change — the last of these
  needed a native run because it applies to the Tauri webview only, so neither the harness nor e2e
  ever exercised it. Full per-milestone text: archive Parts 54-55, with the board's own record of
  the confirmation in Part 60. **The confirmation does not reach two AI-gate items** (P108 `AC11`,
  P91's `logs/*.jsonl` parse) and **is not authorisation to merge `feat/p91-observability`**.

**P91 user decisions that must survive compaction (all 2026-08-27 unless noted).** Build diary:
archive Part 44; the milestone entry is archive Part 54.6.

- Redaction conservative by default, opt-in raw-names toggle; credentials never logged in any mode.
- Metrics storage = **rolled-up JSON**, not SQLite.
- React instrumentation = **SIX surfaces** — the user added the **left sidebar** to the original five.
- Logs are **NOT** auto-deleted when Dev mode goes off (prune by caps only); per-session log files;
  `metrics_reset` ships headless.
- Decision 7 — **"Delete all log files" ships in v1**, model = **roll-then-purge**, scope hard-limited
  to `logs/*.jsonl` + `.tmp`, never `metrics/` or `settings.json`. Success copy must say "Still
  recording" when `rolled:true`.
- **USER GATE (2026-08-27):** each increment requires its own explicit go from the user.
- **Branch policy (USER, 2026-09-02):** everything this session lands on `feat/p91-observability`;
  local commits only, no push.

**P91 — three architectural rulings not to re-open.**

- **NO global cap on metric keys.** Worst case is genuinely per-map-per-bucket, ~616k keys, accepted
  rather than glossed: a global cap would make *today's* recording depend on *history*. 512 is a
  **runaway stop, not a sizing parameter**. File-size pressure has a different lever (size-triggered
  early roll-up) — revisit trigger only, ~8 MB, no work now.
- **The writer's limit, recorded honestly in both contracts:** `raw_args.rs` enforces **shape +
  vocabulary, not semantics**. Content under an identifier-named key, ≤512 chars, single-line,
  **survives**. Its only defence is the positional drift guard in `rawArgPolicy.test.ts`.
  **Neither guard may be removed citing the other.**
- **The raw-args ruling (`834f2d1`):** raw mode widens **IDENTIFIER** fidelity (repo path, file
  paths, ref names, remote URLs, full SHAs) and **never CONTENT** fidelity. Free text and credentials
  are outside both modes, permanently.

---

## 🆕 SECOND ROUND OF USER RULINGS — 2026-09-11 (four more, from the review findings)

These came out of the reviews of the first increment, not from the original 17. Same authority.

| # | Item | Ruling |
|---|---|---|
| 18 | MCP audit scope (`stage_paths` HIGH + MEDIUM + 4 LOWs) | **DO EVERYTHING IN THE AUDIT.** Not just the symlink guard. **QUEUED, not started.** |
| 19 | MCP review gate | **Snapshot test + review trigger.** A test snapshotting `list_all()` tool descriptions, **and** a rule that any diff touching `crates/bonsai-mcp/src/server/tools_*.rs` requires a `security-auditor` pass regardless of the commit subject. **QUEUED, not started.** |
| 20 | macOS `open -a` / `.app` regression from the `{path}` removal | **Teach the ladder app bundles** — Bonsai supplies the arguments itself, so no injection surface. **SUPERSEDED IN PLACE by #21:** this moves into the *detection* logic of the removal milestone rather than into the configured-program path, which is being deleted. Recorded so the intent is not lost. |
| 21 | Security MEDIUM-2, after the auditor showed validation insufficient | **DO THE REMOVAL NOW**, as its own milestone — not the interim native-confirmation dialog. Free-text program entry is eliminated and replaced by a detected-list picker. Contract in flight. |
| 22 | Fail-open home masking + the LOW-1 doc claim | **TAKE BOTH.** |

### Why #21 — shape validation does NOT achieve its claimed property

The first increment's own comment claims "renderer compromise ≠ arbitrary local execution". The
security auditor refuted it with **three routes that survive validation**, all needing only script
execution in the renderer plus a repo that was cloned once:

1. **The absolute branch accepts any existing file** — `external_cmd.rs:145-155` gates on `is_file()`
   alone: not executability, not location, not trust. A hostile repo ships `payload.exe`; the
   renderer points `editorCommand` at that absolute path; it validates; and `external.rs:319` sets
   `hide_console = true`, so it runs under `CREATE_NO_WINDOW` — **with no visible window.** The
   increment's own test (`external_cmd_tests.rs:60-67`) documents the shape: it writes a stub named
   `my-editor.exe` containing `b"stub"` and asserts acceptance.
2. **Bare interpreter + repo as argument** — `node <repo>` executes the repo's own `package.json`
   `main` or `index.js` (`external.rs:244`). More likely than the `python` the doc cited, on a
   developer machine and in a JS project.
3. **Bare build tool as the TERMINAL command** — `make` / `nmake` / `just` / `msbuild` with
   cwd = repo and **zero arguments** (`external.rs:245`). This route **breaks the residual's own
   "at most one argument, which must be an existing directory" wording.**

Root cause: it is a **free-text program string the renderer can write** via `set_ui_settings`.
Validating its shape cannot fix that. The audit status is being downgraded from "Closed 2026-09-11"
to **partially closed** for both MEDIUM-2 and LOW-1.
Also: `src-tauri/src/commands/external.rs:89` `launch_inner` accepts **any existing directory the
renderer names**, not the selected repo — pre-existing P49 design, so the argument was never bounded
to the open repo either.

**Mitigating context (why this was MEDIUM, not HIGH):** the CSP at `src-tauri/tauri.conf.json:23`
is `script-src 'self'`, `object-src 'none'`, `base-uri 'none'`, which blocks inline script and
inline event handlers. Noted by the auditor as premise context: two `dangerouslySetInnerHTML` sites
render syntax-highlighted diff content (`DiffView.tsx:294`, `DiffViewSplit.tsx:162`) whose
highlighter escaping was **not** audited — pre-existing and CSP-mitigated, **not** opened as a
finding, but it is the premise the whole MEDIUM rests on.

### LOW-1's surviving rung is the Windows 10 DEFAULT, not an edge case

`wt` is **not present on stock Windows 10** (this machine is Win10 Enterprise), so the live rung is
`powershell` (`external.rs:278`), which keeps `cwd = <repo>`. A hostile repo shipping a hijackable
DLL at its root therefore has its DLL-planting primitive against the **default** Win10 "Open in
terminal". Keeping cwd is still correct — the alternative,
`powershell -Command "Set-Location '<path>'"`, turns a repo-authored path containing a quote into
PowerShell injection, a strictly worse trade. **`x-terminal-emulator` is being dropped from the
residual list:** neither the Linux dynamic linker nor macOS dyld searches the current directory by
default, so only the Windows rungs carry the risk.

### Home masking was FAIL-OPEN — fixed under #22

`obs/mod.rs:177-182`: if `app.path().home_dir()` returns `Err`, `set_home_dir` is never called,
`HOME` stays unset, `mask_home` returns the input untouched, and raw mode writes
`C:\Users\<account>\…` into every part that goes into the mailed export zip. The only signal was an
`eprintln!`, **which goes nowhere in a release GUI build.** This contradicted `scrub_home.rs:30`'s
own stated failure direction. Compounding it: **nothing tested the wiring** — deleting the
`set_home_dir` call would have failed no test. Fix: move the home string into `writer::WriterConfig`
(testable, no process-global `OnceLock`) and stamp `homeMasking: <bool>` into the session header so
an export reader knows whether to trust it.

### Verified CLEAN by the security auditor — do not re-audit

Every reader of the two settings (exactly two consumption sites, both behind validating entry
points; the MCP server and AI CLI driver read neither key) · validate-then-launch trim equivalence
(both sides `str::trim` on the same `&str`) · argument smuggling through the permissive absolute
branch is **structurally impossible** (the string becomes `LaunchSpec::program` only, never an arg;
`Command::args` with an argv vector; nothing reaches a shell) · UNC, drive-relative, rooted-no-drive,
control/bidi chars all refused and tested · ADS / 8.3 aliases / trailing dots / `..` / symlinks /
the `is_file()`→spawn TOCTOU are all **subsumed** by route 1 rather than separate findings · the cwd
carve-out is exactly the four rungs claimed, with an iff test · `procutil::resolve_program` searches
PATH only, never cwd · the export zip contains only `bonsai-*.jsonl` parts (no manifest, no metrics
file, no environment dump) and every part goes through `append_record`, so no emit site bypasses the
scrub order · `metrics/usage.json` and its `.bak` **cannot carry a path** (no path fields;
`metrics_keys.rs` rejects separators) and are not in the zip regardless · `set_home_dir` runs before
`Sink::start`, and `apply_dev_settings` is the sole production sink constructor.

**Could NOT verify (so the CLEAN register does not over-claim):** Rust std's Windows `resolve_exe`
`.exe`-suffix appending and its `.bat`/`.cmd` → `cmd.exe` wrapping — `rust-src` is not installed in
this sysroot. Medium-high confidence from the post-CVE-2024-24576 implementation; neither changes
route 1, which stands on `.exe` alone. No launch was executed; all findings are from code reading.

---

## 🔄 IN FLIGHT — the implementation queue the 2026-09-11 rulings created

**Current step: architect amending `P91-observability.md` for F6; senior-dev on the security
increment; security-auditor on `2a0b8f1`. ui-designer DONE.**

### ✅ ui-designer pass DONE 2026-09-11 — contracts written, two code changes OWED to senior-dev

- **A3 — SIGNED: the SHIPPED string stays.** `Turn on "Enable AI features" above to change these.`
  ui-designer **withdrew its own preferred reword** after finding `SettingsAiSection.tsx:110`
  byte-identical modulo `this`/`these` and `SettingsDevCaptureSection.tsx:52` a third instance — it
  is a pattern, not a string. Signed into `ui-reference.md` §12.12 + `P68g-ui.md`. **No string in
  `src/` changes, so no test moves.**
  → **OWED to senior-dev:** clear two now-stale comments, `SettingsAiRunSection.tsx:25` and `:46`,
  both reading "pending A3 sign-off".
- **D3 — SPECCED, NOT APPLIED** (`src/styles/**` is outside ui-designer's remit).
  → **OWED to senior-dev:** `src/styles/dialogs.css:238`, `.op-worktree-warning`:
  `var(--danger-strong)` → **`var(--warning-strong)`**.
  Measured on `.dialog-card`'s `--bg-1`: **8.38:1 dark / 6.65:1 light** vs P108's ≥4.5:1 bar. P108
  shipped `--danger-strong` there at 7.62/6.01, so **the tone repaint RAISES contrast in both
  themes** — it does not trade legibility for semantics. No new token; no grep invariant moves.
- **D1 and D2 were BOTH already fixed — my brief to it was stale on both.** F-E was corrected
  2026-09-10 (`P87b-FU1-FU4-git-dock-ui.md:461-478`) and re-verified 2026-09-11.
  `.forge-connect-link:hover` was fixed by P107 at `forge-pr-create.css:209-225` (thickness pinned
  1px at rest, 2px on hover, with a comment naming this exact defect).
  **The actual stale line was `docs/contracts/INDEX.md:54`** — the propagation vector for the third
  false `commitAmend` report. **FIXED by the orchestrator 2026-09-11**; the P91 row's "unmerged by
  user instruction" claim was corrected in the same pass.

### F6 — two backend facts the copy now depends on, ONE OF WHICH FAILS SILENTLY

Recorded here because a later session must not implement half of it:

1. The delete must remove the **`metrics` directory recursively** — `usage.json.bak` restores the
   file if only the file goes.
2. It must **also reset the in-memory `MetricsState`**. Otherwise the next flush writes the
   just-deleted data straight back, **and a test that only asserts the file is gone PASSES while the
   button does nothing.** ui-designer's AC 8 asserts the snapshot, not the file, for this reason.

**Also corrected 2026-09-11 — the board (and my own brief) OVERCLAIMED what is always-on.** The
always-on data is **counts**; the **durations half is Dev-mode-only** (`metrics.rs:148-152`). A
privacy surface claiming always-on durations would be an overclaim. `P91-privacy-copy-ui.md` §6.1.1
now tables the verified field-by-field picture and is the source of truth over the board.

**`lifetime` figures — RULED 2026-09-11 (user): KEEP them.** `first_seen` and `sessions` survive the
90-day fold; the window applies to the **per-day profile only**. Rationale: both are inherently
lifetime values and the Statistics page — the user's stated reason for keeping collection always-on
instead of Dev-mode-gating it — needs the total. Deletion still removes everything, lifetime figures
included: "retained 90 days" and "cleared by the delete action" are **separate promises and both
must hold**. Do not let a later session tidy the lifetime fields into the prune.

**Scope the F6 amendment correctly — §10 is NOT the only place asserting metrics survive deletion.**
Also `P91-observability.md` §6.1 `:713-726`, §8 `:1425`, §13 row 6 `:1936`, §8 prose at `:1404`
`:1494` `:1575` `:1597`, and the rationale in `src-tauri/src/obs/metrics_keys.rs:44`.
`RETAIN_DAYS` is `src-tauri/src/obs/metrics.rs:40`, currently **400**.

**Design hole the ruling opened, fixed in `P91-privacy-copy-ui.md` §6.4:** the delete row was gated
on `hasLogs`, so a user who never enabled Dev mode would face up to 90 days of usage counts behind a
permanently disabled button. Gate dropped. No new IPC field needed — `metrics.rs:210`/`:235` bump
`sessions` at init every launch, so there is no reachable "nothing to delete" state.

---

## ✅ CLOSED 2026-09-11 by orchestrator verification (user assented to my closing these)

Each was verified by reading the commit or the file, not by trusting the board.

- **The four 2026-09-02 file-size refactor follow-ups — CLOSED.** All five commits exist and do what
  the board guessed: `52c815e` "clear the reflog overlay on the repo-went-unusable teardown",
  `6092eb3` "close every overlay on the repo-went-unusable teardown", `338d71f` "add the
  diff/composer/palette overlays to the unusable-repo teardown", `1d9d9bf` "disarm every armed dialog
  on the repo-went-unusable teardown", `734b310` "drive the session watchdog from an injectable clock,
  not wall time".
- **The two 2026-09-01 velocity follow-ups — CLOSED as superseded.** `737cc4b` is literally "band the
  two slowest proptests -- 3.25x and 3.2x"; `5731d37` cut the workspace test wall 14%.
- **`P91-raw-args-privacy.md` — CLOSED, already done.** The file is absent from `docs/contracts/`;
  `INDEX.md` records it folded into `P91-observability.md` §7.4 by `12b0ab6`. The board's
  "consolidation recommended" item and its stale "Contracts:" line were both describing finished work.
- **`P91-observability-ui.md:496`/`:951` vs `INDEX.md` — RESOLVED: `INDEX.md` is right, the BOARD was
  stale.** Verified directly: `:496` correctly states `log_export_session()` takes **no** destination,
  and `:514-516` explicitly refute the `dest` form as webview-supplied. `fc9c36e` did fix it. The
  architect's own `P91-observability.md` §6/§10 are the copies still stale — routed to `architect`.
- **P87b `FU-1..4` — the four-way-stale line is now resolved, three of four were NOT open.**
  FU-1 shipped `1d8c6f9`. **FU-2's premise is false** (`commitAmend` *is* activity-wrapped at
  `src-tauri/src/commands/staging.rs:179`). FU-3 closed by `833f2f9` "dock row gets a role".
  FU-4 answered by *rejecting* the change, `763866a`. Only the `AiActivityPanel` aria-label NIT is
  arguably live — and `src/components/AiActivityPanel.tsx:192-193` **does** carry
  `role="region"` + `aria-label="AI activity"`, so the NIT needs restating against the current file
  or closing. Do not re-open the other three; the board has been wrong about this entry three times.
- **P94 has no contract file — CONFIRMED, accepted as debt.** P94 is shipped; a retroactive contract
  buys nothing. Recorded here so the gap stops being rediscovered as a live defect.

### P77 tag-sync — the ruling's design is settled by measurement, not guesswork

The user chose "fold into the auto-fetch cycle". Verified what that can mean:

- Auto-fetch **already downloads tags**: `src-tauri/src/scheduler/exec.rs:151` → `fetch_all()` →
  `crates/bonsai-core/src/git/remote_activity.rs:52`, `opts.download_tags(AutotagOption::Auto)`.
- **But a purely local compare is NOT sufficient.** Fetched tags land in `refs/tags/*` alongside local
  ones, so after a fetch you cannot distinguish a local-only tag from a fetched one. Classification
  (local-only / remote-only / diverged) genuinely needs the `ls-remote`:
  `crates/bonsai-core/src/git/tag_sync.rs:275-288` → `ls_remote_tags()` → `remote.list()` at `:132-173`.
- **Therefore the increment is: trigger the existing `list_tag_sync` on auto-fetch completion**, not
  a new local-only comparison. That still honours the ruling — it rides a network cycle the user
  already opted into (5-min interval, enabled) and adds no repo-open call and no new network policy.
- Current trigger to replace/augment: `src/components/sidebar/TagsSection.tsx:165-172` → `onExpand` →
  `src/components/RepoWorkspace.tsx:1878` `onTagsExpand={() => void refetchTagSync()}`, with a 10 s
  cache guard at `src/components/repoWorkspace/useTagSync.ts:57-60`.

### ⚠️ NEW — unreviewed MCP write-tool code rode the merge onto `dev`

`2a0b8f1` was described on the board as "a PEER session's MCP work". It is **not docs-only**:

```
 .claude/agents/context-explorer.md          |   2 +-
 crates/bonsai-mcp/src/server/tools_read.rs  |  85 +++++++++++++-
 crates/bonsai-mcp/src/server/tools_write.rs | 147 ++++++++++++++++++++--
```

**222 insertions into the MCP server's read AND write tools**, from a session that ended, reviewed by
nobody in this line of work — and it is now on `dev` and pushed. CLAUDE.md names "the MCP server's
write tools" as a `security-auditor` surface. **A `security-auditor` pass on `2a0b8f1` is owed.**

---

## OPEN follow-ups (genuine unresolved items, not checkpoints)

### Roadmap: REMOVE user-supplied `terminalCommand` / `editorCommand` (user ruling 2026-09-11)

The user chose "validate the shape now **and** drop the feature" for security MEDIUM-2. The
validation is the stopgap; **removal is the end state and is NOT yet scheduled.** Filed here because
the ledger records the decision but a decision without a queue entry is how this board loses things.

- The capability exists for convenience, not necessity: both values are **empty strings** in the
  user's real `settings.json`, so nothing in the current install depends on them.
- Removal must also retire the shape-validation code added in the same increment, and the LOW-1 cwd
  hardening, since both exist only to make this surface safe.
- Until then the validation comment in the launch path must keep saying the capability is slated for
  removal, so a later reader does not mistake the stopgap for the design.


Condensed to one line per item on 2026-09-03; the pre-condensation text is archive Part 50.

### P108 — `AC11` — ✅ CLOSED 2026-09-11 by user ruling: source-derived figures ACCEPTED

> The user accepted the two source-derived 3.05 figures for the unreachable states, with the
> limitation recorded. Kept verbatim below because the qualifier is the record of *why* it could not
> be measured — do not re-open it as owed.

P108 shipped in `42206fd` and its AC12/AC13/AC14 native halves were confirmed by the user
2026-09-10, but **`AC11` is an AI-gate contrast measurement and is not closed by that
confirmation** — a person cannot confirm a 3.05 contrast ratio by eye. Milestone detail:
archive Part 54.5. Contract: `docs/contracts/P108-hue-as-text-on-neutral-ui.md`.

Carried **verbatim**; the qualifier is the point:

> Two states could not be reached: `.file-count-del` selected (3.05) and
> `.context-menu-item[data-tone='danger']` hovered (3.05). **The orchestrator also tried and
> failed**, across ~8 harness rounds: keyboard nav focuses `.graph-scroll` but never mounts the diff
> panes; `?forge=auth` does not render `.file-count-*`; and synthetic `contextmenu` events do not
> open the menu because React requires **trusted** input. Both figures are source-derived and
> unverified. Recording it as owed rather than manufacturing a pass — the same call the implementing
> agent made, and the standard this programme applies to its agents applies to the orchestrator too.

### P91 — open items (checkpoint confirmed 2026-09-10; branch NOT merged, one AI-gate item owed)

Milestone detail: archive Part 54.6. Security arc: Part 42. Audit F1–F9: Part 43. Build diary:
Part 44. SHOULD-FIX full text: Part 45. User decisions + architectural rulings: see
`## Accepted decisions that must survive compaction` above.

- **DO NOT MERGE `feat/p91-observability` to `dev` without the user** (user instruction
  2026-08-31). The 2026-09-10 checkpoint confirmation does **not** authorise a merge.
- **OWED AI-GATE ITEM — the real `logs/*.jsonl` parse from a `pnpm tauri dev` boot+idle.**
  **The orchestrator checked the disk on 2026-09-10: `settings.json` was written that day, so the
  app DID run — but there is no `logs/` directory and no Dev-mode key in the persisted settings.** Dev mode has
  therefore **never been enabled in the real app**, so this needs a boot **with Dev mode turned
  on**, not just any boot. Do not assume the confirmation run produced logs; it did not.
- **F6 — `usage.json` disclosure** → FOR USER item 2 above.
- **Home-directory / username masking in raw log paths** → FOR USER item 3 above.
- **F7 — LOW, mostly latent.** `redact_names` misses bare ref/file names and never touches JSON keys;
  a branch like `feature/acme-client-migration` would be written verbatim into a strict file. Not
  reachable today, but `strict::enforce` is the **sole** enforcement point for both Rust and frontend
  records, so a gap there is a single point of failure.
- **F9 — INFO.** The two redactors cannot disagree, because **only one enforces**: `redact.ts` has no
  equivalent of `redact_names`. That is the correct architecture, and it is why F7's gaps matter more
  than their reachability suggests.
- **Verified CLEAN, so a later session does not re-audit** (CSP + capabilities · updater trust chain ·
  argument-vector process launching · keychain forge credentials · rotation/purge traversal · error
  strings never crossing IPC · zero-cost-when-off): full list in archive Part 54.6.
- **SHOULD-FIX: `SAVE_LOCK` orders the rename pair but NOT the snapshot** (`metrics.rs:379-382`,
  `:409-425`). Two savers can snapshot A→B but acquire `SAVE_LOCK` B→A, so older bytes land last —
  and the dangerous instance is exactly the pair the doc cites as its motivation: **a `metrics_reset`
  can be silently undone on disk**. No deadlock risk (verified). Fix, or amend the overstated doc at
  `metrics_file.rs:52-66`.
- **SHOULD-FIX: the `last_fire` prune assumes non-decreasing `ts`** (`window.rs:52-63`) and the
  comment states it unconditionally. `ts` comes from two unsynchronised clocks (`src/obs/log.ts:43`,
  `sink.rs:385`) with no monotonic clamp. Blast radius is a **duplicate** anomaly record, never a
  missed one.
- **NIT:** `dup_ipc_debounce_map_stays_bounded_over_a_long_session` spaces events 100 ms apart against
  a 300 ms window, making `len <= 4` nearly tautological. `last_fire` has **no numeric cap**, unlike
  `open_calls` (FIFO 1024) and `slow` (LRU 200).
- **`P91-observability-ui.md:496` and `:951` are stale** — both still describe
  `log_export_session(dest)` and a native save dialog **that never existed**. → `ui-designer`.
  **CONTRADICTION (2026-09-10):** `docs/contracts/INDEX.md` records this as **fixed in `fc9c36e`**
  and calls the board's note itself stale. Unresolved — verify before acting on either.
- **Contract consolidation (architect recommendation):** fold `P91-raw-args-privacy.md` into
  `P91-observability.md` (≈ −250 active lines); keep `P91-privacy-copy-ui.md` standalone since it is
  `ui-designer`-owned. **CONTRADICTION (2026-09-10): `P91-raw-args-privacy.md` no longer exists** —
  `INDEX.md` records it folded into `P91-observability.md` §7.4 by `12b0ab6`, and the file is absent
  from `docs/contracts/`. Both this item and the board's old "Contracts:" line that listed the file
  look already-done; **not closed by the curator.**
- **Contract follow-ups owed to `architect`:** §6/§10 still specify the removed
  `log_export_session(dest?)`; §8/§8.1 must record that `cmd.*` keys are camelCase `IpcApi` names
  **and that they recorded nothing before the fix**; `MAX_KEYS_PER_MAP = 512` + the `meta.overflow`
  bucket needs ratification; decision 25's counter-key shape now literally requires the dot.
- **`.forge-connect-link:hover` is now a no-op** — the resting-underline MUST-FIX means hover
  declares the same underline, so the link has **no hover feedback at all**. → `ui-designer`.

### SEC-2026-09-11 — MCP tool-contract audit of `2a0b8f1` (full report: `docs/audit-2026-09-11-mcp-tool-contracts.md`)

**The merge itself was safe.** `2a0b8f1` changed **zero non-doc-comment lines** — proven by
`git show 2a0b8f1 --unified=0 -- tools_read.rs tools_write.rs | grep -vE '^[+-]\s*///'` returning
empty. No new tool, no signature change, no router change. The write gate is structural and
untouched (`crates/bonsai-mcp/src/server.rs:156` merges `write_router()` only inside
`if allow_write`, so unauthorised tools are **unregistered**, not merely refused). Capability and
authorisation: **CLEAN.**
But it is **not inert**: `rmcp-macros` concatenates every doc line into the JSON-Schema
`description`, so all 222 lines ship as **model-facing instruction text** on 33 of 34 tools.
Auditing whether those claims are true is what surfaced the finding below.

#### HIGH — `stage_paths` is missing the symlink-escape guard (PRE-EXISTING, not from `2a0b8f1`)

**Independently verified by the orchestrator 2026-09-11, not taken from the report.**
`crates/bonsai-core/src/git/stage.rs:119-143` calls only the **lexical** `validate_rel_path` and
never `ensure_within_workdir` — which is defined **40 lines above it** at `stage.rs:76`.

Every sibling write primitive DOES call it: `conflict.rs:151`, `:271`, `:348`, `discard.rs:109`,
`stage_partial.rs:104`. There is even a dedicated test module for the guard
(`crates/bonsai-core/src/git/path_traversal_tests.rs`). **Partial staging is guarded; full-file
staging is not.** That asymmetry is an oversight, not a decision.

Consequence: a symlinked **ancestor** is followed (`stage.rs:136` uses
`wd.join(rel).symlink_metadata()`), so `index.add_path` reads a real out-of-repo file into the object
database. libgit2 has **no** "beyond a symbolic link" refusal (the string is absent from all of
`libgit2/src`; the git CLI has it). The file/directory collision does not stop it either —
`git_index_add_bypath` passes `replace=1`.

- **Also reachable from the webview, so this is NOT MCP-only:**
  `src-tauri/src/commands/staging.rs` passes frontend-supplied paths straight into `stage_paths`
  with no extra guard — verified. It is a renderer-compromise primitive too.
- **Not exploitable on this host as configured:** Windows defaults `core.symlinks=false`, so the
  hostile symlink materialises as a text file. That is why HIGH, not CRITICAL. Needs one
  macOS/Linux scratch-repo run to demonstrate empirically.
- **Fix:** call `ensure_within_workdir` in `stage_paths`. The guard already rejects this case —
  canonicalising the parent yields the escape target, which fails its `starts_with(base)` test.
  `unstage_paths` needs no fs guard (index-only).

#### MEDIUM — `add_path` has `git add -f` semantics, and MCP breaks its stated precondition

`stage.rs:117` documents it and justifies it: "acceptable — the UI only offers paths already present
in `StatusSnapshot`". **A model is not the UI.** Gitignored files (`.env`, `secrets.json`) are
stageable and committable. `tools_write.rs:22` restates that precondition as *advice to the model*
with nothing enforcing it. Fix: enforce membership in `read_status()` output on the MCP path — which
is what the description already promises.

#### LOW x 4

1. **A false guarantee introduced BY `2a0b8f1`.** `tools_write.rs:22-23` tells the model "a path that
   does not exist fails the batch". It does not — `stage.rs:134-141` routes a missing path to
   `index.remove_path`, which **stages a deletion** for a tracked path, or silently succeeds for an
   untracked one. A model told nonexistent paths are rejected may pass paths liberally and stage
   deletions it never intended. Notable because that commit message asserts "Every claim was checked
   against the actual signature and outcome enum" — false for at least one write tool.
2. **MCP commit tools run repository hooks with the disclosure structurally unreachable.**
   `tools_write.rs:66` passes `skip_hooks=false`; `src-tauri/src/commands/hooks.rs:1-10` states the
   gate "lives in the frontend (`useHookDisclosure`)". Standalone `bonsai-mcp --repo X --allow-write`
   has no frontend, so a repo whose hooks the user was never shown executes code on the agent's
   commit. CLAUDE.md requires hook execution be user-consented and clearly disclosed; this path is
   neither. Narrow — hooks are not transferred by clone.
3. **Read-tool descriptions carry no untrusted-data labelling** — zero hits for
   `untrusted|instruction|do not follow` in `tools_read.rs`, on tools returning attacker-controlled
   text (conflict blobs, all three diff families, branch names, paths). Credit where due: content
   travels as JSON `structured_content` with a payload-free text summary (`helpers.rs:21-33`), so
   there is **no framing escape** — the residual risk is plain instruction-following.
4. **"Trust the caller" now has a model as the caller.** `conflict.rs:309-311` / `:332` rely on the
   frontend Save-button marker gate. Over MCP a model can write and stage a file still containing
   conflict markers. Fix in the MCP tool, not the shared primitive, to preserve UI behaviour.

#### PROCESS — why this escaped review, and the fix at the right layer

`docs(mcp)` is *literally* accurate and *materially* understating: doc comments here ARE the tool
contracts a model reads before invoking worktree-destructive operations. 222 lines of that landed on
subject-line trust. **Fix at the path layer, not by commit-message discipline:** a review trigger
keyed on `crates/bonsai-mcp/src/server/tools_*.rs`, plus a test snapshotting `list_all()`
descriptions so text drift on the write router produces a reviewable diff. The commit cites
"No test asserts on description text" as reassurance; that IS the gap.
Minor: every write description says "Requires `--allow-write`" — the standalone CLI flag — but the
embedded server's gate is the `mcpAllowWrite` **setting**, so the name is wrong for half the
deployments.

#### Verified CLEAN (do not re-audit) + the boundary of that claim

CLEAN: `resolve_conflict*` path handling (three layers, no escape found); destructive-abort claims
(`abort_merge`, `rebase_abort` both refuse when nothing is in flight, with an untracked-collision
guard); `create_branch_here`; `checkout_branch` no-autostash; `delete_branch` (no force parameter);
`resolution` parsing; `bonsai_stage` **atomicity** (validate-all-then-single-`index.write()`); oid
parsing; no prompt-framing break (MCP descriptions are JSON-encoded; the `prompts_are_single_line`
guard covers a different surface); **the write router exposes no push/force/reset/clean/discard tool,
so there is no direct network exfiltration from MCP.** No commit touched these two files between
`2a0b8f1` and HEAD, so every line number matches the current tree.

**NOT checked, so the CLEAN register does not over-claim:** `merge_branch`/`rebase_branch`
`operationInProgress`; merge autostash-and-restore and `stashPopConflicts`; `create_stash` claims;
stash apply/pop outcome tags; `unstage` atomicity; `commit` `hookRejected` vs `configMissing`
mapping; `rebase_skip`; `list_repos`/`select_repo` session semantics. **LOW 1 is the one factual
error found, not necessarily the only one present.**

---

### SEC-2026-09-03 — external-launch residue (remediated `0806596`; three things left)

Full narrative: archive Part 56. Report: `docs/audit-2026-09-03-external-launch.md` (`7e426c3`).

- **MEDIUM-2** (`terminalCommand`/`editorCommand` unvalidated) and **LOW-1** (cwd DLL search order)
  are **FOR USER item 3** above — product calls, not patches.
- **INFO (CSP `form-action` / `base-uri` / `object-src`) — FIXED `8dd5b24`**, native half confirmed
  by the user 2026-09-10.
- **One residual documented, not closed:** a symlink introduced inside an already-checked-out
  superproject at a not-yet-created leaf bypasses the canonicalize recheck (`canonicalize` fails on
  a missing leaf). Primary vectors are closed lexically regardless of filesystem state.
- **Test gap, partially closed — CONTRADICTION flagged 2026-09-10.** The board said
  `external_tests.rs` has **zero** path-hostility cases and `submodule_info`'s `abs_path` has no
  test at all. Since then `c218258` "added the file-target case MEDIUM-1 was actually about, which
  had no test at all" and `151232d` covered the UNC case end to end. Whether the set is now adequate
  is **unverified**; the curator did not close it.

### Residue of the archived `Queued housekeeping` section (archive Part 57)

The `src/styles/forge-pr.css` split is **DONE** (`e149382`, five modules, emitted stylesheet proven
byte-identical) and `.css` is now in `scripts/check-file-size.mjs` `SCAN_TARGETS`. Still open:

- Three items found during that split and deliberately NOT fixed (each would reorder the cascade or
  cross into another file): `context-menu.css:89` now points at a rule that lives in
  `forge-account.css`; a duplicate `.pr-create-actions` rule in `forge-pr-create.css`; and the
  generic `.btn-secondary-danger` sitting in `forge-pr-create.css` where it belongs with
  `controls.css`.
- **`image_diff_cli_2.rs`** numbered split still owed — renaming changes nextest IDs, so it needs its
  own increment where that IS the expected diff. Path re-verified 2026-09-03:
  `crates/bonsai-core/tests/diff/image_diff_cli_2.rs`.
- **The 90-char branch-name chip** becomes a 50 px two-line stadium at `border-radius: 999px` —
  pre-existing, newly visible because P102/P105 added the fixture that reaches it.
- **`ui-reference.md` is growing fast** (§2 now carries a 16-row evidence table) — worth its own
  curation pass.
- **Two velocity items filed and deliberately NOT taken** (2026-09-03): C1 could drop 17s → 11s by
  giving one surface its own test and its own corrupted repo — **not taken**, it changes the shape of
  a crash-safety test for ~6s; and `crates/bonsai-mcp/tests/common/mod.rs:131-133` still spawns 3
  `git config` calls (board said `:127`; re-measured 2026-09-03 — same fix applies verbatim; left
  alone to keep the blast radius in one crate).
- **The gate script emits Node `DEP0190`** — it passed args to a child with `shell: true`, which
  concatenates rather than escapes. **CONTRADICTION flagged 2026-09-10:** `833f2f9`'s commit message
  is "the gate stops concatenating argv", so this looks already closed. Not closed by the curator.

### P110 / P111 residue (milestones done; archive Parts 55 and 59.1)

- **P110 — latent pre-existing gap, NOT introduced by P110 (candidate follow-up):** op-state files
  (`MERGE_HEAD`, `REBASE_HEAD`, `rebase-merge/**`, `CHERRY_PICK_HEAD`) are excluded by the watcher
  filter and never triggered a refresh before or after P110. Op-state freshness during a conflicted
  rebase rides on incidental worktree churn. Deliberately left alone — changing it would widen which
  bursts fire.
- **P110 — deliberately NOT done:** the canvas selected-row highlight is not sticky. The highlight is
  row-index-based and there is no honest row to draw while the row is absent from the partial layout;
  anchoring it to a stale index is exactly the wrong-commit hazard the fix removes.
- **P111 — filed not fixed:** `.asset-chip` got the R1 line guard but no R2 `max-width`, so a model
  id far longer than the fixture would widen the chip rather than ellipsize.

### From the 2026-09-02 file-size refactor pass (archive Part 36)

Ratchet baseline moved **27 offenders / 6241 excess → 20 / 3528**; full gate green 8/8, 603s.

- **Fold-pill cursor is dead** in `GraphCanvas.handleMouseMove` — P92 §1.4's overflow-cursor write
  unconditionally clobbers spec-004 §1/§2's `foldCursorFor`, and `computeHoverTarget` returns null on
  exactly those rows. Real regression; no vitest mounts `GraphCanvas`, so e2e is the only net.
  **STILL OPEN, re-verified 2026-09-03** at `src/graph/GraphCanvas.tsx` (note: `src/graph/`, not
  `src/components/`): `:535` writes `foldCursorFor(...)`, then `:551` unconditionally overwrites it
  with `next?.kind === 'overflow' ? 'pointer' : ''`. No guard between them.
- **Reflog overlay not torn down** when a repo goes unusable — candidate fix `52c815e` /
  `6092eb3` / `338d71f`, not recorded as closed. See FOR USER item 6.
  **Evidence completed 2026-09-03 (closure still belongs to item 6, not to the curator):** all three
  SHAs are ancestors of HEAD, and `src/components/repoWorkspace/unusableRepoTeardown.ts:232-240`
  now tears down "all three read-overlay siblings" including `setReflog` / `reflogReqId`. The
  described symptom is not reproducible from source.
- **Shortcuts stay live during confirm dialogs** (`pendingForcePush`, `pendingCommitPush`,
  `pendingBisectBad` absent from `dialogOpen`) — ~~candidate fix `1d9d9bf`~~. **STILL OPEN, and the
  candidate-fix citation was wrong.** Re-verified 2026-09-03: `1d9d9bf` IS an ancestor of HEAD, but
  it is `fix(P38): disarm every armed dialog on the repo-went-unusable teardown` — a different
  concern, and it did not close this. `dialogOpen` (`src/components/RepoWorkspace.tsx:1624-1629`) is
  `anyDialogArmed || askOpen || pendingProposedOp || hookGate.pendingHook ||
  hookDisclosure.pendingHookDisclosure`, and `anyDialogArmed`
  (`src/components/repoWorkspace/useWorkspaceDialogState.ts:265-300`) enumerates ~35 flags but
  **not** `pendingForcePush` (`:208`), `pendingCommitPush` (`:205`) or `abortConfirmOpen` (`:178`);
  `pendingBisectBad` lives outside the hook entirely at `RepoWorkspace.tsx:299`. Four flags, not
  three.
- ~~**`ai::session*` is load-flaky** — needs a clock seam, not wider sleeps.~~ — **the clock seam
  LANDED in `734b310`**, an ancestor of HEAD; `crates/bonsai-core/src/ai/clock.rs` +
  `session_watchdog_tests.rs:41/67/115` drive it with `TestClock`. Evidence not in doubt, but the
  **formal close belongs to FOR USER item 6**, so it stays open here. Detail: archive Part 52.3.
- **Contract divergences the tests document as bugs-in-the-contract:** rebase §3.1.5/§9.7
  unstaged-changes precondition, and the libgit2-vs-CLI rename/delete conflict index-entry count.
  Plus a near-tautological `expected_presence` oracle.
- ~~**Duplicated external-tool launchers**~~ — **DONE** `9273238`; hoisted to
  `src/hooks/useExternalTools.ts`, `App.tsx` 602 → 590, `sessionSaveTimer` fixed with a proven-red
  test. **Standing warning:** the toast auto-dismiss timer fix is **deliberately REVERTED** —
  `React.StrictMode` makes cancel-on-unmount strand a toast on screen permanently. Do not re-apply;
  `useToastQueue.test.tsx` carries the finding. Full reasoning: archive Part 52.4.
- **Duplicated helpers left visible, not merged** (behavior risk, not a move): atomic-write helpers
  across `assets/bundle/write.rs` + `assets/profiles/store.rs`; test helper families across
  `tests/diff/` and the four `tests/rebase_merge/*_support.rs`.
- **Three files deliberately stopped short of 500** (each further cut would forward 15-100 values to
  exactly one consumer). Line counts re-measured 2026-09-03: `src/components/RepoWorkspace.tsx`
  **2264** (board said 2309), `src/graph/GraphCanvas.tsx` **784** (unchanged; the board omitted the
  path and it is `src/graph/`, not `src/components/`), `src/App.tsx` **590** (board said 602 — the
  `9273238` bullet above already recorded the 602 → 590 drop, so the two bullets disagreed).

### Velocity follow-ups from the 2026-09-01 measurement pass

Baseline numbers: `docs/history/velocity-2026-09-01.md`. Done in that pass: proptest banding
(`d635464`), doc curation (`0174abf`), 78 → 8 test harnesses (`12882f9`).

- `prop_status::status_matches_porcelain` (23.5s) and `prop_stash_roundtrip` (14.3s) — both banded by
  the 2026-09-03 pass (`737cc4b`); see FOR USER item 6 before closing them.
- **`submodule_cli::oracle_add_deinit_remove_roundtrip` 12-14s** — NOT a proptest (a git-CLI oracle
  roundtrip), so banding does not apply; needs its own look if the ~12s floor matters.
- **vitest jsdom construction dominates the frontend leg** — CPU-aggregate `environment` 613s vs
  `tests` 147s. Try `happy-dom`, or `environmentMatchGlobs` so only DOM-touching files pay. Untried.
- **`pnpm gate --quick` is 305s and only drops e2e**, so it is not a fast tier; `cargo nextest
  --workspace` alone is 181s of it. Either add a genuinely narrow tier or lean on `--rust` /
  `--frontend`.
- **Candidate process changes — ✅ ALL THREE ADOPTED 2026-09-11 (user).** Now rules, recorded in
  `## USER DECISION LEDGER` → "The three process rules adopted 2026-09-11": batch small P-tasks
  through one senior-dev spawn; skip the architect contract for single-component fixes; fold the
  board update into the feat commit.

### Hoisted off milestones archived 2026-09-01

- **keyring 3 → 4** needs a dedicated increment: 4.x moves onto `keyring-core`, renames every
  per-backend feature, drops `crypto-rust`, and requires explicit credential-store registration —
  real changes to `crates/bonsai-forge/src/auth.rs`. (DEP REFRESH, archive Part 24.)
- ~~**`no_proxy_client()` `.expect("build reqwest client")`**~~ — **CLOSED 2026-09-03.** The
  `.expect` survives at `src-tauri/src/mcp/http_support.rs:221`, but the module is `#[cfg(test)]`
  (`src-tauri/src/mcp.rs:410-411`), so it never reaches a shipped binary. **FOR USER item 6 is the
  canonical record**; this line is a pointer, not a second opinion. Detail: archive Part 52.5.
- **P87b FU-1..4** — **this line is four-way stale; corrected 2026-09-10, not closed by the
  curator.** As filed: target row label (FU-1), commitAmend row (FU-2), row `role`/`aria-expanded`
  (FU-3), clickable dock bar (FU-4), plus the `AiActivityPanel` aria-label NIT. (archive Part 27.)
  What the tree says: **FU-1 shipped `1d8c6f9`** (archive Part 59.2); **FU-2's premise is false** —
  `commitAmend` *is* activity-wrapped at `src-tauri/src/commands/staging.rs:179`, which is the stale
  `P87b-FU1-FU4-git-dock-ui.md` F-E line that has now misled two agents (`TODO.md` item `1b`);
  **FU-3 looks closed by `833f2f9`** ("dock row gets a role"); **FU-4 was answered by rejecting the
  change** (`763866a`). Only the `AiActivityPanel` aria-label NIT is unambiguously still open.
  Verify each before ticking any of them — the board has been wrong about this entry twice.
- **RepoWorkspace refactor** still stands for maintainability (not perf); P88's audit re-confirmed it.
- **P90.1 deferred:** per-check timing fields; header commit-summary text; command-palette
  `Refresh checks` / `Show checks`; mock fixtures for noForge/error reachable by click.
- **Known flake (pre-existing, untouched):** `watcher::tests::git_internals_filtered` is a timing
  flake; passes on isolated re-run. Located and the shape corrected 2026-09-03: it lives at
  `src-tauri/src/watcher/tests.rs:127`, and the flaky assertion is `rx.recv_timeout(
  Duration::from_millis(1500)).unwrap_err() == RecvTimeoutError::Timeout` (`:144`) — an `unwrap_err`
  on a **channel-recv Result**, not "on an `Instant`" as the board said. Note the test now carries a
  long comment defending that 1.5 s negative window as sound after `watch_into_channel`'s sentinel
  sync (`29e72a7`), so whether it still flakes is **undetermined** — not re-run in this sweep.
- **FLAG FOR USER (peer session, now ended):**
  `src/components/repoWorkspace/useWorkspaceKeyboard.test.tsx` failed in ISOLATION on the committed
  baseline (1 graph-nav `defaultPrevented` case), introduced by the peer's graph-a11y commit
  `590f2ef`. Likely test-isolation flakiness. **Status unverified since 2026-08-23.**

### Known load-flakes (timing-sensitive, not correctness bugs)

- `ai::session_tests::watchdog_tests::watchdog_does_not_fire_while_awaiting_input` (path updated
  2026-09-02 by the size-ratchet split) — failed once under load, passed on immediate re-run. See the
  clock-seam candidate fix `734b310` under FOR USER item 6.
- `src/App.test.tsx > App shell > an Arrow-key pane nudge persists the POST-nudge width` (added
  2026-09-02) — failed once at 2662ms (`setUiSettings` never called, i.e. the debounced persist had
  not fired), then 4/4 isolated and 2644/2644 on a full re-run.

### Residue of the two dated 2026-08-22 design reviews (archive Part 35)

- **`graph-design-review-2026-08-22.md` M1 is SUPERSEDED — do not implement.** Facts corrected
  2026-09-03; the verdict is unchanged but the citation was wrong twice over. `role="grid"`,
  `aria-rowcount`, `role="row"` and `aria-rowindex` are forbidden by `ui-reference.md` §4.1 — now at
  `:860-864`, **not** `:250-252` (§4.1's header is `:848`; `:250-252` is unrelated text today).
  **`aria-activedescendant` is NOT forbidden** — `ui-reference.md:865` says it "is kept and is
  valid" (amended 2026-09-02, spec-004 merge), so the board was directing sessions to remove a
  shipped, sanctioned attribute.
- **M2/M3/M4/S2/S3/N1/N2 — resolution unverified.** Not checked by the 2026-09-01 sweep; do not
  assume they landed.
- **`review-2026-08-22-ui.md` NIT-1 — Sidebar ignores `panelDensity`** (re-confirmed still open
  2026-09-03: `src/styles/sidebar.css:92` is `.branch-row { height: 24px; }`, a hard literal, and no
  file under `src/components/` that reads `panelDensity` is a sidebar file).
- **NIT-2 —** `src/components/OnboardingOverlay.tsx:229` is still `aria-label="Close"` (re-verified
  2026-09-03, line number still exact); the review preferred "Close the tour".
- SHOULD-3 (`--accent` text over `--selection` fails AA) is the **same item** as P69 **A9** below —
  A9 is the canonical entry.

### P80 forge follow-ups (SHOULD-FIX/NIT, non-blocking)

- (a) `forge_set_token_inner` validates before the `host.is_empty()` guard — guard host first.
  Still open, re-verified 2026-09-03: `src-tauri/src/commands/forge.rs:290` calls
  `validate_repo_token`, `:291` is the `host.is_empty()` guard.
- (b) keychain-write-then-settings ordering: a failed `settings::update` leaves an orphaned keychain
  token (currently `let _ =`) — surface the error.
- (c) re-connecting a migrated legacy `login:None` host creates a 2nd three-part account + orphans the
  bare-host keychain entry (contract §1.2 rekey, optional).
- (e) `ContextMenu` has no separator concept, so the switcher's account/command rows run contiguous.
- (f) Settings Accounts group ordering is alphabetical only (no repoId in scope for "current host
  first").
- (g) disabled Default radio's `aria-describedby` points at a `hidden` span — use a visually-hidden
  class.
- (h) switcher trigger has no busy affordance during a pin/reset write. (i) §1.1 wireframe middot
  between host and caption omitted (cosmetic).

### `cargo fmt` has never been run on this repo

- No `rustfmt.toml` anywhere, no fmt check in any hook or CI (re-verified 2026-09-03: zero
  `rustfmt.toml` in the tree).
- `cargo fmt --all --check` reports **1773 hunks across 221 files**; `--config
  use_small_heuristics=Max` is *worse* (2065). **These two numbers were NOT re-measured in the
  2026-09-03 staleness sweep** (cargo is not on the default PATH and running it is out of that
  sweep's scope) — treat them as of their original measurement date, not as current.
- Right shape: its own commit — pick a config, add `rustfmt.toml`, one-shot reformat, then add
  `cargo fmt --check` to the gate. **Do it between milestones, never inside one.**

### Audit #2 remainder (full audit `docs/audit-2026-08-18.md`; fix-batch mapping archive Part 16)

- **Sections 4.3-4.8 test gaps** — CommandPalette/NumberSlider pins, streaming-graph e2e, 08-stash
  conflicted-apply fixture, Linux case-sensitivity assertions, low-value untested units, and the
  missing journeys: updater / AI-PR-description / clone-init / worktrees.
- **Section 7's 13 NITs** — recorded in the audit, no action required.
- **Section 5.6** perf/visual ACs stay USER CHECKPOINT (the headless harness cannot observe
  rAF/compositing).

### P68 contract debt (P68 is done; its contracts are stale/oversized)

- `docs/contracts/P68e-ai-activity-dock.md` is **1123 lines** (re-measured 2026-09-03; the board and
  `docs/contracts/INDEX.md` both said 1064, 59 lines stale) and under-describes shipped code
  (P68g-1 added an untrusted-model-output attribution line, a fixed "Bonsai never asks for passwords
  or tokens" guard, and a two-id `aria-describedby`). Splice-ready replacements are in
  `docs/contracts/P68g-ui.md` §3.1-§3.5. **Needs: apply the splice, then split the file.**
- `docs/contracts/P68-ai-conflict-streaming.md:304` is one module level stale (`session_drain_tests.rs`
  is now a child of `session::session_drain`). **Invariants D1-D16 remain canonical — do NOT "fix"
  them back.**
- **P68 security follow-ups 7-11 still OPEN**; rationale in `docs/contracts/P68-security-audit.md`
  (canonical): the novel-content gate (structural defeat for H1), proposals shown as a diff, bulk
  path-count cap + per-batch reads + batch count in the dialog, process-group kill off Windows (the
  pid-zeroing half landed in `67539fd`), and a symlink-safe `resolve_conflict_text` write.

### P69 Settings follow-ups — A3 ✅ RULED 2026-09-11 (ui-designer finalises); A8/A9 still backlog

> **A3:** the user handed the gate-note copy to `ui-designer` to finalise with the surrounding copy
> in view; whatever it signs ships. **A8/A9 were deliberately NOT put to the user** — they are
> backlog, not blocked on a decision.

- **A8 — bundle the two specced-but-unimplemented items into one increment** (both `ui-designer` and
  the orchestrator recommend bundling): (a) the help-text highlight fallback
  (`docs/contracts/archive/P69-settings-ui.md` §3.2.1) — the flagship query `graph` returns 5 hits and
  highlights **nothing**; and (b) the half-landed draft-hint feature (§13). The draft-hint CSS is
  genuinely dead but costs no visible layout today.
- **A9 — a scoped a11y sweep of `color: var(--accent)` on text over `--selection`** (measured
  3.51-3.74:1). Now **prohibited** in `ui-reference.md` §2 so new code cannot add to the backlog. The
  one deviation P69k shipped: the rail hit-count is `--text-1`; the exact declaration to flip is
  marked in `settings-shell.css`.
- **A3 — the frozen AI gate-note copy is still unsigned.** §5.4's replacement for `Turn on "Enable AI
  features" above to change these.`; `ui-designer` prefers `These take effect once AI features are
  on.` The current string ships until the user rules.

### P77 tag-sync deferred follow-ups (full detail: `docs/history/todo-archive-2026-08.md` Part 18)

- **Collapsed-rollup needs first expand — ✅ RULED 2026-09-11: fold the check into the existing
  auto-fetch cycle** (enabled, 5-min interval). No repo-open network call and no new trigger; the
  rollup warning appears within one cycle. The old text: the ls-remote check only fires on the first
  Tags expand per session, so the rollup warning cannot appear until the user expands Tags once.
- NITs: rollup aria-label lacks singular/plural ("1 tags") · `useTagSync` re-hits network on rapid
  collapse-then-expand while `unavailable` · confirm dialogs close optimistically so `busy` never
  paints · the tag-filter box gate counts local tags only · item-7 "Delete tag on origin" also shows
  on remote-only ghost rows (coherent).
- Backend NITs: `delete_remote_tag` doesn't `evict_fresh_on_auth_fail` · `validate_tag_name` is
  duplicated from `tags.rs` — promote to shared if a 3rd caller appears.

### macOS ad-hoc code signing — ⏸ PARKED 2026-09-11 (user): blocked-on-release, NOT open work

- `bundle.macOS.signingIdentity: "-"` is in `src-tauri/tauri.conf.json` but **has not shipped**: the
  last tag is `v1.5.0` (2026-08-26), which predates the fix. Verify the sealed ad-hoc signature on
  the next tagged release.
- Not fixed by ad-hoc at all: Gatekeeper "unidentified developer"; a new version re-prompts once for
  TCC (cdhash changes). Full fix = Developer ID + notarization; the `APPLE_*` env block in
  `.github/workflows/release.yml` is already scaffolded. Full detail: archive Part 34.


---

## Archive

**Start at `docs/history/README.md`** — it is the navigable index of every archived milestone and
part number. The table below is the short form.

| File | Covers |
|---|---|
| `docs/history/README.md` | **The archive index** — which file/part holds which milestone. |
| `docs/history/todo-archive-2026-09.md` | **Parts 54-61 (moved 2026-09-10, after all eight USER CHECKPOINTs were confirmed):** the whole checkpoint block — P102+P105, P106, P107, P108, P91 (Part 54) · P110 + P109 (Part 55) · the 2026-09-03 closures + the SEC-2026-09-03 external-launch remediation (Part 56) · the `Queued housekeeping` section incl. the `e149382` CSS-split proof (Part 57) · the superseded `c218258` and earlier gate states (Part 58) · the 2026-09-10 session: P111, FU-1, the six reviewer-follow-up closures (Part 59) · the board's own record of the confirmation (Part 60) · superseded curator bookkeeping — the pre-pass navigation text, the old `Verification state`, and the 2026-09-03 curator note whose “~470 lines” prediction this pass tested (Part 61). **Parts 51-53 (moved 2026-09-03, compaction pass):** the `5c2dcd2` + `c6cd7dd` gate states and the e2e-contention mis-diagnosis story · the full narratives of everything closed 2026-09-03 (P107 F2, the four `112800c` ticks, the `4002ad2` struck entries) · the durable-lessons stories and worked numbers. **Parts 36-50 (moved 2026-09-03):** the file-size refactor pass · the full narratives of P102+P105, P106, P107, P108 and the P91 security arc + audit + build diary · superseded pre-ship filings for P102/P105/P106/P108, the dead-CSS decision block and the resolved `lint:size` blocker · the 2026-09-03 velocity pass · P99, P100, P101, P98, P95, P96, P97 and the P100+P101+DX-e2e banner · built-bundle e2e + P103 + P104 · the DX/velocity stubs · the pre-condensation open-follow-up text. **Parts 33-35 (2026-09-01):** the P84 record gap · macOS ad-hoc signing · the two 2026-08-22 design reviews. **Parts 22-32 (2026-09-01, verbatim):** P94 · P93+P92 · DEP REFRESH · P90+P89 · P88 · the P85-P87 batch · P82+P83 · divergence reconcile + Release 1.1.0 · the DX dev-loop text · the confirmed-checkpoints block · the 2026-08-21 resolved follow-ups. |
| `docs/history/todo-archive-2026-08.md` | Parts 1-9: P65 to P28 build detail, the Phase 1-4 banners, resolved FOR-USER decisions, P69(1.0.0)/P67/P68 detail. Parts 10-16: the P62-P74 checkpoint waiver + P71-P74, the P69 Settings redesign, the Audit #2 fix batch. Parts 17-18: P70 and P77. Part 19: the follow-ups resolved 2026-08-21, verbatim. Part 20: P78/P79/P80. Part 21: P80b/P81/P82. |
| `docs/history/todo-archive.md` | P27 to P2, M0-M6 |
| `docs/history/milestones-mvp.md` | the M0-M6 AI-gate vs USER CHECKPOINT split |
| `docs/history/context-pollution-audit.md` | the context/token-cost audit |
| `docs/history/velocity-2026-09-01.md` | gate wall-clock, test-suite hotspots, inner-loop rebuild cost, ceremony-vs-machine-time split (2026-09-01) |
| `docs/contracts/INDEX.md` | one line per contract file — milestone, scope, status |

Move a milestone's section into the current dated archive file only once **both** halves of its gate
have passed (or the native half is explicitly waived). A milestone with a pending USER CHECKPOINT
stays on this board. **An owed AI-gate item also keeps its entry here** — P108 `AC11` and P91's
`logs/*.jsonl` parse are the live examples.

### Why this board is ~915 lines, not ~300 (curator note, 2026-09-10)

The 2026-09-03 note said "~470 lines become archivable the instant the seven USER CHECKPOINTs and
seven FOR USER decisions clear — and not before". The checkpoints cleared on 2026-09-10 and **557
lines were archived** (Parts 54-60, every range verified byte-identical against the pre-pass copy).
The board did not fall by 557, because ~193 lines of that content were **live remnant embedded
inside the archived sections** — P108's `AC11`, P91's user decisions / architectural rulings /
security findings / SHOULD-FIX list, the SEC residue, the housekeeping items deliberately not
fixed, and the gate-running rules. Those were relocated, not removed. **1244 → 915.**

Residual composition, measured after this pass: header + conventions + pointers **64** · RESUME
HERE (incl. the `1b`/`1c`/`1d` contract-hygiene residue owed to `architect`/`ui-designer`) **123** ·
FOR USER decisions **135** · durable-lesson rules **115** · accepted decisions **78** · open
follow-ups **347** · archive + this note **43**.

**~300 is still not the honest floor, and the reason has changed.** It is no longer pending
checkpoints — it is that **the seven FOR USER decisions and the open-follow-up backlog are 482 of
the 915 lines**, and both are load-bearing: the decisions are briefs only the user can rule on, and
the follow-ups carry the freshly-verified `file:line` citations a cold resume needs most.
**The honest floor is ~900 today**; it drops to roughly **~650** the moment the user rules on the
seven FOR USER decisions, and only reaches ~300 once the follow-up backlog is actually worked down.
Neither is a curation job — going further today would mean deleting open work.
