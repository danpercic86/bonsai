// P121c — the Tags section's §2.3 offline line as a LEAF, so its "Last checked
// 5m ago" tooltip can read the shared clock (`useNow`) without the memoised
// section taking a ticking `now` prop. Rendered only while the check is
// `unavailable`; informational, never an error banner.
import type { ReactElement } from 'react';
import { relativeDate } from '../../graph/dates';
import { useNow } from '../../hooks/useNow';

export function TagSyncOfflineNote({
  remote,
  checkedAt,
}: {
  /** The remote the check targeted (report's echo, else the resolved default). */
  remote: string | null;
  /** Unix secs of the last successful check, or null if none ever succeeded. */
  checkedAt: number | null;
}): ReactElement {
  const nowSec = Math.floor(useNow() / 1000);
  return (
    <p
      className="branch-muted"
      title={checkedAt !== null ? `Last checked ${relativeDate(checkedAt, nowSec)}` : undefined}
    >
      {remote !== null
        ? `Couldn't reach ${remote} — showing local tags only.`
        : `Couldn't reach the remote — showing local tags only.`}
    </p>
  );
}
