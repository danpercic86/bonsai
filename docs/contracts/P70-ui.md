# P70 — UI contract: the "Git is not available" notice bar

Owner: `ui-designer`. Input contract: `docs/contracts/P70-git-resolution.md` (§4.3/§4.4 data, §7.3
scope, §7.4 error routing). Design system: `docs/contracts/ui-reference.md` (updated in the same
pass — new §10).

**Status: all open questions ratified by the orchestrator (2026-08-19), including the §10 reversal,
the §5.6 jargon-ban scoping, and the §5.7 announcement tightening. This file is self-contained —
senior-dev implements from it without further decisions.** The ratified decisions and their
consequences are recorded in §12.

**Behaviour fixed by the architect and honoured verbatim here:** non-dismissable while
`found === false`; never rendered when `found === true`; renders nothing while `status === null`.

**No new theme tokens.** Every colour below is an existing `:root` / `[data-theme='light']` custom
property from `src/styles.css`.

---

## 1. Placement decision

**The notice bar is a direct child of `.app`, immediately after `</header>` and before the
`.workspace-host` tab hosts** (`src/App.tsx:923`). In-flow, full width, `flex: none` — it pushes the
workspace down; it never overlays and never blocks.

```
+---------------------------------------------------------------------------+
| header (40px): TabStrip · theme · lists · AI · health · settings          |
+---------------------------------------------------------------------------+
| ⚠  Git is not available                     [ Re-check ]  [ › Details ]   |  <- P70, flex:none
|    Bonsai couldn't find a runnable git program on this computer. Your     |
|    saved credentials are fine — Bonsai never got as far as checking them. |
|    Quit Bonsai and reopen it from the Start menu — an in-app update can   |
|    leave Bonsai running without your full PATH.                           |
+---------------------------------------------------------------------------+
| workspace toolbar (40px)                                                   |
+-----------+---------------------------------------------+-----------------+
| sidebar   | commit graph (canvas)                       | right panel     |
```

Why here, and what was rejected:

- **App-level, not per-tab.** Git availability is a process-global fact. Mounting inside
  `.workspace-host` would render one banner **per open tab** (each tab host stays mounted at
  `display:none`) and would vanish entirely on the no-repo empty state — which is exactly a state a
  broken install can be stuck in. Rejected.
- **Below the header, not above it.** The header owns the tab strip and window-level controls; an
  app-modal-looking bar above the tabs reads as a title bar and breaks the 40px header rhythm.
- **In-flow, not fixed/overlay.** A floating card that covers the graph while the user works would
  be the "trap" this contract must avoid. See §9.
- **Coexistence with the update surface.** `UpdateNotification` is `position: fixed; bottom:16px;
  right:16px; z-index:90` (`styles.css:5179`). Zero collision — different corner, different layer.
  Both may show at once and neither needs a change. Reading order is correct too: the blocking
  problem is at the top, the optional offer is at the bottom.
- **Layout shift.** Nothing is reserved while `status === null` (§6), so the healthy path
  (`found === true`, the overwhelming majority) never shifts at all. The broken path shifts once,
  within the first frames after mount, and the graph canvas's existing `ResizeObserver` absorbs it.
  The bar **never animates its height** (same rule as the AI dock, ui-reference §9) — a height
  transition would force repeated 20k-row canvas relayouts.

---

## 2. Component decomposition + file paths

| File | Kind | ~lines | Responsibility |
|---|---|---|---|
| `src/components/GitMissingBanner.tsx` | container (replaces the architect's placeholder) | ~150 | Reads `GitAvailabilityState`, owns `detailsOpen` + `lastCheckedAt` + `announcement` local state, renders the announcer + the collapsed row. |
| `src/components/gitBanner/GitBannerDetails.tsx` | presentational | ~110 | The disclosure region: other remedies, degraded-capability list, technical-details block + Copy. |
| `src/components/gitBanner/gitBannerCopy.ts` | pure data/logic | ~140 | **The single source of every user-facing string in this surface.** `bannerCopy(status, os)` → `{ title, explanation, remedy, triedPath }`; `buildAnnouncement(...)` (§5.7, composed from the *same* `bannerCopy` result — see the derivation rule); `resolveOsFamily()` (incl. the `?os=` harness override, §11.3); `sourceLabel()`; `buildTechnicalDetails()`. Unit-testable with zero DOM. |
| `src/utils/platform.ts` | **extend** (92 → ~130 lines) | +40 | `export type OsFamily = 'windows' \| 'mac' \| 'linux'`, `detectOsFamily(probe?)`, `export const osFamily`. Same defensive probe style as `detectIsMac`. **Stays free of harness seams** — the `?os=` override lives in `gitBannerCopy.ts`. |
| `src/components/Toasts.tsx` | **extend** | +2 | `Toast` gains `key?: string` (§10.1). Purely additive; the presentational component ignores it. |
| `src/App.tsx` | **extend** | +4 | Mount `GitMissingBanner`; `pushToast` gains an optional third parameter `key?: string` implementing the replace-in-place rule (§10.1). |
| `src/styles.css` | **append** | ~95 | The `.git-banner*` block (§4) + one line added to the existing `@media (prefers-reduced-motion: reduce)` block at `styles.css:7675`. |

`RepoWorkspace.tsx` and `WorkspaceToolbar.tsx` get **no new UI and no new props** — see §10, whose
reversal (ratified) removed the prop drill the earlier draft required.

Why not reuse an existing component: `EmptyState` is a centred full-pane column; `OpBanner` is
right-panel-scoped, repo-op-shaped and takes `RepoOpState`; `UpdateNotification` is a fixed
toast-like card with a dismiss control (the one thing this must not have); toasts are transient by
definition and are the exact failure mode P70 exists to kill. **The geometry and the border/glyph
severity treatment are lifted from `.op-banner` verbatim** so the two read as one family.

---

## 3. Variants

Keyed only on fields the architect ships (`found`, `path`, `source`).

| Variant | Condition | Meaning |
|---|---|---|
| **A — not found** | `found === false && path === null` | The ladder was exhausted (`source === 'fallback'`). |
| **B — found but unrunnable** | `found === false && path !== null` | A candidate resolved but `--version` failed to spawn or exited non-zero. |

Variant B is in scope: **ratified decision 1** amends the Rust contract so `GitAvailability.path`
is populated whenever a candidate was resolved, regardless of `found`. This is the state produced
by the architect's own USER CHECKPOINT reproduction (`BONSAI_GIT_BIN` pointed at a nonexistent
path, §6.2) and by a corrupt install (§9-Q6). Senior-dev may rely on `path !== null` as the sole
discriminator.

