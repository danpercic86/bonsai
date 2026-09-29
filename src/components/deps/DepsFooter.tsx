// P125-ui §8.3 footer — the Settings link and `Next check in {in}` (omitted
// when there is no schedule, or while a check runs).
import { formatIn } from '../../lib/formatAge';
import { FOOTER_SETTINGS, nextCheckIn } from './depsCopy';

export function DepsFooter({
  nextCheckMs,
  nowMs,
  onOpenSettings,
}: {
  nextCheckMs: number | null;
  nowMs: number;
  onOpenSettings(): void;
}) {
  return (
    <div className="deps-footer">
      <button type="button" className="deps-link" onClick={onOpenSettings}>
        {FOOTER_SETTINGS}
      </button>
      {nextCheckMs !== null && <span className="deps-footer-next">{nextCheckIn(formatIn(nextCheckMs, nowMs))}</span>}
    </div>
  );
}
