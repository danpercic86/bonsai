// Spec-003 (UI contract §2.1/§2.2): the "Graph filters" popover — first-parent
// switch, the solo/hidden ref list (or the empty-state / stale sentence), and
// the "Show full graph" footer. Presentational: all state arrives via the
// useGraphFilter controller. No focus trap (house rule, ui-ref §12.4); Esc /
// click-away / focus-leave close it, restoring focus to the chip.
import { useEffect, useRef } from 'react';
import { EyeOff, ListFilter } from 'lucide-react';

import type { GraphFilterController } from '../hooks/useGraphFilter';
import { shortRefName } from '../hooks/useGraphFilter';
import { SettingsSwitch } from './settings/SettingsSwitch';

export interface GraphFilterPopoverProps {
  controller: GraphFilterController;
  /** Backend truth: saved refs matched nothing → the §2.2 stale sentence. */
  stale: boolean;
  belowBar: boolean;
  /** `restoreFocus` false on click-away/focus-leave (focus already moved). */
  onClose(restoreFocus: boolean): void;
}

export function GraphFilterPopover({
  controller,
  stale,
  belowBar,
  onClose,
}: GraphFilterPopoverProps) {
  const rootRef = useRef<HTMLDivElement | null>(null);
  const switchRef = useRef<HTMLDivElement | null>(null);

  // Initial focus = the switch (§2.1 keyboard).
  useEffect(() => {
    switchRef.current?.querySelector('input')?.focus();
  }, []);

  // §2.1: only click-AWAY closes. A click on the popover's own non-interactive
  // content (help text, header, padding) blurs the focused control with
  // relatedTarget null — that blur must be ignored, so track whether the last
  // mousedown originated inside the root.
  const pointerDownInsideRef = useRef(false);
  useEffect(() => {
    const onPointerDown = (e: MouseEvent) => {
      const inside =
        rootRef.current !== null && rootRef.current.contains(e.target as Node);
      pointerDownInsideRef.current = inside;
      if (!inside) onClose(false); // click-away (focus already moved — don't steal it back)
    };
    document.addEventListener('mousedown', onPointerDown);
    return () => document.removeEventListener('mousedown', onPointerDown);
  }, [onClose]);

  const rf = controller.refFilter;
  const refFilter = !stale && rf !== null && rf.refs.length > 0 ? rf : null;
  const nothingActive = !controller.requested;

  return (
    <div
      ref={rootRef}
      className={`graph-filter-popover${belowBar ? ' graph-filter-below-bar' : ''}`}
      role="dialog"
      aria-label="Graph filters"
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          onClose(true);
        }
      }}
      onBlur={(e) => {
        // Focus leaving the popover closes it (§2.1) — but NOT when the blur
        // came from a click on the popover's own non-interactive content.
        if (pointerDownInsideRef.current) {
          // Consume the flag so a later keyboard Tab-out still closes.
          pointerDownInsideRef.current = false;
          return;
        }
        if (
          rootRef.current !== null &&
          (e.relatedTarget === null || !rootRef.current.contains(e.relatedTarget as Node))
        ) {
          onClose(false);
        }
      }}
    >
      <div className="graph-filter-switch-row" ref={switchRef}>
        <SettingsSwitch
          id="graph-filter-first-parent"
          checked={controller.firstParent}
          describedBy="graph-filter-first-parent-help"
          onChange={() => controller.toggleFirstParent()}
        />
        <div>
          <label className="graph-filter-switch-label" htmlFor="graph-filter-first-parent">
            First-parent only
          </label>
          <p className="graph-filter-help" id="graph-filter-first-parent-help">
            {"Follow each commit's first parent. Merged-in side histories collapse."}
          </p>
        </div>
      </div>
      <hr className="graph-filter-divider" />
      {stale ? (
        // §2.2: with first-parent also on, the two-sentence copy — first-parent
        // DID apply, so never claim the full graph is shown.
        <p className="graph-filter-stale-note">
          {controller.firstParent
            ? "Your saved branch filter doesn't match any branches in this repository. First-parent is still applied."
            : "Your saved branch filter doesn't match any branches in this repository — showing the full graph."}
        </p>
      ) : refFilter !== null ? (
        <>
          <p className="graph-filter-section-header" aria-hidden="true">
            {refFilter.mode === 'solo' ? 'SOLO' : 'HIDDEN'}
          </p>
          <ul className="graph-filter-ref-list">
            {refFilter.refs.map((fullRef) => {
              const name = shortRefName(fullRef);
              const solo = refFilter.mode === 'solo';
              return (
                <li key={fullRef} className="graph-filter-ref-row">
                  <span
                    className={`graph-filter-ref-glyph ${
                      solo ? 'graph-filter-ref-glyph-solo' : 'graph-filter-ref-glyph-hidden'
                    }`}
                    aria-hidden="true"
                  >
                    {solo ? (
                      <ListFilter size={12} strokeWidth={2} />
                    ) : (
                      <EyeOff size={12} strokeWidth={2} />
                    )}
                  </span>
                  <span className="graph-filter-ref-name" title={fullRef}>
                    {name}
                  </span>
                  <button
                    type="button"
                    className="graph-filter-ref-remove"
                    aria-label={solo ? `Stop soloing ${name}` : `Show ${name}`}
                    onClick={() => controller.unfilterRef(fullRef)}
                  >
                    {'✕'}
                  </button>
                </li>
              );
            })}
          </ul>
        </>
      ) : (
        <p className="graph-filter-empty">
          No branch filters. Right-click a branch in the sidebar to solo or hide it.
        </p>
      )}
      <button
        type="button"
        className="btn-secondary graph-filter-footer-btn"
        aria-label={stale ? 'Clear saved filter' : 'Clear all graph filters'}
        disabled={nothingActive && !stale}
        onClick={() => controller.clearAll()}
      >
        {stale ? 'Clear saved filter' : 'Show full graph'}
      </button>
    </div>
  );
}