**Every user-facing string in this surface — visible or announced — is derived from the variant
via `bannerCopy()`. No string is hard-coded to a variant at any call site.** See §5.7 for the
defect this rule exists to prevent.

---

## 4. Geometry, tokens, both themes, both densities

### 4.1 Container `.git-banner`

```
display: flex; flex-wrap: wrap; align-items: flex-start;
gap: 8px 12px;
padding: 8px 12px;
flex: none;
background: var(--bg-1);
border-bottom: 1px solid var(--border);
box-shadow: inset 3px 0 0 var(--warning);   /* severity rail, left edge */
font-size: 13px;
```

- **Severity = warning, not danger.** The app is degraded, not dead: libgit2 (graph, status,
  staging, commit, diff) is untouched, and after ratified decision 5 so is SSH-agent
  authentication. `--danger` red across the top of the window on every launch would be alarmist,
  and P70's whole point is to be *calm and honest*. The warning hue lives in the 3px rail and the
  `⚠` glyph only; **every word is `--text-1` / `--text-2`** per ui-reference §2's
  `--warning`-as-text shortfall note.
- `border-bottom` uses `--border` (not a warning tint) so the bar seats against the workspace
  toolbar exactly like the header does.
- **Both themes are served by one rule set** — every value is a token. Dark: `--bg-1` `#1d2026`,
  rail `#d4a72c`. Light: `--bg-1` `#f6f7f9`, rail `#9a6700`.
- **Density: identical in `cozy` and `compact`.** Padding `8px 12px`, control min-height 24px, in
  both. Justification: ui-reference §3 scopes `panelDensity` to the right panel and the AI dock;
  this is app chrome, like the 40px header and workspace toolbar, which are also density-invariant.
  No `--gb-*` density block is introduced.

### 4.2 Children

| Element | Class | Spec |
|---|---|---|
| Severity glyph | `.git-banner-icon` | `⚠`, 14px, `color: var(--warning)`, `flex: none`, `aria-hidden="true"`, `line-height: 20px` (optically aligns to the title). |
| Text column | `.git-banner-text` | `flex: 1 1 320px; min-width: 240px; max-width: 72ch;` column, `gap: 2px`. The `max-width` stops a 2560px window from producing an unreadable single-line paragraph. |
| Title | `.git-banner-title` | 13px / 600 / `--text-1`, `id="git-banner-title"`. |
| Explanation | `.git-banner-sub` | 12px / 400 / `--text-2`, `overflow-wrap: anywhere`. |
| Remedy | `.git-banner-remedy` | 13px / 400 / `--text-1` (it is the thing the user came for — deliberately *not* `--text-2`). |
| Tried-path line (Variant B only) | `.git-banner-path` | mono 11px / `--text-2`, single line, `white-space: nowrap; overflow: hidden; text-overflow: ellipsis;` + `title={fullPath}`. |
| Actions column | `.git-banner-actions` | `flex: none; margin-left: auto;` column, `align-items: flex-end`, `gap: 4px`. |
| Re-check | `.git-banner-btn` on `btn-primary` | `padding: 4px 10px; font-size: 12px; min-height: 24px;` — same metrics as `.op-banner-btn`. |
| Details toggle | `.git-banner-toggle` on `btn-secondary` | same metrics; leading `<span class="file-chevron">›</span>` reused from the sidebar disclosure pattern. |
| Re-check readout | `.git-banner-checked` | 11px `--text-2`, appears only after the first re-check. |
| Disclosure region | `.git-banner-details` | `flex: 1 0 100%;` (own row), `margin-top: 4px`, `padding-top: 8px`, `border-top: 1px solid var(--border)`, `max-height: 220px; overflow-y: auto;`, column `gap: 8px`. |
| Section labels | `.git-banner-label` | 11px, uppercase, `letter-spacing: .08em`, `--text-3`. Decorative only (duplicates visible structure) — allowed by ui-reference §2. |
| Remedy list | `.git-banner-list` | `list-style: none; margin:0; padding:0;` rows 12px `--text-2`, `gap: 4px`, each with an `aria-hidden` `·` in `--text-3` at 8px inline-start. |
| Capability rows | `.git-banner-cap` | 12px `--text-2`; leader (`Still works:` / `Doesn't work:`) 600 `--text-1`; leading `aria-hidden` glyph `✓` `--success` / `✕` `--danger`. |
| Technical block | `.git-banner-tech` | mono 11px / 16px, `--text-2` on `--bg-0`, `border-radius: 6px`, `padding: 8px`, `white-space: pre-wrap`, `overflow-wrap: anywhere`, `max-height: 120px`, `overflow-y: auto`. |
| Copy | `.git-banner-copy` on `btn-secondary` | 11px, `padding: 2px 8px`, `min-height: 24px`. |
| Announcer | `.git-banner-announce` | Visually hidden (recipe copied from `.ai-dock-announce`, `styles.css:7657`). Always mounted. §5.7. |

