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
    writeFailed: false,
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
      deletedMetrics: 0,
      metricsCleared: false,
    };
    const del = vi.spyOn(mockIpc, 'logsDeleteAll').mockResolvedValue(partial);
    const push = vi.fn();
    renderDev(push);

    // Wait for the mount poll to enable the (logs-exist) Delete button.
    const deleteBtn = await screen.findByRole('button', { name: 'Delete all…' });
    await waitFor(() => expect(deleteBtn).toHaveAttribute('aria-disabled', 'false'));

    // Opening re-reads session info (§8.5.4), so the dialog count is fresh.
    const callsBeforeOpen = info.mock.calls.length;
    fireEvent.click(deleteBtn);
    await waitFor(() => expect(info.mock.calls.length).toBeGreaterThan(callsBeforeOpen));
    const dialog = await screen.findByRole('dialog', {
      name: 'Delete logs and usage counts?',
    });
    expect(dialog).toHaveTextContent(/Delete 5 log files .* and clear Bonsai's usage counts/);
    // §F6 §6.4: the confirmation names the exact destructive target — the FOLDER,
    // because deleting `usage.json` alone is undone by its `.bak` on next load.
    expect(dialog).toHaveTextContent(/removes that whole folder/);

    // Delete is not invoked until confirm.
    expect(del).not.toHaveBeenCalled();
    const callsBeforeDelete = info.mock.calls.length;
    fireEvent.click(screen.getByRole('button', { name: 'Delete all' }));

    // §6.10 R10: `logParts` = 4 - 0 exports - 0 metrics = 4, and the failure is
    // reported as a file rather than as an invented fifth log file.
    await waitFor(() =>
      expect(push).toHaveBeenCalledWith(
        'error',
        expect.stringContaining('Deleted 4 log files.'),
        'dev-delete',
      ),
    );
    expect(push).toHaveBeenCalledWith(
      'error',
      expect.stringContaining('1 file could not be deleted'),
      'dev-delete',
    );
    expect(del).toHaveBeenCalledTimes(1);
    // §8.5.5: re-polled after the outcome so the row/card never go stale.
    expect(info.mock.calls.length).toBeGreaterThan(callsBeforeDelete);
  });

  /** §6.8 R6 — the danger button's accessible name is just `Delete all…`; the
   *  row title is a <span>, not a label, and `SettingsRow` wires no
   *  `aria-describedby` for `hint`. Without this, a screen-reader user
   *  navigating by button hears "Delete all…, button" for a data-destroying
   *  control and "all" of WHAT reaches sighted users only, by proximity. */
  it('points the danger button at its hint as an accessible description', async () => {
    vi.spyOn(mockIpc, 'logSessionInfo').mockResolvedValue(sessionInfo());
    renderDev(vi.fn());

    const deleteBtn = await screen.findByRole('button', { name: 'Delete all…' });
    const id = deleteBtn.getAttribute('aria-describedby');
    expect(id, 'the button must carry a description idref').toBeTruthy();
    // The idref must RESOLVE — a dangling one is worse than none at all.
    const hint = id === null ? null : document.getElementById(id);
    expect(hint).not.toBeNull();
    expect(hint).toHaveTextContent(/Removes all .* and Bonsai's usage counts/);
    // The accessible NAME is untouched, so the DOM↔catalog guard stays green.
    expect(deleteBtn).toHaveAccessibleName('Delete all…');
  });

  it('does not invoke delete when the confirm dialog is cancelled', async () => {
    vi.spyOn(mockIpc, 'logSessionInfo').mockResolvedValue(sessionInfo());
    const del = vi.spyOn(mockIpc, 'logsDeleteAll');
    renderDev(vi.fn());

    const deleteBtn = await screen.findByRole('button', { name: 'Delete all…' });
    await waitFor(() => expect(deleteBtn).toHaveAttribute('aria-disabled', 'false'));
    fireEvent.click(deleteBtn);
    const cancel = await screen.findByRole('button', { name: 'Cancel' });
    await act(async () => {
      fireEvent.click(cancel);
    });
    expect(del).not.toHaveBeenCalled();
  });
});

/** §F6 §6.4 — the delete row is no longer gated on logs existing, so the
 *  no-logs path has to actually WORK: the dialog opens, it says what it will
 *  clear, and confirming still calls the command. A container that keyed the
 *  dialog's `open` off a non-null session info would make this a dead click. */
