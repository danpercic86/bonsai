// Spec-003 (UI contract §3.3): the trailing sidebar row marker for refs in an
// active solo/hide set, plus the context that carries the membership lookup so
// the deep row components don't each thread a prop. Rows stay fully interactive
// (never dimmed); the state is folded into the accessible name via `.sr-only`.
import { useContext } from 'react';
import { EyeOff, ListFilter } from 'lucide-react';

import { RefFilterMarkerContext } from './refFilterMarkerContext';

/** The trailing 12px marker glyph + visually-hidden name suffix for one row.
 *  Renders nothing when the ref is not in an active set. */
export function RefFilterMarker({ fullRef }: { fullRef: string }) {
  const markerFor = useContext(RefFilterMarkerContext);
  const mark = markerFor(fullRef);
  if (mark === null) return null;
  return mark === 'solo' ? (
    <>
      <span
        className="ref-filter-marker ref-filter-marker-solo"
        title="Solo — the graph shows only these branches (plus HEAD)"
      >
        <ListFilter size={12} strokeWidth={2} aria-hidden="true" />
      </span>
      <span className="sr-only">{", solo'd in graph"}</span>
    </>
  ) : (
    <>
      <span className="ref-filter-marker ref-filter-marker-hidden" title="Hidden from the graph">
        <EyeOff size={12} strokeWidth={2} aria-hidden="true" />
      </span>
      <span className="sr-only">, hidden from graph</span>
    </>
  );
}
