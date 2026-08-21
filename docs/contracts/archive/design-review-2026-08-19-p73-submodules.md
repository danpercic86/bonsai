# Design review — P73 submodule reconnect (frontend)

Reviewer: `ui-designer`. Date: 2026-08-19. Scope: working tree vs `7769bbb`, `src/` + `e2e/`.
Contract under review: `docs/contracts/P73-submodule-reconnect-ui.md`.
Evidence: source reads + live harness (`http://localhost:1420`, `VITE_MOCK_IPC=1`) via
`javascript_tool` computed-style/DOM assertions. **No screenshot** — the Browser pane is not
composited in this session ("the page is not compositing frames"), so visual proof is DOM + measured
colour only, and `window.innerWidth/innerHeight` read `0`. Timer throttling in the hidden pane
stretches every mock `delay()` to ~1 s granularity; all timing observations below are therefore
non-evidence and are excluded.

## Verdict per contract section

| § | Subject | Verdict |
|---|---|---|
| 2 | Component decomposition + file paths | **PASS** (one accepted deviation, +1 file) |
| 3.2 | Menu labels + `disabled` expressions | **PASS** (verified live, both row states) |
| 3.3 | Stays out of the command palette | **PASS** |
| 4 | Badge labels + titles | **PASS** |
| 5.1 | Success copy | **PASS** |
| 5.2 | Failure copy, prefixes, dedupe key | **PASS** (reconciled backend copy; 3 new sentences ruled on below) |
| 5.3 | No truncation, 360 px wrapping | **PASS** |
| 6.1 | Row-local busy pill + `aria-busy` | **PASS** |
| 6.2 | `netBusy` → `.header-progress` | **PASS** |
| 6.3 | `prefers-reduced-motion` | **PASS** |
| 6.4 | No live region / banner / cancel | **PASS** |
| 7.1–7.2 | New class, no new tokens, contrast fix | **PASS** (measured 5.80 / 6.22) |
| 7.3 | OPT-1 | **PASS** with one geometry defect (S-1) |
| 7.4 | Density-invariant sidebar geometry | **PASS** (24 px rows in both densities) |
| 7.5 | States matrix | **PASS** |
| 7.6 | Keyboard / SR | **PASS** |
| 8 | Harness seams + fixture | **PASS** (3 accepted deviations) |
| 9 | Acceptance criteria 1–20 | **PASS**, except 16 which was mis-specified by me (see A-2) |

## Measured colour (live, both themes)

| Surface | Pair | Dark | Light | Bar |
|---|---|---|---|---|
| `.submodule-badge-muted` label | `--text-2` on 12 % self-tint | **5.80:1** | **6.22:1** | AA text ✓ (contract predicted 5.79 / 6.22) |
| `.submodule-badge-busy` label | same | **5.80:1** | **6.22:1** | AA text ✓ |
| `.submodule-badge-ok` label | `--text-1` on 14 % tint | **10.90:1** | **12.82:1** | AA text ✓ |
| `.submodule-badge-warn` label | `--text-1` on 14 % tint | **10.49:1** | **12.91:1** | AA text ✓ |
| `.submodule-badge-ok` glyph `✓` | `--success` on the pill | **4.61:1** | **3.94:1** | 3:1 graphics ✓ |
| `.submodule-badge-warn` glyph `⚠` | `--warning` on the pill | **5.64:1** | **3.80:1** | 3:1 graphics ✓ |
| ok/warn 40 % border vs sidebar bg | — | 2.01 / 2.28 | 1.75 / 1.71 | **below 3:1 — decorative only** (see A-1) |
| `.toast-error` label | `--danger` on 14 % tint over `--bg-2` | **3.35:1** | **3.48:1** | **pre-existing OPT-2 shortfall** (see S-2) |

OPT-1 is correctly implemented as **word + `aria-hidden` glyph**, not hue alone: label moved to
`--text-1`, hue confined to the 40 % border and the 100 % glyph, using the house local custom
property `--h` (the same convention as the AI-dock pills, `styles.css:7192-7221`) — **zero new theme
tokens**, and no hardcoded hex anywhere in the diff.

## Live strings observed (verbatim)

```
not checked out   ✓ up to date   ⚠ out of sync   ⚠ modified   checking out…
Checked out vendor/libcore
Couldn't check out vendor/libcore. The folder already has files in it. Move or delete everything inside 'vendor/libcore', then try again.
Couldn't check out vendor/libcore. Bonsai has cached data for a different remote URL ('https://example.com/old-vendor/libcore.git' instead of 'https://example.com/libcore.git'). Run Sync on this submodule, then try again.
Couldn't check out vendor/libcore. Authentication failed for https://example.com/libcore.git.
```

