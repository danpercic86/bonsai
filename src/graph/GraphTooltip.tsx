/** P7 §6 — the graph hover tooltip, extracted verbatim from `GraphCanvas.tsx`
 *  (presentational only; all hit-testing and positioning stays in the container).
 *  Extracted in P95 to keep `GraphCanvas.tsx` shrinking rather than growing. */
import type { Ref } from 'react';

import type { TooltipState } from './hitTest';

interface Props {
  tooltip: TooltipState;
  /** Measured/flipped position; falls back to the raw anchor before measurement. */
  pos: { left: number; top: number } | null;
  tipRef: Ref<HTMLDivElement>;
}

export function GraphTooltip({ tooltip, pos, tipRef }: Props) {
  return (
    <div
      ref={tipRef}
      className="graph-tooltip"
      role="tooltip"
      style={{
        left: `${pos?.left ?? tooltip.anchor.left}px`,
        top: `${pos?.top ?? tooltip.anchor.top + tooltip.anchor.height + 4}px`,
      }}
    >
      {tooltip.kind === 'overflow' ||
      tooltip.kind === 'date' ||
      tooltip.kind === 'pr' ||
      tooltip.kind === 'ci'
        ? tooltip.lines.map((l, i) => <div key={i}>{l}</div>)
        : tooltip.text}
    </div>
  );
}
