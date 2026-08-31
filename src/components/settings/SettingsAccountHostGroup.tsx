// P80 §3.1/§3.9 — one host's account group: badge + host title, the OD-4
// no-default nudge, a radiogroup of account cards (each with its Default
// control), and an "Add another account to {host}" affordance with an inline
// locked-host add form. Presentational: it REQUESTS default/remove/add via
// callbacks; SettingsAccountsSection owns the list fetch + Remove confirm.
import { useId, useState } from 'react';

import type { ForgeAccount, ForgeKind } from '../../ipc';
import { ForgeProviderBadge } from '../ForgeProviderBadge';
import { SettingsAccountAddForm } from './SettingsAccountAddForm';
import { SettingsAccountCard } from './SettingsAccountCard';

export interface SettingsAccountHostGroupProps {
  host: string;
  kind: ForgeKind;
  /** Accounts on this host (≥1). */
  accounts: ForgeAccount[];
  onSetDefault(accountId: string): void;
  onRequestRemove(account: ForgeAccount): void;
  /** Refetch the list after a change (token replace / add). */
  onChanged(): void;
  onOpenUrl(url: string): void;
  /** Success toast after an add-another (`Added {login} to {host}.`). */
  onAdded(host: string, login: string): void;
}

export function SettingsAccountHostGroup({
  host,
  kind,
  accounts,
  onSetDefault,
  onRequestRemove,
  onChanged,
  onOpenUrl,
  onAdded,
}: SettingsAccountHostGroupProps) {
  const [addOpen, setAddOpen] = useState(false);
  const titleId = useId();
  const noteId = useId();

  const isOnlyOnHost = accounts.length === 1;
  // OD-4 nudge: ≥2 CONNECTED accounts and none is the host default.
  const connected = accounts.filter((a) => a.connected);
  const showNudge = connected.length >= 2 && !accounts.some((a) => a.isHostDefault);

  return (
    <section className="settings-account-group" role="group" aria-labelledby={titleId}>
      <div className="settings-account-group-head">
        <ForgeProviderBadge kind={kind} />
        <span className="settings-account-group-host" id={titleId} title={host}>
          {host}
        </span>
      </div>

      {showNudge && (
        <p className="settings-account-group-note" id={noteId} role="note">
          {`Pick a default account for ${host}. Repositories with no pinned account will use it.`}
        </p>
      )}

      <div
        role="radiogroup"
        aria-label={`Default account for ${host}`}
        aria-describedby={showNudge ? noteId : undefined}
      >
        {accounts.map((a) => (
          <SettingsAccountCard
            key={a.accountId}
            account={a}
            isOnlyOnHost={isOnlyOnHost}
            onChanged={onChanged}
            onSetDefault={() => onSetDefault(a.accountId)}
            onRequestRemove={() => onRequestRemove(a)}
            onOpenUrl={onOpenUrl}
          />
        ))}
      </div>

      {addOpen ? (
        <SettingsAccountAddForm
          lockedHost={host}
          lockedKind={kind}
          onCancel={() => setAddOpen(false)}
          onSuccess={(h, login) => {
            setAddOpen(false);
            onAdded(h, login);
          }}
          onOpenUrl={onOpenUrl}
        />
      ) : (
        <button
          type="button"
          className="btn-secondary settings-toggle-btn"
          onClick={() => setAddOpen(true)}
        >
          {`Add another account to ${host}`}
        </button>
      )}
    </section>
  );
}
