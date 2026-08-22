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
