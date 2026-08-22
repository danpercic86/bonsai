# Icon-system migration — UI contract

Replaces the raw Unicode/emoji glyphs used **as icons** with the app's existing inline-SVG idiom
(`src/components/menuIcons.tsx`, P10 §4.2). Ground truth for the verdict + tiers:
`docs/contracts/review-2026-08-22-ui.md` §C (SHOULD-1). This contract is the implementable spec:
a new `src/components/appIcons.tsx` module (full source in §1), a per-glyph migration map (§2),
the CSS sizing additions the map depends on (§3), and shippable increments (§4).

**Scope:** P1 colour-emoji (`🤖 📊 🕑 👁 🗑 ✨ 🕸️`) + P2 dingbat chrome (header `☀ ☾ ☰ ⋔ ⚙`;
toolbar `↶ ↓ ⇣ ↑ ▾ ↺ ⟳`; sidebar `● ⎇ ☁ ⊟ ⌥ ◎`; `⋯` overflow). P3 (`+ − × ›`, ahead/behind
`↑N ↓N`) is intentionally **kept as text** — see §5 for the two P3 glyphs I checked and why they
stay.

**Decisions at a glance (numbers for the report):** 23 new icons authored in `appIcons.tsx`;
4 existing icons reused from `menuIcons.tsx` (`HistoryIcon`, `DeleteIcon`, `RevertIcon`,
`SummarizeIcon`). **No new CSS tokens** — every icon inherits `currentColor` from the button's
existing colour token, so all state/hover/disabled/contrast behaviour is unchanged from today.

---

## 0. House rules this migration follows

- **Recipe is `menuIcons.tsx` verbatim** — 16×16 `viewBox`, `stroke="currentColor"`,
  `stroke-width: 1.4`, round caps/joins, `fill: none` (solid `fill="currentColor"` only where a
  dot/pupil reads better). `appIcons.tsx` is a faithful twin so the two files stay one visual
  language.
