// P58c: effective signing config for one open repo, feeding the commit-box
// sign toggle / indicator. Read once per repo (and on manual Refresh); a read
// failure is non-critical and just hides the toggle. Extracted verbatim from
// RepoWorkspace so the container only wires it.
import { useCallback, useEffect, useState } from 'react';
import { ipc } from '../../ipc';
import type { SigningStatus } from '../../ipc';

export interface UseSigningStatus {
  signingStatus: SigningStatus | null;
  /** Re-read the config (manual Refresh: keyring / commit.gpgsign may have changed). */
  refetchSigningStatus: () => Promise<void>;
}

export function useSigningStatus(repoId: string): UseSigningStatus {
  // P58c: effective signing config for the commit-box toggle/indicator. Read
  // once per repo (and on manual Refresh); a read failure just hides the toggle.
  const [signingStatus, setSigningStatus] = useState<SigningStatus | null>(null);
  const refetchSigningStatus = useCallback(async () => {
    try {
      setSigningStatus(await ipc.signingStatus(repoId));
    } catch {
      setSigningStatus(null); // non-critical read — hide the toggle, follow config
    }
  }, [repoId]);
  useEffect(() => {
    void refetchSigningStatus();
  }, [refetchSigningStatus]);

  return { signingStatus, refetchSigningStatus };
}
