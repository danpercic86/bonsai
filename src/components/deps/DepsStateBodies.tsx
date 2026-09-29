// P125-ui §6 — the four non-list bodies. Exactly one body renders (the
// container picks it from `deriveState`).
import type { DepCheckFailureKind } from '../../ipc/types/depTracker';
import { DepConsentCopy } from './DepConsentCopy';
import {
  EMPTY_REASON,
  EMPTY_TITLE,
  OFF_HEADING,
  OPEN_SETTINGS,
  TRY_AGAIN,
  TURN_ON,
  failureBody,
} from './depsCopy';

/** §8.1: the consent copy is on screen, so clicking Turn on IS the consent. */
export function DepsOffBody({
  intervalHours,
  onTurnOn,
  onOpenSettings,
}: {
  intervalHours: number;
  onTurnOn(): void;
  onOpenSettings(): void;
}) {
  return (
    <div className="deps-off">
      <h3 className="deps-state-title">{OFF_HEADING}</h3>
      <DepConsentCopy intervalHours={intervalHours} />
      <div className="deps-off-actions">
        <button type="button" className="btn-primary" onClick={onTurnOn}>
          {TURN_ON}
        </button>
        <button type="button" className="deps-link" onClick={onOpenSettings}>
          {OPEN_SETTINGS}
        </button>
      </div>
    </div>
  );
}

/** Summary-bar skeleton + 3 skeleton groups × 4 rows (§6). Static under
 *  reduced motion via the shared `.skeleton-row` rule. */
export function DepsLoadingBody() {
  return (
    <div className="deps-loading" aria-hidden="true">
      <div className="deps-skeleton-bar skeleton-row" />
      {[0, 1, 2].map((g) => (
        <div key={g} className="skeleton-group deps-skeleton-group">
          {[0, 1, 2, 3].map((r) => (
            <div key={r} className="skeleton-row" />
          ))}
        </div>
      ))}
    </div>
  );
}

/** Nothing for the user to fix, so no action (Check now stays in the header). */
export function DepsEmptyBody() {
  return (
    <div className="deps-state">
      <p className="deps-state-title">{EMPTY_TITLE}</p>
      <p className="deps-state-reason">{EMPTY_REASON}</p>
    </div>
  );
}

/** No report and the last attempt failed: the pane empty-state block, never
 *  the danger banner (nothing was lost). `message` goes in `title` only. */
export function DepsFailedBody({
  kind,
  details,
  running,
  onRetry,
}: {
  kind: DepCheckFailureKind;
  details: string;
  running: boolean;
  onRetry(): void;
}) {
  const copy = failureBody(kind);
  return (
    <div className="deps-state" title={details}>
      <p className="deps-state-title">{copy.title}</p>
      <p className="deps-state-reason">{copy.reason}</p>
      <button type="button" className="btn-secondary" disabled={running} onClick={onRetry}>
        {TRY_AGAIN}
      </button>
    </div>
  );
}