- **Icons are decorative; the button carries the name.** Every in-scope glyph sits on a control
  that already has a visible text label or an `aria-label`. So `appIcons`' shared `svgProps` adds
  `aria-hidden` + `focusable="false"` (the one deliberate delta from `menuIcons` — justified in §1),
  which keeps every `<Icon/>` out of the a11y tree with **no wrapper `<span>` per call site**. This
  matches the shipped precedent (`FolderOpenIcon` is wrapped in `aria-hidden`) and removes the emoji
  from being announced (resolves the spirit of the review's MUST-3 for the text buttons).
- **Colour comes from the button, not the icon.** `.btn-icon` → `--text-2`/`--text-3` disabled;
  `.row-action` → `--text-2`, discard-hover `--danger`; `.branch-glyph` → `--text-3`, HEAD `--accent`,
  detached `--warning`-mix; `.toolbar-btn` → `--text-2`. `currentColor` flows through unchanged.
- **Two sizes only.** `16px` for standalone icon buttons and menus (`.btn-icon`, `.row-action`,
  `.context-menu-icon` — already 16, `⋯` triggers); `14px` for dense inline-with-text and sidebar
  glyphs (`.toolbar-btn`, `.branch-glyph`, matching the shipped `.sidebar-add-icon svg { 14px }`).
  Onboarding/brand illustrations render larger (§3).

---

## 1. `src/components/appIcons.tsx` — complete source (copy-paste ready)

> Create this file verbatim. Reused icons are **not** duplicated here — import them from
> `./menuIcons` at the call site (see §2). Every component is a zero-prop function, exactly like
> `menuIcons.tsx`.

```tsx
// Icon-system §1 — inline-SVG chrome icons: the app-wide companion to
// `menuIcons.tsx`. SAME recipe as menuIcons (P10 §4.2): 16×16 viewBox,
// stroke="currentColor", 1.4 stroke, round caps/joins, no fill — so every glyph
// inherits color/hover/disabled from its button and stays pixel-consistent with
// the context-menu icons and the graph glyphs in draw.ts.
//
// These replace the raw Unicode/emoji glyphs previously used AS icons across the
// header, workspace toolbar, sidebar rows, file rows, and onboarding.
//
// Icons that already exist in `menuIcons.tsx` are NOT duplicated here — import
// them there. Reuse map (see the migration table):
//   🕑 file history / ↺ Reflog  -> menuIcons.HistoryIcon
//   🗑 delete                    -> menuIcons.DeleteIcon
//   ↺ discard (file row)         -> menuIcons.RevertIcon
//   ✨ any AI action             -> menuIcons.SummarizeIcon
//
// svgProps is menuIcons' recipe PLUS `aria-hidden` + `focusable="false"`
// (deliberate): every app-chrome icon is decorative — its button always carries
// the accessible name (visible text or aria-label) — so hiding the SVG from AT
// avoids ~40 wrapper spans and double announcements. Recommended follow-up:
// backport these two props into menuIcons.svgProps so both files share one recipe.
const svgProps = {
  width: 16,
  height: 16,
  viewBox: '0 0 16 16',
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.4,
  strokeLinecap: 'round' as const,
  strokeLinejoin: 'round' as const,
  'aria-hidden': true,
  focusable: false as const,
};

/* ---------- Theme toggle (☀ / ☾) ---------- */

/** Sun — shown in dark mode ("switch to light"). Disc + eight rays. */
export function SunIcon() {
  return (
    <svg {...svgProps}>
      <circle cx="8" cy="8" r="3" />
      <path d="M8 1.2 V2.8" />
      <path d="M8 13.2 V14.8" />
      <path d="M1.2 8 H2.8" />
      <path d="M13.2 8 H14.8" />
      <path d="M3.2 3.2 L4.3 4.3" />
      <path d="M11.7 11.7 L12.8 12.8" />
      <path d="M12.8 3.2 L11.7 4.3" />
      <path d="M4.3 11.7 L3.2 12.8" />
    </svg>
  );
}

/** Moon — shown in light mode ("switch to dark"). A crescent. */
export function MoonIcon() {
  return (
    <svg {...svgProps}>
      <path d="M13.2 9.9 A5.6 5.6 0 1 1 6.3 3 A4.4 4.4 0 0 0 13.2 9.9 Z" />
    </svg>
  );
}

/* ---------- List-view toggle (☰ / ⋔) ---------- */

/** Flat lists (☰) — three rules. Shown when the current view is tree. */
export function ListIcon() {
  return (
    <svg {...svgProps}>
      <path d="M3 4.5 H13" />
      <path d="M3 8 H13" />
      <path d="M3 11.5 H13" />
    </svg>
  );
}

/** Tree lists (⋔) — a root node forking to two children. */
export function TreeToggleIcon() {
  return (
    <svg {...svgProps}>
      <circle cx="3.5" cy="8" r="1.5" />
      <circle cx="12" cy="4.5" r="1.5" />
      <circle cx="12" cy="11.5" r="1.5" />
      <path d="M5 8 H8" />
      <path d="M8 8 V4.5 H10.5" />
      <path d="M8 8 V11.5 H10.5" />
    </svg>
  );
}

/* ---------- Header chrome (🤖 📊 ⚙) ---------- */

/** AI assets (🤖) — a robot head: antenna, eyes, mouth. */
export function RobotIcon() {
  return (
    <svg {...svgProps}>
      <path d="M8 1.6 V3.4" />
      <circle cx="8" cy="1.4" r="0.9" fill="currentColor" stroke="none" />
      <rect x="3" y="3.6" width="10" height="8.8" rx="2" />
      <circle cx="6" cy="7.6" r="1" fill="currentColor" stroke="none" />
      <circle cx="10" cy="7.6" r="1" fill="currentColor" stroke="none" />
      <path d="M6 10.2 H10" />
    </svg>
  );
}

/** Repository health (📊) — an L-axis with three bars. */
export function ChartIcon() {
  return (
    <svg {...svgProps}>
      <path d="M3 2.5 V13 H13.5" />
      <rect x="5" y="8" width="2.2" height="5" rx="0.4" />
      <rect x="8.4" y="5" width="2.2" height="8" rx="0.4" />
      <rect x="11.8" y="10" width="2.2" height="3" rx="0.4" />
    </svg>
  );
}

/** Settings (⚙) — a cog: pitch circle, hub hole, eight radial teeth. */
export function GearIcon() {
  return (
    <svg {...svgProps}>
      <circle cx="8" cy="8" r="4" />
      <circle cx="8" cy="8" r="1.7" />
      <path d="M8 1.4 V4" />
      <path d="M8 12 V14.6" />
      <path d="M1.4 8 H4" />
      <path d="M12 8 H14.6" />
      <path d="M3.35 3.35 L5.2 5.2" />
      <path d="M10.8 10.8 L12.65 12.65" />
      <path d="M12.65 3.35 L10.8 5.2" />
      <path d="M5.2 10.8 L3.35 12.65" />
    </svg>
  );
}

/* ---------- Workspace toolbar (↶ ↓ ⇣ ↑ ▾ ⟳) ---------- */

/** Undo (↶) — a back-curving arrow (undo the last operation). */
export function UndoIcon() {
  return (
    <svg {...svgProps}>
      <path d="M4 8 H10 A3 3 0 1 1 6.8 11.4" />
      <path d="M6.2 5.6 L4 8 L6.2 10.4" />
    </svg>
  );
}

/** Fetch (↓) — a down arrow landing on a tray line (download refs). */
export function FetchIcon() {
  return (
    <svg {...svgProps}>
      <path d="M8 2.5 V9.8" />
      <path d="M5.2 7 L8 9.8 L10.8 7" />
      <path d="M3.5 13 H12.5" />
    </svg>
  );
}

/** Pull (⇣) — a down arrow with a crossbar (fetch + fast-forward). Distinct
 *  from Fetch: crossbar near the top, no tray line. */
export function PullIcon() {
  return (
    <svg {...svgProps}>
      <path d="M8 2.5 V10.5" />
      <path d="M5.2 7.7 L8 10.5 L10.8 7.7" />
      <path d="M5.5 5 H10.5" />
    </svg>
  );
}

/** Push (↑) — an up arrow rising off a tray line (upload). Mirror of Fetch. */
export function PushIcon() {
  return (
    <svg {...svgProps}>
      <path d="M8 11.5 V4.2" />
      <path d="M5.2 7 L8 4.2 L10.8 7" />
      <path d="M3.5 13 H12.5" />
    </svg>
  );
}

/** Caret down (▾) — a small chevron for dropdown/split triggers. */
export function CaretDownIcon() {
  return (
    <svg {...svgProps}>
      <path d="M4.5 6.5 L8 10 L11.5 6.5" />
    </svg>
  );
}

/** Refresh (⟳) — two circular arrows. (Curvature is senior-dev-tunable.) */
export function RefreshIcon() {
  return (
    <svg {...svgProps}>
      <path d="M3.6 7.2 A4.8 4.8 0 0 1 11.7 4.9" />
      <path d="M9.3 4 L11.9 4.9 L11.4 7.6" />
      <path d="M12.4 8.8 A4.8 4.8 0 0 1 4.3 11.1" />
      <path d="M6.7 12 L4.1 11.1 L4.6 8.4" />
    </svg>
  );
}

/* ---------- Sidebar node glyphs (● ⎇ ☁ ⊟ ⌥ ◎) ---------- */

/** HEAD branch (●) — a solid commit dot (current position). */
export function RefDotIcon() {
  return (
    <svg {...svgProps}>
      <circle cx="8" cy="8" r="3.2" fill="currentColor" stroke="none" />
    </svg>
  );
}

/** Local branch (⎇) — a branch fork: base node, stem, two tips. */
export function RefBranchIcon() {
  return (
    <svg {...svgProps}>
      <circle cx="8" cy="12.6" r="1.4" />
      <path d="M8 11.2 V8" />
      <path d="M8 8 L4.6 4.9" />
      <path d="M8 8 L11.4 4.9" />
      <circle cx="4" cy="4" r="1.4" />
      <circle cx="12" cy="4" r="1.4" />
    </svg>
  );
}

/** Remote (☁) — a cloud. */
export function CloudIcon() {
  return (
    <svg {...svgProps}>
      <path d="M4.8 12 A2.6 2.6 0 0 1 4.9 6.9 A3.4 3.4 0 0 1 11.3 6.4 A2.5 2.5 0 0 1 11.2 12 Z" />
    </svg>
  );
}

/** Stash (⊟) — a two-drawer cabinet (stored changes). Echoes the stash tray
 *  in menuIcons StashApply/Pop, without an arrow. */
export function StashIcon() {
  return (
    <svg {...svgProps}>
      <rect x="2.8" y="4.8" width="10.4" height="7.4" rx="1.3" />
      <path d="M2.8 8.5 H13.2" />
      <path d="M6.6 6.6 H9.4" />
      <path d="M6.6 10.3 H9.4" />
    </svg>
  );
}

/** Worktree (⌥) — a working-copy box branched off the repo node. */
export function WorktreeIcon() {
  return (
    <svg {...svgProps}>
      <circle cx="3.6" cy="3.8" r="1.5" />
      <path d="M3.6 5.3 V12" />
      <path d="M3.6 8 H8" />
      <rect x="8" y="5.5" width="5.6" height="5" rx="1" />
    </svg>
  );
}

/** Detached HEAD (◎) — a target/bullseye (a commit off any branch). */
export function DetachedIcon() {
  return (
    <svg {...svgProps}>
      <circle cx="8" cy="8" r="5" />
      <circle cx="8" cy="8" r="1.7" fill="currentColor" stroke="none" />
    </svg>
  );
}

/* ---------- File-row + overflow (👁 ⋯) ---------- */

/** Blame (👁) — an eye with a pupil (per-line authorship). */
export function EyeIcon() {
  return (
    <svg {...svgProps}>
      <path d="M1.5 8 C3.8 4.5 12.2 4.5 14.5 8 C12.2 11.5 3.8 11.5 1.5 8 Z" />
      <circle cx="8" cy="8" r="2" />
    </svg>
  );
}

/** Overflow (⋯) — three dots. */
export function MoreIcon() {
  return (
    <svg {...svgProps}>
      <circle cx="3.4" cy="8" r="1.2" fill="currentColor" stroke="none" />
      <circle cx="8" cy="8" r="1.2" fill="currentColor" stroke="none" />
      <circle cx="12.6" cy="8" r="1.2" fill="currentColor" stroke="none" />
    </svg>
  );
}

/* ---------- Onboarding + brand (🕸️ 🌱) ---------- */

/** Commit graph (replaces the onboarding 🕸️) — a lane with two commit dots and
 *  a branch forking to a third. More honest than a spider web for "graph". */
export function GraphIcon() {
  return (
    <svg {...svgProps}>
      <path d="M4.5 3 V13" />
      <circle cx="4.5" cy="3.5" r="1.5" />
      <circle cx="4.5" cy="12.5" r="1.5" />
      <circle cx="11.5" cy="8" r="1.5" />
      <path d="M4.5 8 C4.5 5.5 11.5 6 11.5 8" />
    </svg>
  );
}

/** Brand sprout (🌱) — a stem with two leaves. Used at display size in heroes;
 *  see §5 for the brand-mark decision (adopt now vs. dedicated logo). */
export function SproutIcon() {
  return (
    <svg {...svgProps}>
      <path d="M8 14 V7" />
      <path d="M8 9 C5.2 9 3.6 7.4 3.4 5 C6.2 5 7.8 6.6 8 9 Z" />
      <path d="M8 8 C10.8 8 12.4 6.4 12.6 4 C9.8 4 8.2 5.6 8 8 Z" />
    </svg>
  );
}
```

---

## 2. Migration map

`file:line` is where the glyph literal lives today. **Aria** is uniform: the icon is `aria-hidden`
(via `svgProps`) and the accessible name stays on the button — flagged below only where a name is
missing. `[reuse]` = import from `./menuIcons`; `[new]` = from `./appIcons`.

### 2.1 File-row actions — `src/components/StatusFileRow.tsx`

| Glyph | Component | Location | Wrapper + size | Aria |
|---|---|---|---|---|
| `🕑` | `HistoryIcon` **[reuse]** | `StatusFileRow.tsx:124` | `.row-action.row-action-history` · **16px** (`.row-action svg`) | `aria-label` present (`Show history of …`) ✓ |
| `👁` | `EyeIcon` **[new]** | `StatusFileRow.tsx:135` | `.row-action.row-action-blame` · **16px** | `aria-label` present (`Blame …`) ✓ |
| `↺` (discard) | `RevertIcon` **[reuse]** | `StatusFileRow.tsx:147` | `.row-action.row-action-discard` · **16px** | `aria-label` present (`Discard changes to …`) ✓ |
| `🗑` (delete) | `DeleteIcon` **[reuse]** | `StatusFileRow.tsx:159` | `.row-action.row-action-discard` · **16px** | `aria-label` present (`Delete …`) ✓ |
| `+ / −` | — **keep text** (P3) | `StatusFileRow.tsx:170` | `.row-action-primary` | unchanged |

### 2.2 Header — `src/components/HeaderToolbar.tsx`

| Glyph | Component | Location | Wrapper + size | Aria |
|---|---|---|---|---|
| `☀ / ☾` | `SunIcon` / `MoonIcon` **[new]** | `HeaderToolbar.tsx:62` | `.btn-icon.theme-toggle` · **16px** (`.btn-icon svg`) | `aria-label` present (dynamic) ✓ |
| `☰ / ⋔` | `ListIcon` / `TreeToggleIcon` **[new]** | `HeaderToolbar.tsx:71` | `.btn-icon.list-view-toggle` · **16px** | `aria-label` present (dynamic) ✓ |
| `🤖` | `RobotIcon` **[new]** | `HeaderToolbar.tsx:81` | `.btn-icon.ai-assets-toggle` · **16px** | `aria-label="AI Assets"` ✓ |
| `📊` | `ChartIcon` **[new]** | `HeaderToolbar.tsx:92` | `.btn-icon.repo-health-toggle` · **16px** | `aria-label="Health"` ✓ |
| `⚙` | `GearIcon` **[new]** | `HeaderToolbar.tsx:102` | `.btn-icon.settings-toggle` · **16px** | `aria-label="Settings"` ✓ |

Render pattern (dynamic pairs): `{theme === 'dark' ? <SunIcon /> : <MoonIcon />}`,
`{listView === 'tree' ? <ListIcon /> : <TreeToggleIcon />}`.

### 2.3 Workspace toolbar — `src/components/WorkspaceToolbar.tsx`

| Glyph | Component | Location | Wrapper + size | Aria |
|---|---|---|---|---|
| `↶` Undo | `UndoIcon` **[new]** | `WorkspaceToolbar.tsx:137` | `.toolbar-btn` · **14px** (`.toolbar-btn svg`) | visible text `Undo` (no aria-label needed) |
| `↓` Fetch | `FetchIcon` **[new]** | `WorkspaceToolbar.tsx:146` | `.toolbar-btn` · **14px** | visible text `Fetch`/`Fetching…` |
| `⇣` Pull | `PullIcon` **[new]** | `WorkspaceToolbar.tsx:160` | `.toolbar-btn` · **14px** | visible text `Pull`/`Pulling…` |
| `↑` Push | `PushIcon` **[new]** | `WorkspaceToolbar.tsx:170` | `.toolbar-btn.toolbar-split-main` · **14px** | visible text `Push`/`Pushing…` |
| `▾` caret | `CaretDownIcon` **[new]** | `WorkspaceToolbar.tsx:186` | `.toolbar-btn.toolbar-caret` · **14px** (12px optional) | icon-only; `aria-label="More push actions"` ✓ |
| `✨` What changed | `SummarizeIcon` **[reuse]** | `WorkspaceToolbar.tsx:197` | `.toolbar-btn` · **14px** | visible text `What changed…` |
| `✨` Ask | `SummarizeIcon` **[reuse]** | `WorkspaceToolbar.tsx:207` | `.toolbar-btn` · **14px** | visible text `Ask…` |
| `↺` Reflog | `HistoryIcon` **[reuse]** | `WorkspaceToolbar.tsx:217` | `.toolbar-btn` · **14px** | visible text `Reflog` |
| (FolderOpen) | already SVG — **no change** | `WorkspaceToolbar.tsx:231` | `.toolbar-external-icon` | ✓ |
| `⟳` Refresh | `RefreshIcon` **[new]** | `WorkspaceToolbar.tsx:242` | `.btn-icon.toolbar-refresh` · **16px** | `aria-label="Refresh"` ✓ |

**Text buttons keep their label.** Replace only the leading glyph char; wrap the word in a `<span>`
so flex `gap` spaces it: e.g. `<UndoIcon /><span>Undo</span>`. For Fetch/Pull/Push keep the icon
persistent and swap only the word during the op:
`<FetchIcon /><span>{remoteOp === 'fetch' ? 'Fetching…' : 'Fetch'}</span>` (nicer than today, where
the glyph vanishes mid-op). Pairs with review **MUST-3**: explicit `aria-label`s on these buttons are
still fine to add, but once the emoji is gone the visible text is a clean accessible name, so it is no
longer required.

### 2.4 Sidebar node glyphs — `src/components/sidebar/rows.tsx`

All are decorative type-indicators inside a row whose branch/remote/name text is the accessible
content → `aria-hidden` (via `svgProps`), no per-row aria change.

| Glyph | Component | Location | Wrapper + size | Notes |
|---|---|---|---|---|
| `● / ⎇` | `RefDotIcon` / `RefBranchIcon` **[new]** | `rows.tsx:76` | `.branch-glyph` · **14px** (`.branch-glyph svg`) | HEAD (`●`) keeps `--accent` (graphic, 3:1 OK per review §B.2); other branches `--text-3` |
| `☁` | `CloudIcon` **[new]** | `rows.tsx:115`, `rows.tsx:154` | `.branch-glyph` · **14px** | remote-tracking + configured-remote rows |
| `⊟` | `StashIcon` **[new]** | `rows.tsx:205` | `.branch-glyph` · **14px** | |
| `⌥` | `WorktreeIcon` **[new]** | `rows.tsx:259` | `.branch-glyph` · **14px** | |
| `◎` | `DetachedIcon` **[new]** | `rows.tsx:291` | `.branch-glyph` · **14px** | keeps `--warning`-mix color |
| `›` chevron | — **keep** (P3) | `sidebar/SectionHeader.tsx:44` | `.file-chevron` (CSS-rotated) | good cross-platform coverage; rotation animates expand/collapse — see §5 |

### 2.5 Overflow `⋯` (P2 — outside the five primary surfaces)

| Glyph | Component | Location | Wrapper + size | Aria |
|---|---|---|---|---|
| `⋯` | `MoreIcon` **[new]** | `CommitOptionsMenu.tsx:220` | commit-box overflow trigger · **16px** | verify trigger `aria-label`; icon `aria-hidden` |
| `⋯` | `MoreIcon` **[new]** | `ForgeAccountSwitcher.tsx:185` | account-switcher trigger · **16px** | verify `aria-label` |
| `⋯` | `MoreIcon` **[new]** | `ReflogView.tsx:157` | reflog row overflow · **16px** | verify `aria-label` |
| `⋯` | `MoreIcon` **[new]** | `settings/SettingsAccountCard.tsx:156` | account-card overflow · **16px** | verify `aria-label` |

Size `MoreIcon` to each trigger's existing box; **16px** default. Any trigger that becomes icon-only
without an `aria-label` gets one (verb + object, e.g. `Commit options`, `Account actions`).

### 2.6 Onboarding + brand — `OnboardingSteps.tsx`, `EmptyState.tsx`, `WorkspaceGraphPane.tsx`

Tour-card icons live in a `TourCard.icon: string` table (`OnboardingSteps.tsx:224-246`). Change the
field type from `string` to a component (`() => JSX.Element`) and render `<c.icon />`.

| Glyph | Component | Location | Wrapper + size | Notes |
|---|---|---|---|---|
| `🕸️` "Commit graph" | `GraphIcon` **[new]** | `OnboardingSteps.tsx:232` | `.onboarding-tour-icon` · **28px** | `aria-hidden` already on wrapper ✓ |
| `🤖` "AI assets" | `RobotIcon` **[new]** | `OnboardingSteps.tsx:237` | `.onboarding-tour-icon` · **28px** | |
| `📊` "Repository health" | `ChartIcon` **[new]** | `OnboardingSteps.tsx:242` | `.onboarding-tour-icon` · **28px** | |
| inline `🤖` / `📊` in prose | — **microcopy** | `OnboardingSteps.tsx:239,244` | — | replace with the button's name: "The AI assets button…", "The health button…" (don't inline an SVG mid-sentence) |
| `🌱` hero | `SproutIcon` **[new]** | `OnboardingSteps.tsx:18` | `.onboarding-hero` · **32px** | brand — see §5 |
| `🌱` mark | `SproutIcon` **[new]** | `EmptyState.tsx:41` | `.empty-mark` · **32px** | brand |
| `🌱` empty graph | `SproutIcon` **[new]** | `WorkspaceGraphPane.tsx:269` | `.graph-pane-empty-mark` · **32px** | brand |

