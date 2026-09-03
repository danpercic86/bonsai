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
`docs/history/README.md`. The 2026-09-03 sweep is `docs/history/todo-archive-2026-09.md`
**Parts 36-50**; the archive table at the bottom is the short form. Nothing below was closed by the
curator: a milestone with a pending USER CHECKPOINT stays here, and open follow-ups stay here
however old they are.

---

## FOR USER — open decisions (nobody else may resolve these)

### 0. Adopt happy-dom for the vitest DOM project? — MEASURED, HELD FOR THE USER (2026-09-03)

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

### 1. Flip the e2e bundle default? — READY, HELD FOR THE USER (2026-09-02)

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

### 2. Security F6 — `usage.json` disclosure — HELD FOR USER, with a recommendation

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

### 3. Mask home directories / usernames in raw log paths? — OPEN QUESTION

- `scrub.rs` has **no username rule** (verified independently by grep), so a **raw absolute repo path
  carries the OS account name** into a mailed export zip.
- Not a leak of repo content, but it is identifying, and the export workflow mails it to a third party.
- Now **disclosed** in the consent copy (`0a785b3`); **whether to also mask it is unresolved** —
  masking would undercut raw mode's stated purpose of showing real paths.

### 4. D3 — should `.op-worktree-warning` be painted danger at all? — DELIBERATELY UNRESOLVED

- It is a **warning painted danger**: a *tone/semantics* question, not a contrast one.
- Deliberately **not** folded into P108 — changing it alters what the UI means, not whether it can be
  read. P108's fix kept the danger hue and changed only legibility, so the tone question is untouched.

### 5. Two open items from the 1.0.0 release (carried forward)

1. **Back up `.tauri/updater-prod.key`.** Correctly gitignored and untracked, so it exists in exactly
   ONE place: this working copy. Losing it permanently breaks auto-update for every installed client.
   (The committed `tauri.conf.json` pubkey was verified to match it.) **Also: P71 must not touch it.**
2. **GitHub reported 2 Dependabot alerts (1 high, 1 moderate)** on push. The high is the known
   `nanoid` GHSA-2v37-7h3g-55p8 — build/test tooling only, deliberately ignored in
   `pnpm-workspace.yaml`. **The moderate is unidentified** — `gh` is not installed here; both project
   gates are green. Check the Dependabot page.

### 6. Record contradictions surfaced by the 2026-09-03 curation sweep

The curator refuses to resolve these; resolving any would upgrade a status.

- ~~**P107's board heading says "CONTRACT DONE, IMPL PENDING"**~~ — **RESOLVED by the orchestrator
  2026-09-03.** The heading predated `2168057` and was stale; it is restated in P107's own section.
  This records a shipped fact and **does not** touch AC11/AC12/AC13, which remain pending USER
  CHECKPOINTs.
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

## AWAITING USER CHECKPOINT — five live milestones

None may be archived. The orchestrator never self-declares the native half. Each entry below is the
resume summary; the full review/implementation narrative is in the archive part named.

### P102 + P105 — hue audit — AI GATE GREEN, awaiting USER CHECKPOINT (AC18 / AC19 / AC20)

**Current step:** AI gate green; milestone complete bar the checkpoint.
Contract `7c623d8` (`docs/contracts/P102-P105-hue-audit-ui.md`) → impl `0e5dcab` → both reviews
APPROVE after fixes `185c352`. Full narrative: archive Part 37.

- **Gate:** `--quick` all 7 steps green (255.9s) **and** e2e **181 passed / 1 skipped** (2.7m).
- **AC18** — the updater panel is Tauri-only, but its `.btn-danger` shares the exact rule measured at
  4.80/4.93 rest and 5.18/5.49 hover, so the question is "does it look right", not "is it legible".
- **AC19** — `--accent-strong` holds hue within **0.7°** of `--accent` in both themes (219.0° vs
  219.7°); the open question is purely whether the dark `#7fabff` reads washed-out.
- **AC20** — no `filter` remains on any of the four hover targets, so nothing is left to flicker and
  layout cannot move (background-only swap); fill-change feel alone is judged.
- The user's 2026-09-02 checkpoint authority was scoped to P100 + P101 and explicitly does **not**
  reach work created that session.