### 4.3 Responsive

`flex-wrap` on the container + `flex: 1 1 320px` on the text column reproduces `.op-banner`'s
behaviour: below ~560px of banner width the actions column drops to its own row rather than
squeezing the text to zero (the bug documented at `styles.css:5366`). Minimum supported window
width is unchanged.

### 4.4 Contrast (measured, both themes, sRGB)

| Pair | Dark | Light | Bar |
|---|---|---|---|
| `--text-1` on `--bg-1` (title, remedy) | **13.5:1** | **15.4:1** | 4.5 ✓ |
| `--text-2` on `--bg-1` (explanation, lists) | **7.3:1** | **7.4:1** | 4.5 ✓ |
| `--text-2` on `--bg-0` (technical block) | **7.9:1** | **4.9:1** | 4.5 ✓ |
| `--warning` rail + `⚠` on `--bg-1` | **7.3:1** | **4.5:1** | 3.0 ✓ |
| `--success` `✓` on `--bg-1` | **5.7:1** | **4.7:1** | 3.0 ✓ |
| `--danger` `✕` on `--bg-1` | **4.4:1** | **4.6:1** | 3.0 ✓ |
| `--text-3` section labels | 3.4:1 | 3.0:1 | decorative only — every label duplicates structure that the following content states in full words |

`btn-primary` / `btn-secondary` / focus-ring pairs are unchanged and already measured.

---

## 5. Microcopy (the actual strings)

The Rust `git_not_found_message()` (§3.3) is **not** rendered as the banner's prose — it is written
as a single-paragraph error payload and appears here only inside the technical-details block, where
its job is to be pasted into a bug report. The banner owns its own structured copy.

Constraints honoured: names the real problem; explicitly denies the auth reading; leads with the
relaunch remedy; secondary remedies demoted to the disclosure; honest about consequence, including
the SSH/HTTPS distinction from ratified decision 5.

### 5.1 Variant A — not found

- **Title:** `Git is not available`
- **Explanation:** `Bonsai couldn't find a runnable git program on this computer. Your saved
  credentials are fine — Bonsai never got as far as checking them.`
- **Remedy**, by `OsFamily`:
  - windows — `Quit Bonsai and reopen it from the Start menu — an in-app update can leave Bonsai running without your full PATH.`
  - mac — `Quit Bonsai and reopen it from Applications — an in-app update can leave Bonsai running without your full PATH.`
  - linux — `Quit Bonsai and reopen it from your application menu — an in-app update can leave Bonsai running with an incomplete PATH.`

### 5.2 Variant B — found but unrunnable

- **Title:** `Git couldn't be started`
- **Explanation:** `Bonsai found a git program but couldn't run it. Your saved credentials are fine — Bonsai never got as far as checking them.`
- **Remedy:**
  - `source === 'override'` — `BONSAI_GIT_BIN points at a program Bonsai can't run. Correct it or clear it, then restart Bonsai.`
  - otherwise, windows — `Reinstall Git for Windows, then choose Re-check.`
  - otherwise, mac/linux — `Reinstall Git, then choose Re-check.`
- **Tried-path line:** `Tried: {path}` (mono, ellipsised, full value in `title` and in the technical block).

### 5.3 Disclosure — "Other things to try"

Label: `OTHER THINGS TO TRY`

- windows: `Install Git for Windows from git-scm.com, then choose Re-check.`
- mac: `Install Git — run xcode-select --install, or brew install git — then choose Re-check.`
- linux: `Install Git with your package manager (for example, sudo apt install git), then choose Re-check.`
- windows: `Set BONSAI_GIT_BIN to the full path of git.exe, then restart Bonsai.`
- mac/linux: `Set BONSAI_GIT_BIN to the full path of the git binary, then restart Bonsai.`

(The `BONSAI_GIT_BIN` row is suppressed in Variant B with `source === 'override'` — it is already
the headline remedy there.)

### 5.4 Disclosure — "While Git is missing" — REVISED for ratified decision 5

Label: `WHILE GIT IS MISSING`

- `✓ Still works: the commit graph, file status, staging, committing, branches, tags and diffs — these don't use the git program. Remotes you connect to over SSH also keep working.`
- `✕ Doesn't work: commit search, commit signing, Git hooks, and signing in to HTTPS remotes — Bonsai needs Git to read the credential helper that holds those saved logins.`

**Why this is more than a one-line edit.** The pre-ratification design said flatly "fetch, pull and
push don't work", which was correct only under the architect's original pre-ladder short-circuit.
With the narrowing to the credential-**helper** rung, SSH-agent authentication runs entirely inside
libgit2 and is unaffected — so a flat claim would have told a large class of users their remotes
were broken when they were not, which is precisely the species of dishonesty P70 exists to remove.
The consequential rewrites are enumerated in §12-D5; the largest is §10.

### 5.5 Disclosure — technical details

Label: `TECHNICAL DETAILS`, with the `Copy` button on the same row (`margin-left: auto`).

Block content (mono, verbatim, newline-separated):

```
{status.detail}
Resolved from: {sourceLabel(status.source)}
Path: {status.path ?? '(none)'}
```