### 2.7 The `✨` app-wide sweep (P1 — reuse `SummarizeIcon` everywhere)

`✨` is the app's AbI-action marker in ~15 places. The two in the workspace toolbar are in §2.3;
the rest is a mechanical sweep — **all** `<span>✨</span>` / `"✨ "` text prefixes → `<SummarizeIcon />`
(14px inline, `aria-hidden`), keeping the text label. Sites:

`ChangelogDialog.tsx:123` (h2 title) · `AiOutputPanel.tsx:75` · `ComposerDialog.tsx:79` ·
`CommitOptionsMenu.tsx:183/185/186/304` · `CommitPanel.tsx:142` · `CommitBox.tsx:350` ·
`DiffOverlay.tsx:296` · `PrCreateForm.tsx:165` · `HistorySearchPanel.tsx:124,154` ·
`WhatChangedDialog.tsx:140` · `BranchNameSuggest.tsx:69` (trailing `✨`) ·
`StatusConflictsSection.tsx` (the per-row `✨ AI` button, whose label is built in the `aiButtonView`
helper ~`:35/:57` — strip the `"✨ "` prefix from the helper strings and render `<SummarizeIcon />`
before `{view.label}` in the button JSX).

**Deferred (need a type change, not a glyph swap):** `paletteActions.ts:167/290/298/319` (`hint`
is a `string` rendered as text) and `aiDockFormat.ts:162/163` (`glyph: '✨'` string in a formatter).
Converting these to an icon means widening those fields from `string` to `ReactNode` — out of scope
for a glyph swap; leave as-is or file as a small follow-up.

