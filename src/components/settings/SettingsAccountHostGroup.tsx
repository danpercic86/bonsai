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
import { SettingsOutcomeNote, type SettingsOutcome } from './SettingsOutcomeNote';

export interface SettingsAccountHostGroupProps {
  host: string;
  kind: ForgeKind;
  /** Accounts on this host (≥1). */
  accounts: ForgeAccount[];
  onSetDefault(accountId: string): void;
  onRequestRemove(account: ForgeAccount): void;
  /** Refetch the list after a change (token replace / add). */
  onChanged(): void;
  /** P113 §6.2: the SECTION binds the host into this closure, so the signature
   *  here (and in the card / add form) is deliberately unchanged. */
  onOpenUrl(url: string): void;
  /** Reports an add-another success (`Added {login} to {host}.`). */
  onAdded(host: string, login: string): void;
  /** P113 §6.2 — this host's newest action outcome (default-set / token-page /
   *  add-another), rendered directly under the group head. One note per host,
   *  newest wins; N failing hosts show N notes, each naming its own host. */
  outcome: SettingsOutcome | null;
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
  outcome,
}: SettingsAccountHostGroupProps) {
  const [addOpen, setAddOpen] = useState(false);
  const titleId = useId();
  const noteId = useId();
  const outcomeId = useId();

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

      {/* P113 §6.2 — the group's one message zone, above the OD-4 nudge. The
          nudge is NOT hidden while an outcome shows: they are different facts,
          and the nudge is untinted --text-2 prose, so there is no amber-on-amber
          adjacency. `slot` is the host, which is what the harness hit-tests. */}
      <SettingsOutcomeNote slot={host} id={outcomeId} outcome={outcome} />

      {showNudge && (
        <p className="settings-account-group-note" id={noteId} role="note">
          {`Pick a default account for ${host}. Repositories with no pinned account will use it.`}
        </p>
      )}

      <div
        role="radiogroup"
        aria-label={`Default account for ${host}`}
        /* P113 §9: composed, never replaced. The outcome id is unconditional
           (the element is always mounted, and an id resolving to an empty
           element reads as nothing); the nudge id stays conditional. */
        aria-describedby={showNudge ? `${noteId} ${outcomeId}` : outcomeId}
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
          aria-describedby={outcomeId}
          onClick={() => setAddOpen(true)}
        >
          {`Add another account to ${host}`}
        </button>
      )}
    </section>
  );
}
