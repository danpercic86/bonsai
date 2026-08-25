// P90 §5/§6: a single visually-hidden live region that announces the Checks
// verdict on SETTLE only (not while a fetch is in flight), debounced ~150 ms so a
// burst of fetches doesn't flood the screen reader.
import { useEffect, useRef, useState } from 'react';
import { rollupPill } from './checkVisuals';
import type { ChecksState } from './useBranchChecks';

function settleMessage(state: ChecksState): string | null {
  switch (state.kind) {
    case 'loaded': {
      const pill = rollupPill(state.status);
      return pill !== null ? `Checks updated: ${pill.aria}.` : null;
    }
    case 'noChecks':
      return state.reason === 'no-upstream'
        ? null
        : state.reason === 'waiting'
          ? 'Waiting for checks.'
          : 'No checks configured.';
    case 'error':
      return "Couldn't refresh checks.";
    default:
      return null;
  }
}

export function ChecksAnnouncer({
  state,
  refreshing,
}: {
  state: ChecksState;
  refreshing: boolean;
}) {
  const [message, setMessage] = useState('');
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    // Announce only once the view has settled (no in-flight refetch).
    if (refreshing) return;
    const next = settleMessage(state);
    if (next === null) return;
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = setTimeout(() => setMessage(next), 150);
    return () => {
      if (timer.current !== null) clearTimeout(timer.current);
    };
  }, [state, refreshing]);

  return (
    <span className="sr-only" role="status" aria-live="polite">
      {message}
    </span>
  );
}
