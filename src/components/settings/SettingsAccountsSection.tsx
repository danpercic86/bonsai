// P80 §3 — the "Accounts" pane body: forge sign-ins grouped by host, each host
// group owning its accounts, per-account Default control, and an add-another
// affordance. Owns the `forgeListAccounts` fetch, the global (new-host) add form,
// and the per-account Remove confirm (with a fallback warning). Composes
// SettingsAccountHostGroup. Precedent: SettingsProfilesSection.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { ipc } from '../../ipc';
import type { ForgeAccount } from '../../ipc';
import { usePushToast } from '../../ToastContext';
import { errorMessage } from '../../utils/errors';
import { SkeletonRows } from '../CommitPanel';
import { ConfirmDialog } from '../ConfirmDialog';
import { SettingsAccountAddForm } from './SettingsAccountAddForm';
import { SettingsAccountHostGroup } from './SettingsAccountHostGroup';
import { SettingsEmpty } from './SettingsEmpty';
import { SettingsGroup } from './SettingsGroup';
import { SettingsRow } from './SettingsRow';

export function SettingsAccountsSection() {
  const pushToast = usePushToast();
  const [accounts, setAccounts] = useState<ForgeAccount[]>([]);
  const [loading, setLoading] = useState(true);
  const [listError, setListError] = useState<string | null>(null);
  const [addOpen, setAddOpen] = useState(false);
  const [removeTarget, setRemoveTarget] = useState<ForgeAccount | null>(null);
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

  // Group accounts by host; alphabetical host order (stable, deterministic).
  const groups = useMemo(() => {
    const byHost = new Map<string, ForgeAccount[]>();
    for (const a of accounts) {
      const bucket = byHost.get(a.host);
      if (bucket === undefined) byHost.set(a.host, [a]);
      else bucket.push(a);
    }
    return [...byHost.entries()]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([host, list]) => ({ host, kind: list[0].kind, accounts: list }));
  }, [accounts]);

  const setDefault = useCallback(
    (host: string, accountId: string) => {
      void ipc.forgeSetHostDefault(host, accountId).then(refetch, (e: unknown) =>
        pushToast('error', `Could not set the default account: ${errorMessage(e)}`),
      );
    },
    [pushToast, refetch],
  );

  const confirmRemove = () => {
    if (removeTarget === null) return;
    const { accountId, host } = removeTarget;
    setRemoving(true);
    void ipc.forgeRemoveAccount(accountId).then(
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

  const removeLabel = removeTarget?.login ?? removeTarget?.host ?? 'this account';

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
        groups.map((g) => (
          <SettingsAccountHostGroup
            key={g.host}
            host={g.host}
            kind={g.kind}
            accounts={g.accounts}
            onSetDefault={(accountId) => setDefault(g.host, accountId)}
            onRequestRemove={setRemoveTarget}
            onChanged={refetch}
            onOpenUrl={onOpenUrl}
            onAdded={(host, login) => {
              refetch();
              pushToast('success', `Added ${login} to ${host}.`);
            }}
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
        title={`Remove ${removeLabel}?`}
        confirmLabel="Remove"
        busy={removing}
        onConfirm={confirmRemove}
        onCancel={() => setRemoveTarget(null)}
      >
        {'This deletes the saved token for '}
        <span className="mono">{removeTarget?.login ?? removeTarget?.host ?? ''}</span>
        {' on '}
        <span className="mono">{removeTarget?.host ?? ''}</span>
        {' from your OS keychain.'}
        {removeTarget?.isHostDefault === true && (
          <>
            {" It's the default for "}
            <span className="mono">{removeTarget.host}</span>
            {'; another account will become the default, or '}
            <span className="mono">{removeTarget.host}</span>
            {' will have none.'}
          </>
        )}
        {' Any repository pinned to this account will fall back to the host default.'}
        {" This can't be undone — you'll need a new token to sign in again."}
      </ConfirmDialog>
    </SettingsGroup>
  );
}
