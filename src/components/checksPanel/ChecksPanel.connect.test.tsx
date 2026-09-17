/** P114 Addendum B — the `Could not connect: ${message}` toast is GONE from
 *  `ChecksPanel.handleConnect`: the inline `role="alert"` banner (`ForgeConnect`)
 *  IS the notification, and the toast double-framed it with a prefix that cannot
 *  be true for both the cause-shaped and outcome-shaped rejections
 *  `forgeSetToken` produces. PrPanel's sibling deletion is pinned by
 *  `PrPanel.reauth.test.tsx`; this is the same pin for ChecksPanel, which had
 *  none (its e2e coverage never reaches the connect path).
 *
 *  The `pushToast` PROP stays — `openUrl` still uses it (ChecksPanel.tsx:93).
 *  Exercised through mode `'connect'` (an unauthenticated context puts
 *  `useBranchChecks` in the `connect` state); the deleted line lived in
 *  `handleConnect`'s shared rejection arm, so the mode is irrelevant to the pin. */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

import { ipc } from '../../ipc';
import type { AppError, ForgeRepoContext } from '../../ipc';
import { FORGE_REPO_CONTEXT } from '../../ipc/fixtures/forge';
import { ToastContext } from '../../ToastContext';
import { ChecksPanel } from './ChecksPanel';
import type { ChecksTarget } from './checksTarget';

const TARGET: ChecksTarget = { name: 'main', tip: 'a'.repeat(40), hasUpstream: true };

/** Provider known, not authenticated ⇒ ChecksState `connect`. */
const UNAUTHENTICATED: ForgeRepoContext = {
  ...FORGE_REPO_CONTEXT,
  authenticated: false,
  viewer: null,
};

const SET_TOKEN_FAILURE: AppError = {
  kind: 'other',
  message:
    "The credential in the OS keychain is up to date, but the account details couldn't be saved. Details: access denied",
};

describe('ChecksPanel — a failing connect is inline only (P114 Addendum B)', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('renders the message inline and raises NO toast', async () => {
    vi.spyOn(ipc, 'forgeRepoContext').mockResolvedValue(UNAUTHENTICATED);
    vi.spyOn(ipc, 'forgeSetToken').mockRejectedValue(SET_TOKEN_FAILURE);
    const pushToast = vi.fn();

    render(
      <ToastContext.Provider value={pushToast}>
        <ChecksPanel repoId="r1" target={TARGET} refreshSeq={0} active />
      </ToastContext.Provider>,
    );

    // `useBranchChecks` debounces the bootstrap by 300 ms (real timers).
    const token = await screen.findByLabelText('Personal access token', {}, { timeout: 2000 });
    fireEvent.change(token, { target: { value: 'ghp_nope' } });
    fireEvent.click(screen.getByRole('button', { name: 'Connect' }));

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
});
