# P82 — Color-coded git identity profiles (UI contract)

Owner: ui-designer. Input: `docs/contracts/P82-color-profiles.md` (data/IPC). This contract owns the
visual language: the palette tokens, where the swatch appears, the picker control, and a11y.

**User decision in force:** colors are **auto-distinct on upgrade** (architect Option B behaviour,
applied in the UI layer, not in persistence) — each existing profile that still reads `neutral` is
shown with a *deterministic distinct hue derived from its position*, and the create-flow auto-picks
the next free hue. See §6.

---

## 1. Palette tokens (new — added to `ui-reference.md` §12.8, both themes)

Nine identity swatch tokens. These are **identity** colors, deliberately separate from the semantic
set (`--success`/`--warning`/`--danger`/`--accent`) and from the graph `--lane-*` set — an identity
swatch must never be mistaken for a status or a branch lane.

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

**Contrast.** Each is used only as a **filled swatch/ring**, never as text and never as the sole
carrier of meaning (§4). All nine are vetted ≥3:1 (WCAG non-text/graphics) against the panel
backgrounds they render on — dark against `--bg-1 #1d2026` (all ≥3.4:1; neutral 3.4, blue 5.7) and
light against `#ffffff`/`--bg-1 #f6f7f9` (all ≥3.1:1; neutral 3.2, blue 4.6). No swatch is required
to hit 4.5:1 because no swatch carries text.

Do **not** add per-token "-text" variants — the label is always real DOM text in `--text-1`/`--text-2`,
never text painted in a profile hue.

---

## 2. Swatch primitive — new file `src/components/IdentityColorSwatch.tsx`

One tiny presentational component reused by every surface. Nothing existing renders a
color-token dot, so this is a new (deliberately ~30-line) primitive.

- Renders `<span class="identity-swatch" data-profile-color={color} aria-hidden="true" />`.
  The fill is chosen in CSS by attribute selector (`.identity-swatch[data-profile-color='blue']
  { background: var(--profile-blue); }`) — **no inline `style` color, no hardcoded hex in TSX.**
- Geometry: **10px** circle, `border-radius: 50%`, `flex: none`. A **1px inset ring** in
  `--bg-1` (`box-shadow: inset 0 0 0 1px color-mix(...)` is over-engineering — use a
  `border: 1px solid var(--border)`) so a light swatch stays crisp on a light row and a dark one on
  a dark row.
- `size` prop optional: `'sm'` = 8px (menu rows / compact), default 10px. No other sizes.
- Never focusable, never carries the accessible name — see §4.

Helper — new file `src/components/identityProfileColor.ts`:
- `PROFILE_COLORS: readonly ProfileColor[]` — canonical order = the §1 table order
  (`neutral, slate, blue, teal, green, amber, orange, purple, pink`).
- `ASSIGNABLE_COLORS` = `PROFILE_COLORS` minus `neutral` (the 8 hues, in table order).
- `profileColorLabel(c): string` — `'Neutral' | 'Slate' | …` (Title Case, for ARIA/SR).
- `resolveProfileColor(p): ProfileColor` — `p.color ?? 'neutral'` (single read-through helper so
  `undefined` → `neutral` is defined in exactly one place).
- `nextFreeHue(profiles): ProfileColor` and `autoDistinctColors(profiles)` — see §6.

---

## 3. Where color appears (the three surfaces + the menu header)

### 3.1 Header active-profile indicator — `src/components/IdentityAvatar.tsx` (edit)

The avatar is the always-visible answer to "which identity is this repo on?" Today it is an
initials circle with a `--warning` ring only for the unset state.

- When a profile is **matched** (`matchedProfile !== null`) and its resolved color is **not
  neutral**, wrap the 22px initials circle in a **2px ring** of the profile hue:
  `data-profile-color` on `.identity-avatar`, styled `box-shadow: 0 0 0 2px var(--profile-<c>)`
  (ring sits *outside* the 22px circle, inside the 32px button — no layout shift).
- Neutral / no match / unset: **no** hue ring. The existing `?`+`--warning` unset ring keeps
  priority and is unchanged — an unset identity never shows a profile hue (there is no profile).
- The initials remain the primary signal; the ring is redundant reinforcement. Color is **not** the
  sole carrier (initials + accessible name already differentiate). No copy change; `copy.ariaLabel`
  already names the identity. Do **not** append the color name to the header aria-label — it would
  be noise on the one control the user reads dozens of times a day.

### 3.2 Identity menu rows — `src/components/IdentityMenu.tsx` (edit)

Reuse the **existing** `ContextMenuItem.icon` slot (leading 16×16 column) — **no change to the
shared `ContextMenu` primitive.** Set `icon: <IdentityColorSwatch color={resolveProfileColor(p)} size="sm" />`
on each profile row. The swatch sits left of the label; the `checked` ✓ column and the `detail`
line are unchanged. A neutral profile shows the neutral (grey) swatch — every row has a swatch, so
the column never looks ragged.

