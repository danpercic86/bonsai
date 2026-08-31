/** P80 §3 — the Accounts settings section: accounts grouped by host, a per-account
 *  Default control (forgeSetHostDefault), add-a-token (forgeAddAccount, Azure
 *  disabled), add-another-to-host (locked form), and per-account Remove
 *  (ConfirmDialog → forgeRemoveAccount). Spies each `ipc` method for determinism;
 *  provides ToastContext for usePushToast. */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

import { ipc } from '../../ipc';
import type { ForgeAccount } from '../../ipc';
import { SettingsAccountsSection } from './SettingsAccountsSection';
import { ToastContext } from '../../ToastContext';

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
  const pushToast = vi.fn();
  render(
    <ToastContext.Provider value={pushToast}>
      <SettingsAccountsSection />
    </ToastContext.Provider>,
  );
  return { pushToast };
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
