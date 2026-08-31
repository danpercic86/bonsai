/** PrDetailContainer — the merge/close WIRING (P83). This is where the action
 *  buttons meet the mutating IPC: a click opens a confirmation (merge dialog /
 *  close ConfirmDialog), and only an explicit confirm dispatches
 *  forgeMergePr / forgeClosePr. Pinned here: nothing mutates on mount or on merely
 *  opening a dialog, the per-forge close verb routes through its confirm, and a
 *  success hands the updated detail back up + refreshes the list. */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/react';
import { PR_DIFF_STATS } from '../../ipc/fixtures/prDiff';
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

function renderContainer(kind: ForgeKind = 'gitHub', onClosePrFileDiff = vi.fn()) {
  const pushToast = vi.fn();
  const onDetailReplaced = vi.fn();
  const onListChanged = vi.fn();
  const onReload = vi.fn();
  const onAuthFailed = vi.fn().mockReturnValue(false);
  const onOpenFileDiff = vi.fn();
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
        onOpenFileDiff={onOpenFileDiff}
        onClosePrFileDiff={onClosePrFileDiff}
        prOverlayPath={null}
      />
    </ToastContext.Provider>,
  );
  return {
    pushToast,
    onDetailReplaced,
    onListChanged,
    onReload,
    onOpenFileDiff,
    onClosePrFileDiff,
  };
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

/** P93: the changed-files list hands ONE file up to the container that owns the
 *  center overlay, and orphaned overlays are collapsed. */
describe('PrDetailContainer — P93 PR file diff wiring', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('a row click reports the PR number + the resolved base…head oids', async () => {
    vi.spyOn(ipc, 'forgePrDiff').mockResolvedValue(PR_DIFF_STATS);
    const { onOpenFileDiff } = renderContainer();

    const row = await screen.findByRole('button', { name: /README\.md/ });
    fireEvent.click(row);
    expect(onOpenFileDiff).toHaveBeenCalledTimes(1);
    expect(onOpenFileDiff).toHaveBeenCalledWith({
      prNumber: 128,
      baseOid: PR_DIFF_STATS.mergeBaseOid,
      headOid: PR_DIFF_STATS.headOid,
      header: PR_DIFF_STATS.files.find((f) => f.path === 'README.md'),
    });
  });

  it('collapses the center overlay when the detail unmounts (tab switch / Back)', async () => {
    vi.spyOn(ipc, 'forgePrDiff').mockResolvedValue(PR_DIFF_STATS);
    const onClose = vi.fn();
    renderContainer('gitHub', onClose);
    await screen.findByRole('button', { name: /README\.md/ });
    expect(onClose).not.toHaveBeenCalled();
    cleanup();
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('collapses the center overlay on a head advance (new headOid)', async () => {
    const advanced = { ...PR_DIFF_STATS, headOid: 'd'.repeat(40) };
    vi.spyOn(ipc, 'forgePrDiff')
      .mockResolvedValueOnce(PR_DIFF_STATS)
      .mockResolvedValue(advanced);
    const onClose = vi.fn();
    // A PR number no other test used: usePrDiff's re-open cache is module-level.
    const detail: PrDetail = {
      ...FORGE_PR_DETAIL,
      summary: { ...FORGE_PR_DETAIL.summary, number: 9931, headSha: 'e'.repeat(40) },
    };
    const props = {
      repoId: 'r1',
      detail,
      kind: 'gitHub' as ForgeKind,
      host: 'github.com',
      comments: [],
      commentsLoading: false,
      commentsError: null,
      onBack: vi.fn(),
      onOpenUrl: vi.fn(),
      onDetailReplaced: vi.fn(),
      onListChanged: vi.fn(),
      onReload: vi.fn(),
      onAuthFailed: vi.fn().mockReturnValue(false),
      onOpenFileDiff: vi.fn(),
      onClosePrFileDiff: onClose,
      prOverlayPath: 'README.md',
    };
    const { rerender } = render(
      <ToastContext.Provider value={vi.fn()}>
        <PrDetailContainer {...props} />
      </ToastContext.Provider>,
    );
    await screen.findByRole('button', { name: /README\.md/ });
    expect(onClose).not.toHaveBeenCalled();

    // The PR head moved: usePrDiff re-keys on headSha and returns new stats.
    const moved: PrDetail = {
      ...detail,
      summary: { ...detail.summary, headSha: 'd'.repeat(40) },
    };
    rerender(
      <ToastContext.Provider value={vi.fn()}>
        <PrDetailContainer {...props} detail={moved} />
      </ToastContext.Provider>,
    );
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    // The list stays rendered and clickable underneath.
    expect(screen.getByRole('button', { name: /README\.md/ })).toBeEnabled();
  });

  // P93 §6.1 / AC14+AC17 regression: a head advance collapses the overlay, so
  // `prOverlayPath` goes null while the row still exists. That must NOT move
  // focus — the rev-1 activePath-transition rule stole it from wherever the user
  // was (e.g. the graph scroller). No dismissal token is produced by C3.
  it('a head advance does not move focus into the changed-files list', async () => {
    vi.spyOn(ipc, 'forgePrDiff').mockResolvedValue({ ...PR_DIFF_STATS, headOid: '1'.repeat(40) });
    const detail: PrDetail = {
      ...FORGE_PR_DETAIL,
      summary: { ...FORGE_PR_DETAIL.summary, number: 9932, headSha: 'f'.repeat(40) },
    };
    const props = {
      repoId: 'r1',
      detail,
      kind: 'gitHub' as ForgeKind,
      host: 'github.com',
      comments: [],
      commentsLoading: false,
      commentsError: null,
      onBack: vi.fn(),
      onOpenUrl: vi.fn(),
      onDetailReplaced: vi.fn(),
      onListChanged: vi.fn(),
      onReload: vi.fn(),
      onAuthFailed: vi.fn().mockReturnValue(false),
      onOpenFileDiff: vi.fn(),
      onClosePrFileDiff: vi.fn(),
      prOverlayPath: 'README.md' as string | null,
      prRestoreFocusTo: null,
    };
    const { rerender } = render(
      <ToastContext.Provider value={vi.fn()}>
        <PrDetailContainer {...props} />
      </ToastContext.Provider>,
    );
    await screen.findByRole('button', { name: /README\.md/ });

    // The user is interacting somewhere else entirely (stand-in for the graph).
    const elsewhere = document.createElement('button');
    document.body.appendChild(elsewhere);
    elsewhere.focus();

    const moved: PrDetail = {
      ...detail,
      summary: { ...detail.summary, headSha: '1'.repeat(40) },
    };
    rerender(
      <ToastContext.Provider value={vi.fn()}>
        <PrDetailContainer {...props} detail={moved} prOverlayPath={null} />
      </ToastContext.Provider>,
    );
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /README\.md/ })).toBeEnabled(),
    );
    expect(document.activeElement).toBe(elsewhere);
    elsewhere.remove();
  });
});
