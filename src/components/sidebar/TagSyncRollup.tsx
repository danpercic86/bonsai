// P121e — the Tags header's rollup pill as a LEAF: it alone reads the
// in-flight `checking` flag (via context), so a check starting/settling
// re-renders this pill, not the memoised `TagsSection` body or its rows.
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
  const busy = useTagSyncChecking() || state === 'checking';
  return (
    <SectionRollupBadge
      count={count}
      busy={busy}
      label={'checking…'}
      ariaLabel={`${count} tags out of sync${remote !== null ? ` on ${remote}` : ''}`}
      title={`${count} tags differ from ${remote ?? 'the remote'}. Expand to resolve.`}
    />
  );
}
