### 12.6 Identity in the header

- The identity control is the far-right item of `.header-toolbar` (§1) and reads the **effective**
  Git identity — `local` if set, otherwise `global` — and **names its source** in the menu. A
  local-only read is wrong: Git resolves local-over-global, so a repo with no local identity still
  commits fine, and a control that showed nothing there would be lying.
- Trigger: 32×32 button containing a 22px circle of **initials** (first letters of the first two
  name words, max 2). No identity → glyph `?` with a 1px `--warning` ring (**7.3:1** / **4.5:1**);
  the glyph and the accessible name, not the hue, carry the state. Loading → `·` + `aria-busy`.
  `aria-haspopup="menu"` + `aria-expanded`.
- Menu: a `ContextMenu` anchored with the house idiom (`rect.right`, `rect.bottom + 2`), a
  non-interactive `header` block stating name / email / source, then one row per saved identity with
  `checked` (⇒ `role="menuitemradio"`) and a `detail` second line, then `Manage identities…`.
- The menu owns its open state and lifts it via `onMenuOpenChange` (the `TabStrip.tsx:35-37`
  precedent) because App early-returns global shortcuts while a menu is open.
- **Writing an identity into a repo confirms only when it would overwrite a differing *local*
  value** — writing into an empty slot destroys nothing. The confirm names both identities and the
  consequence, uses `confirmVariant='primary'` (recoverable), and says
  `Commits you have already made are not changed.`
- P69 added **four** additive fields: `ContextMenuItem` gained `checked`, `detail` and `busy`, and
  `ContextMenuProps` gained `header` and `busy` (`ContextMenu.tsx:50` / `:72`). All are additive:
  absent ⇒ byte-identical rendering to before. The **check column belongs to the list, not the
  row** — it is reserved whenever any item declares `checked`, so plain rows in the same menu stay
  aligned with the labelled ones.

### 12.7 Forge accounts (P79/P80)

- **Provider display without color as sole carrier:** `ForgeProviderBadge` (2-letter monogram, GH /
  GL / BB / AZ / ??) + `ForgeAvatar` (image or login-initial monogram, 22px cozy / 20px compact).
  Both reuse `.identity-avatar` / `.pr-draft-tag` geometry; no hue carries meaning.
- **PR-panel account switcher (P80)** reuses the §12.6 identity-menu idiom exactly: a `ContextMenu`
  anchored `rect.right` / `rect.bottom + 2`, a non-interactive `header` block (`Accounts on {host}`,
  plus the no-default nudge line when applicable), one `checked` (`role="menuitemradio"`) row per
  account with a `detail` second line, then `Use host default` + `Add another account…`. The
  left group (avatar + login + host) is the trigger, shown as a button **only when the host has ≥2
  accounts** (no switcher chrome for a single account). Writes are optimistic + `busy`.
- **`AccountSource` label vocabulary** (canonical microcopy — do not reword per surface;
  `src/components/forgeAccountSource.ts`):

  | `accountSource` | header caption | tooltip |
  |---|---|---|
  | `override`    | `Pinned to this repo` | `Pinned to this repository. Other repositories on this host use the default.` |
  | `ownerMatch`  | `Matched by owner`    | `Chosen because its username matches this repository's owner.` |
  | `hostDefault` | `Host default`        | `The default account for this host.` |
  | `single`      | *(none)*              | *(none)* |
  | `none`        | *(none)*              | *(n/a — connect view shows)* |

- **Per-repo vs global semantics (P80):** the PR panel is **per-repo** — it pins/unpins the repo's
  account override (`Reset to host default` is nondestructive, no confirm) and never signs out.
  **Full sign-out (keychain token deletion via `forgeRemoveAccount`) lives only in Settings →
  Accounts**, always behind a danger `ConfirmDialog` that names the account and states pinned repos
  fall back. Never label a per-repo unpin "Disconnect" (it reads as sign-out).
- Connected-state chip: `● Connected` (`--success` dot, `--text-2` word) / `○ Token missing`
  (`--text-3` dot) — the word carries meaning, never the dot color alone.
