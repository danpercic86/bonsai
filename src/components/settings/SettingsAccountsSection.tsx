// P79 §3.2 — the "Accounts" pane body: the global list of forge sign-ins Bonsai
// knows a token for (repo-independent). Owns the `forgeListAccounts` fetch, the
// add-form open state and the Remove confirm; composes SettingsAccountCard +
// SettingsAccountAddForm. Precedent: SettingsProfilesSection.
import { useCallback, useEffect, useRef, useState } from 'react';

import { ipc } from '../../ipc';
import type { ForgeAccount } from '../../ipc';
import { usePushToast } from '../../ToastContext';
import { errorMessage } from '../../utils/errors';
import { SkeletonRows } from '../CommitPanel';
import { ConfirmDialog } from '../ConfirmDialog';
import { SettingsAccountAddForm } from './SettingsAccountAddForm';
import { SettingsAccountCard } from './SettingsAccountCard';
import { SettingsEmpty } from './SettingsEmpty';
import { SettingsGroup } from './SettingsGroup';
import { SettingsRow } from './SettingsRow';

interface RemoveTarget {
  host: string;
  login: string | null;
}

export function SettingsAccountsSection() {
  const pushToast = usePushToast();
  const [accounts, setAccounts] = useState<ForgeAccount[]>([]);
  const [loading, setLoading] = useState(true);
  const [listError, setListError] = useState<string | null>(null);
  const [addOpen, setAddOpen] = useState(false);
  const [removeTarget, setRemoveTarget] = useState<RemoveTarget | null>(null);
  const [removing, setRemoving] = useState(false);
  const reqRef = useRef(0);

  const refetch = useCallback(() => {
    const id = ++reqRef.current;
    setLoading(true);
    setListError(null);
    void ipc.forgeListAccounts().then(
      (list) => {
        if (id !== reqRef.current) return;
        setAccounts(list);
        setLoading(false);
      },
      (e: unknown) => {
        if (id !== reqRef.current) return;
        setListError(errorMessage(e));
        setLoading(false);
      },
    );
  }, []);

  useEffect(() => {
    refetch();
  }, [refetch]);

  const onOpenUrl = useCallback(
    (url: string) => {
      void ipc
        .openUrl(url)
        .catch((e: unknown) =>
          pushToast('error', `Could not open the token page: ${errorMessage(e)}`),
        );
    },
    [pushToast],
  );

  const confirmRemove = () => {
    if (removeTarget === null) return;
    const { host } = removeTarget;
    setRemoving(true);
    void ipc.forgeClearTokenForHost(host).then(
      () => {
        setRemoving(false);
        setRemoveTarget(null);
        refetch();
      },
      (e: unknown) => {
        setRemoving(false);
        pushToast('error', `Could not remove ${host}: ${errorMessage(e)}`);
      },
    );
  };

  return (
    <SettingsGroup id="accounts" title="Connected accounts">
      {loading && <SkeletonRows />}

      {!loading && listError !== null && (
        <div className="error-banner error-banner-dismissible" role="alert">
          <span className="error-banner-text">{`Couldn't load your accounts. ${listError}`}</span>
          <button type="button" className="section-action" onClick={refetch}>
            Retry
          </button>
        </div>
      )}

      {!loading && listError === null && accounts.length === 0 && (
        <SettingsEmpty
          title="No accounts connected"
          body="Connect a forge account to view and open pull requests and see CI status. You can also connect from a repository's Pull requests tab."
        />
      )}

      {!loading &&
        listError === null &&
        accounts.map((a) => (
          <SettingsAccountCard
            key={a.host}
            account={a}
            onChanged={refetch}
            onRequestRemove={() => setRemoveTarget({ host: a.host, login: a.login })}
            onOpenUrl={onOpenUrl}
          />
        ))}

      {addOpen && (
        <SettingsAccountAddForm
          onCancel={() => setAddOpen(false)}
          onSuccess={(host, login) => {
            setAddOpen(false);
            refetch();
            pushToast('success', `Connected to ${host} as ${login}.`);
          }}
          onOpenUrl={onOpenUrl}
        />
      )}

      <SettingsRow id="accounts.add" rowLabel="Add a token for a host">
        <button
          type="button"
          className="btn-secondary settings-toggle-btn"
          disabled={addOpen}
          onClick={() => setAddOpen(true)}
        >
          Add a token for a host
        </button>
      </SettingsRow>

      <ConfirmDialog
        open={removeTarget !== null}
        title={`Remove ${removeTarget?.host ?? 'this host'}?`}
        confirmLabel="Remove"
        busy={removing}
        onConfirm={confirmRemove}
        onCancel={() => setRemoveTarget(null)}
      >
        {'This deletes the saved token for '}
        <span className="mono">{removeTarget?.host ?? ''}</span>
        {removeTarget?.login != null ? ` (${removeTarget.login})` : ''}
        {' from your OS keychain. Any repository on '}
        <span className="mono">{removeTarget?.host ?? ''}</span>
        {' will need a new token to view pull requests.'}
      </ConfirmDialog>
    </SettingsGroup>
  );
}
