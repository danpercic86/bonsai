// P121e — the Tags header's rollup pill as a LEAF: it alone reads the
// in-flight `checking` flag (via context), so a check starting/settling
// re-renders this pill, not the memoised `TagsSection` body or its rows.
// P123: a re-check KEEPS the settled verdict pill (+ re-check cue); `checking…`
// shows only before the first settled result.
import type { ReactElement } from 'react';
import type { TagSyncState } from './TagsSection';
import { SectionRollupBadge } from './SectionRollupBadge';
import { useTagSyncChecking } from './tagSyncCheckingContext';

export function TagSyncRollup({
  count,
  remote,
  state,
}: {
  /** True divergences only (stale / deleted-on-remote). */
  count: number;
  /** The remote the verdict is against, for the accessible name + tooltip. */
  remote: string | null;
  /** The settled lifecycle; `'checking'` only on the very first check. */
  state: TagSyncState;
}): ReactElement {
  const busy = state === 'checking';
  const refreshing = useTagSyncChecking() && state !== 'checking';
  const tags = count === 1 ? '1 tag' : `${count} tags`;
  const name = `${tags} out of sync${remote !== null ? ` on ${remote}` : ''}`;
  const differ = `${tags} ${count === 1 ? 'differs' : 'differ'} from ${remote ?? 'the remote'}`;
  return (
    <SectionRollupBadge
      count={count}
      busy={busy}
      refreshing={refreshing}
      label={'checking…'}
      ariaLabel={refreshing ? `${name}, checking again` : name}
      title={refreshing ? `${differ} — checking again…` : `${differ}. Expand to resolve.`}
    />
  );
}
