# UI Contract — Lucide icon migration

Status: ready for senior-dev. Owner: ui-designer. Decision LOCKED: replace all hand-drawn inline-SVG
chrome icons with `lucide-react`. Full migration.

## 0. Why / scope

The current glyphs are drawn on a 16×16 grid at `strokeWidth 1.4` with many fractional coordinates.
That renders crisply on Windows/Chromium but blurry and stylistically off on macOS/WebKit. Lucide
ships on a **24×24 integer grid at strokeWidth 2** with consistent geometry, which is what fixes the
cross-OS blur.

In scope: every exported icon in `src/components/appIcons.tsx` and `src/components/menuIcons.tsx`.

**Out of scope:** the canvas commit-graph glyphs in `src/graph/draw.ts` — they are drawn
imperatively on `<canvas>`, not SVG DOM, and have no cross-OS antialiasing problem of this kind. Do
not touch them. (Their `1.4` stroke stays the canvas reference weight; see §2 note.)

New runtime dependency: `lucide-react` must be added to `package.json` (`pnpm add lucide-react`).
Import icons by name (tree-shaken per-icon), never the barrel.

## 1. Rendering spec

Lucide components accept `size`, `color`, `strokeWidth`, `absoluteStrokeWidth`, and pass through any
other SVG prop. Defaults: `size=24`, `strokeWidth=2`, `color="currentColor"`.

**Standard props for ALL Bonsai chrome icons** — one shared object per file:

```
const ICON_PROPS = {
  size: 16,               // render at the current 16px chrome size (unchanged)
  strokeWidth: 2,         // Lucide-native weight; scales to 16/24 * 2 = 1.33px effective
  'aria-hidden': true,    // decorative — the button carries the accessible name
  focusable: false,
} as const;
```

- **Size:** `16` — matches today's `width/height=16`. The `.btn-icon` 32×32 hit box and all other
  hit-target geometry (ui-reference §3.1) are unchanged; only the painted glyph changes.
- **Stroke weight:** use `strokeWidth={2}` (Lucide default). At `size=16` this scales to a **1.33px
  effective stroke** — visually indistinguishable from today's 1.4 and cleaner on the integer grid.
  Do **not** set `absoluteStrokeWidth` (it would pin the stroke at a literal 2px at 16px render size,
  far too heavy). If exact 1.4px parity with the canvas glyphs is ever wanted, `strokeWidth={2.1}`
  gives it — but 2 is recommended for Lucide-native crispness and is the spec.
- **Color:** rely on Lucide's `currentColor` default. Do **not** pass `color` — every glyph must keep
  inheriting `color`/hover/disabled from its enclosing button/span exactly as today. No hardcoded hex.
- **`shape-rendering` / `vector-effect`:** none. Leave Lucide's default `geometricPrecision`. The blur
  was caused by fractional coordinates on a 16-grid, which Lucide's integer 24-grid removes; forcing
  `crispEdges` would degrade diagonals and curves and must not be used. Since we set an explicit
  `size`, `vector-effect: non-scaling-stroke` is unnecessary.
- **A11y recipe (preserved exactly):** every icon stays decorative — `aria-hidden` + `focusable={false}`
  on the SVG, accessible name always on the button/menuitem. No `role`, `title`, or `aria-label` on the
  glyph itself.

## 2. Wrapper strategy (CONFIRMED)

Keep the **same exported component names** in both files so none of the ~26 call sites change. Each
wrapper simply returns its mapped Lucide component with `ICON_PROPS` spread:

```
import { Sun, Settings, GitBranch /* ... */ } from 'lucide-react';
export const SunIcon = () => <Sun {...ICON_PROPS} />;
export const GearIcon = () => <Settings {...ICON_PROPS} />;
```

- Both files stay. `appIcons.tsx` and `menuIcons.tsx` each declare their own `ICON_PROPS` (identical);
  do not add a shared module for two lines — but keep the two definitions byte-identical.
- The verbose per-glyph JSDoc/`<path>` bodies are deleted; keep a one-line comment per export naming
  the Lucide icon and any semantic caveat (esp. the bespoke-kept ones and the imperfect matches).