- **The predicted grep residue matched reality on every line** — the first milestone in the
  programme to do so, and the standard the later ones inherited.
- **Orchestrator decision (§5.4, Option A):** added `--merged` / `--merged-text` rather than keep the
  merged-PR purple `#8957e5` literal — a token-consistency call inside the design system's own
  logic, **not** a product-identity call, therefore not a checkpoint.
- **AC16 is partial:** its stylelint clause is **unsatisfiable — no stylelint exists in this repo**.
- **No architect pass**, consistent with P100/P101: a CSS contrast audit has no module boundary, IPC
  surface or algorithm. Recorded so the skipped step is not read as an oversight.

### P106 — status-badge ink — SHIPPED `10ce967`, awaiting USER CHECKPOINT (AC14 / AC15 + the real-repo half of AC9)

Contract `docs/contracts/P106-status-badge-ink-ui.md`. Full narrative: archive Part 38.

- 8 render sites in 7 files; buckets **5 FIX · 1 KEEP · 2 no-hue**.
- **The letter is the SOLE status carrier at all 8 sites**, so nothing is exempt; the badge does not
  scale with `--rp-row-font`, so this holds in both densities.
- Three new **ink-only** tokens (`--danger-strong`, `--success-strong`, `--warning-strong`) mirroring
  `--accent-strong`. **`--warning-text` was explicitly NOT the answer** — the `--*-text` family is
  **fill-ink** and measures 1.19/1.16 as ink on the staged tint.
- **Shipped result:** worst live badge anywhere is **5.18** (the `C` on a selected row in light) —
  exactly the predicted floor, verified live in both themes with the full ancestor stack composited.
- A **fourth** search pass was added over P107's three — imperative canvas rendering
  (`rg "FileStatus" src/graph` → 0) — because "the search couldn't see a whole class" is how both
  prior counts in this programme went wrong.
- Orchestrator decisions: D1 accepted (the two colourless badges get hue, isolated as the
  deliberately droppable AC12); `--warning-strong` dark = `#e3b341`; AC13's `ui-reference.md` update
  lands **after** implementation, from the shipped commit.

### ✅ P107 — hue-over-own-tint — SHIPPED `2168057`, ⏳ AWAITING USER CHECKPOINT (AC11/AC12/AC13)

Contract `docs/contracts/P107-hue-over-own-tint-ui.md` (`59061b2`). Impl shipped **`2168057`**
(7/7 predicted residue metrics matched); errata **`ef06e6b`**. Full narrative: archive Part 40.

**Status restated by the orchestrator, 2026-09-03.** The old heading "CONTRACT DONE, IMPL PENDING"
was written before `2168057` and was simply stale. The curator correctly refused to change it —
restating a status is not a curator's call — so it is corrected here instead. **This records what
shipped; it does not upgrade a checkpoint:** AC11 / AC12 / AC13 remain **pending USER CHECKPOINTs**
and only the user can close them.

- Population is **38 call sites**, not the 16 found first and not the **6** `ui-reference.md` §2
  claimed. Buckets: **17 failing text · 19 compliant glyph KEEPS · 1 failing glyph state · 1 owned
  by P106**. All seven predicted residue metrics matched exactly.
- The 19 keeps matter as much as the fixes — a blanket sweep would have wrecked the toast,
  submodule, ai-dock and git-dock pill recipes.