The menu `header` block (`IdentityMenuHeader`) is the *effective* identity, which may match a
profile: when `matched !== null`, prepend an `IdentityColorSwatch` (10px) before the
`identity-menu-name` line. When unmatched, no swatch. Purely visual; the header stays
`role="presentation"`.

### 3.3 Settings profile card — `src/components/settings/IdentityProfileCard.tsx` (edit)

In `.settings-profile-head` (line ~129), place a 10px `IdentityColorSwatch` **before** the title
`span`. The `in use` badge (line ~133) is unchanged and remains the textual carrier of "active".
The card also gains the color **picker** row (§5).

---

## 4. Accessibility — hard rules (senior-dev MUST NOT violate)

1. **Color is never the sole carrier.** Every swatch is accompanied by real text — the profile
   label (card, menu row, menu header) or the initials + aria-label (avatar). The picker options
   each carry an SR-only / visible color name.
2. **Swatches are decorative where text already names the thing.** `IdentityColorSwatch` renders
   `aria-hidden="true"` in the avatar, menu rows, menu header, and card head — the adjacent text is
   the accessible name; a second announcement of "blue" there would be chatter.
3. **The picker is the one place the color name IS announced** (§5): each radio's accessible name is
   exactly the color name (`profileColorLabel`), so a screen-reader user can both perceive and set
   the value.
4. **Contrast:** all nine tokens ≥3:1 vs their panel background in both themes (§1). The 1px
   `--border` swatch outline guarantees an edge even when a hue is close to the row background.
5. **Focus ring:** the picker uses the house `:focus-visible` ring (2px `--accent`, 1px offset) on
   the focused swatch — never remove the outline on the native radios.
6. **Hit target:** each picker swatch button is ≥24px (§5), even though the painted dot is smaller.
7. **`prefers-reduced-motion`:** the only motion is the ≤120ms selected-swatch scale/ring
   transition (§5) — must collapse to none under reduced-motion (the app's global rule already
   covers `.identity-*`; confirm the new classes are inside it).

---

## 5. Color picker — new file `src/components/settings/IdentityColorPicker.tsx`

A compact 9-swatch chooser inside the profile card. The `SettingsSegmented` control is text-only and
capped at 3 options (`SettingsSegmented.tsx:37`), so a swatch grid is a distinct control — but it
copies that component's proven idiom exactly: a `role="radiogroup"` of **native
`<input type="radio">`**, so arrow-key nav, roving focus, `aria-checked` and
`getByRole('radio',{name})` all come for free.

### Placement & geometry

A new stamped catalog row `identities.profile-color` (see §7) rendered as a `SettingsRow`
(`stacked`) between `identities.profile-signing-key` and the action cell. The control is the swatch
grid:

```
Color
[N][S][B][T][G][A][O][P][K]      ← 9 swatch buttons, one row, wraps if narrow
 └ selected: N   (2px --accent ring + hue)
```

- Grid: `display: flex; flex-wrap: wrap; gap: 8px;` (cozy) / `gap: 6px` (compact).
- Each option = a `<label class="identity-swatch-option">` wrapping a visually-hidden native radio +
  a painted swatch span. **Hit target ≥24px** (24×24 label), painted dot 16px centered.
- Selected: 2px `--accent` ring around the 24px cell + the dot at full size; unselected dots sit at
  14px and grow to 16px on selection (≤120ms ease-out transform, reduced-motion → none).
- Hover (unselected): 1px `--border` → `--text-3` cell outline. No color change to the dot.

### States

- **Default/unselected**, **hover**, **selected** (accent ring), **`:focus-visible`** (accent ring
  already present on selected; focus adds the standard 1px-offset outline so keyboard focus ≠
  selection are distinguishable), **disabled** (whole card is never disabled here; no disabled
  state needed — omit).
- No loading/empty/error states — the value is always one of nine and always present after §6.

### A11y / keyboard

- `role="radiogroup"`, `aria-labelledby={rowId-label}` (the `Color` row label).
- Each radio's accessible name = `profileColorLabel(c)` (`'Neutral'`, `'Slate'`, …). Provide it via
  a visually-hidden `<span class="sr-only">` inside the label (the swatch itself is
  `aria-hidden`), matching the pattern that keeps `SettingsSegmented` labels real text.
- Arrow keys move+select within the group (native radio behaviour); Tab enters/leaves at the
  checked option; Space/Enter is a no-op-beyond-native. Esc does nothing special (card-level Esc
  only cancels the delete confirm, unchanged).
- Two profiles picking the **same hue is ALLOWED** (labels always disambiguate; forbidding it would
  be surprising and there are only 8 hues for N profiles). No warning, no nudge, no block — the
  auto-distinct assignment (§6) merely makes *collisions rare by default*, it does not enforce
  uniqueness.

### Microcopy

