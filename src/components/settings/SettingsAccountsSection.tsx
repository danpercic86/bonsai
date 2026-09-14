// P80 §3 — the "Accounts" pane body: forge sign-ins grouped by host, each host
// group owning its accounts, per-account Default control, and an add-another
// affordance. Owns the `forgeListAccounts` fetch, the global (new-host) add form,
// and the per-account Remove confirm (with a fallback warning). Composes
// SettingsAccountHostGroup. Precedent: SettingsProfilesSection.
//
// P113 — this section raises NO toasts: Settings renders inside
// `.dialog-overlay` (z-index 100) and `.toast-stack` is 90, so a toast raised
// here is unclickable, not merely dim (ui-reference §12.14). Its five outcomes
// go to three kinds of home: the raising HOST's group slot, the section slot in
// the `accounts.add` row (which renders in all four pane states, so the
// just-connected host needs no group yet), and — for the remove failure — a
// `.dialog-error` inside the confirm dialog, which deliberately STAYS OPEN on
// failure so Remove can be retried in place. A host-group note there would sit
// behind a second overlay, reproducing the very defect one layer up.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { ipc } from '../../ipc';
import type { ForgeAccount } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import { SkeletonRows } from '../CommitPanel';
import { ConfirmDialog } from '../ConfirmDialog';
import { SettingsAccountAddForm } from './SettingsAccountAddForm';
import { SettingsAccountHostGroup } from './SettingsAccountHostGroup';
import { settingsRowHelpId } from './settingsCatalog';
import { SettingsEmpty } from './SettingsEmpty';
import { SettingsGroup } from './SettingsGroup';
import { SettingsOutcomeNote } from './SettingsOutcomeNote';
import { SettingsRow } from './SettingsRow';
import { useOutcomeNotes } from './useOutcomeNotes';

/** The section slot: outcomes with no host group to land in (the global add
 *  form's token-page failure, and the connect success whose group does not exist
 *  until `refetch` resolves — and may never, if `refetch` fails). */
const ADD_SLOT = 'accounts.add';
const ADD_OUTCOME_ID = 'accounts-add-outcome';

