/** P7 §6 hover tooltip positioning — moved out of GraphCanvas.tsx (spec-004
 *  size split): renders at the un-clamped anchor first, then a layout effect
 *  clamps it inside the host before paint (flicker-free). The hover TARGET
 *  resolution stays in GraphCanvas and the tooltip's DOM lives in
 *  `GraphTooltip.tsx` (P95 extraction, reunified in the 2026-09-02 dev merge —
 *  both branches had split this component out independently); this component
 *  owns only the clamped position and the measurement ref. */

import { useLayoutEffect, useRef, useState } from 'react';
import type { RefObject } from 'react';
import type { TooltipState } from './hitTest';
import { clampTooltipPos } from './viewport';
import { GraphTooltip } from './GraphTooltip';

export function GraphTooltipOverlay({
  tooltip,
  hostRef,
}: {
  tooltip: TooltipState | null;
  hostRef: RefObject<HTMLDivElement | null>;
}) {
  const tipRef = useRef<HTMLDivElement>(null);
  const [tipPos, setTipPos] = useState<{ left: number; top: number } | null>(null);

  // P7 §6.2: clamp the tooltip inside the host. Runs synchronously after the
  // tooltip renders (at its un-clamped anchor point) but before paint, so the
  // correction is flicker-free. Default below the anchor; flip above / pull
  // left when it would overflow the host edges.
  useLayoutEffect(() => {
    if (tooltip === null) {
      setTipPos(null);
      return;
    }
    const tip = tipRef.current;
    const host = hostRef.current;
    if (tip === null || host === null) return;
    setTipPos(
      clampTooltipPos(
        tooltip.anchor,
        tip.offsetWidth,
        tip.offsetHeight,
        host.clientWidth,
        host.clientHeight,
      ),
    );
  }, [tooltip, hostRef]);

  if (tooltip === null) return null;
  // The tooltip DOM itself lives in GraphTooltip.tsx (P95 extraction); this
  // overlay owns only the clamped position and the ref.
  return <GraphTooltip tooltip={tooltip} pos={tipPos} tipRef={tipRef} />;
}
