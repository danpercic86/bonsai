/**
 * P91 §12 row-7 — Developer-page states that the browser harness cannot reach
 * (the mock reports `totalFiles: 0` when Dev mode is off, and has no partial /
 * exports scenario). These are asserted here against stubbed shapes.
 */
import { describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach } from 'vitest';

import type { LogSessionInfo, LogsDeleteResult } from '../../ipc';
import { DevSessionStatus } from './DevSessionStatus';
import { DeleteLogsConfirmDialog } from './DevConfirmDialogs';
import { deleteResultToast, deleteErrorText, exportErrorText } from './devLogMessages';

afterEach(cleanup);

function info(over: Partial<LogSessionInfo> = {}): LogSessionInfo {
  return {
    sessionId: 's1',
    dir: '/logs',
    files: ['bonsai-a.jsonl'],
    bytes: 3_248_112,
    records: 41_208,
    anomalies: 0,
    dropped: 0,
    redaction: 'strict',
    salt: 'deadbeef',
    totalFiles: 2,
    totalBytes: 3_248_112,
    droppedParts: 0,
    writeFailed: false,
    exportFiles: 0,
    exportBytes: 0,
    ...over,
  };
}

describe('DevSessionStatus', () => {
  it('Dev mode off with logs kept shows the persistence line, not "No logs yet"', () => {
    render(<DevSessionStatus info={info({ totalFiles: 2 })} enabled={false} />);
    expect(screen.getByText(/kept on this computer/)).toBeInTheDocument();
    expect(screen.queryByText('No logs yet.')).toBeNull();
  });

  it('with no logs ever, renders "No logs yet."', () => {
    render(<DevSessionStatus info={info({ totalFiles: 0, totalBytes: 0 })} enabled={false} />);
    expect(screen.getByText('No logs yet.')).toBeInTheDocument();
  });

  it('renders the flagged count only when anomalies > 0', () => {
    render(<DevSessionStatus info={info({ anomalies: 2 })} enabled />);
    expect(screen.getByText(/2 flagged/)).toBeInTheDocument();
  });

  it('renders the §16.5 truncation line only when droppedParts > 0, distinct from dropped', () => {
    const { container } = render(
      <DevSessionStatus info={info({ droppedParts: 3, dropped: 1204 })} enabled />,
    );
    expect(screen.getByText(/reached its size limit/)).toBeInTheDocument();
    // The two signals never merge — dropped is an inline fact, truncation its own
    // warning line; both present, distinct elements.
    expect(container.querySelector('.dev-status-dropped')?.textContent).toMatch(
      /records dropped/,
    );
    expect(container.querySelectorAll('.dev-status-warn').length).toBe(1);
  });

  it('renders the §8.4 "Not writing" state when writeFailed, and not otherwise', () => {
    const { container, rerender } = render(
      <DevSessionStatus info={info({ writeFailed: true })} enabled />,
    );
    expect(screen.getByText('Not writing')).toBeInTheDocument();
    expect(screen.queryByText('Recording')).toBeNull();
    expect(screen.getByText(/Bonsai stopped writing the log/)).toBeInTheDocument();
    expect(container.querySelector('.dev-status-write-failed')).not.toBeNull();
    // Privacy: the generic copy never interpolates a path or errno.
    expect(screen.queryByText(/os error|denied|\/logs/)).toBeNull();

    rerender(<DevSessionStatus info={info({ writeFailed: false })} enabled />);
    expect(screen.queryByText('Not writing')).toBeNull();
    expect(screen.queryByText(/Bonsai stopped writing the log/)).toBeNull();
  });

  it('never renders the session salt (§16.1)', () => {
    const { container } = render(
      <DevSessionStatus info={info({ salt: 'SECRETSALT123' })} enabled />,
    );
    expect(container.textContent).not.toContain('SECRETSALT123');
  });
});

describe('DeleteLogsConfirmDialog', () => {
  it('states count + bytes + exports clause + out-of-scope caveat + Dev-on sentence', () => {
    render(
      <DeleteLogsConfirmDialog
        open
        info={info({ totalFiles: 5, totalBytes: 16_567_501, exportFiles: 1 })}
        devEnabled
        busy={false}
        onConfirm={() => {}}
        onCancel={() => {}}
      />,
    );
    expect(screen.getByText(/Delete 5 log files/)).toBeInTheDocument();
    expect(screen.getByText(/1 exported log archive\b/)).toBeInTheDocument();
    expect(screen.getByText(/session being recorded right now/)).toBeInTheDocument();
    expect(screen.getByText(/Exports you saved elsewhere are not removed\./)).toBeInTheDocument();
    // Default focus is Cancel (a destructive dialog must not confirm on a stray Return).
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Cancel' }));
  });
});