---

## 3. CSS additions (sizing only — no tokens, no colour)

`senior-dev` adds these to the matching stylesheet. Every rule is size/layout; colour already flows
via `currentColor`.

```css
/* controls.css — standalone icon buttons (header + toolbar-right). 16px in a 32px box. */
.btn-icon svg { width: 16px; height: 16px; }

/* controls.css — toolbar text buttons: icon + label with a gap. */
.toolbar-btn { gap: 6px; }                 /* add to the existing rule (already inline-flex) */
.toolbar-btn svg { width: 14px; height: 14px; flex: none; }
.toolbar-caret svg { width: 12px; height: 12px; }   /* optional: a smaller caret */

/* sidebar.css — node type glyphs. 14px fills the existing 14px-wide .branch-glyph slot. */
.branch-glyph svg { width: 14px; height: 14px; }

/* commit-box.css — file-row action buttons (20px box). */
.row-action svg { width: 16px; height: 16px; }

/* onboarding.css / empty-state.css / graph.css — display-size illustrations. */
.onboarding-tour-icon svg { width: 28px; height: 28px; }
.onboarding-hero svg,
.empty-mark svg,
.graph-pane-empty-mark svg { width: 32px; height: 32px; }
```

**Caveat (flag):** if any `.btn-icon` in the header contains a non-icon SVG (check the
`IdentityMenu` avatar trigger — per `identity-menu.css` it is a text-initial circle, not an SVG, so
this is expected to be safe), scope the first rule to
`.header-toolbar .btn-icon svg, .toolbar-right .btn-icon svg` instead of the bare `.btn-icon svg`.