`sourceLabel`: `override` → `BONSAI_GIT_BIN`, `path` → `PATH`, `registry` → `Windows registry`,
`wellKnown` → `standard install folder`, `fallback` → `not found`.

### 5.6 Action + status strings

| String | Where |
|---|---|
| `Re-check` | button, idle |
| `Checking…` | button, pending (disabled) |
| `Details` | disclosure toggle (label constant in both states; `aria-expanded` + chevron carry the state) |
| `Still not found — checked {HH:MM}.` | `.git-banner-checked`, after a failed re-check |
| `Copy` → `Copied` | technical-details copy button (1200 ms flip, matching `AiOutputPanel.tsx:54-58`) |
| `Git is available again — Bonsai found Git {version}.` | success **toast**, fired once on a user-initiated re-check that transitions `false → true` (ratified decision 4). No dedupe key needed — it can fire at most once per transition. |
| `{Op} failed — Bonsai can't run Git to read your saved sign-in.` | error **toast**, dedupe key `'git-not-found'` (§10.1). `{Op}` ∈ `Fetch` / `Pull` / `Push` / `Fetch all` / `Clone`. |
| `Search needs Git — see the notice at the top of the window.` | commit-search result surface, SHOULD (§10.3) |
| `Check Git availability` | command-palette action label (§8) |
| `Git {version} — {path} ({sourceLabel})` | palette-action success toast, i.e. `status.detail` verbatim |

#### Jargon ban — scope (AMENDED, ratified 2026-08-19)

The earlier blanket wording ("no string contains … credential helper") contradicted §5.4 and §5.5,
which legitimately need precise terminology. senior-dev and the reviewer independently converged on
the only reading that satisfies all three sections; it is now the contract:

**The ban applies to the banner's plain-language copy only — the collapsed prose inside
`.git-banner-text`: title, explanation, remedy, and the toast strings above.** In that scope, no
string may contain "authentication", "credential helper", "cached credentials", or any raw libgit2
text; "sign-in" and "saved sign-in" are the plain-language substitutes.

**Exempt, by design:**
- **§5.4's capability rows** — inside the `Details` disclosure. "Credential helper" is the correct
  and necessary term there: the user has opted into detail, and vagueness at that depth would make
  the SSH-vs-HTTPS distinction (the whole point of D5) unstatable.
- **§5.5's technical block** — carries `status.detail`, i.e. the raw Rust message, **verbatim**.
  Its job is to be pasted into a bug report; paraphrasing it would defeat the purpose.

Rationale for the split: the collapsed prose is what a distressed user reads in the first three
seconds and must be jargon-free; disclosure content is opt-in and is allowed to be precise.

### 5.7 Announcer strings (visually hidden live region) — TIGHTENED 2026-08-19

> **This section closes a real defect in the previous revision of this contract.** The earlier §5.6
> gave a single first-appearance announcement, `Git is not available. Bonsai can't find a runnable
> git program.`, and required only that "a live region exists". It did **not** require the
> announcement to track the variant, and it omitted the remedy. Harness verification under
> `?git=badpath` found the consequence: the visible banner read *"Git couldn't be started / Bonsai
> found a git program but couldn't run it"* while the live region announced the Variant A text — a
> screen-reader user in the mis-set-`BONSAI_GIT_BIN` case received **the opposite diagnosis from
> the sighted user, with no remedy at all**. An announcement that contradicts the visible text is
> worse than no announcement. The contract, not just the code, was at fault; both are fixed.

**Derivation rule (normative).** The announced string is **composed from the same `bannerCopy()`
result that renders the visible banner** — never assembled from literals at the call site, never
branched independently. `buildAnnouncement()` lives beside `bannerCopy()` in `gitBannerCopy.ts` and
takes its output as input, so the two cannot drift:

```
buildAnnouncement(copy) = `${copy.title}. ${copy.explanation} ${copy.remedy}`
```

The announcement **must include the remedy** — a screen-reader user who is told only that something
is broken, with no way forward, is worse off than a sighted user glancing at the same bar. It
**must not** include the Variant B `Tried:` path (a 250-char path read aloud is hostile; it remains
available in the technical block) and must not include the disclosure content.

**Resulting exact strings** (windows `OsFamily`; the remedy clause swaps per §5.1/§5.2):

| Event | Announced text |
|---|---|
| Banner first appears — **Variant A** | `Git is not available. Bonsai couldn't find a runnable git program on this computer. Your saved credentials are fine — Bonsai never got as far as checking them. Quit Bonsai and reopen it from the Start menu — an in-app update can leave Bonsai running without your full PATH.` |
| Banner first appears — **Variant B**, `source === 'override'` | `Git couldn't be started. Bonsai found a git program but couldn't run it. Your saved credentials are fine — Bonsai never got as far as checking them. BONSAI_GIT_BIN points at a program Bonsai can't run. Correct it or clear it, then restart Bonsai.` |
| Banner first appears — **Variant B**, other sources | `Git couldn't be started. Bonsai found a git program but couldn't run it. Your saved credentials are fine — Bonsai never got as far as checking them. Reinstall Git for Windows, then choose Re-check.` |
| Latch fired while `status === null` | Variant A text above (the generic copy is what is rendered, so it is what is announced). |
| Re-check failed again | `Git is still not available.` — deliberately short: the diagnosis and remedy have already been announced and are unchanged; repeating them on every retry is noise. |
| Re-check succeeded | `Git is available. Bonsai found Git {version}.` |
| `status === null`, no latch · `found === true` | *(empty string — the region is mounted but silent)* |