- Signatures stay `() => JSX.Element` (no props) — call sites pass nothing today; do not add a props
  API in this pass.
- File size drops well under the 500-line limit; no split needed.
- The `svgProps` const in each file is removed once no bespoke glyph remains that uses it. Any icon
  kept bespoke (§3) keeps using the existing `svgProps` recipe unchanged.

## 3. Mapping table

`appIcons.tsx`:

| Export | Lucide | Notes |
|---|---|---|
| `SunIcon` | `Sun` | exact |
| `MoonIcon` | `Moon` | exact |
| `ListIcon` | `List` | flat-list view toggle; `AlignJustify` is a purer 3-rule match if the `List` bullets read wrong — prefer `List` for semantics |
| `TreeToggleIcon` | `ListTree` | tree-list view toggle; exact semantic |
| `RobotIcon` | `Bot` | exact |
| `ChartIcon` | `ChartColumn` | repo health bars (formerly `BarChart3`; use whichever the installed lucide-react version exports) |
| `GearIcon` | `Settings` | the cog; house-standard settings glyph |
| `UndoIcon` | `Undo2` | undo last operation |
| `FetchIcon` | `ArrowDownToLine` | arrow onto a tray line — matches current fetch glyph; mirror of Push |
| `PullIcon` | `Download` | inbox/tray glyph, kept visually **distinct** from Fetch (fetch = arrow-to-line, pull = download) |
| `PushIcon` | `ArrowUpToLine` | mirror of Fetch |
| `CaretDownIcon` | `ChevronDown` | exact |
| `RefreshIcon` | `RefreshCw` | two circular arrows; exact |
| `RefDotIcon` | **KEEP BESPOKE** | a solid filled disc; no Lucide filled-dot primitive (`Circle`/`Dot` are hollow/small). A plain filled circle renders identically on every OS, so there is no blur to fix. Keep the current 4-line SVG. If uniformity is preferred, `Circle` with a `fill="currentColor"` override is acceptable but not required. |
| `RefBranchIcon` | `GitBranch` | exact git-flavored match |
| `CloudIcon` | `Cloud` | exact |
| `StashIcon` | `Archive` | drawer/box for stored changes; closest match |
| `WorktreeIcon` | `FolderGit2` | separate working copy; best available (alt `GitFork`). Not exact — flagged. |
| `DetachedIcon` | `Target` | bullseye = detached HEAD off any branch; matches current ◎ (alt `Crosshair`) |
| `EyeIcon` | `Eye` | exact |
| `MoreIcon` | `Ellipsis` | three dots (formerly `MoreHorizontal`) |
| `GraphIcon` | `GitGraph` | exact git-flavored match — better than the old ad-hoc glyph |
| `SproutIcon` | `Sprout` | exact — Lucide ships it |

`menuIcons.tsx`:

| Export | Lucide | Notes |
|---|---|---|
| `CheckoutIcon` | `Check` | exact |
| `BranchIcon` | `GitBranchPlus` | "create branch here" — the `Plus` variant carries the create intent |
| `CopyIcon` | `Copy` | exact |
| `MergeIcon` | `GitMerge` | exact |
| `RebaseIcon` | **KEEP BESPOKE** | Lucide has no rebase glyph; the arc-onto-baseline is meaningful. Fallback if full-migration is enforced: `GitPullRequestArrow` (imperfect). Recommend keeping bespoke. |
| `RebaseInteractiveIcon` | **KEEP BESPOKE** | same as Rebase, plus the todo-list edit cue. No Lucide equivalent. Keep bespoke. |
| `ResetIcon` | `RotateCcw` | rewind branch pointer; closest (alt: keep bespoke). Not exact — flagged. |
| `CherryPickIcon` | `Cherry` | exact — Lucide ships it |
| `RevertIcon` | `Undo` | single-arrow undo, kept distinct from `UndoIcon`→`Undo2` |
| `CompareIcon` | `Columns2` | two side-by-side panes (split diff) |
| `DeleteIcon` | `Trash2` | exact |
| `TagIcon` | `Tag` | exact |
| `SummarizeIcon` | `Sparkles` | AI affordance; matches the ✨ used app-wide for AI |
| `StashApplyIcon` | `ArchiveRestore` | restore from stash (keeps in stack); pairs with `StashIcon`→`Archive` |
| `StashPopIcon` | `ArchiveX` | restore-and-remove from stack; the `X` reads "removed from stash" (alt: reuse `ArchiveRestore`). Imperfect pair — flagged. |
| `BisectIcon` | **KEEP BESPOKE** | range-bar + midpoint (binary search). No Lucide equivalent (`SearchCheck` is a weak fallback). Keep bespoke. |
| `HistoryIcon` | `History` | clock + ccw rewind; exact |
| `TerminalIcon` | `SquareTerminal` | windowed terminal, matches the current bordered rect |
| `FolderOpenIcon` | `FolderOpen` | exact |
| `EditorIcon` | `Code` | the `</>` chevrons; exact |