describe('DevCategory — delete with no log files (§F6)', () => {
  it('opens the dialog on its n === 0 copy and still deletes', async () => {
    vi.spyOn(mockIpc, 'logSessionInfo').mockResolvedValue({
      ...sessionInfo(),
      files: [],
      totalFiles: 0,
      totalBytes: 0,
      exportFiles: 0,
      exportBytes: 0,
    });
    const del = vi.spyOn(mockIpc, 'logsDeleteAll').mockResolvedValue({
      deletedFiles: 1,
      deletedBytes: 2_048,
      failedFiles: 0,
      activeFile: null,
      rolled: false,
      deletedExports: 0,
      deletedMetrics: 1,
      metricsCleared: true,
    });
    const push = vi.fn();
    renderDev(push);

    const deleteBtn = await screen.findByRole('button', { name: 'Delete all…' });
    // Never gated on logs: it is enabled with zero of them.
    await waitFor(() => expect(deleteBtn).toHaveAttribute('aria-disabled', 'false'));
    fireEvent.click(deleteBtn);

    const dialog = await screen.findByRole('dialog', {
      name: 'Delete logs and usage counts?',
    });
    expect(dialog).toHaveTextContent("Clear Bonsai's usage counts? This cannot be undone.");
    expect(dialog).not.toHaveTextContent(/0 log files/);

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Delete all' }));
    });
    expect(del).toHaveBeenCalledTimes(1);
  });
});

/** §6.8 R3 — a FAILED count read is not a count of zero. Every `logSessionInfo`
 *  read on the page rejects (including the one on mount), so the container has
 *  nothing to fall back on and hands the dialog `null`. Before the fix `?? 0`
 *  collapsed that into the known-zero branch, so the dialog promised to clear
 *  usage counts only and then deleted however many log files were on disk — a
 *  confirmation naming a smaller target than it destroys, which is the exact
 *  defect §6.4 exists to prevent. `refresh` swallows the rejection by design, so
 *  the page stays usable and the delete must still be reachable.
 *
 *  §6.10 R8 — the lead line was only ONE of the state's three strings. The row
 *  hint (R8a, and §6.8 R6 wired it to the danger button as `aria-describedby`)
 *  and the dialog's archives line (R8b) collapsed the same `null` into a known
 *  zero, so both are asserted here too — this is the only test that renders the
 *  whole page with every read failing. */
describe('DevCategory — delete when the count read fails (§6.8 R3, §6.10 R8)', () => {
  it('states the widest scope instead of the known-zero copy, and still deletes', async () => {
    vi.spyOn(mockIpc, 'logSessionInfo').mockRejectedValue(new Error('logs dir unreadable'));
    const del = vi.spyOn(mockIpc, 'logsDeleteAll');
    renderDev(vi.fn());

    const deleteBtn = await screen.findByRole('button', { name: 'Delete all…' });
    await waitFor(() => expect(deleteBtn).toHaveAttribute('aria-disabled', 'false'));

    // §6.10 R8a — the understating copy R3 removed from the dialog had been
    // relocated verbatim into this button's accessible description by §6.8 R6's
    // `aria-describedby`. An unknown count is not "No log files yet".
    const hintId = deleteBtn.getAttribute('aria-describedby');
    const hint = hintId === null ? null : document.getElementById(hintId);
    expect(hint).toHaveTextContent(
      "Bonsai could not count the log files. Removes all of them and Bonsai's usage counts, including the log being recorded now. Recording continues in a new file.",
    );
    expect(hint).not.toHaveTextContent('No log files yet');

    await act(async () => {
      fireEvent.click(deleteBtn);
    });

    const dialog = await screen.findByRole('dialog', {
      name: 'Delete logs and usage counts?',
    });
    // §6.10 R8b — the archives line must not vanish while the reassurance it
    // qualifies ("saved elsewhere") stays, which reads as "no archives involved".
    expect(dialog).toHaveTextContent(
      "This includes any exported log archives in Bonsai's exports folder.",
    );
    expect(dialog).toHaveTextContent('Exports you saved elsewhere are not removed.');
    expect(dialog).toHaveTextContent(
      "Delete all log files and clear Bonsai's usage counts? Bonsai could not count the log files first. This cannot be undone.",
    );
    // The regression itself: the known-zero lead line must NOT appear.
    expect(dialog).not.toHaveTextContent("Clear Bonsai's usage counts? This cannot be undone.");
    // ...and no fabricated count in either direction.
    expect(dialog).not.toHaveTextContent(/Delete 0 log file/);

    // R3: the dialog stays actionable rather than dying silently.
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Delete all' }));
    });
    expect(del).toHaveBeenCalledTimes(1);
  });
});