**Optional cleanup (dead after migration):** `.theme-toggle`/`.list-view-toggle { font-size: 16px }`
(controls.css:131-139) targeted the emoji glyph and no longer does anything; `.branch-glyph`'s
`font-size: 12px` is likewise moot. Harmless to leave; tidy if convenient.

---

## 4. Increments (ship one at a time; lowest-risk first)

1. **File-row actions** — `StatusFileRow.tsx` + `.row-action svg`. **Lowest risk.** 4 glyphs,
   1 new icon (`EyeIcon`), 3 reused; labels already present; single isolated file.
2. **Header** — `HeaderToolbar.tsx` + `.btn-icon svg`. **Low risk.** 7 new icons; `aria-label`s
   already correct; isolated. Delivers the most visible win (the emoji cluster in the header).
3. **Workspace toolbar** — `WorkspaceToolbar.tsx` + `.toolbar-btn`/`.btn-icon svg`. **Medium risk**
   (icon+text layout, loading-text swap, the split caret). 6 new icons + 2 reused. Pair with review
   MUST-3 aria pass while here.
4. **Sidebar glyphs** — `sidebar/rows.tsx` + `.branch-glyph svg`. **Low risk.** 6 new icons, all
   decorative. Independent of the §D sidebar-keyboard work (touches the glyph child only).
