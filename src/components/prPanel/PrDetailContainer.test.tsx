/** PrDetailContainer — the merge/close WIRING (P83). This is where the action
 *  buttons meet the mutating IPC: a click opens a confirmation (merge dialog /
 *  close ConfirmDialog), and only an explicit confirm dispatches
 *  forgeMergePr / forgeClosePr. Pinned here: nothing mutates on mount or on merely
 *  opening a dialog, the per-forge close verb routes through its confirm, and a
 *  success hands the updated detail back up + refreshes the list. */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ipc } from '../../ipc';
import type { ForgeKind, PrDetail } from '../../ipc';
import { ToastContext } from '../../ToastContext';
import { PrDetailContainer } from './PrDetailContainer';
import { FORGE_PR_DETAIL } from '../../ipc/fixtures/forge';

// A distinct resolved detail so we can assert it is handed back up verbatim.
const CLOSED_DETAIL: PrDetail = {
  ...FORGE_PR_DETAIL,
  summary: { ...FORGE_PR_DETAIL.summary, state: 'closed' },
};
const MERGED_DETAIL: PrDetail = {
  ...FORGE_PR_DETAIL,
  summary: { ...FORGE_PR_DETAIL.summary, state: 'merged' },
};

function renderContainer(kind: ForgeKind = 'gitHub') {
  const pushToast = vi.fn();
  const onDetailReplaced = vi.fn();
  const onListChanged = vi.fn();
  const onReload = vi.fn();
  const onAuthFailed = vi.fn().mockReturnValue(false);
  render(
    <ToastContext.Provider value={pushToast}>
      <PrDetailContainer
        repoId="r1"
        detail={FORGE_PR_DETAIL}
        kind={kind}
        host="github.com"
        comments={[]}
        commentsLoading={false}
        commentsError={null}
        onBack={vi.fn()}
        onOpenUrl={vi.fn()}
        onDetailReplaced={onDetailReplaced}
        onListChanged={onListChanged}
        onReload={onReload}
        onAuthFailed={onAuthFailed}
      />
    </ToastContext.Provider>,
  );
  return { pushToast, onDetailReplaced, onListChanged, onReload };
}

describe('PrDetailContainer — merge/close wiring', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('dispatches nothing on mount and opens no dialog', () => {
    const merge = vi.spyOn(ipc, 'forgeMergePr');
    const close = vi.spyOn(ipc, 'forgeClosePr');
    renderContainer();
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(merge).not.toHaveBeenCalled();
    expect(close).not.toHaveBeenCalled();
  });

  it('Merge opens the merge dialog WITHOUT calling IPC; confirming dispatches forgeMergePr', async () => {
    const merge = vi.spyOn(ipc, 'forgeMergePr').mockResolvedValue(MERGED_DETAIL);
    const { onDetailReplaced, onListChanged, pushToast } = renderContainer('gitHub');

    fireEvent.click(screen.getByRole('button', { name: /Merge/ }));
    // Dialog is open; still no IPC until the explicit confirm.
    expect(screen.getByText('Merge pull request #128?')).toBeInTheDocument();
    expect(merge).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: /^Merge pull request$/ }));
    expect(merge).toHaveBeenCalledTimes(1);
    expect(merge).toHaveBeenCalledWith(
      'r1',
      128,
      expect.objectContaining({ method: 'merge' }),
    );
    await waitFor(() => expect(onDetailReplaced).toHaveBeenCalledWith(MERGED_DETAIL));
    expect(onListChanged).toHaveBeenCalledTimes(1);
    expect(pushToast).toHaveBeenCalledWith('success', expect.stringContaining('#128'));
  });

  it('Close opens a confirm WITHOUT calling IPC; confirming dispatches forgeClosePr', async () => {
    const close = vi.spyOn(ipc, 'forgeClosePr').mockResolvedValue(CLOSED_DETAIL);
    const { onDetailReplaced, onListChanged, pushToast } = renderContainer('gitHub');

    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    // The confirm dialog appears (distinct accessible name from the trigger).
    expect(screen.getByRole('dialog', { name: /Close pull request #128/ })).toBeInTheDocument();
    expect(close).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Close pull request' }));
    expect(close).toHaveBeenCalledTimes(1);
    expect(close).toHaveBeenCalledWith('r1', 128);
    await waitFor(() => expect(onDetailReplaced).toHaveBeenCalledWith(CLOSED_DETAIL));
    expect(onListChanged).toHaveBeenCalledTimes(1);
    expect(pushToast).toHaveBeenCalledWith('success', expect.stringContaining('#128'));
  });

  it('routes the per-forge close verb through its confirm (Bitbucket → Decline)', async () => {
    const close = vi.spyOn(ipc, 'forgeClosePr').mockResolvedValue(CLOSED_DETAIL);
    renderContainer('bitbucket');

    fireEvent.click(screen.getByRole('button', { name: 'Decline' }));
    expect(screen.getByRole('dialog', { name: /Decline pull request #128/ })).toBeInTheDocument();
    expect(close).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Decline pull request' }));
    await waitFor(() => expect(close).toHaveBeenCalledWith('r1', 128));
  });

  it('an IPC merge failure surfaces an error toast and asks the parent to reload', async () => {
    vi.spyOn(ipc, 'forgeMergePr').mockRejectedValue({ kind: 'forgeApi', message: 'boom' });
    const { onDetailReplaced, onReload, pushToast } = renderContainer('gitHub');

    fireEvent.click(screen.getByRole('button', { name: /Merge/ }));
    fireEvent.click(screen.getByRole('button', { name: /^Merge pull request$/ }));

    await waitFor(() =>
      expect(pushToast).toHaveBeenCalledWith('error', expect.stringContaining('#128')),
    );
    expect(onDetailReplaced).not.toHaveBeenCalled();
    expect(onReload).toHaveBeenCalledWith(128);
  });
});