- No new tokens were introduced for any forge-account surface; all classes are built from existing
  `--bg/-text/-border/-accent/-success/-warning/-danger` tokens.

### 12.8 Identity profile colors (P82)

- **Purpose:** an at-a-glance answer to "which identity is this repo on?", robust to duplicate
  labels. A curated 9-value palette (`ProfileColor`: `neutral` + 8 hues), never free-form hex.
  Deliberately separate from the semantic (`--success/--warning/--danger/--accent`) and graph
  (`--lane-*`) sets — an identity swatch must not read as a status or a branch lane.
- **Tokens (new, both themes).** Swatch/ring fills only — never used as text, never the sole carrier
  of meaning (always paired with the profile label or the avatar initials + accessible name):

  | `ProfileColor` | token | dark | light |
  |---|---|---|---|
  | `neutral` | `--profile-neutral` | `#6b7280` | `#8a919e` |
  | `slate`   | `--profile-slate`   | `#7d8aa3` | `#5d6b85` |
  | `blue`    | `--profile-blue`    | `#4f8cff` | `#2f6fe4` |
  | `teal`    | `--profile-teal`    | `#3ec6c0` | `#0f8f89` |
  | `green`   | `--profile-green`   | `#57ab5a` | `#1a7f37` |
  | `amber`   | `--profile-amber`   | `#e8c341` | `#9a6700` |
  | `orange`  | `--profile-orange`  | `#f2994a` | `#c2410c` |
  | `purple`  | `--profile-purple`  | `#9b6dff` | `#7c3aed` |
  | `pink`    | `--profile-pink`    | `#f26d9c` | `#c2266f` |

  **Contrast:** all nine ≥3:1 (non-text/graphics) against their panel background in both themes —
  dark vs `--bg-1 #1d2026` (min 3.4:1, neutral), light vs `#ffffff`/`--bg-1` (min 3.1:1, neutral).
  Each swatch also carries a 1px `--border` outline so its edge survives when a hue is close to the
  row background. No `-text` variants exist: profile text is always `--text-1`/`--text-2`.
- **Swatch primitive** `IdentityColorSwatch` (`src/components/IdentityColorSwatch.tsx`): a 10px
  (`size="sm"` 8px) circle, fill chosen by `.identity-swatch[data-profile-color='<c>']` CSS
  attribute selector — **no inline color, no hex in TSX**. `aria-hidden` everywhere except the
  picker (adjacent text is the accessible name).
- **Appears in:** the header avatar (2px hue ring when a non-neutral profile is matched; unset
  `?`+`--warning` ring keeps priority), identity-menu rows (reuses the existing `ContextMenuItem.icon`
  slot — no `ContextMenu` change), the menu header block (when the effective identity matches a
  profile), and the Settings profile card head (beside the title; the `in use` badge stays the
  textual "active" carrier).
- **Picker** `IdentityColorPicker` (`src/components/settings/IdentityColorPicker.tsx`): a
  `role="radiogroup"` of native `<input type="radio">` (the `SettingsSegmented` idiom, but a swatch
  grid — segmented is text-only and caps at 3). Nine ≥24px swatch cells; selected = 2px `--accent`
  ring + full-size dot; each radio's accessible name is the color name (`Neutral`…`Pink`). Duplicate
  hues across profiles are **allowed** (labels disambiguate). New catalog control type `'color'` on
  `SettingsIndexEntry`; catalog row `identities.profile-color` (`requires:'profile'`,
  `repeats:'perProfile'`).
- **Auto-distinct (UI layer, no persistence rewrite):** create-flow and the header save-as draft use
  `nextFreeHue(profiles)` (first unused hue in table order, wrap to least-used). Pre-P82 profiles
  (`color` absent) render a distinct **display-fallback** hue by array index (`ASSIGNABLE_COLORS[i%8]`);
  an explicit `neutral` is honoured as grey. The concrete color is written through the whole-array
  patch the moment the user touches the picker.
- **Motion:** only the ≤120ms selected-swatch grow/ring on the picker; collapses under
  `prefers-reduced-motion`.

### 12.9 PR actions — merge & close/decline (P83)