**Regression guard (required unit test).** For each of Variant A and Variant B, assert that
`buildAnnouncement(...)` **begins with the same title string that the visible `.git-banner-title`
renders** and **ends with the same remedy string that `.git-banner-remedy` renders**. This is the
assertion that makes the defect above impossible to reintroduce. Harness verification row added at
§11.4.

**Destructive-action UX: not applicable.** Nothing in this surface mutates a repository or can lose
work; `Re-check` is idempotent and read-only. No confirmation is required or specified.

---

## 6. States

| State | Rendering |
|---|---|
| `status === null` (not probed / probe threw) | The component is **always mounted** and returns only its visually-hidden announcer span with empty text. Zero height, zero border, zero layout cost — no skeleton, no reserved strip. This is what makes "no layout shift when it resolves" true for the healthy path. |
| `found === true` | Same as above (announcer only, empty). The banner never appears. |
| `found === false`, `path === null` | Variant A, collapsed. |
| `found === false`, `path !== null` | Variant B, collapsed. |
| `checking === true` | The banner stays fully rendered and unchanged; **only** the button changes to `Checking…` + `disabled` + `aria-busy="true"`. Never hide or dim the banner while checking — that is the flicker this contract forbids. |
| Re-check failed again | Banner unchanged (no remount, no re-animation, **no toast**). `.git-banner-checked` appears with `Still not found — checked {HH:MM}.` and the announcer says `Git is still not available.` Once present, that line stays, so there is exactly one 16px shift ever. |
| Re-check succeeded | Banner unmounts; one success toast (§5.6); announcer says `Git is available. Bonsai found Git {version}.` |
| `noteGitNotFound()` latch fires while `status === null` | Render Variant A with the generic copy and **omit** the technical-details section (there is nothing truthful to put in it). Ratified decision 4: the latch also kicks a probe, so the block fills in when it lands — and because the announcement is derived (§5.7), it refines with it. |
| Disclosure open | `.git-banner-details` on its own row; container grows; `max-height: 220px` + `overflow-y: auto` bounds it. Session-only state, default closed, not persisted. |
| Hover / active | `btn-primary` / `btn-secondary` / `btn-icon` house rules only. The bar itself has no hover state (it is not clickable). |
| `:focus-visible` | Global rule: 2px `--accent` outline, 1px offset. Nothing overrides it inside the bar. |
| Disabled | Only `Re-check` while `checking`. Global `:disabled` opacity applies. |
| Long content | Variant B path: single-line ellipsis + `title`. Technical block: `pre-wrap` + `overflow-wrap: anywhere` + 120px scroll. Text column capped at `72ch`. Collapsed bar is at most 3 lines (A) / 4 lines (B) at any window width ≥ 900px. |
| Error | There is no error state: `checkGitAvailability` never rejects for git state (§4.2), and an invoke-level throw leaves `status === null`, i.e. renders nothing. Correct — a failed probe must not itself produce chrome. |

---

## 7. Interaction

- **Re-check** (`btn-primary`) — the single primary action of this surface. Calls `recheck()`.
  Pending state is **text-only** (`Checking…`); no spinner glyph and no CSS animation, so it is
  reduced-motion-safe by construction and adds nothing to the graph render budget.
- **Minimum pending window: 400 ms** (ratified decision 4). `useGitAvailability` holds
  `checking === true` for at least 400 ms after the call, so the state change is perceptible on a
  fast machine.
- **No toast storm on repeated re-checks.** Failure produces only the in-banner
  `.git-banner-checked` line. Success produces exactly one toast, and only for a user-initiated
  re-check (never for the mount probe, never for the latch-triggered probe).
- **Details** — `<button aria-expanded aria-controls="git-banner-details">`, the house disclosure
  pattern (`Sidebar.tsx:112-120`, `DiffBrowser.tsx:377`). Enter/Space native. Chevron reuses
  `.file-chevron` / `.file-chevron-open`.
- **Copy** — `navigator.clipboard?.writeText(...)`, label flips to `Copied` for 1200 ms, silent on
  failure (matches `AiOutputPanel.tsx`). No toast. The path is copied as part of the block; there
  is no separate copy-just-the-path control (one control, not two).
- **Keyboard order** — pure DOM order, no `tabindex` anywhere: header toolbar → `Re-check` →
  `Details` → (when open) `Copy` → workspace toolbar → sidebar.
- **Focus is never moved to the banner.** It appears asynchronously a few hundred ms after launch;
  stealing focus from whatever the user started typing into would be hostile. Focus restore: N/A
  (nothing is trapped, no overlay).
- **Esc** — no binding. It is not a dialog and not dismissable.
- **No new global shortcut.** The palette entry (§8) is the keyboard route.

**Motion.** The bar's appearance, the disclosure and the height changes are **unanimated**
(ui-reference §9 rule: nothing that forces repeated canvas relayouts). The only transition in the
whole surface is the 120 ms `transform` on `.file-chevron`, which already exists. Add
`.file-chevron { transition: none; }` to the existing
`@media (prefers-reduced-motion: reduce)` block at `styles.css:7675` — a one-line, transform-only,
app-wide fix this milestone legitimately triggers (ratified decision 8).

### 7.1 Accessibility (normative)

- Container: `<section class="git-banner" role="region" aria-labelledby="git-banner-title">`.
- **Not** `role="alert"` — an assertive region would re-announce the entire bar on every retry.
- Announcements go through a separate, always-mounted, visually-hidden
  `<span class="git-banner-announce" role="status" aria-live="polite">` that starts empty and is
  populated one state-change later (mount-then-populate; a live region inserted *with* content is
  unreliably announced). Recipe copied from `.ai-dock-announce` (`styles.css:7657`) — the app still
  has no `.sr-only` utility.