5. **Onboarding** — `OnboardingSteps.tsx` (+ `TourCard.icon` type change) + tour-icon CSS + the
   two prose microcopy tweaks. **Low risk**; `GraphIcon` new, `Robot`/`Chart` reused from inc 2.
6. **`✨` sweep** — the §2.7 sites (reuse `SummarizeIcon`). **Mechanical, spans many files.** Do the
   two `WorkspaceToolbar` `✨` in inc 3; the rest here. Leave the two deferred string-typed cases.
7. **`⋯` overflow** — §2.5 (reuse `MoreIcon`). **Low-medium.** Verify each trigger's `aria-label`.
8. **Brand `SproutIcon`** — §2.6 hero/mark/empty-graph. **Optional / lowest priority** — gated on
   the §5 decision.

Increments 1–5 are the requested order (file-row → header → toolbar → sidebar → onboarding); 6–8 are
the two extra tracks (`✨`, `⋯`) and the brand mark, sliced out so no single diff is large.

---

## 5. Flags & decisions for the orchestrator

- **P3 kept as text (verified, no misrender):** `›` (`SectionHeader.tsx:44`) is a clean chevron with
  good coverage and is **CSS-rotated** to animate expand/collapse — replacing it with a rotated
  `CaretDownIcon` is possible but gains nothing and risks the rotation transition; keep it. `×`
  (`OnboardingOverlay.tsx:234` and other dialog closes) and `+ − ✓` render fine as text. **None of the
  P3 glyphs I checked misrender** — leaving them as text is correct.
