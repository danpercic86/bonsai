// Spec-003 (UI contract §2): the graph-pane filter chip — icon-only ghost
// button when nothing is requested, a labeled chip (the AC5 "filtered"
// indicator) when a filter is active, and the §2.2 warning variant when the
// backend reports the saved seed-ref restriction did not apply. Owns the
// popover-open state; everything else is threaded from RepoWorkspace via
// useGraphFilter. Presentational beyond that one bit of UI state.
import { useCallback, useRef, useState } from 'react';
import { ListFilter } from 'lucide-react';

import type { GraphFilterController } from '../hooks/useGraphFilter';
import { graphFilterSummary } from '../hooks/useGraphFilter';
import { GraphFilterPopover } from './GraphFilterPopover';

export interface GraphFilterChipProps {
  controller: GraphFilterController;
  /** Backend truth: the request carried seedRefs but they did not apply. */
  stale: boolean;
  /** Shifted below the open search bar (§2 gate). */
  belowBar: boolean;
  /** While the popover is open App's global shortcuts are suppressed
   *  (TabStrip/identity-menu precedent). */
  onMenuOpenChange(open: boolean): void;
  /** Focus fallback when the chip unmounted on close (§2.1): `.graph-scroll`. */
  focusGraph(): void;
}

export function GraphFilterChip({
  controller,
  stale,
  belowBar,
  onMenuOpenChange,
  focusGraph,
}: GraphFilterChipProps) {
  const [open, setOpen] = useState(false);
  const chipRef = useRef<HTMLButtonElement | null>(null);

  const setOpenLifted = useCallback(
    (next: boolean) => {
      setOpen(next);
      onMenuOpenChange(next);
    },
    [onMenuOpenChange],
  );
  const close = useCallback(
    (restoreFocus: boolean) => {
      setOpenLifted(false);
      if (restoreFocus) {
        if (chipRef.current !== null) chipRef.current.focus();
        else focusGraph();
      }
    },
    [setOpenLifted, focusGraph],
  );

  const active = controller.requested;
  const shift = belowBar ? ' graph-filter-below-bar' : '';
  // Spec-004 §4.2: stale concerns only the REF filter. While fold and/or
  // first-parent are also on, the chip keeps the normal active recipe with the
  // honest segments (ref segment dropped) and a ⚠ glyph; the stale sentence
  // lives in the popover. The full warning chip renders only when stale is the
  // SOLE state.
  const staleOthers = stale
    ? graphFilterSummary(controller.firstParent, null, controller.foldLinear)
    : '';
  const fullWarning = stale && staleOthers === '';
  const label = stale ? staleOthers : controller.activeSummary;

  return (
    <>
      {!active ? (
        // Quiet chrome, not the indicator — identical weight to the search fab.
        <button
          ref={chipRef}
          type="button"
          className={`graph-filter-fab${shift}`}
          aria-label="Graph filters"
          title="Graph filters"
          aria-haspopup="dialog"
          aria-expanded={open}
          onClick={() => setOpenLifted(!open)}
        >
          <ListFilter size={16} strokeWidth={2} aria-hidden="true" />
        </button>
      ) : fullWarning ? (
        // §2.2: stale is the SOLE state → the full warning chip.
        <span className={`graph-filter-chip graph-filter-chip-stale${shift}`}>
          <button
            ref={chipRef}
            type="button"
            className="graph-filter-chip-body"
            aria-haspopup="dialog"
            aria-expanded={open}
            aria-label="Graph filters: Filter not applied"
            title="Your saved branch filter doesn't match any branches in this repository — showing the full graph."
            onClick={() => setOpenLifted(!open)}
          >
            <span className="graph-filter-chip-glyph" aria-hidden="true">
              {'⚠'}
            </span>
            <span className="graph-filter-chip-label">Filter not applied</span>
          </button>
          <button
            type="button"
            className="graph-filter-clear"
            aria-label="Clear graph filters"
            title="Clear graph filters — show the full graph"
            onClick={() => controller.clearAll()}
          >
            {'✕'}
          </button>
        </span>
      ) : (
        <span className={`graph-filter-chip${shift}`}>
          <button
            ref={chipRef}
            type="button"
            className="graph-filter-chip-body"
            aria-haspopup="dialog"
            aria-expanded={open}
            aria-label={`Graph filters: ${label}`}
            title={`${label}. Graph is filtered — some commits are hidden. Click to review or clear.`}
            onClick={() => setOpenLifted(!open)}
          >
            {/* Spec-004 §4.2: stale-with-other-declutter swaps only the glyph. */}
            <span
              className={`graph-filter-chip-glyph${stale ? ' graph-filter-chip-glyph-warning' : ''}`}
              aria-hidden="true"
            >
              {stale ? '⚠' : <ListFilter size={14} strokeWidth={2} />}
            </span>
            <span className="graph-filter-chip-label">{label}</span>
          </button>
          <button
            type="button"
            className="graph-filter-clear"
            aria-label="Clear graph filters"
            title="Clear graph filters — show the full graph"
            onClick={() => controller.clearAll()}
          >
            {'✕'}
          </button>
        </span>
      )}
      {open && (
        <GraphFilterPopover
          controller={controller}
          stale={stale}
          belowBar={belowBar}
          onClose={close}
        />
      )}
    </>
  );
}