- **The announced string must be derived from the same copy source as the visible banner and must
  carry the remedy — see §5.7 for the derivation rule, the exact strings for both variants, and the
  required regression test.** It is not sufficient that a live region merely exists: a live region
  whose text contradicts the visible text actively misleads, and that is precisely the defect
  §5.7 was written to close.
- Every control has a visible text label; no icon-only buttons in this surface.
- Hit targets: `min-height: 24px` on all three controls, ≥8px horizontal padding.
- Colour is never the sole carrier: `⚠` pairs with the title words, `✓`/`✕` pair with
  `Still works:` / `Doesn't work:`.

---

## 8. Command palette

One new action (ratified decision 6), in the existing app/diagnostics group:

- **`Check Git availability`** — always available (no repo required). Runs `recheck()`. On success
  it pushes an info toast with `status.detail` (`Git 2.47.1 — /usr/bin/git (path)`), which is
  otherwise the only place the healthy diagnostic is ever visible. On failure the banner is already
  the answer, so it pushes nothing.

Restraint check: one entry, no new top-level chrome, and it gives the `found === true` case a
surface without adding a Settings section for it.

---

## 9. Non-dismissable without feeling like a trap

The architect fixes "no dismiss while `found === false`". The design keeps that honest rather than
hostile:

1. **No close control at all** — not a disabled `✕`. A greyed-out dismiss reads as a broken button
   and invites repeated clicking.
2. **It never blocks anything.** In-flow, no overlay, no focus trap, no scrim, no modal. Every part
   of the app that still works stays fully reachable and fully keyboard-navigable — and after
   ratified decision 5 that includes SSH-authenticated fetch/pull/push.
3. **It is small.** 3 lines collapsed, ~64px tall, ~5% of a 1080p window; the disclosure is opt-in.
4. **It tells the user how to make it go away**, in the first sentence, with a button that actually
   does it (`Re-check`) — this is the substantive difference from a permanent scold. The same is
   true for screen-reader users, by §5.7's remedy requirement.
5. **It states what still works**, so the user isn't left assuming the app is bricked.
6. **It is warning-toned, not danger-toned** — it doesn't shout every second it is on screen.

---

## 10. Adjacent surfaces — remote-op feedback

> **Reversal of decision 2 — ORCHESTRATOR-RATIFIED 2026-08-19, supersedes decision 2. Settled; do
> not re-litigate.** Reason, in one line: **SSH-agent auth survives a missing git (decision 5) and
> the transport is not knowable at the toolbar** — it depends on the resolved upstream for the
> branch being acted on — so disabling Fetch/Pull/Push would break a working workflow for every SSH
> user, and a scheme-conditional disable is not implementable. Decision 2's actual requirement —
> *never a dead click* — is met by §10.2 instead. The reported three-toast symptom stays fixed
> because those toasts came from background auto-fetch retries, which remain silent.

### 10.1 The coalescing mechanism — `pushToast(tone, text, key?)`

`App.tsx`'s `pushToast` gains an optional third parameter and `Toast` gains `key?: string`. Rule,
applied inside the existing `setToasts` updater **before** the 5-toast cap logic:

```
push(tone, text, key?):
  if key is undefined            -> current behaviour, unchanged
  existing = cur.find(t => t.key === key)
  if existing exists AND existing.text === text   -> return cur   # NO-OP: no remount, no flicker
  if existing exists AND existing.text !== text   -> replace it IN PLACE
                                                     (same array index, NEW id, new text,
                                                      cancel/restart its auto-dismiss timer)
  otherwise                                       -> append as normal
```

`gitNotFound` toasts all use the key `'git-not-found'`. Nothing else in the app passes a key, so
this is inert for every existing call site.

Note the interaction with the existing toast model: **error toasts are `sticky`**
(`App.tsx:165`) — they persist until dismissed. Without this rule, three failed presses would leave
three permanent, identical toasts on screen, which is the exact shape of the symptom P70 exists to
kill.

### 10.2 The rule in behavioural terms

| Sequence | Result |
|---|---|
| **Fetch pressed 3× in a row** | Exactly **one** toast on screen, from the first press. Presses 2 and 3 are no-ops (same key, same text) — no stacking, no remount, **no flicker**, no re-announcement. |
| **Fetch fails, then Pull fails while the Fetch toast is still visible** | The Fetch toast is **replaced in place** by the Pull toast — never queued, never stacked. The visible message always names the operation the user pressed **last**, because that is the one they are waiting on. |
| **User dismisses the toast, then presses Fetch again** | The slot is free, so a fresh toast appears. The user is never permanently silenced. |
| **Background auto-fetch / scheduler failure** | **Nothing.** No toast, no key consumed, no interference with a visible user-initiated toast. It calls `noteGitNotFound()` only. |
| **A non-`gitNotFound` error arrives** | Unaffected — no key, appended normally, and it does not displace the `git-not-found` toast. |
| **Git becomes available again (successful Re-check)** | The success toast is a separate, keyless toast. Any lingering `git-not-found` toast is left alone; it is sticky and the user dismisses it. (Auto-clearing it was considered and rejected: silently removing an error the user may not have read yet is worse than one stale line.) |

Invariant senior-dev can test directly: **at most one toast with `key === 'git-not-found'` exists at
any moment**, regardless of press count or op mix.

### 10.3 Scope of the §7.4 suppression, restated

- **User-pressed** Fetch / Pull / Push / Fetch all / Clone rejecting with `gitNotFound`:
  call `noteGitNotFound()` **and** push the keyed toast (§5.6).