- **`svgProps` delta from `menuIcons` (aria-hidden + focusable):** deliberate; justified in §1.
  Recommendation: backport the same two props into `menuIcons.svgProps` in a later pass so the two
  files are one identical recipe (low priority — menu items already have text labels).
- **`RevertIcon` reused for file-row discard:** its menu use is git-revert; "discard changes"
  (restore to staged/committed) is the same "undo changes" intent, so the shared glyph reads
  correctly. If `reviewer` prefers a distinct mark, add a `DiscardIcon`; I recommend the reuse.
- **Brand `🌱` (decision needed):** authored `SproutIcon` (16×16, 1.4 stroke). At 32px hero size the
  stroke scales to ~2.8px (bold but on-brand). Options: **(a)** adopt `SproutIcon` now for
  consistency [my recommendation for the tour/empty chrome]; **(b)** keep the emoji at hero sizes
  and use `SproutIcon` only for any future small brand use; **(c)** commission a dedicated
  multi-detail logo SVG (review §C.4). The review rates this low priority — increment 8 is optional.
- **`GraphIcon` for `🕸️`:** the spider web was a poor stand-in for "commit graph"; I authored a
  small commit-graph glyph instead. Flagging the semantic substitution.
- **Deferred `✨` (string-typed):** `paletteActions.ts` hints + `aiDockFormat.ts` glyph carry `✨`
  as `string` data; they need a `string → ReactNode` widening to take an icon. Out of scope here.

