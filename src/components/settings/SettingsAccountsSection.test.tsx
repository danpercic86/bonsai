/** P80 §3 — the Accounts settings section: accounts grouped by host, a per-account
 *  Default control (forgeSetHostDefault), add-a-token (forgeAddAccount, Azure
 *  disabled), add-another-to-host (locked form), and per-account Remove
 *  (ConfirmDialog → forgeRemoveAccount). Spies each `ipc` method for determinism.
 *
 *  P113: this section raises no toasts — Settings renders inside
 *  `.dialog-overlay`, so a toast raised here is unclickable. Outcomes land in
 *  the raising host's group slot, the `accounts.add` section slot, or (for the
 *  remove failure, whose dialog stays open) the dialog's own `.dialog-error`.
 *  No ToastContext is provided: the lint guard forbids importing it here. */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

import { ipc } from '../../ipc';
import type { ForgeAccount } from '../../ipc';
import { SettingsAccountsSection } from './SettingsAccountsSection';

const VIEWER = { login: 'octocat', avatarUrl: null };

const GH_ACCOUNT: ForgeAccount = {
  accountId: 'gitHub:github.com:octocat',
  host: 'github.com',
  kind: 'gitHub',
  login: 'octocat',
  avatarUrl: null,
  connected: true,
  isHostDefault: true,
};
const GH_ALT: ForgeAccount = {
  accountId: 'gitHub:github.com:danpercic86',
  host: 'github.com',
  kind: 'gitHub',
  login: 'danpercic86',
  avatarUrl: null,
  connected: true,
  isHostDefault: false,
};

function renderSection() {
  return render(<SettingsAccountsSection />);
}

/** The always-mounted outcome slot for a host, or for the section (`accounts.add`).
 *  P113 §10.1: it exists even when idle, with empty text. */
function outcomeNote(slot: string): HTMLElement {
  const el = document.querySelector<HTMLElement>(`[data-outcome-note="${slot}"]`);
  expect(el, `the ${slot} outcome slot must be mounted at all times`).not.toBeNull();
  if (el === null) throw new Error('unreachable');
  return el;
}