- **Background / scheduler-originated** failures: call `noteGitNotFound()` and push **nothing**.
- Toolbar buttons stay **enabled**. No new prop, no `RepoWorkspace.tsx` change, no
  `WorkspaceToolbar.tsx` change.

**SHOULD** (droppable if the increment runs long). The commit-search result surface, when a search
rejects with `gitNotFound`, renders its existing empty/error line as `Search needs Git — see the
notice at the top of the window.` instead of the generic failure text. Search has no SSH caveat —
it genuinely cannot work — so this one is unambiguous.

---

## 11. Harness states (`VITE_MOCK_IPC=1`) — all seams ratified

Implementation lives in `src/ipc/mock/handlers/gitEnv.ts`, spread into `src/ipc/mock.ts` beside
`updateHandlers`, except `?os=` which lives in `gitBannerCopy.ts`.

### 11.1 `?git=<value>` — read once at module init via the existing `query()` helper

`checkGitAvailability()` resolves the fixture below. Unknown values fall back to `default`.

| Value | Fixture |
|---|---|
| *(absent)* / unknown → `default` | `{ found: true, path: '/usr/bin/git', version: '2.47.1', source: 'path', detail: 'Git 2.47.1 — /usr/bin/git (path)' }` |
| `registry` | `{ found: true, path: 'C:\\Users\\dev\\AppData\\Local\\Programs\\Git\\cmd\\git.exe', version: '2.47.1.windows.1', source: 'registry', detail: 'Git 2.47.1.windows.1 — C:\\Users\\dev\\AppData\\Local\\Programs\\Git\\cmd\\git.exe (registry)' }` |
| `missing` | `{ found: false, path: null, version: null, source: 'fallback', detail: <the §3.3 Windows text, verbatim> }` — **and** `fetch` / `pull` / `push` / `fetchAll` reject with `{ kind: 'gitNotFound', message: <same text> }` |
| `badpath` | `{ found: false, path: 'C:\\Users\\dev\\AppData\\Local\\Programs\\Git\\cmd\\git.exe', version: null, source: 'override', detail: <the §3.3 Windows text, verbatim> }` — no remote rejections needed |
| `longpath` | `{ found: false, path: <LONG_PATH>, version: null, source: 'wellKnown', detail: <LONG_DETAIL> }` |

`LONG_PATH` — a single string literal, ≥250 chars, containing one ≥70-char segment so the
single-line ellipsis is provably exercised:

```
C:\Users\dev\AppData\Local\Programs\a-very-long-vendor-directory-name-used-to-prove-single-line-truncation\nested\deeper\even-deeper\still-going\almost-there\finally\here\Git\cmd\git.exe
```

`LONG_DETAIL` — the §3.3 Windows text, then `\n`, then `PATH: ` followed by twelve
semicolon-joined 60-character directory strings (≥900 chars total). Proves the technical block's
120px scroll and `overflow-wrap: anywhere`.

### 11.2 `?gitDelay=<ms>` — orthogonal, composable

Parsed once at module init: `Number.parseInt(query('gitDelay') ?? '', 10)`, `NaN` → `0`, clamped to
`[0, 10000]`. `checkGitAvailability()` awaits that many ms before resolving — the mount probe and
every `recheck()` alike. Composable with any `?git=` value (`?git=missing&gitDelay=1200`). This is
the only way to observe `Checking…` and the 400 ms floor in the harness.

### 11.3 `?os=windows|mac|linux` — mock-gated, in `gitBannerCopy.ts`

```
resolveOsFamily(): OsFamily
  if import.meta.env.VITE_MOCK_IPC is not '1' -> return osFamily
  read new URLSearchParams(window.location.search).get('os') inside try/catch
  accept exactly 'windows' | 'mac' | 'linux' -> return it
  anything else / throw -> return osFamily
```

Keeps `src/utils/platform.ts` free of harness seams and lets all three copy variants be verified
from one browser on one OS.

### 11.4 State → seam coverage (every §6 state is reachable)

| State | Seam |
|---|---|
| `status === null`, no layout shift | *(none)* — plus `?gitDelay=3000` to hold the state long enough to inspect |
| `found === true` | *(none)* and `?git=registry` |
| Variant A | `?git=missing` |
| Variant B (override) | `?git=badpath` |
| **Announcement matches the visible variant** | `?git=missing` **and** `?git=badpath`: read `.git-banner-announce`'s text and assert its first sentence equals `.git-banner-title` and its tail equals `.git-banner-remedy`. **This is the check that caught the §5.7 defect — run it for both variants, not just A.** |
| `checking === true` | `?git=missing&gitDelay=1200`, press **Re-check** |
| Re-check failed again | `?git=missing`, press **Re-check** — banner must not flicker; `Still not found — checked HH:MM.` appears; announcer becomes `Git is still not available.`; no toast |
| Disclosure open | any failing seam, press **Details** |
| Long content | `?git=longpath` |
| §10.2 row 1 (Fetch ×3) | `?git=missing`, press **Fetch** three times — exactly one toast, no stacking |
| §10.2 row 2 (Fetch then Pull) | `?git=missing`, press **Fetch**, then **Pull** — still one toast, text now names Pull |
| Copy variants per OS | `?git=missing&os=mac`, `&os=linux`, `&os=windows` |
| Both themes / narrow width | any failing seam + the header theme toggle + `resize_window` |
| Palette action | `Ctrl/Cmd-K` → `Check Git availability` under `?git=registry` (toast shows the registry detail) |