---

## 6. Harness verification (AI-gate)

All of this is visible in the browser harness (`pnpm dev`, `VITE_MOCK_IPC=1`) — static rendering,
no rAF needed. Confirm per increment, in **dark and light** (`resize_window` colorScheme) and both
`panelDensity` values (icons are density-independent; check nothing clips):

- **Colour inheritance:** each icon takes the button's colour at rest, on `:hover`, when
  `:disabled` (`--text-3`), and `.row-action-discard:hover` turns the trash/revert `--danger`.
- **Focus ring:** `:focus-visible` 2px `--accent` ring sits on the button box, not the SVG (unchanged).
- **Sizing:** header/refresh 16px, toolbar text-button icons 14px aligned to the label baseline,
  sidebar glyphs 14px centered in the 14px slot, file-row actions 16px uncramped with 3–4 in a row.
- **Pathological content:** long branch names still ellipsize with the 14px glyph fixed at the left;
  the icon never shrinks or wraps.
- **Fixtures:** existing mock states already cover branches/remotes/tags/stashes/worktrees/detached
  HEAD, the conflict rows (`✨ AI`), the unborn-HEAD empty graph (`🌱`), and onboarding — no new
  fixtures required.

No motion is introduced, so there is no frame-timing checkpoint. Screen-reader announcement of the
now-`aria-hidden` icons + button names is a **USER CHECKPOINT** (NVDA/VoiceOver), not AI-gate.