### Bespoke-kept summary

`RefDotIcon`, `RebaseIcon`, `RebaseInteractiveIcon`, `BisectIcon` stay hand-drawn — no adequate Lucide
match. They keep the existing `svgProps` recipe unchanged, so both files retain that const. Everything
else migrates. `WorktreeIcon`, `DetachedIcon`, `ResetIcon`, `StashPopIcon` migrate to a **close but
not exact** Lucide icon (noted above); orchestrator may downgrade any of these to bespoke if the
substitute reads wrong in review.

## 4. States, themes, densities

No change from today, and that is the point:
- **Color inheritance** carries every state — default `--text-*`, hover, active, `:focus-visible` ring
  (on the button), and disabled (`--text-3`) — through `currentColor`. The glyph itself has no state.
- **Both themes** work automatically: `currentColor` resolves per theme; no new token, no hex.
- **Both densities:** the glyph stays 16px in cozy and compact; density changes the hit box, not the
  glyph (ui-reference §3.1). Unchanged.
- **Long-content / overflow:** icons are fixed-size chrome, unaffected.
- **Motion:** none introduced. Icons that spin/pulse today (e.g. a refresh-in-progress class, if any)
  keep their existing CSS animation targeting the button/svg — verify the animated selector still
  matches Lucide's emitted `<svg class="lucide ...">` (Lucide adds a `lucide` class + `lucide-<name>`;
  it does not remove classes you pass). If any existing CSS selects the icon by element/class, confirm
  it still applies; prefer selecting via the button, not the svg.

## 5. Contrast / a11y

No new token pairs — all icons inherit existing text tokens that already pass ui-reference §2 contrast
(≥4.5:1 text, ≥3:1 UI edges) in both themes. Decorative `aria-hidden` glyphs are exempt from contrast
as information carriers, but Bonsai never uses color as sole meaning anyway (status badges pair
letter+color). Nothing regresses. Hit targets (≥24px) unchanged.

## 6. Harness / verification

Fully visible in the browser harness (`VITE_MOCK_IPC=1`) — every icon appears in existing chrome
(header toolbar, workspace toolbar, sidebar rows, file rows, context menus). No new fixtures needed.
Verification checklist for review:
1. Header (Sun/Moon, Bot, ChartColumn, Settings), workspace toolbar (Undo2, ArrowDownToLine,
   Download, ArrowUpToLine, RefreshCw, ChevronDown) render and inherit hover/disabled color.
2. Sidebar node glyphs (GitBranch, Cloud, Archive, FolderGit2, Target, GitGraph, RefDot) align on the
   existing rows.
3. Open a commit context menu: all `menuIcons` render at the same optical size as before.
4. Toggle light mode (`resize_window` colorScheme light) — glyphs recolor via `currentColor`.
Cross-OS crispness on macOS/WebKit is the whole reason for the change and is a **USER CHECKPOINT** —
the AI harness is Chromium-only and cannot judge WebKit antialiasing.

## 7. ui-reference.md

A new **§13 Icon system (SVG chrome)** is added in the same pass recording the Lucide adoption, the
render spec, the a11y recipe, and the bespoke-kept exceptions.
