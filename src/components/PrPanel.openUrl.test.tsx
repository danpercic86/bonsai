/** P72 (contract §3.3 aa): `PrPanel` owns the single `ipc.openUrl` call for both
 *  external links, and a rejection surfaces exactly ONE error toast whose prefix
 *  names the intent per site (UI contract §3.3). Runs against the mock IPC layer
 *  (VITE_MOCK_IPC=1 in the jsdom project), so no per-test IPC module mock. */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { ipc } from '../ipc';
import type { AppError } from '../ipc';
import { PrPanel } from './PrPanel';
import { ToastContext } from '../ToastContext';

const LAUNCH_FAILED: AppError = {
  kind: 'externalToolFailed',
  message: 'could not launch browser (rundll32): boom',
};

async function renderPanel() {
  const { repoId } = await ipc.openRepo('/mock/p72-repo');
  const pushToast = vi.fn();
  render(
    <ToastContext.Provider value={pushToast}>
      <PrPanel repoId={repoId} />
    </ToastContext.Provider>,
  );
  return { pushToast };
}

const tokenLink = () => screen.getByRole('link', { name: 'Create a token' });

describe('PrPanel — P72 openUrl wiring', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('a successful openUrl raises no toast', async () => {
    const spy = vi.spyOn(ipc, 'openUrl').mockResolvedValue(undefined);
    const { pushToast } = await renderPanel();
    await waitFor(() => expect(tokenLink()).toBeInTheDocument());
    fireEvent.click(tokenLink());
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
    expect(spy).toHaveBeenCalledWith('https://github.com/settings/personal-access-tokens/new');
    expect(pushToast).not.toHaveBeenCalled();
  });

  it('a rejected openUrl raises exactly one intent-prefixed error toast', async () => {
    vi.spyOn(ipc, 'openUrl').mockRejectedValue(LAUNCH_FAILED);
    const { pushToast } = await renderPanel();
    await waitFor(() => expect(tokenLink()).toBeInTheDocument());
    fireEvent.click(tokenLink());
    await waitFor(() => expect(pushToast).toHaveBeenCalledTimes(1));
    expect(pushToast).toHaveBeenCalledWith(
      'error',
      `Could not open the token page: ${LAUNCH_FAILED.message}`,
    );
    // The connect form is untouched — a failed launch changes no view state.
    expect(screen.getByLabelText('Personal access token')).toBeInTheDocument();
  });
});
