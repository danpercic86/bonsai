// P125-ui §4.1 — title, age caption (+ progress while running), the hueless
// `stale` pill, Check now, close ×, and the permanently mounted polite live
// region. Progress numbers are shown, never announced.
import type { DepProgress } from '../../hooks/useDepTracker';
import { formatAge } from '../../lib/formatAge';
import {
  CHECKING,
  CHECK_NOW,
  CLOSE,
  DIALOG_TITLE,
  STALE_PILL,
  STALE_PILL_TITLE,
  checkedCaption,
  checkedTitle,
  progressCaption,
} from './depsCopy';
import { RowTag } from './DepsPills';

export interface DepsHeaderProps {
  titleId: string;
  /** `report.checkedAtMs`, or null with no report. */
  checkedAtMs: number | null;
  nowMs: number;
  running: boolean;
  progress: DepProgress | null;
  stalePill: boolean;
  /** Absent while the tracker is off. */
  showCheckNow: boolean;
  live: string;
  onCheckNow(): void;
  onClose(): void;
}

export function DepsHeader({
  titleId,
  checkedAtMs,
  nowMs,
  running,
  progress,
  stalePill,
  showCheckNow,
  live,
  onCheckNow,
  onClose,
}: DepsHeaderProps) {
  const parts: string[] = [];
  if (checkedAtMs !== null) parts.push(checkedCaption(formatAge(checkedAtMs, nowMs)));
  if (running) {
    parts.push(
      progress !== null && progress.total > 0 ? progressCaption(progress.done, progress.total) : CHECKING,
    );
  }
  const caption = parts.join(' · ');
  return (
    <div className="shortcut-header deps-header">
      <h2 id={titleId} className="dialog-title shortcut-title">
        {DIALOG_TITLE}
      </h2>
      <div className="asset-header-actions">
        {caption !== '' && (
          <span
            className="health-caption deps-caption"
            title={checkedAtMs !== null ? checkedTitle(new Date(checkedAtMs).toLocaleString()) : undefined}
          >
            {caption}
          </span>
        )}
        {stalePill && <RowTag title={STALE_PILL_TITLE}>{STALE_PILL}</RowTag>}
        {showCheckNow && (
          <button type="button" className="btn-secondary deps-check-now" disabled={running} onClick={onCheckNow}>
            {running ? CHECKING : CHECK_NOW}
          </button>
        )}
        <button type="button" className="btn-icon shortcut-close" aria-label={CLOSE} title={CLOSE} onClick={onClose}>
          {'×'}
        </button>
      </div>
      <p className="sr-only" role="status" aria-live="polite">
        {live}
      </p>
    </div>
  );
}
