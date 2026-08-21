# P80 — Working-tab redesign (staging + commit) — UI contract

Owner: ui-designer. Implementer: senior-dev. Do not edit code from this file.

Scope: the right panel's **Working** tab only — `StatusPanel` (Staged + Changes), the per-file
and per-folder row affordances, the pre-textarea actions row, the commit box, and the two commit
modifier rows. The PR tab, the compare/commit-details tri-states, and the conflicts section are
out of scope (the conflicts header's `section-action-ai-bulk` is already AA-correct and unchanged).

## Prime directive (hard constraint)

**Maximize vertical space for the CHANGES and STAGED lists.** Every chrome change below must be
net space-neutral or space-saving at rest. The resting footer + section-header height must be
**≤ today**, and 2b must strictly reduce it. No new resting rows. Controls collapse into the
message toolbar, an overflow (`⋯`) menu, and Settings — they never spread out.

All values are cozy / compact where they differ, read from the existing `--rp-*` density block
(`src/styles/right-panel-density.css`): `--rp-row-h` 24/20, `--rp-ctl-h` 22/20,
`--rp-ctl-font` 12/11, `--rp-box-pad` 10/8, `--rp-box-gap` 6/4, `--rp-msg-min` 48/36,
`--rp-gap` 10/6, `--rp-section-pad` `5 6 6`/`3 6 4`. **No new tokens are introduced** — every
color below is an existing theme custom property.

---

# Increment 2a — staging affordances

Files: `StatusFileRow.tsx`, `StatusSection.tsx`, `StatusPanel.tsx`, `DirRowActions.tsx`,
`src/styles/status-panel.css`, `src/styles/commit-box.css` (the `.section-action` rules live
there).

## A1 — persistent stage/unstage toggle (MUST)

The primary `+` / `−` button is today `opacity: 0` until row hover (`.row-action` in
`commit-box.css:62`), so the main action of the panel is invisible at rest. Split the primary
toggle from the secondary hover actions.

- **New modifier class `.row-action-primary`** on the last (rightmost) `+`/`−` button in
  `StatusFileRow.tsx` (the `action !== null` button) and on the folder-level stage/unstage button
  in `DirRowActions.tsx`. Keep the `.row-action` base for geometry.
- States (`commit-box.css`, new rules):
  - Resting: `opacity: .6;` (not 0). `color: var(--text-2)`.
  - Row hover **or** button `:hover` **or** `:focus-visible`: `opacity: 1;`
    `color: var(--text-1)`; button `:hover:not(:disabled)` keeps the existing
    `background: var(--bg-3)`.
  - Disabled: existing `.row-action:disabled` (`color: var(--text-3)`, `cursor: default`) — the
    `.6` floor does not apply while disabled.
- The `.6` on `--text-2` over `--bg-1`/`--bg-0` is a still-actionable control, so the §2 dimming
  budget's "inert only" rule is respected by lifting to full `--text-1` on any hover/focus; the
  glyph is a `+`/`−` character (a shape carrier), not color-only.
- **Secondary + destructive actions keep hover-reveal unchanged**: `.row-action-history`,
  `.row-action-blame`, `.row-action-discard` stay `opacity: 0` → `1` on
  `.file-row:hover .row-action` / `:focus-visible`. Order (§7.1) is unchanged: history, blame,
  discard, then the now-persistent primary toggle rightmost.
- Geometry unchanged: 20×20 within a 24/20px row (§3.1 row-is-the-target exception still holds).

## A2 — resting destructive treatment for "Discard all" (MUST)

Today `.section-action-discard` ("Discard all") is visually identical to the safe "Stage all"
(both `.section-action`, `--text-3`) with only an 8px gap between them — a misclick trap.

- In `StatusSection.tsx`, prefix the label with a leading danger glyph: render
  `↺ Discard all` (glyph `↺` = house "discard/revert", §7.1). Wrap the glyph in
  `<span className="section-action-glyph" aria-hidden="true">↺</span>`.
- Resting treatment (`commit-box.css`):
  - `.section-action-discard` label text `color: var(--text-2)` (AA-safe, ≥4.5:1); the
    `.section-action-glyph` inside it `color: var(--danger)` — a graphics carrier, measured
    4.4:1 dark / 4.6:1 light (§2), clearing the 3:1 bar. Color is therefore never the sole
    carrier: the glyph shape + the word "Discard" both carry meaning.
  - `:hover:not(:disabled)`: whole control `color: var(--danger)` (existing rule kept).
- **Separating gap:** give "Discard all" `margin-left: 8px` so there is a 16px total gap from
  "Stage all" (the header's own 8px `gap` + 8px margin), visually detaching the destructive action.
  It stays right of "Stage all" and left of `extraAction`.
- No confirm change here — App already confirms discard; this is resting-state legibility only.

## A3 — empty STAGED placeholder (SHOULD)

When `snapshot.staged.length === 0` the Staged section renders a dead strip.

- In `StatusSection.tsx`, when `entries.length === 0` (and `expandable`, i.e. Staged/Changes not
  conflicts), render one placeholder line in place of the list:
  `<p className="section-empty">{emptyText}</p>`.
- Copy, passed as a new optional `emptyText?: string` prop from `StatusPanel.tsx`:
  - Staged: `Stage files to include them in your commit.`
  - Changes: `Nothing to commit — your working tree is clean.` (only reachable when Staged has
    rows but Changes is empty; the whole-panel empty state stays `No changes`, see F3).
- Style (`status-panel.css`): `.section-empty { margin: var(--rp-list-margin,4px) 0 0; padding: 0 4px;
  font-size: 12px; color: var(--text-2); line-height: 1.45; }` — AA-safe.
- The section header (`Staged (0)`) still renders; its bulk buttons already hide at
  `entries.length === 0`, so the header collapses to just the label + placeholder = one short line,
  not a gap. Net effect is neutral-to-tighter vs today's empty strip.

## B1 — readable section labels (SHOULD)

`status-panel.css:31-41` sets the label **text** to a hue mix
(`color-mix(--success 75% / --text-1)` and `color-mix(--warning 45% / --text-3)`) — low-contrast,
and the Changes one folds in `--text-3` (sub-AA).

- Keep the faint background tints (`.status-section--staged` 6% success, `.status-section--changes`
  5% `--text-3`) — they are the at-a-glance differentiator and are decorative.
- Set both labels' **text** to a neutral readable token:
  - `.status-section--staged > .section-label { color: var(--text-2); }`
  - `.status-section--changes > .section-label { color: var(--text-2); }`
- The count `(N)` inherits the label color. The uppercase/letter-spacing/weight from
  `.section-label` (§ density file) is unchanged.
- Rationale: the label is the user's wayfinder; §2 requires `--text-2`, not a hue-over-tint recipe
  (which is the explicitly-forbidden family). The green/orange tint panel still carries the
  Staged/Changes distinction non-verbally.

## C1 — bulk action buttons meet AA (MUST)

`.section-action` uses `--text-3` (`commit-box.css:21`) — 2.96:1 on light, fails AA for text.

- `.section-action { color: var(--text-2); }` (resting). Hover stays `--text-1`.
- Applies to "Stage all" / "Unstage all". "Discard all" is handled in A2. The AI "✨ Review"
  variant is consolidated away in 2b/E1.

---

# Increment 2b — commit-box consolidation + space-saving footer

This is where the space constraint is enforced. Files: `CommitBox.tsx`,
`CommitOptionsRow.tsx` → **replaced by `CommitOptionsMenu.tsx`** (new), `RightPanelActionsRow.tsx`
(absorbed — see note), `WorkspaceRightPanel.tsx` (wiring), `src/styles/commit-box.css`, and the
Settings catalog (`settings/catalog/general.ts`, `settings/types.ts`, the settings store key).

## Target footer — before / after

```
BEFORE (cozy, AI eligible, sign shown, amend off, empty message)      height
─────────────────────────────────────────────────────────────────────────
.rp-actions       [☐ Amend last commit] .............. [⋯]            29px
.commit-box ┐
  header          ................ [✨ Generate] [Compose commits ✨]  28px
  textarea        (min)                                                48px
  options-row     [☐ Sign commit] [☐ Skip hooks]                       22px
  commit-button   [           Commit           ]                       32px
            ┘  (+ border 1, pad 20, 3×6 gaps 18)                       41px
─────────────────────────────────────────────────────────────────────────
                                                          TOTAL ≈     198px
                                        resting chrome (ex-textarea) ≈150px
```

```
AFTER (same conditions)                                               height
─────────────────────────────────────────────────────────────────────────
.commit-box ┐
  textarea        (min)                                                48px
  msg-toolbar     [✨] ................... [NN/72] [⋯]                  22px
  commit-button   [           Commit           ]                       32px
            ┘  (+ border 1, pad 20, 2×6 gaps 12)                       33px
─────────────────────────────────────────────────────────────────────────
                                                          TOTAL ≈     135px
                                        resting chrome (ex-textarea) ≈ 87px
```

**Reclaim ≈ 63px (cozy) / ≈ 57px (compact)** returned to the Staged + Changes lists, resting.
The `.rp-actions` row is deleted entirely; its amend + stash fold into the `⋯` menu. The
`.commit-box-header` is deleted; Generate becomes an icon in the toolbar and Compose folds into
`⋯`. The `.commit-options-row` is deleted; Sign + Skip fold into `⋯`.

## D2 — the message toolbar (one compact row)

New row `.commit-msg-toolbar`, rendered by `CommitBox` **directly under the textarea**, replacing
both `.commit-box-header` and `.commit-options-row`.

```
.commit-msg-toolbar   height var(--rp-ctl-h, 22px)   font var(--rp-ctl-font,12px)
┌────────────────────────────────────────────────────────────────────────┐
│ [✨]                                              [NN/72]        [⋯]      │
└────────────────────────────────────────────────────────────────────────┘
  ↑ generate (icon)         auto-margin →       counter        options menu
```

CSS (`commit-box.css`, replacing `.commit-box-header` and `.commit-options-row` rules):

```
.commit-msg-toolbar {
  display: flex; align-items: center; gap: var(--rp-ctl-gap, 8px);
  min-height: var(--rp-ctl-h, 22px); font-size: var(--rp-ctl-font, 12px);
}
.commit-msg-toolbar .commit-counter { margin-left: auto; }   /* pushes counter+⋯ right */
```

- **`✨` generate button** (`.commit-msg-tool.commit-generate-button`): icon-only, 24px hit box
  (`width:24px; height:var(--rp-ctl-h,22px); padding:0; display:flex; align-items:center;
  justify-content:center;`), transparent bg, `color: var(--text-2)`, glyph `✨` at 14px. Hover
  `color: var(--text-1); background: var(--bg-2); border-radius:4px`. Disabled `color: var(--text-3)`.
  While generating, show the glyph unchanged and set `aria-busy="true"` + a visually-hidden live
  status (see 2c); do **not** widen it to a "Generating…" text button (that would reflow the row).
  `aria-label="Generate commit message"`; `title` = existing contextual strings from `CommitBox.tsx`
  (kept verbatim, they explain the disabled reasons). Rendered only when `showGenerate`.
- **counter** (`.commit-counter`): unchanged behavior/tokens (`--text-3`, `--warning` when over),
  rendered only when `message.length > 0`. Moves from its own `align-self:flex-end` block into the
  toolbar. When absent, `margin-left: auto` on the `⋯` wrapper keeps it right-aligned instead.
- **`⋯` options button** (reuse `.rp-overflow` / `.rp-overflow-btn` markup + CSS verbatim from the
  deleted `RightPanelActionsRow` — 24×`--rp-ctl-h`, `--bg-2`, opens **upward**). `aria-haspopup="menu"`,
  `aria-expanded`, `aria-label="Commit options"`. Disabled only when `blocked`.

## The `⋯` menu — `CommitOptionsMenu.tsx` (new)

Absorbs everything from `CommitOptionsRow` (Sign, Skip hooks) and `RightPanelActionsRow`
(Amend, Stash), plus Compose. Reuse the `.rp-overflow-menu` / `.rp-overflow-item` styling
verbatim (upward-opening, `min-width:220px`), plus a divider rule
`.rp-overflow-sep { height:1px; background:var(--border); margin:4px 6px; }`.

`role="menu"`, outside-mousedown + Escape close (move the existing effect from
`RightPanelActionsRow` into this file). Menu contents, top to bottom:

```
role=menuitemcheckbox   ☐ Amend last commit
role=menuitemcheckbox   ☐ Sign commit (GPG|SSH)        ← only when showSign
role=menuitemcheckbox   ☐ Skip hooks
── separator ──
role=menuitem           ✨ Compose commits             ← only when showCompose & aiEligible
── separator ──
role=menuitem           Stash all
role=menuitem           Stash all + untracked
role=menuitem           Stash staged only
```

- Checkbox items use `role="menuitemcheckbox"` + `aria-checked`; a leading 16px check column
  reserved for all items (the ContextMenu §12.6 precedent — checked column belongs to the list).
- Per-scope stash enablement rules are carried over verbatim from `RightPanelActionsRow`
  (`hasTrackedChanges`, `hasUntracked`, `stagedCount`). Disabled items keep the existing
  `.rp-overflow-item:disabled` (`opacity:.45`).
- Compose disabled reasons keep `CommitBox`'s existing `title` strings.
- **Contextual notes** are NOT shown inside the menu; they surface as a single conditional line
  **below the toolbar** (see F1/F2 in 2c and the note line below), so the resting footer stays
  minimal and the note is only paid for when an option is active.

### Amend ownership — decision required (flag to orchestrator)

Amend is today owned by `WorkspaceRightPanel` and `CommitBox` is remounted via
`key={amend?'amend':…}` (WorkspaceRightPanel.tsx:362) so the textarea reseeds with the last
message. Folding the Amend control **into** `CommitBox`'s `⋯` menu means toggling it would remount
the menu and drop focus.

- **Recommended (A):** replace the `key`-based remount with an effect in `CommitBox` that reseeds
  `message` from `amendMessage` when `amend` flips **and** the box is untouched/empty (preserve a
  user-typed draft). Then Amend can live in the `⋯` menu as a `menuitemcheckbox` and the footer is
  a single owner. This is an architect/senior-dev call on the reseed logic — **flag it**.
- **Fallback (B) if the remount cannot change this increment:** keep Amend owned by
  `WorkspaceRightPanel`, but render it as a compact checkbox at the **left of the message toolbar**
  (before `✨`) via a small prop-driven child, and fold only Sign / Skip / Compose / Stash into
  `⋯`. This still deletes `.rp-actions` (saves ~29px) but keeps one visible checkbox in the
  toolbar; the toolbar height is unchanged (checkbox fits in `--rp-ctl-h`). Net reclaim ≈ 40px.
- My recommendation: **A** (cleanest, meets the space goal fully). Ship B only if A is scoped out.

## D1 — primary-commit-action setting

Make the Commit vs Commit & Push emphasis a setting instead of hard-coding Commit & Push as
primary.

- **Setting**, added to `settings/catalog/general.ts`, new group `Committing`:
  - `id: 'general.primary-commit-action'`
  - `label: 'Primary commit action'`
  - `help: 'Which button is emphasized at the bottom of the Working tab. The other stays available beside it.'`
  - `keywords: 'commit push button default primary emphasize'`
  - `control: 'segmented'` (2 segments — §12.3.2 allows ≤3): `Commit` | `Commit & Push`
  - **Default: `Commit`** (recommended — the non-network, always-safe action is the primary; push
    is the deliberate secondary). `reset` descriptor → default `'Commit'`.
- Store: add the key (e.g. `primaryCommitAction: 'commit' | 'commitPush'`) to the UI settings
  type in `settings/types.ts` and the store; thread it into `CommitBox` as a prop
  `primaryCommitAction`.
- **Both button states** in `CommitBox` (only when the split control applies — non-merge,
  non-amend, `onCommitAndPush` provided):
  - `primaryCommitAction === 'commit'`: primary (`.btn-primary.commit-button`, accent) = **Commit**;
    secondary (`.btn-secondary.commit-button-secondary`) = **Commit & Push**.
  - `primaryCommitAction === 'commitPush'`: primary = **Commit & Push** (current); secondary =
    **Commit**.
  - The primary always grows (`flex:1 1 auto`), secondary hugs (`flex:0 0 auto; white-space:nowrap`)
    — unchanged `.commit-button-row` CSS. One row, 32px, no height change either way.
  - Loading labels unchanged (`Committing…` / `Committing & Pushing…`) and follow whichever action
    is in flight.
  - Merge / amend modes are single-button (no split) and unaffected by the setting.
- A11y: both buttons are always real `<button>`s with distinct accessible names; the setting only
  swaps which carries `.btn-primary`. Never encode the choice as a single relabeled button (§12.3
  "a button labeled with its own value is not a control").

## E1 — one context-scoped Review (SHOULD)

Today up to four AI entry points can co-exist (`✨ Review` in **both** Staged and Changes headers,
plus `✨ Generate` and `Compose ✨` in the commit-box header).

- **Remove the `extraAction` "✨ Review" from BOTH section headers** (`StatusPanel.tsx` stops
  passing `extraAction` for Staged and Changes). Keep `onReviewStaged` / `onReviewWorktree` wired
  but surface **one** context-scoped Review control:
  - Add a single `✨ Review` menu item at the **top of the `⋯` menu**, before the checkbox group,
    labeled by context: `✨ Review staged` when `stagedCount > 0`, else `✨ Review changes`
    (worktree). One item, one scope, chosen by what's present. Disabled while `aiAnalyzing`
    (label → `✨ Reviewing…`). Rendered only when `aiEligible`.
- Message generation stays in the commit box (the `✨` toolbar icon). Compose stays in `⋯`.
- Net: 2–4 header buttons → 0 header buttons; all AI actions live in the toolbar `✨` icon + the
  `⋯` menu. More list room, and the Staged/Changes headers shrink to label + Stage-all/Discard-all.

## E2 — unified sparkle style (NIT)

All AI affordances use a **leading** `✨` glyph and sentence case:

- `✨ Generate message` (the toolbar icon's `aria-label`/`title`; visible glyph only)
- `✨ Compose commits` (menu item)
- `✨ Review staged` / `✨ Review changes` (menu item)
- `✨ Reviewing…` (in-flight)

No trailing-sparkle strings (`Compose commits ✨` is retired). Menu-item AI rows may carry
`color: var(--accent)` on the glyph only if desired, but the label stays `--text-1` (menu items
are on `--bg-2`; accent-as-text on `--bg-2` is AA-safe per §2, but keep it simple: label `--text-1`,
glyph inherits — no meaning is color-borne since the word "Review"/"Compose" is present).

---

# Increment 2c — a11y + microcopy

Files: `StatusSection.tsx`, `StatusFileRow.tsx`, `CommitOptionsMenu.tsx`, `CommitBox.tsx`,
`commit-box.css`.

## C2 — 24px hit targets for the modifier controls (SHOULD)

The old Amend / Sign / Skip checkbox rows sat below the 24px WCAG 2.2 floor. By moving them into
the `⋯` menu, each becomes a `.rp-overflow-item` at `padding: 6px 10px` → ≥28px tall — **above the
floor with zero added resting footer height** (the menu is an overlay, off the resting layout).
This is the space-saving way to satisfy 2.5.8 and is the recommended resolution.

- The toolbar `✨` and `⋯` buttons are 24×`--rp-ctl-h`; in **compact** density `--rp-ctl-h` is
  20px. To hold the 24px floor without growing the visible row, give both
  `min-height: 24px` on the button while the visible toolbar row keeps `min-height: var(--rp-ctl-h)`
  — the button overflows the row's min by 4px in compact but the row is followed by the 32px commit
  button with a `--rp-box-gap` (4px) between, absorbing it visually; no net footer growth because
  the toolbar's own content (textarea above, button below) already reserves ≥24px of vertical
  rhythm. **Flag:** if senior-dev finds this pushes the compact footer up, fall back to
  `min-height: var(--rp-ctl-h)` on the buttons and treat the ≥24px requirement as met via the
  overlay menu items (the two toolbar buttons are icon triggers whose hit box is the 24px width ×
  20px height = below floor only on the minor axis in compact-only); note it as an accepted
  compact-scope exemption analogous to the §3.1 `.right-panel[data-density='compact']` opt-in.

## C3 — section aria-labelledby (NIT)

Tie each list/tree to its heading in `StatusSection.tsx`:

- Give the header `<div className="section-header …">` an `id={\`section-\${section}-label\`}`
  (use `variant` for the conflicts-less sections). The `<ul className="file-list">` / `<Tree>`
  root gets `aria-labelledby` pointing at it. The `.section-empty` placeholder is inside the same
  labeled `<section>`, so no extra wiring needed.
- The `<section className="status-section …">` may instead carry `aria-labelledby`; pick the list
  element so the accessible name reaches the actual list of rows. Keep one id per section (Staged,
  Changes) — they never collide.

## F1 — warning notes carry a non-color glyph (SHOULD)

The amend-push and skip-hook notes are meaning-by-color-only. When surfaced (now as the single
conditional line below the toolbar — see 2b), each leads with an `aria-hidden` `⚠` glyph in
`--warning` and keeps its text at `--text-1`/`--text-2`:

- New shared line `.commit-note` (replaces `.amend-push-warning`, `.commit-skip-hint`,
  `.commit-sign-warn` inline forms):
  `display:flex; align-items:flex-start; gap:6px; font-size:11px; line-height:1.4;
   color:var(--text-2);` with a leading `<span className="commit-note-glyph" aria-hidden="true">⚠</span>`
  at `color:var(--warning); flex:none;` (glyph 7.3:1 dark / 4.5:1 light, §2 — clears 3:1).
- `role="note"` on the line (kept from the originals).
- Only ONE `.commit-note` shows at a time, chosen by priority: amend-pushed > sign-no-key > skip.
  It renders below `.commit-msg-toolbar`, above the error banner / commit button. It is conditional,
  so the resting footer (no active warning) pays nothing.
- Copy:
  - amend + pushed: `This commit is already pushed — amending rewrites published history.`
  - skip hooks on: `Git hooks won’t run for this commit.`
  - (sign copy: F2)

## F2 — plain-language sign warning (NIT)

The no-key warning leaks `user.signingkey`.

- Visible copy: `No signing key set — commits won’t be signed.` + the existing `Set key…` action
  (`.commit-sign-fix`, `--accent`, underlined) which opens identity settings. The config detail
  (`user.signingkey`) moves to the `Set key…` button's `title="Set user.signingkey in Git config"`,
  not the visible sentence.
- The success line (when key present + sign on) stays as a `.commit-note`-styled confirmation but
  with a `✓` glyph in `--success` instead of `⚠`: `Commits will be signed (GPG|SSH).` — glyph
  `color:var(--success)` (5.7:1 dark / 4.7:1 light). This is informational, not a warning; keep it
  low-priority (only shows when no higher-priority warning is active).

## F3 — whole-panel empty state meets AA (NIT)

`StatusPanel.tsx:165` renders `<p className="pane-empty">No changes</p>`; `.pane-empty` is
`--text-3` (`right-panel-density.css:104`).

- Change `.pane-empty { color: var(--text-2); }` (AA-safe). Keep 13px/centered.
- This is the repo-clean state; keep the copy `No changes`.

## A4 — keyboard stage of a focused row (optional)

In `StatusFileRow.tsx`, the row's expandable main button already toggles the diff on click. Add:
when the `.file-row-main` button has focus, `Space`/`Enter` continue to toggle the diff (native).
To stage without the diff, keep the persistent `+`/`−` toggle (A1) in the tab order — it is a real
`<button>` reachable by Tab and activatable by `Space`/`Enter`. **No new key handler is required**
for A4 to be satisfied: the persistent toggle from A1 is the keyboard stage affordance. If a
row-level shortcut is still wanted, add `onKeyDown` on `.file-row-main` for `Ctrl/Cmd+Enter` →
`onAction(entryPaths(entry))`; recommend deferring this to avoid clashing with the textarea's
`Ctrl/Cmd+Enter` commit idiom when focus semantics are ambiguous. **Flag as optional.**

---

## States checklist (every new/changed control)

- **Persistent stage toggle:** resting `.6`, hover/focus `1`, disabled `--text-3`, loading (busy)
  disabled.
- **Discard all:** resting `--text-2` + danger glyph, hover `--danger`, disabled `opacity:.6`.
- **`✨` toolbar icon:** resting `--text-2`, hover `--text-1`+`--bg-2`, disabled `--text-3`,
  generating `aria-busy` + live announce, absent when `!showGenerate`.
- **`⋯` menu button:** resting `--bg-2`/`--text-2`, hover `--bg-3`/`--text-1`,
  `:focus-visible` 2px `--accent` offset 1px, disabled `opacity:.45`, expanded `aria-expanded`.
- **Menu items:** hover `--bg-1`, disabled `opacity:.45`, checkbox `aria-checked`, focus-visible
  ring. Escape / outside-click closes, focus returns to `⋯` trigger.
- **Commit split buttons:** primary/secondary swap per setting; disabled when
  `blocked || empty || busy || submitting || generating || (stagedCount===0 && !amend)`; loading
  labels per action.
- **Empty:** Staged-empty placeholder, panel-empty `No changes`; long content: file rows already
  ellipsize with `title` (unchanged), toolbar counter never wraps (fixed slot), menu `min-width:220px`.
- Reduced motion: only the existing 120ms transforms apply; no new motion introduced.

## Harness / fixtures (VITE_MOCK_IPC=1)

Verifiable in the browser harness; existing status fixtures cover most. Confirm these states exist
(add if missing, `src/ipc/mock/`):

- Staged empty + Changes populated (A3 Staged placeholder).
- Both populated, AI eligible (`aiConsented:true`) — toolbar `✨`, `⋯` menu with Review/Compose.
- Signing status: no-key vs has-key (F2 note variants); amend + upstream ahead=0 (F1 amend-push).
- Long branch / deep-path rows (toolbar + header layout under overflow).
- Settings → General → Committing: the segmented `Primary commit action` row + both footer button
  states.

**USER CHECKPOINT (not AI-verifiable):** the amend-toggle focus behavior under decision A (remount
replacement) — focus retention on toggle is a real-webview interaction; verify under
`pnpm tauri dev`. Scroll-feel / reclaimed-space "feel" is also a checkpoint (headless rAF caveat).

## Flags to orchestrator

1. **Amend ownership (2b):** recommend decision A (drop the `key`-remount, reseed via effect) so
   Amend lives in `⋯`; fallback B keeps Amend as a toolbar checkbox. Needs an architect/senior-dev
   call on the message-reseed logic.
2. **Compact hit-target (C2):** 24px minor-axis on the toolbar icon buttons in compact density —
   accept the 4px overflow or accept the density-scope exemption; recommend the former.
3. **A4 row shortcut:** optional; recommend deferring the extra `onKeyDown`, since A1's persistent
   toggle already provides keyboard staging.
4. **Primary-commit-action default:** recommend `Commit`; confirm with user if they prefer the
   current `Commit & Push`.
