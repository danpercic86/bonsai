// P90: idle / noForge / noChecks / error empty-and-hint variants (§4). Each is
// the in-pane empty pattern: title + one-line reason (+ an action where one exists).
import type { ChecksTarget } from './checksTarget';

interface CommonProps {
  target: ChecksTarget | null;
}

export function ChecksIdle() {
  return (
    <div className="checks-state">
      <p className="pane-empty">
        <strong className="pane-empty-title">Pick a branch</strong>
        <span className="pane-empty-reason">
          Select a branch in the sidebar to see its CI checks.
        </span>
      </p>
    </div>
  );
}

export function ChecksNoForge({ host }: { host: string | null }) {
  return (
    <div className="checks-state">
      <p className="pane-empty">
        <strong className="pane-empty-title">No CI checks here</strong>
        <span className="pane-empty-reason">
          {host !== null
            ? `${host} isn't a supported forge yet, so Bonsai can't read its checks.`
            : "This repository's origin isn't a supported forge."}
        </span>
      </p>
    </div>
  );
}

/** §4.4 — local branch with no upstream: offer a Push action. */
export function ChecksNoUpstream({
  target,
  onPush,
}: CommonProps & { onPush?(): void }) {
  const branch = target?.name ?? 'this branch';
  return (
    <div className="checks-state">
      <p className="pane-empty">
        <strong className="pane-empty-title">No checks for this branch</strong>
        <span className="pane-empty-reason">
          {`${branch} hasn't been pushed, so there are no CI results yet.`}
        </span>
        {onPush !== undefined && (
          <button type="button" className="btn-primary pane-empty-action" onClick={onPush}>
            Push branch
          </button>
        )}
      </p>
    </div>
  );
}

/** §4.5 — pushed, but the tip has no status reported yet. */
export function ChecksWaiting({ target }: CommonProps) {
  const shortSha = target?.tip.slice(0, 7) ?? 'the tip';
  return (
    <div className="checks-state">
      <p className="pane-empty">
        <strong className="pane-empty-title">Waiting for checks</strong>
        <span className="pane-empty-reason">
          {`No checks have reported for ${shortSha} yet.`}
        </span>
      </p>
    </div>
  );
}

/** §4.6 — forge returned an empty set: no CI configured for this branch. */
export function ChecksNoChecks({ target }: CommonProps) {
  const branch = target?.name ?? 'this branch';
  return (
    <div className="checks-state">
      <p className="pane-empty">
        <strong className="pane-empty-title">No checks configured</strong>
        <span className="pane-empty-reason">
          {`This repository doesn't run CI on ${branch}.`}
        </span>
      </p>
    </div>
  );
}

export function ChecksError({ message, onRetry }: { message: string; onRetry(): void }) {
  return (
    <div className="checks-state">
      <div className="error-banner error-banner-dismissible checks-error" role="alert">
        <span className="error-banner-text">{`Couldn't load checks. ${message}`}</span>
        <button type="button" className="section-action" onClick={onRetry}>
          Retry
        </button>
      </div>
    </div>
  );
}
