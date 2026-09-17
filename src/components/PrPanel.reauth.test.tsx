/** P79/P80 §1.3 + §2/§4 — PrPanel owns (a) the per-repo override reset (P80:
 *  "Reset to host default" → forgeSetRepoAccount(repoId, null), no confirm) and
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

  /** P114 Addendum B — the `Could not connect: ${message}` toast is GONE. The
   *  inline `role="alert"` banner (`ForgeConnect`) IS the notification, so the
   *  absence of the toast is the fix: `forgeSetToken` rejects with BOTH
   *  cause-shaped (`authFailed`) and outcome-shaped messages, no single prefix
   *  can be right for both, and telling them apart at runtime is the string
   *  sniffing P114 rejected. It was also a DOUBLE announcement. */
  it('a failing reconnect renders the message inline and raises NO toast', async () => {
    vi.spyOn(ipc, 'forgeRepoContext').mockResolvedValue(CTX);
    vi.spyOn(ipc, 'forgeListPrs').mockRejectedValue(AUTH_FAILED);
    vi.spyOn(ipc, 'forgeInvalidateViewer').mockResolvedValue(undefined);
    vi.spyOn(ipc, 'forgeSetToken').mockRejectedValue({
      kind: 'other',
      message:
        "The credential in the OS keychain is up to date, but the account details couldn't be saved. Details: access denied",
    } satisfies AppError);

    const { pushToast } = renderPanel();

    const token = await screen.findByLabelText('Personal access token');
    fireEvent.change(token, { target: { value: 'ghp_nope' } });
    fireEvent.click(screen.getByRole('button', { name: 'Reconnect' }));

    // Verbatim, in the banner's own live region — one utterance.
    const banner = await waitFor(() => {
      const el = document.querySelector<HTMLElement>('.pr-error[role="alert"]');
      if (el === null) throw new Error('the failure must render inline');
      return el;
    });
    expect(banner).toHaveTextContent(
      "The credential in the OS keychain is up to date, but the account details couldn't be saved.",
    );
    expect(banner.textContent).not.toContain('Could not connect');
    // The deletion itself: no toast, not a reworded one.
    expect(pushToast).not.toHaveBeenCalled();
  });

  it('P80: kebab "Reset to host default" clears the override (no confirm) under an override', async () => {
    vi.spyOn(ipc, 'forgeRepoContext').mockResolvedValue({ ...CTX, accountSource: 'override' });
    vi.spyOn(ipc, 'forgeListPrs').mockResolvedValue({ items: [], page: 1, hasNext: false });
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([]);
    const setRepo = vi.spyOn(ipc, 'forgeSetRepoAccount').mockResolvedValue(undefined);

    renderPanel();

    // Header appears once the viewer is warm + list resolves.
    const kebab = await screen.findByRole('button', { name: 'Account actions' });
    fireEvent.click(kebab);
    fireEvent.click(screen.getByRole('menuitem', { name: 'Reset to host default' }));

    // Nondestructive, no confirm dialog — the override is cleared immediately.
    expect(screen.queryByRole('dialog')).toBeNull();
    await waitFor(() => expect(setRepo).toHaveBeenCalledWith('r1', null));
  });
});