**Re-check succeeded (`false → true`) is NOT reachable in the harness** — the `?git=` value is
fixed at module init, so the mock can never flip. It is a USER CHECKPOINT item (§13).

---

## 12. Ratified decisions (2026-08-19) and their consequences

| # | Decision | Consequence in this file |
|---|---|---|
| D1 | `GitAvailability.path` is populated whenever a candidate resolved, regardless of `found`. **Approved.** | §3 Variant B is in scope and keys on `path !== null`. |
| D2 | Disable the remote-op toolbar buttons while git is missing. **Approved, then SUPERSEDED — the reversal in §10 is orchestrator-ratified 2026-08-19.** One-line reason: *SSH-agent auth survives a missing git (D5), and the transport is not knowable at the toolbar, so disabling would break working SSH workflows.* Settled; not to be re-litigated. | §10: buttons stay enabled; blanket suppression narrows to background failures; one coalesced toast per user press (§10.1/§10.2). Net effect: *less* code — no prop drill, no `RepoWorkspace.tsx` / `WorkspaceToolbar.tsx` change. |
| D3 | `osFamily` by user-agent in `src/utils/platform.ts`. **Approved.** | §2 file table; §11.3 keeps the `?os=` seam out of that file. |
| D4 | 400 ms minimum `checking` window · one success toast on a user-initiated `false → true` · probe kick when the latch fires at `status === null`. **All approved.** | §6 latch row, §7 pending rules, §5.6 toast string. |
| D5 | `acquire_cred` short-circuit narrowed to the credential-**helper** rung; SSH agent and default rungs still run. **Approved.** | §5.4 rewritten; §5.1/§5.2 explanation lines reworded; §4.1 severity rationale; §9 item 2; **§10 reversed**; §13 item 2 is now a blocking checkpoint. |
| D6 | Palette action `Check Git availability`. **Approved.** | §8. |
| D8 | `.file-chevron` added to the reduced-motion block. **Approved.** | §7 motion; ui-reference §9. |
| D9 | **Jargon ban is scoped to the collapsed plain-language copy**, not to the disclosure or the technical block. **Approved 2026-08-19** — resolves a self-contradiction between the old §5.6 and §§5.4/5.5, converged on independently by senior-dev and the reviewer. | §5.6's new "Jargon ban — scope" subsection. |
| D10 | **The live-region announcement must be variant-correct and carry the remedy.** Contract defect found during harness verification under `?git=badpath` (announcement was hard-coded to Variant A and contradicted the visible banner). Contract tightened + code fix routed to senior-dev, 2026-08-19. | New §5.7 (derivation rule, exact strings, required regression test); §7.1 a11y bullet; §11.4 verification row; §3 blanket derivation rule. |

### D5's blast radius — everything it changed beyond one line

1. **§5.4** — both capability rows rewritten. `fetch, pull and push` moved out of the "doesn't
   work" row entirely; HTTPS-with-helper named as the actual casualty; a new positive clause
   ("Remotes you connect to over SSH also keep working") added to the "still works" row, because
   silence there would still leave an SSH user assuming the worst.
2. **§5.1 / §5.2 explanation lines** — the old wording, "This is not a sign-in problem", became
   half-false: signing in to HTTPS remotes genuinely *is* affected. Replaced with `Your saved
   credentials are fine — Bonsai never got as far as checking them.`, which denies the
   wrong-password/expired-token reading (the actual misdiagnosis the user suffered) without
   claiming remote work is unaffected.
3. **§10 — reversed outright** (ratified). The one place where two decisions were in direct
   conflict; resolved in favour of not breaking SSH users.
4. **§5.6 / §10.1 / §11.4** — a new keyed toast string, the `pushToast` dedupe mechanism, and two
   new harness verification rows.
5. **§4.1 and §9** — the warning-not-danger severity argument now also rests on SSH auth surviving,
   which strengthens it.

---

## 13. USER CHECKPOINT items (native only — for the user's checklist)

These cannot be verified in the browser harness and must not be marked passed by the orchestrator.

1. **Re-check recovery (`false → true`).** The harness cannot flip the `?git=` value after module
   init. Native: launch with Git unresolvable, confirm the banner; install Git (or point
   `BONSAI_GIT_BIN` at a real one and relaunch), press **Re-check** → the banner disappears, the
   success toast reads `Git is available again — Bonsai found Git {version}.`, and a remote op
   works without restarting the app.

2. **🔴 BLOCKING — SSH-agent auth survives the banner.** With the banner showing, fetch or push
   against an **SSH** remote must **succeed**. This is the item that proves the §10 reversal correct
   and §5.4's "still works" row honest. **If it fails, the copy is wrong and must be corrected
   before release, not after** — §5.4's "still works" row and §10's enabled-buttons rule would both
   have to change, and decision 2's disabling would have to be reinstated. Do not ship on a red or
   an unrun result here.

3. **HTTPS-with-helper auth fails honestly.** Same state, an **HTTPS** remote → exactly one toast,
   correct wording, and **no** "no cached credentials" / "authentication failed" message anywhere.

4. **First paint is not delayed** and the healthy launch shows no flash or jump of the notice bar.

5. **Screen-reader announcement.** NVDA (Windows) / VoiceOver (macOS): the polite status is spoken
   once when the bar appears, it **matches the visible diagnosis and ends with the remedy** (§5.7 —
   verify under the Variant B / bad-`BONSAI_GIT_BIN` reproduction specifically, since that is where
   the mismatch was found), and the whole bar is *not* re-announced after a failed Re-check.

6. **Both themes on the real webview** — rail, glyph and text legibility in dark and light, plus a
   visible focus ring on `Re-check` when tabbed to.
