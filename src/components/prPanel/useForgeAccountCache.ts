// P80: the PR-panel account cache + per-repo override writes, extracted from
// PrPanel (container) to keep it under the file-size limit. Owns the lazily-
// fetched account list backing the switcher and the pin/reset writes; the header
// still renders instantly from `ctx` alone. Refetches after any write and drops
// the cache when the repo changes (same-repo refreshes are explicit refetches).
import { useCallback, useEffect, useRef, useState } from 'react';

import { ipc } from '../../ipc';
import type { ForgeAccount } from '../../ipc';
import { usePushToast } from '../../ToastContext';
import { errorMessage } from '../../utils/errors';

export interface ForgeAccountCache {
  accounts: ForgeAccount[] | null;
  accountsError: string | null;
  accountsBusy: boolean;
  /** Lazy fetch on first switcher open (no-op once loaded). */
  handleOpenAccountMenu(): void;
  /** Force a refetch (after an add/connect write). */
  refetchAccounts(): void;
  handleSelectAccount(accountId: string): void;
  handleUseHostDefault(): void;
  handleResetToDefault(): void;
}

/** @param bumpBootstrap re-run `forgeRepoContext` so the header reflects the new
 *  resolved account after a pin/reset write. */
export function useForgeAccountCache(
  repoId: string,
  bumpBootstrap: () => void,
): ForgeAccountCache {
  const pushToast = usePushToast();
  const [accounts, setAccounts] = useState<ForgeAccount[] | null>(null);
  const [accountsError, setAccountsError] = useState<string | null>(null);
  const [accountsBusy, setAccountsBusy] = useState(false);
  const reqRef = useRef(0);

  const fetchAccounts = useCallback(() => {
    const id = ++reqRef.current;
    setAccountsError(null);
    void ipc.forgeListAccounts().then(
      (list) => {
        if (id !== reqRef.current) return;
        setAccounts(list);
      },
      (e: unknown) => {
        if (id !== reqRef.current) return;
        setAccounts(null);
        setAccountsError(errorMessage(e));
      },
    );
  }, []);

  // The cache is per-repo — drop it when the repo changes so the switcher
  // refetches. Same-repo refreshes are handled by explicit refetches below.
  useEffect(() => {
    ++reqRef.current; // cancel any in-flight fetch
    setAccounts(null);
    setAccountsError(null);
  }, [repoId]);

  const handleOpenAccountMenu = useCallback(() => {
    if (accounts === null) fetchAccounts();
  }, [accounts, fetchAccounts]);

  const writeRepoAccount = useCallback(
    (accountId: string | null, failMsg: string) => {
      setAccountsBusy(true);
      void ipc.forgeSetRepoAccount(repoId, accountId).then(
        () => {
          setAccountsBusy(false);
          fetchAccounts();
          bumpBootstrap();
        },
        (e: unknown) => {
          setAccountsBusy(false);
          pushToast('error', `${failMsg}: ${errorMessage(e)}`);
        },
      );
    },
    [repoId, fetchAccounts, bumpBootstrap, pushToast],
  );

  const handleSelectAccount = useCallback(
    (accountId: string) => writeRepoAccount(accountId, 'Could not switch account'),
    [writeRepoAccount],
  );
  const handleUseHostDefault = useCallback(
    () => writeRepoAccount(null, 'Could not reset to the host default'),
    [writeRepoAccount],
  );

  return {
    accounts,
    accountsError,
    accountsBusy,
    handleOpenAccountMenu,
    refetchAccounts: fetchAccounts,
    handleSelectAccount,
    handleUseHostDefault,
    // OD-2: kebab "Reset to host default" is the same nondestructive unpin.
    handleResetToDefault: handleUseHostDefault,
  };
}