describe('deleteResultToast (§16.4)', () => {
  const base: LogsDeleteResult = {
    deletedFiles: 4,
    deletedBytes: 1_248_130,
    failedFiles: 0,
    activeFile: null,
    rolled: false,
    deletedMetrics: 0,
    metricsCleared: true,
  };

  it('rolled:false, no exports', () => {
    const t = deleteResultToast(base);
    expect(t.tone).toBe('success');
    expect(t.text).toMatch(/^Deleted 4 log files\. .* freed\. Usage counts cleared\.$/);
    expect(t.text).not.toContain('Still recording');
  });

  // §F6: `metricsCleared` is the ONLY justification for the usage-count clause.
  // `deletedMetrics === 0` is a legitimate success (a launch younger than the
  // first flush has nothing on disk yet), so it must NOT weaken the claim — and
  // `deletedMetrics > 0` must not be double-counted as log files.
  it('claims the usage counts cleared even when no metrics FILE was deleted', () => {
    const t = deleteResultToast({ ...base, deletedMetrics: 0, metricsCleared: true });
    expect(t.text).toContain('Usage counts cleared.');
    expect(t.text).toMatch(/^Deleted 4 log files\./);
  });

  it('does not count the metrics files as log files', () => {
    const t = deleteResultToast({ ...base, deletedFiles: 6, deletedMetrics: 2 });
    expect(t.text).toMatch(/^Deleted 4 log files\./);
  });

  it('says so plainly when the usage counts were NOT cleared', () => {
    const t = deleteResultToast({ ...base, deletedMetrics: 0, metricsCleared: false });
    expect(t.text).toContain('Usage counts were not cleared.');
    expect(t.announce).toContain('Usage counts were not cleared.');
  });

  it('rolled:true names the "Still recording" clause', () => {
    const t = deleteResultToast({ ...base, rolled: true, activeFile: 'new.jsonl' });
    expect(t.text).toContain('Still recording — Bonsai started a new log file.');
  });

  it('names exports separately when deletedExports > 0', () => {
    const t = deleteResultToast({ ...base, deletedFiles: 5, deletedExports: 1 });
    expect(t.text).toMatch(/^Deleted 4 log files and 1 export\. /);
  });

  // §6.8 R5 — the zero-log outcome. "Deleted 0 log files" reads as a bug in the
  // exact state §F6 made this row serve, and the mock's hard-coded counts are
  // what kept this branch out of the harness. Two rows, because a saved export
  // zip outlives the logs it came from: zero logs WITH exports is reachable.
  it('leads with the usage counts when no log file was deleted', () => {
    const t = deleteResultToast({ ...base, deletedFiles: 1, deletedMetrics: 1 });
    expect(t.tone).toBe('success');
    expect(t.text).toMatch(/^Usage counts cleared\. .* freed\.$/);
    expect(t.text).not.toContain('0 log files');
    expect(t.announce).toBe('Usage counts cleared.');
  });

  it('still names the exports when no log file was deleted', () => {
    const t = deleteResultToast({
      ...base,
      deletedFiles: 3,
      deletedExports: 2,
      deletedMetrics: 1,
    });
    expect(t.text).toMatch(/^Deleted 2 exports\. .* freed\. Usage counts cleared\.$/);
    expect(t.text).not.toContain('0 log files');
    expect(t.announce).toBe('Usage counts cleared.');
  });

  // The lead is driven by `metricsCleared`, never hard-coded: an unreadable (but
  // still present) `metrics/` clears the aggregate in memory, deletes no file and
  // fails none, so this reaches the success branch with `metricsCleared: false`.
  it('does not claim a clear that did not happen in the zero-log branch', () => {
    const t = deleteResultToast({
      ...base,
      deletedFiles: 0,
      deletedBytes: 0,
      deletedMetrics: 0,
      metricsCleared: false,
    });
    expect(t.text).toMatch(/^Usage counts were not cleared\./);
    expect(t.announce).toBe('Usage counts were not cleared.');
  });

  it('partial failure is a danger toast', () => {
    const t = deleteResultToast({
      ...base,
      deletedFiles: 4,
      failedFiles: 1,
      metricsCleared: false,
    });
    expect(t.tone).toBe('error');
    expect(t.text).toContain('Deleted 4 of 5 log files.');
    expect(t.text).toContain('could not be deleted');
    // §6.8 R5 NIT — an error says what to do next, and a retry is what actually
    // clears the counts. Partial-failure path ONLY: the total-failure path maps
    // to `deleteErrorText`, whose branches already name a cause.
    expect(t.text).toContain('Usage counts were not cleared. Try again.');
    expect(t.announce).toContain('Usage counts were not cleared. Try again.');
  });
});

describe('mapped error copy (never raw OS text)', () => {
  it('delete permission / folder-missing', () => {
    expect(deleteErrorText('permission denied')).toBe(
      "Bonsai isn't allowed to delete files in that folder.",
    );
    expect(deleteErrorText('the folder is no longer there')).toBe(
      'The logs folder is no longer there.',
    );
    // §6.8 R5 — the fallback names the action's FULL scope. It told the zero-log
    // user nothing about the counts they actually asked to clear.
    expect(deleteErrorText('os error 1392')).toBe(
      "Couldn't delete the logs and usage counts.",
    );
  });
  it('export disk-full / permission', () => {
    expect(exportErrorText('no space left on device')).toMatch(/Not enough space/);
    // F4 removed the destination picker, so the sentence names the exports
    // folder Bonsai actually failed to write (P91 §8.3 amendment).
    expect(exportErrorText('permission denied')).toBe(
      "Bonsai isn't allowed to write to its exports folder. Check the folder's permissions and try again.",
    );
  });
});
