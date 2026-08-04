// P10 §4.2: inline-SVG icon set for context-menu actions. No external deps.
// Each is a 16×16 monochrome glyph that inherits color from the enclosing
// `.context-menu-icon` span (so hover/disabled color flows through). Default
// stroke style matches the graph glyphs in draw.ts: currentColor, 1.4 stroke,
// no fill, round caps/joins. `fill="currentColor"` is used only where a solid
// shape reads better.

// Shared root props so every glyph is pixel-consistent.
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

/** Checkout — a check-mark (branch switch confirmed). */
export function CheckoutIcon() {
  return (
    <svg {...svgProps}>
      <path d="M3.5 8.5 L6.5 11.5 L12.5 4.5" />
    </svg>
  );
}

/** Create branch here — a trunk with a fork branching to a new dot. */
export function BranchIcon() {
  return (
    <svg {...svgProps}>
      <circle cx="4.5" cy="3" r="1.5" />
      <circle cx="4.5" cy="13" r="1.5" />
      <circle cx="11.5" cy="6.5" r="1.5" />
      <path d="M4.5 4.5 V11.5" />
      <path d="M4.5 8 C4.5 6 7 6.5 10 6.5" />
    </svg>
  );
}

/** Copy — two overlapping rounded rectangles (classic copy glyph). */
export function CopyIcon() {
  return (
    <svg {...svgProps}>
      {/* back sheet: L-shaped outline peeking out top-left */}
      <path d="M6 5.5 V3.5 a1 1 0 0 1 1 -1 H12.5 a1 1 0 0 1 1 1 V10 a1 1 0 0 1 -1 1 H11.5" />
      {/* front sheet */}
      <rect x="2.5" y="5" width="8" height="8.5" rx="1.4" />
    </svg>
  );
}

/** Merge — two branches converging into one (git-merge glyph). */
export function MergeIcon() {
  return (
    <svg {...svgProps}>
      <circle cx="4" cy="4" r="1.5" />
      <circle cx="12" cy="4" r="1.5" />
      <circle cx="8" cy="12.5" r="1.5" />
      <path d="M4 5.5 C4 8.5 8 8 8 11" />
      <path d="M12 5.5 C12 8.5 8 8 8 11" />
    </svg>
  );
}

/** Rebase — a branch lifted (arc) onto a baseline, with an up arrow. */
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

/** Compare — two side-by-side panes (split diff). */
export function CompareIcon() {
  return (
    <svg {...svgProps}>
      <rect x="2" y="3" width="12" height="10" rx="1.4" />
      <path d="M8 3 V13" />
    </svg>
  );
}

/** Delete / Drop — a trash can (lid + handle + tapering body + two strokes). */
export function DeleteIcon() {
  return (
    <svg {...svgProps}>
      {/* lid */}
      <path d="M2.5 4.5 H13.5" />
      {/* handle */}
      <path d="M6 4.5 V3.4 a0.8 0.8 0 0 1 0.8 -0.8 h2.4 a0.8 0.8 0 0 1 0.8 0.8 V4.5" />
      {/* body (tapering can) */}
      <path d="M3.8 4.5 L4.5 13.1 a1 1 0 0 0 1 0.9 H10.5 a1 1 0 0 0 1 -0.9 L12.2 4.5" />
      {/* two vertical strokes */}
      <path d="M7 7 V11.5" />
      <path d="M9 7 V11.5" />
    </svg>
  );
}

/** Create tag here (P22) — a luggage/price tag with a punch hole. */
export function TagIcon() {
  return (
    <svg {...svgProps}>
      <path d="M7.5 2.5 H12 a1.5 1.5 0 0 1 1.5 1.5 V8.5 L8 14 L2 8 Z" />
      <circle cx="10.5" cy="5.5" r="1" />
    </svg>
  );
}

/** Summarize (P15c) — a four-point sparkle (AI affordance, matches the ✨
 *  glyph used elsewhere for AI actions). */
export function SummarizeIcon() {
  return (
    <svg {...svgProps}>
      <path d="M8 2 L9.2 6.8 L14 8 L9.2 9.2 L8 14 L6.8 9.2 L2 8 L6.8 6.8 Z" />
    </svg>
  );
}

/** Stash Apply — the drawer/tray (echoes draw.ts drawStashIcon) with a down
 *  arrow going into the worktree. */
export function StashApplyIcon() {
  return (
    <svg {...svgProps}>
      {/* down arrow */}
      <path d="M8 1.5 V6" />
      <path d="M6 4 L8 6 L10 4" />
      {/* tray/drawer */}
      <rect x="3" y="8" width="10" height="6" rx="1.2" />
      <path d="M6 10.4 H10" />
    </svg>
  );
}

/** Stash Pop — the same tray with an up-and-out arrow (remove from stack). */
export function StashPopIcon() {
  return (
    <svg {...svgProps}>
      {/* up-and-out arrow */}
      <path d="M8 6 V1.5" />
      <path d="M6 3.5 L8 1.5 L10 3.5" />
      {/* tray/drawer */}
      <rect x="3" y="8" width="10" height="6" rx="1.2" />
      <path d="M6 10.4 H10" />
    </svg>
  );
}

/** History / reflog (P38) — a clock face with a counter-clockwise rewind arrow. */
export function HistoryIcon() {
  return (
    <svg {...svgProps}>
      <circle cx="8" cy="8.5" r="5" />
      {/* clock hands */}
      <path d="M8 5.5 V8.5 L10 10" />
      {/* rewind arrowhead at the top-left of the dial */}
      <path d="M3.4 5 L3.2 7.4 L5.6 7.2" />
    </svg>
  );
}