- The three-pass search is the durable deliverable: (i) same-rule-block; (ii) a multiline
  descendant-combinator grep; (iii) two classes even (ii) cannot see — **unqualified child classes
  whose only parent is tinted** (found by reading the 25 tinted containers' components) and
  **custom-property indirection via `--h`** (11 instances across 4 families no hue-name grep reaches).
- **Zero new tokens.** Recorded trap: `--danger-text` on a 14% danger tint is **1.27:1**.
- A `ui-reference.md` §2 recipe was **retracted, not patched**: a 35% hue edge measures 1.58/1.69 —
  decoration, never an identity carrier. Solid-hue bars pass (4.41-7.92) and stay.
- **Errata reported rather than papered over** (`ef06e6b`): AC2's thresholds were default-state
  minima, restated per state; §5's "2.76 / 3.07" does not reproduce — the real value is 3.37 / 3.30.
  **P107 must not be re-opened against the original AC2 wording.**

### P108 — hue-as-text on neutral surfaces — SHIPPED `42206fd`, awaiting USER CHECKPOINT (AC12 / AC13 / AC14), and **AC11 is OWED**

Contract `8027cef` (`docs/contracts/P108-hue-as-text-on-neutral-ui.md`) → impl `42206fd`.
17 CSS files, no TS/TSX, no DOM change, no new tokens. Full narrative: archive Part 39.

- All five residue rows matched, and all five pre-fix baselines were **re-measured in this tree**
  rather than inherited. Post-fix minimum anywhere is **5.10** (`--danger-strong` on `--bg-3`,
  light), matching the predicted family minimum exactly.
- **The count was 62, not the 48 handed over — the FIFTH wrong count in this programme.**
- Two defects a grep structurally could not find: `settings-legacy-sections.css:134` was
  `color: var(--warn, #b8860b)` and **`--warn` is defined NOWHERE**, so it had always painted a
  theme-invariant literal at 3.25:1 in light; and `commit-panel.css:98-100` served **one declaration
  to two selectors**, icon half a compliant glyph, text half failing at 4.41 dark.
- The canvas pass found **zero** — 7 draw sites in `src/graph/**`, all glyph, all ≥3:1. 0 fixes,
  **but only because it was measured**.
- `--warning-text` is **undefined**, so a naive probe returns the *inherited* colour (13.54/15.42),
  not the 1.09/1.07 the contract cites — same undefined-property trap class as `--warn`.
- §9's four mock fixture states were **not** added: three are already reachable; only the long-error
  and `dev-status-write-failed` states would need new mock code.

**AC11 is OWED — recorded, not quietly dropped** (carried verbatim; the qualifier is the point):

> Two states could not be reached: `.file-count-del` selected (3.05) and
> `.context-menu-item[data-tone='danger']` hovered (3.05). **The orchestrator also tried and
> failed**, across ~8 harness rounds: keyboard nav focuses `.graph-scroll` but never mounts the diff
> panes; `?forge=auth` does not render `.file-count-*`; and synthetic `contextmenu` events do not
> open the menu because React requires **trusted** input. Both figures are source-derived and
> unverified. Recording it as owed rather than manufacturing a pass — the same call the implementing
> agent made, and the standard this programme applies to its agents applies to the orchestrator too.

### P91 — Observability: Dev mode, structured logs, local telemetry & metrics — code + contract complete, USER CHECKPOINT never presented

Branch `feat/p91-observability` (this branch). **Not merged to `dev`; do not merge without the user**
— the user confirmed 2026-08-31 that the branch is WIP, and its own commit messages must not be read
as an authoritative status. Build diary: archive Part 44. Security arc: Part 42. Audit: Part 43.

- **Contracts:** `docs/contracts/P91-observability.md` · `P91-observability-ui.md` ·
  `P91-raw-args-privacy.md` · `P91-privacy-copy-ui.md` · `ui-reference.md` §12.11.
- **Goal:** make unintended app behaviour mechanically visible — double triggers, redundant IPC,
  effects on unchanged deps, echo-induced refreshes, superseded results — without reading 50k lines.
- All 7 increments implemented, reviewed and committed; the security arc is complete
  (`a287a53` → `834f2d1` → `120cadd` → `c0abbe1` → `2523426` → `2fb647e` → `0a785b3` → `b26833f`).
- **Full gate green at `b26833f`** (7 non-e2e steps); **e2e 181 passed / 1 skipped / 0 failed** after
  `23ee2b0`. `cargo obs::` went **135 → 146 → 158 → 162** across the arc.
- **Still owed:** the real `logs/*.jsonl` parse from a `pnpm tauri dev` boot+idle, and USER
  CHECKPOINT (a)-(f) plus the 7b/7c native items — **asked, not declared**.

**User decisions on P91 that must survive compaction (all 2026-08-27 unless noted):**

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

**Three architectural rulings not to re-open:**

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

**Security findings still open** (F1-F5 fixed; full text archive Part 43):

- **F6** — `usage.json` disclosure → FOR USER item 2 above.
- **Home-directory / username masking** → FOR USER item 3 above.
- **F7 — LOW, mostly latent.** `redact_names` misses bare ref/file names and never touches JSON keys;
  a branch like `feature/acme-client-migration` would be written verbatim into a strict file. Not
  reachable today, but `strict::enforce` is the **sole** enforcement point for both Rust and frontend
  records, so a gap there is a single point of failure.
- **F9 — INFO.** The two redactors cannot disagree, because **only one enforces**: `redact.ts` has no
  equivalent of `redact_names`. That is the correct architecture, and it is why F7's gaps matter more
  than their reachability suggests.
- **Verified CLEAN, so a later session does not re-audit:** CSP + capabilities (script-src self-only,
  no unsafe-inline/eval, no shell or fs plugin) · updater trust chain, no key material in the repo ·
  external-process launching uses argument vectors, never a shell string · forge credentials via the
  OS keychain · log rotation/purge have no traversal · error strings never cross IPC ·
  zero-cost-when-off is structural on both sides.

**P91 SHOULD-FIX follow-ups, still open** (non-blocking; full text archive Part 45):

- **`SAVE_LOCK` orders the rename pair but NOT the snapshot** (`metrics.rs:379-382`, `:409-425`).
  Two savers can snapshot A→B but acquire `SAVE_LOCK` B→A, so older bytes land last — and the
  dangerous instance is exactly the pair the doc cites as its motivation: **a `metrics_reset` can be
  silently undone on disk**. No deadlock risk (verified). Fix, or amend the overstated doc at
  `metrics_file.rs:52-66`.
- **The `last_fire` prune assumes non-decreasing `ts`** (`window.rs:52-63`) and the comment states it
  unconditionally. `ts` comes from two unsynchronised clocks (`src/obs/log.ts:43`, `sink.rs:385`)
  with no monotonic clamp. Blast radius is a **duplicate** anomaly record, never a missed one.
- **NIT:** `dup_ipc_debounce_map_stays_bounded_over_a_long_session` spaces events 100 ms apart against
  a 300 ms window, making `len <= 4` nearly tautological. `last_fire` has **no numeric cap**, unlike
  `open_calls` (FIFO 1024) and `slow` (LRU 200).
- **`P91-observability-ui.md:496` and `:951` are stale** — both still describe
  `log_export_session(dest)` and a native save dialog **that never existed**. → `ui-designer`.
- **Contract consolidation (architect recommendation, not done):** fold `P91-raw-args-privacy.md`
  into `P91-observability.md` (≈ −250 active lines) when §13 rows 1-15 are archived; keep
  `P91-privacy-copy-ui.md` standalone since it is `ui-designer`-owned.
- **Contract follow-ups owed to `architect`:** §6/§10 still specify the removed
  `log_export_session(dest?)`; §8/§8.1 must record that `cmd.*` keys are camelCase `IpcApi` names
  **and that they recorded nothing before the fix**; `MAX_KEYS_PER_MAP = 512` + the `meta.overflow`
  bucket needs ratification; decision 25's counter-key shape now literally requires the dot.
- **`.forge-connect-link:hover` is now a no-op** — the resting-underline MUST-FIX means hover
  declares the same underline, so the link has **no hover feedback at all**. → `ui-designer`.

---

## PENDING / queued

### P109 — the status badge has no accessible name, and `added`/`untracked` both render `A` — pending (filed 2026-09-03 from P106)

- Two distinct statuses render the **same character** (`StatusFileRow.tsx:15`), and the badge carries
  **no accessible name** — the distinction is unavailable to a screen reader *and* ambiguous visually.
- Deliberately **not** rolled into P106: one defect class per milestone is what kept
  P95/P98/P100/P102/P105/P107 reviewable, and this is a11y/semantics, not contrast.
- P106 makes the letter *legible*; P109 is about the letter being *insufficient*.

### Queued housekeeping (none blocking)

- **`src/styles/forge-pr.css` is ~710 lines**, over the ~500-line soft limit → `refactorer`.
- **`image_diff_cli_2.rs`** numbered split still owed — renaming changes nextest IDs, so it needs its
  own increment where that IS the expected diff.
- **`.settings-toggle-btn.is-active` is dead styling** — closed as (b), NOT a product bug. `cf174ff`
  added the rule for the git-config Local|Global toggle; `7354aca` (P69h) replaced that with
  `SettingsSegmented` (`.settings-segment.is-selected`), orphaning `is-active`. All 29 surviving call
  sites are one-shot **action** buttons. Honest close: delete both rules (optionally rename the
  class). CSS left in place — correct but unreachable.
- **`docs/contracts/pr-badge-placement-ui.md:106,116,156`** still documents the canvas merged pill as
  `#8957e5`, stale since the `--merged` token landed. → contract owner.
- **The 90-char branch-name chip** becomes a 50 px two-line stadium at `border-radius: 999px` —
  pre-existing, newly visible because P102/P105 added the fixture that reaches it.
- **`ui-reference.md` is growing fast** (§2 now carries a 16-row evidence table) — worth its own
  curation pass.
- **Two velocity items filed and deliberately NOT taken** (2026-09-03): C1 could drop 17s → 11s by
  giving one surface its own test and its own corrupted repo — **not taken**, it changes the shape of
  a crash-safety test for ~6s; and `crates/bonsai-mcp/tests/common/mod.rs:127` still spawns 3
  `git config` calls (same fix applies verbatim; left alone to keep the blast radius in one crate).

---

## ✅ GATE STATE at `5c2dcd2` — **ALL 8 STEPS PASSED** (2026-09-03)

Run on a machine **verified idle first** — CPU sampled six times (~1% after the sampler's own
startup spike), **zero** `bonsai-scratch`/`load.mjs` processes, 42 GB free. 362.0 s total:

| step | time |
|---|---|
| `cargo nextest` | 108.0 s |
| doctests | 3.8 s |
| clippy | 8.5 s |
| eslint | 11.1 s |
| **file-size ratchet** | 0.73 s |
| vitest | 56.2 s |
| tsc + vite build | 10.9 s |
| **e2e playwright** | **162.7 s** |

This covers everything shipped in the session, including P109's badge consolidation, the metrics
snapshot-ordering fix, and both contract ratifications.

**Verifying machine state before running is now a standing pre-gate step**, not a nicety — see the
caveat section below for why. The earlier gate state at `c6cd7dd` is kept underneath because the
reasoning is the point.

---

## 📌 The earlier gate state at `c6cd7dd` — kept for the reasoning, not the status

**All 7 non-e2e steps have been green in every run tonight, regardless of machine load** —
`cargo nextest`, doctests, clippy, eslint, the file-size ratchet, vitest, and tsc+build.

**e2e: 181 passed / 1 skipped / 0 failed in 172.2 s**, run **alone on a verified-idle machine**
(CPU sampled 2-19%, 42 GB free, zero scratch processes). That is the only uncontaminated reading of
the night and it is clean.

### ⚠ The e2e leg is contention-sensitive, and I mis-diagnosed that twice before getting it right

Three consecutive full-gate runs showed e2e failures. **None was a code defect.** Each was the
machine being saturated — and twice the saturation was caused by this session's own tooling:

1. **Gate-5** failed at the *newly raised* 15 s `FIRST_PAINT_TIMEOUT`, which looked like proof the
   new number was still too small. It was not. **My own diagnosis agent had left an 18-thread
   synthetic CPU load generator running** (`bonsai-scratch/load.mjs`, 342 CPU-minutes, machine pinned
   at 100%), inflating every timing ~2× — `cargo nextest` 151 s against a 73 s norm. **Had I trusted
   the surface reading I would have bumped the timeout a second time to hide an artifact of my own
   tooling.**
2. **Gate-6** I asserted was run on a clean machine. **It was not** — the diagnosis agent was still
   finishing its own e2e and load runs and overlapped it; its completion notice arrived in the same
   block as the gate start, and I read that as "already done".
3. I also claimed **34 stray Playwright browsers were leaking** and degrading the box. **False** —
   the count had matched `msedgewebview2` as well; a proper `Win32_Process` check found **zero**
   Playwright-owned processes. The 22 `msedge` are the user's own browser.

**RULE, earned the hard way: before trusting any timing-sensitive failure, verify machine state AT
THE TIME IT RAN** — sample CPU repeatedly, look for scratch/load processes, and confirm no agent is
mid-run. A slow timing number is evidence about the machine until proven otherwise. This is the
timing analogue of the grep-counting rule: *measure the baseline, do not infer it.*

### What this means for the gate
The `pnpm gate` e2e leg will fail intermittently on a loaded machine, and that is **expected**
behaviour documented at P104 (Edge misses a hardcoded 30 s CDP close window, then a blocking
`taskkill` runs). `FIRST_PAINT_TIMEOUT = 15 s` (`c6cd7dd`) removes the largest source, measured at
7.6× p99. **Run the gate on an otherwise-idle machine, or run e2e with `--workers=1`.**

---

## Durable lessons — the audit method, and what it cost to learn

These are the reusable findings. They are on the board, not in the archive, because every one of
them was learned by a claim that was green the whole time it was wrong.

### The six failed app-wide claims, and the distinction between them

Every one had the same shape: **a sentence claiming an app-wide property, with a call-site count
that nobody enumerated.** The first five failed for want of an enumeration. The sixth is different
and worse — the enumeration **existed** and was still blind, because it was **scoped by token name**.

1. **P95** — the enabled-control class; 3 escapes found by P101.
2. **P98** — "`--text-3` family closed"; 122 declarations were never classified.
3. **P74** — the hue-as-text sweep; became P105.
4. **`ui-reference.md` §2** — "6 live hue-over-own-tint instances"; the real population is **38**.
5. **P106's hand-over count of 48** for P108; the real inventory was **62**.
6. **P101** — an *exhaustive* `--text-3` audit recorded as CLOSED, which still missed a **2.96
   light** glyph, because **`--badge-unknown` is byte-identical to `--text-3`**.

**The rule:** a bucket + verdict per call site, P101 §3 style, or it is not closed. Enumerate,
bucket, record a verdict per site, predict the post-fix residue, then verify the prediction.
**Do not accept a "~N call sites and it's fine" sentence as evidence.**

### The aliasing rule

- **An audit scoped by token NAME cannot see an alias. Scope by resolved VALUE.**
- Token aliasing has hidden instances three times: `--badge-good`/`--badge-warn` are byte-identical
  to `--success`/`--danger` (the first pair), then `--badge-unknown` to `--text-3`.
- A `var()` **fallback masking a missing token is invisible to any hue-name search**, because the
  hue name appears only in the fallback (`--warn`, which is defined nowhere).
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
  over `--bg-1`** (P107's figure) and **5.16/4.52 over `--bg-2`** (P106's). All three reproduce under
  one method — the two contracts never disagreed, **neither stated its base**, and the base alone
  accounts for **1.26** of dark-theme spread.

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

### The measurement lessons (archive Part 46 for the numbers)

- **Optimising the measured-slowest test did not move workspace wall, because a different test
  became the floor.** Banding `prop_status` (3.25x) and `prop_stash_roundtrip` (3.2x) moved wall
  only 108.8s → 102.9s. The real floor was `corrupt_repo_matrix_never_panics` — 44.0s contended but
  **24.7s alone**, so intrinsic, and it held **13** cells, not the 10 the diagnosis assumed. Net for
  the pass: `cargo nextest --workspace` **106.5s → 92.5s** with tests *increasing* 2298 → 2316.
- **Concurrent agents on this box produce outliers** — one 136s run came purely from CPU contention;
  single-run numbers are worthless, use paired or repeated runs.
- **proptest regression seeds were worse than useless**: proptest keys its persistence file **per
  source file, not per test fn**, so after banding all 4 bands replayed all 3 seeds — and a `cc`
  seed regenerates values through the *current* strategy, so they no longer reproduced the inputs in
  their own shrink comments. **Random cases wearing a regression label.**
- **Do not run the e2e suite concurrently with other heavy jobs.** Playwright's Edge teardown has a
  hardcoded 30 s CDP close window and a blocking `taskkill`; under load that is minutes of dead air
  that reads as a hang. `gate.mjs` is strictly serial, so the gate itself is safe.
- **A flake that reproduces deterministically in a production bundle is not a flake** (P103).

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

---

## OPEN follow-ups (genuine unresolved items, not checkpoints)

Condensed to one line per item on 2026-09-03; the pre-condensation text is archive Part 50.

### From the 2026-09-02 file-size refactor pass (archive Part 36)

Ratchet baseline moved **27 offenders / 6241 excess → 20 / 3528**; full gate green 8/8, 603s.

- **Fold-pill cursor is dead** in `GraphCanvas.handleMouseMove` — P92 §1.4's overflow-cursor write
  unconditionally clobbers spec-004 §1/§2's `foldCursorFor`, and `computeHoverTarget` returns null on
  exactly those rows. Real regression; no vitest mounts `GraphCanvas`, so e2e is the only net.
- **Reflog overlay not torn down** when a repo goes unusable — candidate fix `52c815e` /
  `6092eb3` / `338d71f`, not recorded as closed. See FOR USER item 6.
- **Shortcuts stay live during confirm dialogs** (`pendingForcePush`, `pendingCommitPush`,
  `pendingBisectBad` absent from `dialogOpen`) — candidate fix `1d9d9bf`, not recorded as closed.
- **`ai::session*` is load-flaky** — wall-clock watchdog margins; needs a clock seam, not wider
  sleeps. Candidate fix `734b310`, not recorded as closed.
- **Contract divergences the tests document as bugs-in-the-contract:** rebase §3.1.5/§9.7
  unstaged-changes precondition, and the libgit2-vs-CLI rename/delete conflict index-entry count.
  Plus a near-tautological `expected_presence` oracle.
- **Duplicated external-tool launchers** — `App.tsx`'s trio is statement-for-statement identical to
  `repoWorkspace/useExternalTools.ts`; hoist to `src/hooks/`. Also two timers with no unmount cleanup
  (`sessionSaveTimer`, toast auto-dismiss).
- **Duplicated helpers left visible, not merged** (behavior risk, not a move): atomic-write helpers
  across `assets/bundle/write.rs` + `assets/profiles/store.rs`; test helper families across
  `tests/diff/` and the four `tests/rebase_merge/*_support.rs`.
- **Three files deliberately stopped short of 500** (each further cut would forward 15-100 values to
  exactly one consumer): `RepoWorkspace.tsx` 2309, `GraphCanvas.tsx` 784, `App.tsx` 602.

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
- **Candidate process changes, NOT adopted — needs a USER decision:** batch small P-tasks through one
  senior-dev spawn; skip the architect contract for single-component fixes; fold the board update
  into the feat commit.

### Hoisted off milestones archived 2026-09-01

- **keyring 3 → 4** needs a dedicated increment: 4.x moves onto `keyring-core`, renames every
  per-backend feature, drops `crypto-rust`, and requires explicit credential-store registration —
  real changes to `crates/bonsai-forge/src/auth.rs`. (DEP REFRESH, archive Part 24.)
- **`no_proxy_client()`** in `src-tauri/src/mcp/http_support.rs` still uses `.expect("build reqwest
  client")`. **Contradicted by the P91 audit, which says this can be CLOSED** — see FOR USER item 6.
- **P87b FU-1..4** still open: target row label, commitAmend row, row `role`/`aria-expanded`,
  clickable dock bar. Plus the `AiActivityPanel` aria-label NIT. (archive Part 27.)
- **RepoWorkspace refactor** still stands for maintainability (not perf); P88's audit re-confirmed it.
- **P90.1 deferred:** per-check timing fields; header commit-summary text; command-palette
  `Refresh checks` / `Show checks`; mock fixtures for noForge/error reachable by click.
- **Known flake (pre-existing, untouched):** `watcher::tests::git_internals_filtered` is a timing
  flake (`unwrap_err` on an `Instant`); passes on isolated re-run.
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

- **`graph-design-review-2026-08-22.md` M1 is SUPERSEDED — do not implement.** `role="grid"` /
  `aria-rowcount` / `aria-activedescendant` are forbidden by `ui-reference.md` §4.1 (`:250-252`).
- **M2/M3/M4/S2/S3/N1/N2 — resolution unverified.** Not checked by the 2026-09-01 sweep; do not
  assume they landed.
- **`review-2026-08-22-ui.md` NIT-1 — Sidebar ignores `panelDensity`** (confirmed still open
  2026-09-01: `.branch-row` is a fixed height).
- **NIT-2 —** `src/components/OnboardingOverlay.tsx:229` is still `aria-label="Close"`; the review
  preferred "Close the tour".
- SHOULD-3 (`--accent` text over `--selection` fails AA) is the **same item** as P69 **A9** below —
  A9 is the canonical entry.

### P80 forge follow-ups (SHOULD-FIX/NIT, non-blocking)

- (a) `forge_set_token_inner` validates before the `host.is_empty()` guard — guard host first.
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

- No `rustfmt.toml` anywhere, no fmt check in any hook or CI.
- `cargo fmt --all --check` reports **1773 hunks across 221 files**; `--config
  use_small_heuristics=Max` is *worse* (2065).
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

- `docs/contracts/P68e-ai-activity-dock.md` is **1064 lines** and under-describes shipped code
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

### P69 Settings follow-ups awaiting a user decision (nothing is blocked on them)

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

- **Collapsed-rollup needs first expand (FOR-USER decision):** the ls-remote check only fires on the
  first Tags expand per session, so the rollup warning cannot appear until the user expands Tags
  once. Decide whether a cheap unprompted check on repo-open is worth the network cost.
- NITs: rollup aria-label lacks singular/plural ("1 tags") · `useTagSync` re-hits network on rapid
  collapse-then-expand while `unavailable` · confirm dialogs close optimistically so `busy` never
  paints · the tag-filter box gate counts local tags only · item-7 "Delete tag on origin" also shows
  on remote-only ghost rows (coherent).
- Backend NITs: `delete_remote_tag` doesn't `evict_fresh_on_auth_fail` · `validate_tag_name` is
  duplicated from `tags.rs` — promote to shared if a 3rd caller appears.

### macOS ad-hoc code signing — config DONE 2026-08-30, RELEASE STILL PENDING

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
| `docs/history/todo-archive-2026-09.md` | **Parts 36-50 (moved 2026-09-03):** the file-size refactor pass · the full narratives of P102+P105, P106, P107, P108 and the P91 security arc + audit + build diary (their milestone entries stay live above) · superseded pre-ship filings for P102/P105/P106/P108, the dead-CSS decision block and the resolved `lint:size` blocker · the 2026-09-03 velocity pass · P99, P100, P101, P98, P95, P96, P97 and the P100+P101+DX-e2e banner · built-bundle e2e + P103 + P104 · the DX/velocity stubs · the pre-condensation open-follow-up text. **Parts 33-35 (2026-09-01):** the P84 record gap · macOS ad-hoc signing · the two 2026-08-22 design reviews. **Parts 22-32 (2026-09-01, verbatim):** P94 · P93+P92 · DEP REFRESH · P90+P89 · P88 · the P85-P87 batch · P82+P83 · divergence reconcile + Release 1.1.0 · the DX dev-loop text · the confirmed-checkpoints block · the 2026-08-21 resolved follow-ups. |
| `docs/history/todo-archive-2026-08.md` | Parts 1-9: P65 to P28 build detail, the Phase 1-4 banners, resolved FOR-USER decisions, P69(1.0.0)/P67/P68 detail. Parts 10-16: the P62-P74 checkpoint waiver + P71-P74, the P69 Settings redesign, the Audit #2 fix batch. Parts 17-18: P70 and P77. Part 19: the follow-ups resolved 2026-08-21, verbatim. Part 20: P78/P79/P80. Part 21: P80b/P81/P82. |
| `docs/history/todo-archive.md` | P27 to P2, M0-M6 |
| `docs/history/milestones-mvp.md` | the M0-M6 AI-gate vs USER CHECKPOINT split |
| `docs/history/context-pollution-audit.md` | the context/token-cost audit |
| `docs/history/velocity-2026-09-01.md` | gate wall-clock, test-suite hotspots, inner-loop rebuild cost, ceremony-vs-machine-time split (2026-09-01) |
| `docs/contracts/INDEX.md` | one line per contract file — milestone, scope, status |

Move a milestone's section into the current dated archive file only once **both** halves of its gate
have passed (or the native half is explicitly waived). A milestone with a pending USER CHECKPOINT
stays on this board.