Menu, uninitialized row: `Initialize and check out` **enabled**; `Update`, `Deinitialize…`,
`Open in new tab` `aria-disabled="true"`; `Sync`, `Remove…`, and the three external-tool items
enabled. On `upToDate` the pair inverts exactly. No item is labelled `Init`; `data-tone="danger"`
appears on `Remove…` only. `role="menu"` / `role="menuitem"` intact.

Busy (`?submodule=slow`): the acted-on `<li>` alone carries `aria-busy="true"`, its pill is
`branch-badge submodule-badge-busy` with text `checking out…`, `title` **absent**, pill 17.94 px
inside a 24 px row, the other four rows unchanged, one `.header-progress[aria-hidden="true"]` in the
DOM, and right-clicking the busy row still opens the menu. All cleared after the op.

Failure: badge stays `not checked out`; toast is `role="alert"` and sticky; `.toast-stack` keeps
`aria-live="polite"`. Dedupe holds — repeating the failing action on `vendor/libcore` left the error
count unchanged, and failing a second row (`docs/spec`) made it two. No toast contained
`attempt to reinitialize` or `.git/modules`.

Overflow (91-char row): name `scrollWidth > clientWidth` (ellipsized) with the full 91-char path in
`title`; pill fully visible, not clipped, row still 24 px at a 240 px sidebar. Its `urlMismatch`
toast rendered 360 px wide × 225 px tall, `overflow-wrap: anywhere`, `-webkit-line-clamp: none`,
`text-overflow: clip`, `scrollWidth == clientWidth` — wraps, no ellipsis, no overflow.

## Findings

### MUST-FIX
None.

### SHOULD-FIX

- **S-1 · Pill heights are ragged inside one list.** `src/styles.css:4420-4425` gives ok/warn a
  `1px` border while `.submodule-badge-muted` / `.submodule-badge-busy` (4405-4415) have none, so
  adjacent pills measure **19.94 px vs 17.94 px** — visible as a 1 px step up and down the
  Submodules and Worktrees sections, and the busy pill visibly *shrinks* the moment an op starts.
  Fix: add `border: 1px solid transparent;` to the shared selector at `styles.css:4390-4400` (the
  four classes already share sizing there). No other change; verdict pills keep their hue border,
  hueless pills stay hueless, everything lands at 19.94 px. This is a real regression introduced by
  OPT-1 and is the one item I would hold the increment for.
- **S-2 · P73's whole payload is delivered at 3.35:1.** The milestone's deliverable is prose — six
  refusal sentences — and it renders only inside `.toast-error`, measured **3.35:1 dark / 3.48:1
  light**. Pre-existing and app-wide (my §7.3 OPT-2, correctly deferred by the orchestrator, and
  nothing in P73 *depends* on it), but P73 changes the stakes: this is no longer decorative tone on a
  five-word confirmation, it is a multi-sentence instruction the user must read to recover. I am not
  asking for it in P73; I am asking that the toast-tone pass be scheduled next rather than parked.
- **S-3 · Sidebar section chrome misses the 24 px hit-target floor.** Measured live: the
  `Add submodule` `+` button is **20 × 20 px** and the `Submodules` collapse toggle is **183 × 16 px**
  (`Sidebar.tsx:814-835`). Pre-existing (P60d), untouched by this diff, and it applies to every
  sidebar section — but it is in the surface this review covers, so it is recorded here rather than
  rediscovered. Fix belongs in a sidebar-wide pass: pad the header row to 24 px and the `+` to
  24 × 24 px without changing the 16 px visual glyph.

### NIT

- **N-1** `useSubmoduleActions.ts:143` — `handleAddSubmodule` still toasts bare `errorMessage(e)`
  with no prefix and no dedupe key, the one submodule op that does not follow §5.2's shape. Out of
  contract scope (add has no row and no verb prefix specced), but `Couldn't add ${path}. ` would make
  the six handlers uniform.
- **N-2** `SubmoduleRow.tsx:35-37` — the busy branch wraps its label in a redundant inner `<span>`
  purely to mirror the glyph branch's structure. Harmless; the `gap: 3px` has nothing to space.
- **N-3** Worktree pills (`Sidebar.tsx:309-313`) render `title="current"` / `title="stale"` — the
  title-repeats-the-label pattern §4 removed from submodule badges, still live for worktrees, and now
  also missing the OPT-1 glyph while carrying the OPT-1 border. Not a regression (words carry the
  meaning, so §7 holds) but the two sibling row types now differ. Fold into the sidebar pass.

### Non-findings (checked, no action)

- `RepoWorkspace.tsx` blank-line removal: sampled at 228-243, 1590-1616, 2494-2509. Declarations
  stayed grouped, comments survived, nothing was joined that should not be. **No objection** — though
  a ratchet that forces cosmetic line-squeezing in a 2946-line file is a process smell, not a design
  one.