export function SettingsAccountsSection() {
  const { notes, announce, begin, report } = useOutcomeNotes();
  const [accounts, setAccounts] = useState<ForgeAccount[]>([]);
  const [loading, setLoading] = useState(true);
  const [listError, setListError] = useState<string | null>(null);
  const [addOpen, setAddOpen] = useState(false);
  const [removeTarget, setRemoveTarget] = useState<ForgeAccount | null>(null);
  const [removing, setRemoving] = useState(false);
  /** P113 §3.3 — the remove failure's home, OUTSIDE the note hook: it is the
   *  dialog's own `role="alert"` and the section announcer must stay silent for
   *  it, so one event yields one utterance. */
  const [removeError, setRemoveError] = useState<string | null>(null);
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

  /** P113 §6.2 — host binding with NO prop-signature change: the section builds
   *  a per-group closure, so `SettingsAccountHostGroup`, `SettingsAccountCard`
   *  and `SettingsAccountAddForm` keep `onOpenUrl(url: string)` as it was. A
   *  `null` host is the global add form, which has no group yet. */
  const openUrl = useCallback(
    (url: string, host: string | null) => {
      const slot = host ?? ADD_SLOT;
      begin(slot);
      void ipc.openUrl(url).catch((e: unknown) => {
        // §12.2 A1: `errorMessage(e)` is raw keychain/OS text appended to a
        // mapped lead. Rendering it verbatim is deliberate for this increment —
        // mapping it needs the backend error-kind inventory — and
        // `overflow-wrap: anywhere` is what keeps a space-free token in its box.
        report(slot, 'error', `Could not open the token page: ${errorMessage(e)}`);
      });
    },
    [begin, report],
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
      begin(host);
      void ipc.forgeSetHostDefault(host, accountId).then(refetch, (e: unknown) =>
        report(host, 'error', `Could not set the default account: ${errorMessage(e)}`),
      );
    },
    [begin, refetch, report],
  );

  const confirmRemove = () => {
    if (removeTarget === null) return;
    const { accountId, host } = removeTarget;
    setRemoving(true);
    // The `begin` equivalent for this slot: clearing FIRST is what makes a
    // retried failure a real `null → text` change, so the alert fires again.
    setRemoveError(null);
    void ipc.forgeRemoveAccount(accountId).then(
      () => {
        setRemoving(false);
        setRemoveTarget(null);
        refetch();
      },
      (e: unknown) => {
        // NOTE: `removeTarget` is deliberately NOT cleared — the dialog stays
        // open so Remove can be retried in place, which is why this outcome
        // cannot be a host-group note (§1.3).
        setRemoving(false);
        setRemoveError(`Could not remove ${host}: ${errorMessage(e)}`);
      },
    );
  };

  const removeLabel = removeTarget?.login ?? removeTarget?.host ?? 'this account';

  const openRemove = (account: ForgeAccount) => {
    // §7: cleared when the dialog OPENS (and on cancel), so a reopened dialog
    // never shows the previous attempt's failure.
    setRemoveError(null);
    setRemoveTarget(account);
  };

  const closeRemove = () => {
    setRemoveError(null);
    setRemoveTarget(null);
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
        groups.map((g) => (
          <SettingsAccountHostGroup
            key={g.host}
            host={g.host}
            kind={g.kind}
            accounts={g.accounts}
            onSetDefault={(accountId) => setDefault(g.host, accountId)}
            onRequestRemove={openRemove}
            onChanged={refetch}
            onOpenUrl={(url) => openUrl(url, g.host)}
            onAdded={(host, login) => {
              refetch();
              report(host, 'success', `Added ${login} to ${host}.`);
            }}
            outcome={notes.get(g.host) ?? null}
          />
        ))}

      {addOpen && (
        <SettingsAccountAddForm
          onCancel={() => setAddOpen(false)}
          onSuccess={(host, login) => {
            setAddOpen(false);
            refetch();
            // The section slot, not a host slot: the new host's group does not
            // exist until `refetch` resolves, and may never if it fails.
            report(ADD_SLOT, 'success', `Connected to ${host} as ${login}.`);
          }}
          onOpenUrl={(url) => openUrl(url, null)}
        />
      )}

      <SettingsRow
        id="accounts.add"
        rowLabel="Add a token for a host"
        hint={
          <SettingsOutcomeNote
            slot={ADD_SLOT}
            id={ADD_OUTCOME_ID}
            outcome={notes.get(ADD_SLOT) ?? null}
          />
        }
      >
        <button
          type="button"
          className="btn-secondary settings-toggle-btn"
          disabled={addOpen}
          aria-describedby={`${settingsRowHelpId('accounts.add')} ${ADD_OUTCOME_ID}`}
          onClick={() => setAddOpen(true)}
        >
          Add a token for a host
        </button>
      </SettingsRow>

      {/* P113 §8.2 — the section's ONE live region, always mounted so its text
          always arrives in a LATER commit than its mount (the condition for
          being announced at all). Every `[data-outcome-note]` above is
          description-only: no `aria-live`, no `role`. The remove failure is not
          routed here — it speaks through the dialog's own `role="alert"`. */}
      <p className="sr-only" role="status" aria-live="polite">
        {announce}
      </p>

      <ConfirmDialog
        open={removeTarget !== null}
        title={`Remove ${removeLabel}?`}
        confirmLabel="Remove"
        busy={removing}
        onConfirm={confirmRemove}
        onCancel={closeRemove}
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
        {/* P113 §3.3/§8.2 — rendered ALWAYS (empty while idle) from the moment
            the dialog opens, so its text arrives in a later tick than its mount.
            `role="alert"` is the only channel that reaches the user without a
            focus move: focus is on the re-enabled Remove button inside an
            `aria-modal` dialog. `ConfirmDialog` has no passthrough to the
            confirm button's attributes and is NOT touched for this — the
            `.dialog-error` is simply the last child of its `children`. */}
        <p className="dialog-error" role="alert">{removeError ?? ''}</p>
      </ConfirmDialog>
    </SettingsGroup>
  );
}
