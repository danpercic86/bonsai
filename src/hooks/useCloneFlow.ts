// P21: the clone dialog's whole lifecycle — dest picker, progress channel,
// busy/error rows, and the session token that makes a cancelled or superseded
// clone stop writing to the current dialog. Extracted verbatim from App so the
// container only renders `CloneDialog` against it.
import { useCallback, useRef, useState } from 'react';
import { deriveRepoName, joinRepoPath } from '../components/CloneDialog';
import { ipc } from '../ipc';
import type { CloneProgress } from '../ipc';
import { isGitNotFound } from '../ipc/errors';
import { gitNotFoundToastText, noteGitNotFound } from '../ipc/gitNotFound';
import { errorMessage } from '../utils/errors';

export interface UseCloneFlow {
  cloneOpen: boolean;
  cloneDest: string | null;
  cloneProgress: CloneProgress | null;
  cloneBusy: boolean;
  cloneError: string | null;
  handleCloneOpen: () => void;
  handleCloneCancel: () => void;
  handleClonePickDest: () => Promise<void>;
  handleCloneSubmit: (url: string) => Promise<void>;
}

export function useCloneFlow(openTab: (path: string) => Promise<void>): UseCloneFlow {
  // ----- Clone/init lifecycle (P21) -----
  const [cloneOpen, setCloneOpen] = useState(false);
  const [cloneDest, setCloneDest] = useState<string | null>(null);
  const [cloneProgress, setCloneProgress] = useState<CloneProgress | null>(null);
  const [cloneBusy, setCloneBusy] = useState(false);
  const [cloneError, setCloneError] = useState<string | null>(null);
  // Session token: a late progress tick / resolution from a cancelled (or
  // superseded) clone must not write state for the current dialog session.
  const cloneSessionRef = useRef(0);

  // ----- Clone (P21) -----
  const handleCloneOpen = useCallback(() => {
    cloneSessionRef.current += 1; // invalidate any in-flight clone's UI updates
    setCloneDest(null);
    setCloneProgress(null);
    setCloneError(null);
    setCloneBusy(false);
    setCloneOpen(true);
  }, []);

  const handleCloneCancel = useCallback(() => {
    // The backend clone keeps running (no cancellation in v1); we simply stop
    // updating the UI — invalidate the session so late ticks are ignored.
    cloneSessionRef.current += 1;
    setCloneOpen(false);
  }, []);

  const handleClonePickDest = useCallback(async () => {
    try {
      const path = await ipc.pickFolder();
      if (path !== null) setCloneDest(path);
    } catch (e) {
      // Surface in the clone dialog; a picker failure is non-fatal.
      setCloneError(errorMessage(e));
    }
  }, []);

  const handleCloneSubmit = useCallback(
    async (url: string) => {
      if (cloneDest === null) return;
      // Frontend derives the repo name from the URL and computes the full dest
      // = <parent>/<name>; the backend clones INTO an empty/new dest.
      const dest = joinRepoPath(cloneDest, deriveRepoName(url));
      const session = cloneSessionRef.current + 1;
      cloneSessionRef.current = session;
      setCloneBusy(true);
      setCloneError(null);
      setCloneProgress(null);
      try {
        const path = await ipc.cloneRepo(url, dest, (p) => {
          if (cloneSessionRef.current === session) setCloneProgress(p);
        });
        if (cloneSessionRef.current !== session) return; // cancelled/superseded
        setCloneOpen(false);
        await openTab(path);
      } catch (e) {
        if (cloneSessionRef.current !== session) return;
        // P70: latch so the notice bar appears, and replace the raw payload with
        // the plain-language line. NOT toasted: the clone dialog is still open
        // and owns its own error row — a toast on top would say it twice.
        if (isGitNotFound(e)) {
          noteGitNotFound();
          setCloneError(gitNotFoundToastText('Clone'));
        } else {
          setCloneError(errorMessage(e));
        }
      } finally {
        if (cloneSessionRef.current === session) setCloneBusy(false);
      }
    },
    [cloneDest, openTab],
  );

  return {
    cloneOpen,
    cloneDest,
    cloneProgress,
    cloneBusy,
    cloneError,
    handleCloneOpen,
    handleCloneCancel,
    handleClonePickDest,
    handleCloneSubmit,
  };
}
