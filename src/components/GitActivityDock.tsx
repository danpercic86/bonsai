/**
 * P87b §3 — View D, the git activity dock. The git twin of the AI dock (§9):
 * a bottom, full-width, collapsible/resizable disclosure with its own small
 * component tree. Deliberate divergences from the AI dock (§3.1):
 *  - it returns `null` only BEFORE the first git op of the session, then stays
 *    mounted for the session (an always-on record + a persistent live region);
 *  - it NEVER auto-expands (git ops are frequent).
 *
 * NO height animation (the §9 canvas-relayout prohibition): collapse/expand/resize
 * snap. ONE polite live region announces phase transitions + terminal results only
 * (never output lines, never progress ticks).
 */
import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
} from 'react';

import { GitActivityHeader } from './GitActivityHeader';
import { GitActivityRow } from './GitActivityRow';
import { PaneDivider } from './PaneDivider';
import {
  GIT_DOCK_HEIGHT_DEFAULT,
  GIT_DOCK_HEIGHT_MAX,
  GIT_DOCK_HEIGHT_MIN,
  GIT_DOCK_NUDGE_PX,
  clampGitDockHeight,
  gitAnnounceFor,
} from './gitActivityFormat';
import type { PanelDensity } from '../ipc';
import type { GitActivityRun } from './repoWorkspace/useGitActivity';

export interface GitActivityDockProps {
  /** Newest-first (View D log). `[]` before the first op ⇒ the shell renders null. */
  runs: GitActivityRun[];
  activeRun: GitActivityRun | null;
  tick: number;
  collapsed: boolean;
  onToggleCollapsed(next: boolean): void;
  /** 120..600, session-scoped (§3.1 / Q5). */
  height: number;
  onResizeHeight(next: number): void;
  onClear(): void;
  hasTerminalRuns: boolean;
  density: PanelDensity;
}

export interface GitActivityDockHandle {
  /** Palette / `Ctrl-Shift-L` / the clickable toolbar readout: expand + focus the
   *  list (the caller flips `collapsed`; this moves focus once mounted). */
  focusLog(): void;
}

export const GitActivityDock = forwardRef<GitActivityDockHandle, GitActivityDockProps>(
  function GitActivityDock(props, ref) {
    const {
      runs,
      activeRun,
      tick,
      collapsed,
      onToggleCollapsed,
      height,
      onResizeHeight,
      onClear,
      hasTerminalRuns,
      density,
    } = props;

    // §3.1 divergence: once shown, stay mounted for the session. A ref, so a Clear
    // that empties `runs` cannot unmount the live region.
    const everShown = useRef(false);
    if (runs.length > 0) everShown.current = true;

    const listRef = useRef<HTMLOListElement | null>(null);
    const [dragHeight, setDragHeight] = useState<number | null>(null);
    const dragRef = useRef<number | null>(null);
    const [announce, setAnnounce] = useState('');
    const seenRef = useRef(new Map<string, string>());

    // §6: announce phase transitions + terminal results only.
    useEffect(() => {
      const next = gitAnnounceFor(runs, seenRef.current);
      if (next !== null) setAnnounce(next);
    }, [runs]);

    useImperativeHandle(ref, () => ({
      focusLog() {
        window.setTimeout(() => listRef.current?.focus(), 0);
      },
    }));

    const commitHeight = useCallback(() => {
      const live = dragRef.current;
      dragRef.current = null;
      setDragHeight(null);
      if (live !== null) onResizeHeight(live);
    }, [onResizeHeight]);

    const onDrag = useCallback(
      (delta: number) => {
        const next = clampGitDockHeight((dragRef.current ?? height) + delta, window.innerHeight);
        dragRef.current = next;
        setDragHeight(next);
      },
      [height],
    );

    const onDragStart = useCallback(() => {
      dragRef.current = height;
    }, [height]);

    // §6 keyboard: ArrowUp/Down move row focus among the summary rows.
    const onListKeyDown = useCallback((e: React.KeyboardEvent<HTMLOListElement>) => {
      if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
      const ol = listRef.current;
      if (ol === null) return;
      const rows = Array.from(ol.querySelectorAll<HTMLElement>('[data-run-row]'));
      if (rows.length === 0) return;
      e.preventDefault();
      const active = document.activeElement;
      const current = rows.findIndex((r) => r === active || r.contains(active));
      const delta = e.key === 'ArrowDown' ? 1 : -1;
      const nextIdx =
        current === -1
          ? e.key === 'ArrowDown'
            ? 0
            : rows.length - 1
          : Math.max(0, Math.min(rows.length - 1, current + delta));
      rows[nextIdx]?.focus();
    }, []);

    // §6: Esc collapses the dock when focus is inside it (without losing the log).
    const onSectionKeyDown = (e: React.KeyboardEvent<HTMLElement>) => {
      if (e.key === 'Escape' && !collapsed) {
        e.stopPropagation();
        onToggleCollapsed(true);
      }
    };

    if (!everShown.current) return null;

    const effectiveHeight = dragHeight ?? height;
    const lead = activeRun ?? runs[0] ?? null;

    return (
      <section
        className="git-activity-dock"
        role="region"
        aria-label="Git activity"
        data-density={density}
        onKeyDown={onSectionKeyDown}
      >
        {activeRun !== null && <div className="git-dock-progress" aria-hidden="true" />}

        {!collapsed && (
          <PaneDivider
            side="git-dock"
            onResize={onDrag}
            onResizeStart={onDragStart}
            onResizeEnd={commitHeight}
            onReset={() => onResizeHeight(GIT_DOCK_HEIGHT_DEFAULT)}
            onExtreme={(edge) =>
              onResizeHeight(
                edge === 'min'
                  ? GIT_DOCK_HEIGHT_MIN
                  : clampGitDockHeight(GIT_DOCK_HEIGHT_MAX, window.innerHeight),
              )
            }
            ariaLabel="Resize git activity dock"
            ariaValues={{ now: effectiveHeight, min: GIT_DOCK_HEIGHT_MIN, max: GIT_DOCK_HEIGHT_MAX }}
            nudgePx={GIT_DOCK_NUDGE_PX}
          />
        )}

        <GitActivityHeader
          lead={lead}
          tick={tick}
          collapsed={collapsed}
          onToggleCollapsed={onToggleCollapsed}
          onClear={onClear}
          clearDisabled={!hasTerminalRuns}
        />

        {!collapsed && (
          <div className="git-dock-body" id="git-dock-body" style={{ height: `${effectiveHeight}px` }}>
            {runs.length === 0 ? (
              <div className="git-dock-empty">
                <p className="git-dock-empty-title">No git activity yet.</p>
                <p className="git-dock-empty-hint">
                  Fetch, pull, push, or commit and it will show up here.
                </p>
              </div>
            ) : (
              <ol
                className="git-activity-rows"
                ref={listRef}
                tabIndex={0}
                aria-label="Git activity log"
                onKeyDown={onListKeyDown}
              >
                {runs.map((run) => (
                  <GitActivityRow key={run.id} run={run} tick={tick} />
                ))}
              </ol>
            )}
          </div>
        )}

        <p
          className="git-dock-announce sr-only"
          role="status"
          aria-label="Git activity"
          aria-live="polite"
          aria-atomic="true"
        >
          {announce}
        </p>
      </section>
    );
  },
);
