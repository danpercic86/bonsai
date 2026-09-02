// P91 §5 / §8.4 — container for the Dev-mode header pill.
//
// Mounted by `HeaderToolbar` ONLY while `dev.enabled` is true, so its poll is
// alive exactly when the pill is on screen — never background load (§6, same
// principle as the Settings Dev page's status poll). It reads `writeFailed` off
// `logSessionInfo` on a slow cadence and hands the presentational
// `DevModeIndicator` the one bool it needs to swap into the §8.4 danger variant.
//
// Only a BOOL is read here; the sink's underlying io-error string never crosses
// IPC (privacy — increment 1 MUST-FIX).

import { useEffect, useRef, useState } from 'react';
import { ipc } from '../ipc';
import { DevModeIndicator } from './DevModeIndicator';

/** Slow poll — the pill only needs to catch a stuck disk within a few seconds,
 *  and this is chrome that is always mounted while Dev mode is on.
 *
 *  DELIBERATELY DIFFERENT from the Dev settings card's 2 s (`DevCategory`) — do
 *  not unify them. This one is always-mounted background chrome reading a single
 *  bool, so it pays the cheaper cadence; the card is a focused surface whose byte
 *  counts and file list the user is actively watching, so it refreshes faster.
 *  Different cadences also keep the two polls from phase-locking into one
 *  synchronised IPC burst. */
const POLL_MS = 3000;

export function DevModePill({ onOpen }: { onOpen(): void }) {
  const [writeFailed, setWriteFailed] = useState(false);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    const read = async () => {
      try {
        const info = await ipc.logSessionInfo();
        if (mounted.current) setWriteFailed(info.writeFailed);
      } catch {
        // A failed poll is not itself the disk-write error; leave the last known
        // state rather than flapping the pill on a transient IPC hiccup.
      }
    };
    void read();
    const timer = window.setInterval(() => void read(), POLL_MS);
    return () => {
      mounted.current = false;
      window.clearInterval(timer);
    };
  }, []);

  return <DevModeIndicator onOpen={onOpen} writeFailed={writeFailed} />;
}
