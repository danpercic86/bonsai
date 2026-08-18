/**
 * P68e §1.6 — the run strip, shown only when more than one AI run exists.
 *
 * Split out of `AiActivityPanel.tsx` (which was heading past its ~150-line budget):
 * a `role="tablist"` with a roving tabindex and its own Arrow/Home/End handling is a
 * self-contained concern, and keeping it here leaves the panel a composition site.
 *
 * PURE — no state. The selected key is owned by the container, so the strip cannot
 * disagree with the body it labels.
 */
import { useRef } from 'react';

import { AI_MAX_CONCURRENT_RUNS } from '../settings/ranges';
import { pillFor, type AiActivityRun } from './aiDockFormat';
import { splitPath } from './StatusFileRow';

export interface AiRunStripProps {
  runs: AiActivityRun[];
  activeKey: string | null;
  onSelectRun(key: string): void;
  /** Store `atCapacity` → the `N of N running` counter (§D concurrency cap). */
  atCapacity: boolean;
}

export function AiRunStrip({ runs, activeKey, onSelectRun, atCapacity }: AiRunStripProps) {
  // The ARIA tabs pattern requires FOCUS to follow selection: without this the
  // previously-focused chip keeps DOM focus while dropping to `tabIndex={-1}`, so the
  // focus ring disagrees with `aria-selected` and the next `Tab` leaves from the wrong
  // place. Focusing the target node directly is legal even before React re-renders it
  // with `tabIndex={0}` (programmatic focus ignores the tab order).
  const chips = useRef(new Map<string, HTMLButtonElement>());

  function onKeyDown(e: React.KeyboardEvent<HTMLButtonElement>) {
    const at = runs.findIndex((r) => r.key === activeKey);
    const to =
      e.key === 'ArrowRight'
        ? at + 1
        : e.key === 'ArrowLeft'
          ? at - 1
          : e.key === 'Home'
            ? 0
            : e.key === 'End'
              ? runs.length - 1
              : -1;
    if (to < 0 || to >= runs.length) return;
    e.preventDefault();
    const next = runs[to];
    if (next === undefined) return;
    onSelectRun(next.key);
    chips.current.get(next.key)?.focus();
  }

  return (
    <div className="ai-dock-runs" role="tablist" aria-label="AI runs">
      {runs.map((run) => {
        const selected = run.key === activeKey;
        // U8: the strip uses the SAME glyph set as the pill — a `failed` or `cancelled`
        // chip showing ✨ would say "still working" in the one place colour is the only
        // other cue. §1.6: the chip is the basename; the full path is the `title`.
        const pill = pillFor(run.status, run.cancelRequested);
        return (
          <button
            key={run.key}
            ref={(el) => {
              if (el === null) chips.current.delete(run.key);
              else chips.current.set(run.key, el);
            }}
            type="button"
            id={`ai-dock-tab-${run.key}`}
            className="ai-dock-run-chip"
            role="tab"
            data-status={run.status}
            aria-selected={selected}
            tabIndex={selected ? 0 : -1}
            title={run.label}
            onClick={() => onSelectRun(run.key)}
            onKeyDown={onKeyDown}
          >
            <span aria-hidden="true">{pill.glyph}</span>
            {` ${splitPath(run.label).name}`}
          </button>
        );
      })}
      {atCapacity && (
        <span className="ai-dock-capacity">
          {`${AI_MAX_CONCURRENT_RUNS} of ${AI_MAX_CONCURRENT_RUNS} running`}
        </span>
      )}
    </div>
  );
}
