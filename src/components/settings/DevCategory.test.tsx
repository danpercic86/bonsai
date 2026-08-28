/**
 * P91 §12 row-7 — the DevCategory INTEGRATION path the pure-function tests miss:
 * mock `logSessionInfo` → 2s poll → status card, and `logsDeleteAll` result →
 * confirm → toast + announce + re-poll. Driven through the real SettingsPanel
 * (so the real SettingsContext + confirm dialog are in the loop) with `mockIpc`
 * spies standing in for the query-param harness scenarios (`?obsTruncated=1`,
 * `?obsDeleteFail=1`) that jsdom cannot set.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';

import { SettingsPanel } from '../SettingsPanel';
import { MINIMAL } from './coverageFixtures';
import { ToastContext, type PushToast } from '../../ToastContext';
import { mockIpc } from '../../ipc/mock';
import type { LogSessionInfo, LogsDeleteResult } from '../../ipc';

const DEV_ON = {
  enabled: true,
  level: 'debug' as const,
  captureIpc: true,
  captureReact: true,
  captureFrames: false,
  includeRawNames: false,
};

function sessionInfo(over: Partial<LogSessionInfo> = {}): LogSessionInfo {
  return {
    sessionId: 's1',
    dir: '/logs',
    files: ['bonsai-a.jsonl'],
    bytes: 500_000,
    records: 1200,
    anomalies: 0,
    dropped: 0,
    redaction: 'strict',
    salt: 'x',
    totalFiles: 5,
    totalBytes: 16_567_501,
    droppedParts: 0,
    exportFiles: 0,
    exportBytes: 0,
    ...over,
  };
}

function renderDev(push: PushToast, over: Partial<React.ComponentProps<typeof SettingsPanel>> = {}) {
  return render(
    <ToastContext.Provider value={push}>
      <SettingsPanel
        open
        initialCategory="dev"
        onClose={vi.fn()}
        requestSeq={0}
        onChange={vi.fn()}
        onToggleTheme={vi.fn()}
        onToggleListView={vi.fn()}
        onRequestEnableAi={vi.fn()}
        onSetMcpEnabled={vi.fn()}
        onRequestEnableMcp={vi.fn()}
        onSetMcpAllowWrite={vi.fn()}
        onRequestEnableMcpWrite={vi.fn()}
        onRegisterMcp={vi.fn(async () => {})}
        onShowOnboarding={vi.fn()}
        onOpenRepository={vi.fn()}
        onCheckUpdate={vi.fn()}
        onOpenUpdateDialog={vi.fn()}
        {...MINIMAL}
        dev={DEV_ON}
        {...over}
      />
    </ToastContext.Provider>,
  );
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('DevCategory — poll wiring', () => {
  it('flows a truncated session (droppedParts>0) from the mock into the status card', async () => {
    vi.spyOn(mockIpc, 'logSessionInfo').mockResolvedValue(sessionInfo({ droppedParts: 3 }));
    renderDev(vi.fn());
    expect(await screen.findByText(/reached its size limit/)).toBeInTheDocument();
  });
});

describe('DevCategory — delete wiring', () => {
  it('confirm-gates delete, then surfaces the partial-failure toast and re-polls', async () => {
    const info = vi.spyOn(mockIpc, 'logSessionInfo').mockResolvedValue(sessionInfo());
    const partial: LogsDeleteResult = {
      deletedFiles: 4,
      deletedBytes: 1_000_000,
      failedFiles: 1,
      activeFile: 'new.jsonl',
      rolled: true,
      deletedExports: 0,
    };
    const del = vi.spyOn(mockIpc, 'logsDeleteAll').mockResolvedValue(partial);
    const push = vi.fn();
    renderDev(push);

    // Wait for the mount poll to enable the (logs-exist) Delete button.
    const deleteBtn = await screen.findByRole('button', { name: 'Delete logs…' });
    await waitFor(() => expect(deleteBtn).toHaveAttribute('aria-disabled', 'false'));

    // Opening re-reads session info (§8.5.4), so the dialog count is fresh.
    const callsBeforeOpen = info.mock.calls.length;
    fireEvent.click(deleteBtn);
    await waitFor(() => expect(info.mock.calls.length).toBeGreaterThan(callsBeforeOpen));
    const dialog = await screen.findByRole('dialog', { name: 'Delete all log files?' });
    expect(dialog).toHaveTextContent(/Delete 5 log files/);

    // Delete is not invoked until confirm.
    expect(del).not.toHaveBeenCalled();
    const callsBeforeDelete = info.mock.calls.length;
    fireEvent.click(screen.getByRole('button', { name: 'Delete logs' }));

    await waitFor(() =>
      expect(push).toHaveBeenCalledWith(
        'error',
        expect.stringContaining('Deleted 4 of 5 log files.'),
        'dev-delete',
      ),
    );
    expect(del).toHaveBeenCalledTimes(1);
    // §8.5.5: re-polled after the outcome so the row/card never go stale.
    expect(info.mock.calls.length).toBeGreaterThan(callsBeforeDelete);
  });

  it('does not invoke delete when the confirm dialog is cancelled', async () => {
    vi.spyOn(mockIpc, 'logSessionInfo').mockResolvedValue(sessionInfo());
    const del = vi.spyOn(mockIpc, 'logsDeleteAll');
    renderDev(vi.fn());

    const deleteBtn = await screen.findByRole('button', { name: 'Delete logs…' });
    await waitFor(() => expect(deleteBtn).toHaveAttribute('aria-disabled', 'false'));
    fireEvent.click(deleteBtn);
    const cancel = await screen.findByRole('button', { name: 'Cancel' });
    await act(async () => {
      fireEvent.click(cancel);
    });
    expect(del).not.toHaveBeenCalled();
  });
});
