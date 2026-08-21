/** P79 §3 — the global Accounts settings section: lists hosts via forgeListAccounts,
 *  add-a-token form (forgeSetTokenForHost, Azure disabled), and per-card Remove
 *  (ConfirmDialog → forgeClearTokenForHost). Spies each `ipc` method for
 *  determinism; provides ToastContext for usePushToast. */
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

function renderSection() {
  const pushToast = vi.fn();
  render(
    <ToastContext.Provider value={pushToast}>
      <SettingsAccountsSection />
    </ToastContext.Provider>,
  );
  return { pushToast };
}

describe('SettingsAccountsSection — P79', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(ipc, 'openUrl').mockResolvedValue(undefined);
  });

  it('lists connected accounts from forgeListAccounts', async () => {
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([GH_ACCOUNT]);
    renderSection();
    expect(await screen.findByText('octocat')).toBeInTheDocument();
    expect(screen.getByText('github.com')).toBeInTheDocument();
    expect(screen.getByText('Connected')).toBeInTheDocument();
  });

  it('shows the empty state when there are no accounts', async () => {
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([]);
    renderSection();
    expect(await screen.findByText('No accounts connected')).toBeInTheDocument();
  });

  it('add-form submit calls forgeSetTokenForHost with the chosen kind/host/token', async () => {
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([]);
    const setToken = vi.spyOn(ipc, 'forgeSetTokenForHost').mockResolvedValue(VIEWER);
    renderSection();

    fireEvent.click(await screen.findByRole('button', { name: 'Add a token for a host' }));

    fireEvent.change(screen.getByLabelText('Host'), { target: { value: 'GitHub.com' } });
    fireEvent.change(screen.getByLabelText('Personal access token'), {
      target: { value: 'ghp_valid_token' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add account' }));

    await waitFor(() => expect(setToken).toHaveBeenCalledTimes(1));
    // Host lowercased on submit (backend keys are lowercased); default kind gitHub.
    expect(setToken).toHaveBeenCalledWith('github.com', 'gitHub', 'ghp_valid_token');
  });

  it('the Azure DevOps provider option is disabled (cannot be selected)', async () => {
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([]);
    renderSection();
    fireEvent.click(await screen.findByRole('button', { name: 'Add a token for a host' }));

    const azure = screen.getByRole('radio', { name: 'Azure DevOps' });
    expect(azure).toBeDisabled();
    expect(azure).toHaveAttribute('aria-disabled', 'true');
    expect(azure).not.toBeChecked();
    // NOTE: the AZURE_HINT paragraph only renders when Azure is the *selected*
    // kind, but Azure is disabled and thus never selectable — so the hint that
    // explains the disable is effectively never shown (see discrepancy report).
  });

  it('a rejected add token shows an inline authFailed error and keeps the form open', async () => {
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([]);
    vi.spyOn(ipc, 'forgeSetTokenForHost').mockRejectedValue({
      kind: 'authFailed',
      message: 'rejected',
    });
    renderSection();

    fireEvent.click(await screen.findByRole('button', { name: 'Add a token for a host' }));
    fireEvent.change(screen.getByLabelText('Host'), { target: { value: 'github.com' } });
    fireEvent.change(screen.getByLabelText('Personal access token'), {
      target: { value: 'ghp_bad' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add account' }));

    expect(await screen.findByText(/That token was rejected/i)).toBeInTheDocument();
    // Form still present (old token intact, user can retry).
    expect(screen.getByLabelText('Host')).toBeInTheDocument();
  });

  it('Remove account → confirm calls forgeClearTokenForHost after the ConfirmDialog', async () => {
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([GH_ACCOUNT]);
    const clear = vi.spyOn(ipc, 'forgeClearTokenForHost').mockResolvedValue(undefined);
    renderSection();

    const kebab = await screen.findByRole('button', { name: /Actions for octocat/i });
    fireEvent.click(kebab);
    fireEvent.click(screen.getByRole('menuitem', { name: 'Remove account' }));

    const dialog = await screen.findByRole('dialog');
    expect(dialog).toHaveTextContent(/Remove github\.com\?/);
    // Nothing cleared until the confirm is pressed.
    expect(clear).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Remove' }));

    await waitFor(() => expect(clear).toHaveBeenCalledWith('github.com'));
  });
});
