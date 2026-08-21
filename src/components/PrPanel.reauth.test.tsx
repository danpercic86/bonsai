/** P79 §1.3 + §2/§4 — PrPanel owns (a) the Disconnect confirm → forgeClearToken and
 *  (b) the expiry → reauth flow: an authFailed from a forge read invalidates the
 *  cache-warm viewer WITHOUT clearing the token, then routes to ForgeConnect in
 *  reauth mode with the warning banner. Runs against the mock IPC layer (jsdom
 *  project) but spies each `ipc` method so the flow is deterministic regardless of
 *  URL sentinels. */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

import { ipc } from '../ipc';
import type { AppError, ForgeRepoContext } from '../ipc';
import { PrPanel } from './PrPanel';
import { ToastContext } from '../ToastContext';

const CTX: ForgeRepoContext = {
  provider: 'gitHub',
  host: 'github.com',
  owner: 'octo-org',
  repo: 'bonsai',
  project: null,
  remoteName: 'origin',
  webUrl: 'https://github.com/octo-org/bonsai',
  authenticated: true,
  viewer: { login: 'octocat', avatarUrl: null },
  resolvedAccountId: 'gitHub:github.com:octocat',
  accountSource: 'ownerMatch',
};

const AUTH_FAILED: AppError = { kind: 'authFailed', message: 'mock: token expired' };

function renderPanel() {
  const pushToast = vi.fn();
  render(
    <ToastContext.Provider value={pushToast}>
      <PrPanel repoId="r1" />
    </ToastContext.Provider>,
  );
  return { pushToast };
}

describe('PrPanel — P79 reauth + disconnect', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('an authFailed from the PR list invalidates the viewer, shows the reauth banner, and does NOT clear the token', async () => {
    vi.spyOn(ipc, 'forgeRepoContext').mockResolvedValue(CTX);
    vi.spyOn(ipc, 'forgeListPrs').mockRejectedValue(AUTH_FAILED);
    const invalidate = vi.spyOn(ipc, 'forgeInvalidateViewer').mockResolvedValue(undefined);
    const clear = vi.spyOn(ipc, 'forgeClearToken').mockResolvedValue(undefined);

    const { pushToast } = renderPanel();

    // The reauth banner (role=status) is shown with the expiry copy.
    await waitFor(() =>
      expect(screen.getByText(/expired or was revoked/i)).toBeInTheDocument(),
    );
    // Viewer invalidated for the connected host; token NOT cleared.
    expect(invalidate).toHaveBeenCalledWith('github.com');
    expect(clear).not.toHaveBeenCalled();
    // Reconnect copy (reauth heading + submit) is visible.
    expect(screen.getByRole('button', { name: 'Reconnect' })).toBeInTheDocument();
    // OD-3: no extra error toast — the banner is the notification.
    expect(pushToast).not.toHaveBeenCalled();
  });

  it('Disconnect → confirm calls forgeClearToken(repoId)', async () => {
    vi.spyOn(ipc, 'forgeRepoContext').mockResolvedValue(CTX);
    vi.spyOn(ipc, 'forgeListPrs').mockResolvedValue({ items: [], page: 1, hasNext: false });
    const clear = vi.spyOn(ipc, 'forgeClearToken').mockResolvedValue(undefined);

    renderPanel();

    // Header appears once the viewer is warm + list resolves.
    const kebab = await screen.findByRole('button', { name: 'Account actions' });
    fireEvent.click(kebab);
    fireEvent.click(screen.getByRole('menuitem', { name: 'Disconnect' }));

    // The confirm dialog opens; confirming clears the token for the repo.
    const dialog = await screen.findByRole('dialog');
    expect(dialog).toHaveTextContent(/Disconnect from github\.com\?/);
    fireEvent.click(screen.getByRole('button', { name: 'Disconnect' }));

    await waitFor(() => expect(clear).toHaveBeenCalledTimes(1));
    expect(clear).toHaveBeenCalledWith('r1');
  });
});