- Section regressions: header, `+` (relabels and un-collapses correctly), `No submodules` empty
  state, and collapse all intact — toggling took the sidebar from 8 badge rows to 3 and back, with
  `aria-expanded` tracking.
- File sizes: `Sidebar.tsx` 918 → **892**, `workspaceMenus.ts` **803** (flat, not grown),
  `SubmoduleRow.tsx` 50, `submoduleBadges.ts` 42.

## Rulings on declared deviations — all accepted

- **A-1 · `submoduleBadges.ts` as a third file.** Accepted, and better than my §2 table. The
  `react-refresh/only-export-components` constraint is real, and the copy table now reads as a copy
  table. My §2 should have anticipated it.
- **A-2 · Acceptance criterion 16 was mis-specified by me.** I wrote "items 1-5 disabled and 6-9
  enabled" while the row is busy. Live, on a busy *uninitialized* row, item 6 `Open in new tab` is
  also disabled — correctly, per §3.2's `disabled: uninit`. The criterion should read "items 1-5
  disabled; the external-tools trio enabled; `Open in new tab` per its own `uninit` gate". The
  implementation is right and my criterion was wrong.
- **A-3 · `auth` seam uses the row's real URL** instead of my literal sentence. Accepted and
  preferred — `Authentication failed for https://example.com/libcore.git.` proves §5.2 passes remote
  copy through *and* is honest about which remote failed.
- **A-4 · `urlMismatch` uses the orchestrator's two-URL wording.** Accepted. Ruling on readability,
  which I was asked for: at 360 px the real-world case renders as
  `Couldn't check out vendor/libcore. Bonsai has cached data for a different remote URL ('https://example.com/old-vendor/libcore.git' instead of 'https://example.com/libcore.git'). Run Sync on this submodule, then try again.`
  — long, but it wraps cleanly, the parenthetical is skippable on first read, and the imperative
  remedy is the last clause where the eye lands. **Keep it.** The two URLs earn their length: without
  them the user cannot tell which side is stale. Worst case (91-char path + two long URLs) is a
  225 px, ~14-line toast — ugly but readable, non-overflowing, and dismissible. No replacement
  proposed.
- **A-5 · pathological fixture is 91 chars, not 98.** Accepted; irrelevant to what it proves.

## Verdict on the three refusal sentences new to me

All three are **accepted as written** — complete, capitalised, period-terminated, no libgit2 prose,
no `.git/modules`, and each ends in an actionable next step.

- `No URL is configured for this submodule, so its cached data cannot be verified. Run Sync on this
  submodule, then try again.` — good. Composed: `Couldn't check out <name>. No URL is configured…`
- `Bonsai's cached data for this submodule has no remote URL recorded. Run Sync on this submodule,
  then try again.` — good, but it is near-indistinguishable from the previous one at reading speed
  (both mean "we can't verify the cache; run Sync"). They are two different backend conditions with
  one user-visible remedy. **Optional simplification, not a fix:** if the distinction never changes
  what the user does, collapse both to the first sentence and keep the condition in the log.
- `This submodule resolves to a path outside the repository. Bonsai will not touch it.` — good, and
  correctly refuses to offer a remedy where there is none. It is the only one with no "try again",
  which is right.
- `Could not reconnect this submodule to its existing local data. Run "git submodule update --init"
  in a terminal to repair it.` — acceptable as the last-resort fallback. One reservation: it hands the
  user a CLI command, which the app exists to avoid. Prefer the straight quotes it already uses (not
  curly) so copy-paste works, and pair it with the existing `Open in terminal` row action in a later
  pass so the remedy is one click rather than a transcription.

## Pending edit to `docs/contracts/ui-reference.md` §11

**APPLIED 2026-08-19 by the orchestrator** — all three parts are now in `ui-reference.md` §11.
Recorded below verbatim as the rationale trail.

The delta was:

1. In the **Verdict pills** bullet, after "hue in the 40% border and in a 100% `aria-hidden` glyph":
   add — *"The glyph is the accessible hue carrier (measured 2026-08-19: `✓` **4.61:1** dark /
   **3.94:1** light, `⚠` **5.64:1** / **3.80:1** — both clear the 3:1 graphics bar). The 40% border is
   decorative delineation only and measures **1.7–2.3:1** against the row background; never rely on
   it to carry meaning. Set the hue through the local `--h` custom property, the AI-dock convention."*
2. In the **Hueless / informational pills** bullet, replace "No glyph, no border" with — *"No glyph,
   no hue — but keep `border: 1px solid transparent` so hueless and verdict pills are the same
   **19.94 px** height in a shared list."*
3. In the **Busy pill** bullet: note the pill drops its `title` entirely while busy (the participle
   is the whole message).

## One-line verdict

**Approve with S-1** — contract fidelity is essentially exact, tokens and a11y are clean and
measured, all five declared deviations are improvements; ship once the hueless pills get a
transparent 1px border so pill heights stop stepping.
