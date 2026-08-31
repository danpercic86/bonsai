/**
 * P87b §3.3 — the git-activity dock's COLLAPSED status bar: the active (or latest)
 * run's glyph + noun, its status pill and live phase detail, the live elapsed, a
 * Clear action and the collapse chevron. PURE — every decision is made in
 * `GitActivityDock` and handed down.
 */
import {
  categoryMeta,
  durationLabel,
  objectsReadout,
  phaseLabel,
  statusPill,
} from './gitActivityFormat';
import type { GitActivityRun } from './repoWorkspace/useGitActivity';

export interface GitActivityHeaderProps {
  /** `activeRun ?? runs[0]`. Null only in the (rare) forced-open empty state. */
  lead: GitActivityRun | null;
  tick: number;
  collapsed: boolean;
  onToggleCollapsed(next: boolean): void;
  onClear(): void;
  clearDisabled: boolean;
}

export function GitActivityHeader(props: GitActivityHeaderProps) {
  const { lead, tick, collapsed, onToggleCollapsed, onClear, clearDisabled } = props;

  const meta = lead !== null ? categoryMeta(lead.category) : null;
  const pill = lead !== null ? statusPill(lead.status) : null;
  const Glyph = meta?.glyph ?? null;
  const running = lead?.status === 'running';
  const detail =
    running && lead !== null
      ? (objectsReadout(lead) ?? phaseLabel(lead.category, lead.phase))
      : null;

  return (
    <div className="git-dock-header">
      <button
        type="button"
        className="git-dock-toggle"
        aria-expanded={!collapsed}
        aria-controls="git-dock-body"
        aria-label="Git activity"
        title={collapsed ? 'Show git activity' : 'Hide git activity'}
        onClick={() => onToggleCollapsed(!collapsed)}
      >
        <span aria-hidden="true">{collapsed ? '⌃' : '⌄'}</span>
      </button>

      {lead !== null && meta !== null && pill !== null ? (
        <>
          <span className="git-dock-glyph" aria-hidden="true">
            {Glyph !== null && <Glyph />}
          </span>
          <span className="git-dock-noun">{meta.noun}</span>
          <span className="git-dock-status" data-status={pill.dataStatus}>
            <span className="git-run-pill-glyph" aria-hidden="true">
              {pill.glyph}
            </span>
            {pill.label}
          </span>
          {detail !== null && (
            <span className="git-dock-detail" title={phaseLabel(lead.category, lead.phase)}>
              {`· ${detail}`}
            </span>
          )}
          <span className="git-dock-elapsed">{durationLabel(lead, tick)}</span>
        </>
      ) : (
        <span className="git-dock-noun git-dock-idle">Git activity</span>
      )}

      <button
        type="button"
        className="git-dock-clear"
        aria-label="Clear git activity log"
        disabled={clearDisabled}
        onClick={onClear}
      >
        Clear
      </button>
    </div>
  );
}