- **Footer action bar** `.pr-actions-bar`: the canonical pattern for a panel's terminal, commit-point
  actions. Pinned under the scrollable content region (e.g. `PrDetailView`), not in the header
  (header = navigation + metadata; actions read as a distinct commit-point). Full panel width, top
  `1px solid var(--border)`, `display:flex; justify-content:space-between; gap:8px`; padding
  `12px 16px` cozy / `8px 12px` compact. Buttons at the standard height (32px cozy / 28px compact;
  hit target ≥24px met on both densities). Rendered only while the item is actionable (PRs:
  `summary.state === 'open'`; a merged/closed PR shows its state pill and no bar).
- **Hierarchy:** exactly one primary + one quieter danger-secondary — no third button. Affirmative
  action on the right (`btn-primary`, label ends in `…` when a dialog follows, e.g. `Merge…`);
  the destructive/abandoning action on the far left (`.btn-secondary-danger`). This mirrors dialog
  button order (destructive-left, affirmative-right) so muscle memory transfers. The left label is
  per-context/per-forge (`Close` / `Decline` / `Abandon`); meaning is carried by the verb, never hue.
- **`.btn-secondary-danger`** recipe: a `btn-secondary` modifier whose base state is quiet
  (`--text-2` text, `--border`) so it does not compete with the primary, tinting text/border to
  `--danger` on `:hover`/`:focus-visible`. Built from `--danger` on `--bg-1` — the same pair as
  `btn-danger` text, which already clears ≥4.5:1 in both themes. Color is never the sole signal: the
  verb and the follow-up confirm dialog restate the consequence.
- **Form dialog pattern** `.pr-merge-card`: when a confirmation needs form fields (a picker + optional
  text + a checkbox) that the shared `ConfirmDialog` cannot host, build a dedicated dialog on the
  existing `.dialog-card` chrome — and that dialog *is* the confirmation (no second modal). Width
  420px (matches `.ai-consent-card`, the form-bearing-dialog precedent; the 360px default is too
  tight for a picker + fields). Same overlay, Esc, and overlay-click-cancels behaviour as
  `ConfirmDialog`. Structure top→bottom: `dialog-title` → lead summary paragraph (names consequence +
  irreversibility) → a method **radiogroup** (`SettingsSegmented`, `role="radiogroup"`, options
  filtered to `SUPPORTED_MERGE_METHODS[kind]`, with a one-line `.pr-merge-method-desc` in `--text-3`
  updated per selection) → optional commit title/message fields (`.pr-input` / `.pr-textarea`, shown
  only for methods that consume them) → a delete-source-branch checkbox (`.pr-draft-toggle` idiom,
  default OFF, **hidden when `kind === 'gitHub'`** since GitHub ignores it on merge) → `.dialog-buttons`
  Cancel + confirm. The merge confirm is `btn-primary` (constructive happy path; irreversibility is
  carried by the copy per house destructive-copy rules), busy label `Merging…` with `disabled` in
  flight.
- **Confirm-dialog reuse for close/decline:** the form-less destructive path reuses `ConfirmDialog`
  verbatim (`confirmVariant='danger'`) with per-forge title/label/body (Close / Decline / Abandon).
  No new dialog for it.
- **A11y (hard rules, consistent with the §12.x dialogs):** initial focus lands on **Cancel** in both
  the merge and close dialogs (a stray Enter never fires an irreversible action); Esc and
  overlay-click cancel; focus returns to the invoking bar button on close (focus restore). The merge
  dialog is `role="dialog" aria-modal="true"` labelled by its `dialog-title`; the method group is
  `role="radiogroup"` with an id-wired label; the checkbox is a real `<input type="checkbox">` +
  `<label>`. `aria-busy="true"` on the panel (`.pr-detail`) while an action is in flight, with the
  confirm button's busy label as the visible cue (no spinner-only state). Focus ring 2px `--accent`,
  1px offset, `:focus-visible` only. No new tokens; the only motion is the dialog's existing ≤150ms
  fade/scale-in, which already honours `prefers-reduced-motion`.