- Row label: `Color`
- Row help: `Shown as a dot on this identity everywhere it appears, so profiles with the same name stay easy to tell apart.`
- Each swatch title/aria-name: the color name (`Neutral`, `Slate`, `Blue`, `Teal`, `Green`,
  `Amber`, `Orange`, `Purple`, `Pink`).

---

## 6. Auto-distinct assignment (deterministic, UI layer)

Per the user decision, existing profiles must look distinct immediately on upgrade, and creation
picks the next free hue.

- **`nextFreeHue(profiles)`** — return the first color in `ASSIGNABLE_COLORS` (table order, `neutral`
  excluded) whose token is not already used by any profile's resolved color; if all 8 are taken,
  wrap to the least-used, ties broken by table order. Used by:
  - the create flow (`identities.add` / `Add identity`), and
  - the header menu's `saveEffectiveAsProfile` draft (`IdentityMenu.tsx:210-220`) — set
    `color: nextFreeHue(profiles)` on the draft instead of leaving it neutral.
- **`autoDistinctColors(profiles)`** — map profiles that still read `neutral` (i.e. pre-P82
  persisted, `color === undefined`) to distinct hues **by array index**: profile *i* →
  `ASSIGNABLE_COLORS[i % 8]`. This is applied **at render/read time as a display fallback** — it
  does **not** rewrite persisted state (matching the architect's "no rewrite on load" rule). A
  profile the user *deliberately sets* to neutral (an explicit `color: 'neutral'`) is honoured and
  shows grey; only `undefined` gets the auto hue. So `resolveProfileColor` used on a raw list yields
  neutral, while the card/menu/avatar consume the `autoDistinctColors`-mapped list.

  **Flagged ambiguity → recommendation:** applying auto-distinct as a *display fallback* keeps the
  data pure and reversible but means the hue only "sticks" once the user edits the profile (any save
  writes the concrete color through the whole-array patch). Alternative: normalize-on-first-edit
  writes all auto hues to disk the first time any profile is saved. **Recommended: display-fallback
  only** (above) — it never mutates data behind the user's back and needs zero new persistence code;
  the concrete color is written the moment they touch the picker. If the orchestrator wants the hues
  persisted eagerly on upgrade, that is architect Option B and belongs in the data layer, not here.

---

## 7. Files senior-dev will touch

New (presentational, small):
- `src/components/IdentityColorSwatch.tsx` — the dot primitive.
- `src/components/identityProfileColor.ts` — palette order + label + resolve/next-free/auto-distinct.
- `src/components/settings/IdentityColorPicker.tsx` — the radiogroup swatch picker.

Edit (additive):
- `src/components/IdentityAvatar.tsx` — hue ring when a non-neutral profile is matched.
- `src/components/IdentityMenu.tsx` — swatch `icon` per row; swatch in header; `nextFreeHue` on
  save-as draft.
- `src/components/settings/IdentityProfileCard.tsx` — swatch in head; render `IdentityColorPicker`
  as the new `identities.profile-color` row; wire `onChange({ color })`; consume auto-distinct list.
- `src/components/settings/catalog/repo.ts` — add the `identities.profile-color` catalog entry
  (`control: 'segmented'` is wrong; use a new `control` value or the generic — see below),
  `requires: 'profile'`, `repeats: 'perProfile'`, `keywords: 'colour hue swatch dot distinguish'`,
  `help` per §5.
- CSS (senior-dev owns): `src/styles/identity-menu.css` (`.identity-swatch`, avatar
  `[data-profile-color]` ring, menu-header swatch) and `src/styles/settings-primitives.css`
  (`.identity-swatch-option`, `.identity-swatch-grid`). **Tokens only — no hardcoded hex.**

**Catalog control type:** `SettingsIndexEntry.control` has no `'swatch'`/`'color'` member today.
Add one (`'color'`) to `settings/types.ts`'s control union and let the coverage/renderer switch on
it — do **not** shoehorn it into `'segmented'` (that renders text labels and caps at 3).

No change to the shared `ContextMenu` primitive (the `icon` slot already exists).

## 8. Harness / fixture states (mock, `VITE_MOCK_IPC=1`)

Verifiable in the browser harness (architect already seeds two colored profiles — `Work → blue`,
`Personal → green` in `mock/persistence.ts`). Add fixtures to exercise:
- **Same-label distinct-hue:** two profiles both labelled `Work` with different colors — proves the
  core goal (color tells them apart in menu + card + avatar).
- **Legacy neutral (auto-distinct):** ≥3 profiles with `color` absent — proves index-based
  auto-distinct fallback renders distinct hues without a persisted rewrite.
- **All-9 / overflow:** ≥9 profiles — proves `nextFreeHue` wrap and picker wrap layout.
- **Both themes:** toggle `data-theme='light'` — every swatch stays visible (contrast §1).

Native-only (USER CHECKPOINT): real `settings.json` persistence of a chosen color across restart,
and the header avatar ring on the native window.
