// P121c — a relative-age label ("5m") that re-renders ITSELF on the shared
// clock (`useNow`), so the parent row/section never sees the tick and can stay
// memoised on its real props.
import type { ReactElement } from 'react';
import { relativeDate } from '../graph/dates';
import { useNow } from '../hooks/useNow';

/** Re-renders itself on the shared clock; parents never see the tick. */
export function RelativeAge({ tsSec }: { tsSec: number }): ReactElement {
  const nowSec = Math.floor(useNow() / 1000);
  return <>{relativeDate(tsSec, nowSec)}</>;
}