describe('SettingsAccountsSection — P80', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(ipc, 'openUrl').mockResolvedValue(undefined);
  });

  it('lists connected accounts grouped by host', async () => {
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([GH_ACCOUNT]);
    renderSection();
    expect(await screen.findByText('octocat')).toBeInTheDocument();
    expect(screen.getAllByText('github.com').length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText('Connected')).toBeInTheDocument();
  });

  it('shows the empty state when there are no accounts', async () => {
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([]);
    renderSection();
    expect(await screen.findByText('No accounts connected')).toBeInTheDocument();
  });

  it('a single-account host shows "(only account)" instead of a Default radio', async () => {
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([GH_ACCOUNT]);
    renderSection();
    expect(await screen.findByText('(only account)')).toBeInTheDocument();
    expect(screen.queryByRole('radio', { name: 'Default' })).toBeNull();
  });

  it('two accounts render Default radios; selecting the non-default calls forgeSetHostDefault', async () => {
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([GH_ACCOUNT, GH_ALT]);
    const setDefault = vi.spyOn(ipc, 'forgeSetHostDefault').mockResolvedValue(undefined);
    renderSection();

    await screen.findByText('danpercic86');
    const radios = screen.getAllByRole('radio', { name: 'Default' });
    expect(radios).toHaveLength(2);
    expect(radios[0]).toBeChecked(); // octocat is the host default
    fireEvent.click(radios[1]);
    await waitFor(() =>
      expect(setDefault).toHaveBeenCalledWith('github.com', GH_ALT.accountId),
    );
  });

  it('shows the OD-4 no-default nudge when ≥2 connected accounts and none is default', async () => {
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([
      { ...GH_ACCOUNT, isHostDefault: false },
      GH_ALT,
    ]);
    renderSection();
    expect(
      await screen.findByText(
        'Pick a default account for github.com. Repositories with no pinned account will use it.',
      ),
    ).toBeInTheDocument();
  });

  it('add-form submit calls forgeAddAccount with the chosen kind/host/token', async () => {
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([]);
    const add = vi.spyOn(ipc, 'forgeAddAccount').mockResolvedValue(VIEWER);
    renderSection();

    fireEvent.click(await screen.findByRole('button', { name: 'Add a token for a host' }));
    fireEvent.change(screen.getByLabelText('Host'), { target: { value: 'GitHub.com' } });
    fireEvent.change(screen.getByLabelText('Personal access token'), {
      target: { value: 'ghp_valid_token' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add account' }));

    await waitFor(() => expect(add).toHaveBeenCalledTimes(1));
    expect(add).toHaveBeenCalledWith('github.com', 'gitHub', 'ghp_valid_token');
  });

  it('the Azure DevOps provider option is disabled (cannot be selected)', async () => {
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([]);
    renderSection();
    fireEvent.click(await screen.findByRole('button', { name: 'Add a token for a host' }));

    const azure = screen.getByRole('radio', { name: 'Azure DevOps' });
    expect(azure).toBeDisabled();
    expect(azure).toHaveAttribute('aria-disabled', 'true');
    expect(azure).not.toBeChecked();
  });

  it('add-another-to-host reveals a locked-host form calling forgeAddAccount for that host', async () => {
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([GH_ACCOUNT]);
    const add = vi.spyOn(ipc, 'forgeAddAccount').mockResolvedValue({ login: 'alt', avatarUrl: null });
    renderSection();

    fireEvent.click(
      await screen.findByRole('button', { name: 'Add another account to github.com' }),
    );
    // Host is pre-filled + read-only; provider is a static badge (no radios).
    const host = screen.getByLabelText('Host') as HTMLInputElement;
    expect(host).toHaveValue('github.com');
    expect(host).toHaveAttribute('readonly');
    fireEvent.change(screen.getByLabelText('Personal access token'), {
      target: { value: 'ghp_second' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add account' }));
    await waitFor(() => expect(add).toHaveBeenCalledWith('github.com', 'gitHub', 'ghp_second'));
  });

  it('a rejected add token shows an inline authFailed error and keeps the form open', async () => {
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([]);
    vi.spyOn(ipc, 'forgeAddAccount').mockRejectedValue({ kind: 'authFailed', message: 'rejected' });
    renderSection();

    fireEvent.click(await screen.findByRole('button', { name: 'Add a token for a host' }));
    fireEvent.change(screen.getByLabelText('Host'), { target: { value: 'github.com' } });
    fireEvent.change(screen.getByLabelText('Personal access token'), {
      target: { value: 'ghp_bad' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add account' }));

    expect(await screen.findByText(/That token was rejected/i)).toBeInTheDocument();
    expect(screen.getByLabelText('Host')).toBeInTheDocument();
  });

  it('Remove account → confirm names the account + fallback, then calls forgeRemoveAccount(accountId)', async () => {
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([GH_ACCOUNT]);
    const remove = vi.spyOn(ipc, 'forgeRemoveAccount').mockResolvedValue(undefined);
    renderSection();

    const kebab = await screen.findByRole('button', { name: /Actions for octocat/i });
    fireEvent.click(kebab);
    fireEvent.click(screen.getByRole('menuitem', { name: 'Remove account' }));

    const dialog = await screen.findByRole('dialog');
    expect(dialog).toHaveTextContent(/Remove octocat\?/);
    expect(dialog).toHaveTextContent(/Any repository pinned to this account will fall back/);
    expect(remove).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Remove' }));

    await waitFor(() => expect(remove).toHaveBeenCalledWith(GH_ACCOUNT.accountId));
  });
});

/** P113 — the outcome sweep: every message this section used to raise as a toast
 *  behind its own scrim now has an inline home. */
describe('SettingsAccountsSection — inline outcomes (P113)', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('mounts an empty, role-less note per host and one for the section', async () => {
    vi.spyOn(ipc, 'openUrl').mockResolvedValue(undefined);
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([GH_ACCOUNT]);
    const { container } = renderSection();
    await screen.findByText('octocat');

    for (const slot of ['github.com', 'accounts.add']) {
      const note = outcomeNote(slot);
      expect(note.textContent).toBe('');
      // §10.1: `:empty` collapses the chrome, so the modifier must be present
      // even while idle — a bare `.settings-row-note` would keep its margin.
      expect(note).toHaveClass('settings-row-note', 'settings-row-note--result');
      expect(note).not.toHaveAttribute('aria-live');
      expect(note).not.toHaveAttribute('role');
    }
    // §8.2 AC6: exactly ONE live region in the populated / no-error state.
    expect(
      container.querySelectorAll('[aria-live],[role="status"],[role="alert"]'),
    ).toHaveLength(1);
  });

  it('a failed default-set lands in THAT host group and is announced once', async () => {
    vi.spyOn(ipc, 'openUrl').mockResolvedValue(undefined);
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([
      { ...GH_ACCOUNT, isHostDefault: false },
      GH_ALT,
    ]);
    vi.spyOn(ipc, 'forgeSetHostDefault').mockRejectedValue({
      kind: 'other',
      message: 'account is not on the given host',
    });
    const { container } = renderSection();

    await screen.findByText('danpercic86');
    fireEvent.click(screen.getAllByRole('radio', { name: 'Default' })[1]);

    // P113 A5: the string names its HOST. One announcer serves N host groups, so
    // a failure that names neither host nor login announces no subject — a
    // sighted user reads it off the note's placement, which no screen reader
    // conveys. `setDefault(host, accountId)` has no null branch, so this is
    // unconditional.
    const text =
      'Could not set the default account for github.com: account is not on the given host';
    await waitFor(() => expect(outcomeNote('github.com')).toHaveTextContent(text));
    expect(outcomeNote('github.com')).toHaveClass('settings-row-note--warn');
    // The section slot is NOT a fallback — the outcome is keyed by host.
    expect(outcomeNote('accounts.add').textContent).toBe('');
    expect(container.querySelector('[role="status"]')?.textContent).toBe(text);
    expect(document.querySelectorAll('.toast')).toHaveLength(0);
  });

  // A5's other half: the global add form has NO host yet (`host ?? ADD_SLOT`), so
  // this one string keeps the subject-less wording — the fallback is required,
  // not defensive.
  it('a token-page failure raised by the GLOBAL add form lands in the section slot', async () => {
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([]);
    vi.spyOn(ipc, 'openUrl').mockRejectedValue({
      kind: 'externalToolFailed',
      message: 'could not launch browser (rundll32): not found',
    });
    renderSection();

    fireEvent.click(await screen.findByRole('button', { name: 'Add a token for a host' }));
    // The form's provider-hint link ("Create a token"), whose click is
    // intercepted and routed through `ipc.openUrl` (P80 §3.4).
    fireEvent.click(screen.getByRole('link', { name: 'Create a token' }));

    await waitFor(() =>
      expect(outcomeNote('accounts.add')).toHaveTextContent(
        'Could not open the token page: could not launch browser (rundll32): not found',
      ),
    );
  });

  it('a connect success lands in the section slot, whose host group does not exist yet', async () => {
    vi.spyOn(ipc, 'openUrl').mockResolvedValue(undefined);
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([]);
    vi.spyOn(ipc, 'forgeAddAccount').mockResolvedValue(VIEWER);
    renderSection();

    fireEvent.click(await screen.findByRole('button', { name: 'Add a token for a host' }));
    fireEvent.change(screen.getByLabelText('Host'), { target: { value: 'gitlab.com' } });
    fireEvent.change(screen.getByLabelText('Personal access token'), {
      target: { value: 'glpat_valid' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add account' }));

    await waitFor(() =>
      expect(outcomeNote('accounts.add')).toHaveTextContent('Connected to gitlab.com as octocat.'),
    );
    // §4.2: the quieter recipe — no tint, no bar, no alert role.
    expect(outcomeNote('accounts.add')).toHaveClass('settings-row-note--result');
  });

  /** §1.3/§3.3 — `confirmRemove`'s failure branch never clears `removeTarget`,
   *  so the dialog STAYS OPEN. A host-group note would sit behind a second
   *  overlay, which is the very defect this sweep removes one layer up. */
  it('a failed remove keeps the dialog open and reports inside it, not in the host group', async () => {
    vi.spyOn(ipc, 'openUrl').mockResolvedValue(undefined);
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([GH_ACCOUNT]);
    vi.spyOn(ipc, 'forgeRemoveAccount').mockRejectedValue({
      kind: 'other',
      message: 'cannot resolve app config dir: unknown path',
    });
    const { container } = renderSection();

    fireEvent.click(await screen.findByRole('button', { name: /Actions for octocat/i }));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Remove account' }));
    const dialog = await screen.findByRole('dialog');
    // §8.2: the error `<p>` is mounted EMPTY with the dialog, so its text
    // arrives in a later tick — the condition for being announced at all.
    const err = dialog.querySelector('.dialog-error');
    expect(err?.textContent).toBe('');
    expect(err).toHaveAttribute('role', 'alert');

    fireEvent.click(screen.getByRole('button', { name: 'Remove' }));

    await waitFor(() =>
      expect(dialog.querySelector('.dialog-error')).toHaveTextContent(
        'Could not remove github.com: cannot resolve app config dir: unknown path',
      ),
    );
    // The dialog is still open and Remove is retryable in place.
    expect(screen.getByRole('dialog')).toBe(dialog);
    expect(screen.getByRole('button', { name: 'Remove' })).toBeEnabled();
    // §8.2: ONE utterance for one event — the section announcer stays silent.
    expect(container.querySelector('[role="status"]')?.textContent).toBe('');
    expect(outcomeNote('github.com').textContent).toBe('');
    // §7: reopening after a failure shows no error.
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    fireEvent.click(await screen.findByRole('button', { name: /Actions for octocat/i }));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Remove account' }));
    expect((await screen.findByRole('dialog')).querySelector('.dialog-error')?.textContent).toBe('');
  });
});
