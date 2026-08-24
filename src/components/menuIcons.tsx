// Context-menu action icons, now backed by `lucide-react` (see
// docs/contracts/lucide-icons-ui.md). Every export name is preserved so call
// sites are unchanged. Each wrapper returns its mapped Lucide component with the
// shared ICON_PROPS spread — inheriting color/hover/disabled from the enclosing
// `.context-menu-icon` span via `currentColor`.
//
// KEPT BESPOKE (no adequate Lucide match): RebaseIcon, RebaseInteractiveIcon,
// BisectIcon. They keep the hand-drawn `svgProps` recipe below.
import {
  Check,
  GitBranchPlus,
  Copy,
  GitMerge,
  RotateCcw,
  Cherry,
  Undo,
  Columns2,
  Trash2,
  Tag,
  Sparkles,
  ArchiveRestore,
  ArchiveX,
  History,
  SquareTerminal,
  FolderOpen,
  Code,
} from 'lucide-react';

// Shared render props for all Lucide chrome icons (identical to appIcons').
const ICON_PROPS = {
  size: 16,
  strokeWidth: 2,
  'aria-hidden': true,
  focusable: false as const,
} as const;

// Hand-drawn recipe — used ONLY by the bespoke-kept glyphs below.
const svgProps = {
  width: 16,
  height: 16,
  viewBox: '0 0 16 16',
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.4,
  strokeLinecap: 'round' as const,
  strokeLinejoin: 'round' as const,
};

/** Check (Lucide) — checkout / branch switch confirmed. */
export const CheckoutIcon = () => <Check {...ICON_PROPS} />;

/** GitBranchPlus (Lucide) — create branch here. */
export const BranchIcon = () => <GitBranchPlus {...ICON_PROPS} />;

/** Copy (Lucide) — copy. */
export const CopyIcon = () => <Copy {...ICON_PROPS} />;

/** GitMerge (Lucide) — merge. */
export const MergeIcon = () => <GitMerge {...ICON_PROPS} />;

/** Rebase — a branch lifted (arc) onto a baseline. KEPT BESPOKE (no Lucide match). */
export function RebaseIcon() {
  return (
    <svg {...svgProps}>
      {/* baseline */}
      <path d="M2 12.5 H14" />
      {/* lifted branch arc rising from the baseline and returning */}
      <path d="M4 12.5 C4 4.5 12 4.5 12 12.5" />
      {/* up arrowhead at the apex */}
      <path d="M6.4 6.6 L8 5 L9.6 6.6" />
    </svg>
  );
}

/** Rebase (interactive) — the rebase arc plus a short todo-list. KEPT BESPOKE. */
export function RebaseInteractiveIcon() {
  return (
    <svg {...svgProps}>
      {/* lifted branch arc on the left (echoes RebaseIcon) */}
      <path d="M2 13 C2 6.5 6 6.5 6 13" />
      {/* todo-list lines on the right (the editable plan) */}
      <path d="M9 5 H14" />
      <path d="M9 8.5 H14" />
      <path d="M9 12 H14" />
    </svg>
  );
}

/** RotateCcw (Lucide) — reset (rewind branch pointer). Close, not exact. */
export const ResetIcon = () => <RotateCcw {...ICON_PROPS} />;

/** Cherry (Lucide) — cherry-pick. */
export const CherryPickIcon = () => <Cherry {...ICON_PROPS} />;

/** Undo (Lucide) — revert; single-arrow undo, kept distinct from UndoIcon→Undo2. */
export const RevertIcon = () => <Undo {...ICON_PROPS} />;

/** Columns2 (Lucide) — compare (two side-by-side panes). */
export const CompareIcon = () => <Columns2 {...ICON_PROPS} />;

/** Trash2 (Lucide) — delete / drop. */
export const DeleteIcon = () => <Trash2 {...ICON_PROPS} />;

/** Tag (Lucide) — create tag here. */
export const TagIcon = () => <Tag {...ICON_PROPS} />;

/** Sparkles (Lucide) — summarize (AI affordance). */
export const SummarizeIcon = () => <Sparkles {...ICON_PROPS} />;

/** ArchiveRestore (Lucide) — stash apply (keeps in stack). */
export const StashApplyIcon = () => <ArchiveRestore {...ICON_PROPS} />;

/** ArchiveX (Lucide) — stash pop (restore and remove from stack). Close, not exact. */
export const StashPopIcon = () => <ArchiveX {...ICON_PROPS} />;

/** Bisect — a range bar with a midpoint marker. KEPT BESPOKE (no Lucide match). */
export function BisectIcon() {
  return (
    <svg {...svgProps}>
      {/* good..bad range endpoints */}
      <circle cx="3" cy="8" r="1.5" />
      <circle cx="13" cy="8" r="1.5" />
      {/* baseline between them */}
      <path d="M4.5 8 H11.5" />
      {/* midpoint split marker */}
      <path d="M8 3.5 V12.5" />
    </svg>
  );
}

/** History (Lucide) — history / reflog. */
export const HistoryIcon = () => <History {...ICON_PROPS} />;

/** SquareTerminal (Lucide) — open in terminal. */
export const TerminalIcon = () => <SquareTerminal {...ICON_PROPS} />;

/** FolderOpen (Lucide) — reveal in file manager. */
export const FolderOpenIcon = () => <FolderOpen {...ICON_PROPS} />;

/** Code (Lucide) — open in editor. */
export const EditorIcon = () => <Code {...ICON_PROPS} />;
