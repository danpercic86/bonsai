// Icon-system §1 — app-wide chrome icons, now backed by `lucide-react`
// (Lucide's 24×24 integer grid at strokeWidth 2 renders crisply cross-OS;
// see docs/contracts/lucide-icons-ui.md). Every export name is preserved so the
// ~26 call sites are unchanged. Each wrapper returns its mapped Lucide component
// with the shared ICON_PROPS spread — decorative (aria-hidden), inheriting
// color/hover/disabled from its enclosing button via `currentColor`.
//
// KEPT BESPOKE (no adequate Lucide match): RefDotIcon (solid filled disc). It
// keeps the hand-drawn `svgProps` recipe below; a plain filled circle renders
// identically on every OS, so there is nothing to fix.
import {
  Sun,
  Moon,
  List,
  ListTree,
  Bot,
  ChartColumn,
  Settings,
  Undo2,
  ArrowDownToLine,
  Download,
  ArrowUpToLine,
  ChevronDown,
  RefreshCw,
  GitBranch,
  Cloud,
  Archive,
  FolderGit2,
  Target,
  Eye,
  Ellipsis,
  GitGraph,
  Sprout,
} from 'lucide-react';

// Shared render props for all Lucide chrome icons (identical to menuIcons').
const ICON_PROPS = {
  size: 16,
  strokeWidth: 2,
  'aria-hidden': true,
  focusable: false as const,
} as const;

// Hand-drawn recipe — used ONLY by the bespoke-kept glyph below.
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

/* ---------- Theme toggle ---------- */

/** Sun (Lucide) — shown in dark mode ("switch to light"). */
export const SunIcon = () => <Sun {...ICON_PROPS} />;

/** Moon (Lucide) — shown in light mode ("switch to dark"). */
export const MoonIcon = () => <Moon {...ICON_PROPS} />;

/* ---------- List-view toggle ---------- */

/** List (Lucide) — flat-list view toggle. */
export const ListIcon = () => <List {...ICON_PROPS} />;

/** ListTree (Lucide) — tree-list view toggle. */
export const TreeToggleIcon = () => <ListTree {...ICON_PROPS} />;

/* ---------- Header chrome ---------- */

/** Bot (Lucide) — AI assets. */
export const RobotIcon = () => <Bot {...ICON_PROPS} />;

/** ChartColumn (Lucide) — repository health bars. */
export const ChartIcon = () => <ChartColumn {...ICON_PROPS} />;

/** Settings (Lucide) — the cog / settings. */
export const GearIcon = () => <Settings {...ICON_PROPS} />;

/* ---------- Workspace toolbar ---------- */

/** Undo2 (Lucide) — undo the last operation. */
export const UndoIcon = () => <Undo2 {...ICON_PROPS} />;

/** ArrowDownToLine (Lucide) — fetch (arrow onto a tray line); mirror of Push. */
export const FetchIcon = () => <ArrowDownToLine {...ICON_PROPS} />;

/** Download (Lucide) — pull; kept visually distinct from Fetch. */
export const PullIcon = () => <Download {...ICON_PROPS} />;

/** ArrowUpToLine (Lucide) — push; mirror of Fetch. */
export const PushIcon = () => <ArrowUpToLine {...ICON_PROPS} />;

/** ChevronDown (Lucide) — dropdown / split triggers. */
export const CaretDownIcon = () => <ChevronDown {...ICON_PROPS} />;

/** RefreshCw (Lucide) — two circular arrows. */
export const RefreshIcon = () => <RefreshCw {...ICON_PROPS} />;

/* ---------- Sidebar node glyphs ---------- */

/** HEAD branch (●) — a solid commit dot. KEPT BESPOKE (no Lucide filled-dot). */
export function RefDotIcon() {
  return (
    <svg {...svgProps}>
      <circle cx="8" cy="8" r="3.2" fill="currentColor" stroke="none" />
    </svg>
  );
}

/** GitBranch (Lucide) — local branch. */
export const RefBranchIcon = () => <GitBranch {...ICON_PROPS} />;

/** Cloud (Lucide) — remote. */
export const CloudIcon = () => <Cloud {...ICON_PROPS} />;

/** Archive (Lucide) — stash (stored changes). */
export const StashIcon = () => <Archive {...ICON_PROPS} />;

/** FolderGit2 (Lucide) — worktree (separate working copy). Close, not exact. */
export const WorktreeIcon = () => <FolderGit2 {...ICON_PROPS} />;

/** Target (Lucide) — detached HEAD (bullseye off any branch). */
export const DetachedIcon = () => <Target {...ICON_PROPS} />;

/* ---------- File-row + overflow ---------- */

/** Eye (Lucide) — blame (per-line authorship). */
export const EyeIcon = () => <Eye {...ICON_PROPS} />;

/** Ellipsis (Lucide) — overflow (three dots). */
export const MoreIcon = () => <Ellipsis {...ICON_PROPS} />;

/* ---------- Onboarding + brand ---------- */

/** GitGraph (Lucide) — commit graph (onboarding). */
export const GraphIcon = () => <GitGraph {...ICON_PROPS} />;

/** Sprout (Lucide) — brand sprout. */
export const SproutIcon = () => <Sprout {...ICON_PROPS} />;
