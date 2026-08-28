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
  };

  it('rolled:false, no exports', () => {
    const t = deleteResultToast(base);
    expect(t.tone).toBe('success');
    expect(t.text).toMatch(/^Deleted 4 log files\. .* freed\.$/);
    expect(t.text).not.toContain('Still recording');
  });

  it('rolled:true names the "Still recording" clause', () => {
    const t = deleteResultToast({ ...base, rolled: true, activeFile: 'new.jsonl' });
    expect(t.text).toContain('Still recording — Bonsai started a new log file.');
  });

  it('names exports separately when deletedExports > 0', () => {
    const t = deleteResultToast({ ...base, deletedFiles: 5, deletedExports: 1 });
    expect(t.text).toMatch(/^Deleted 4 log files and 1 export\. /);
  });

  it('partial failure is a danger toast', () => {
    const t = deleteResultToast({ ...base, deletedFiles: 4, failedFiles: 1 });
    expect(t.tone).toBe('error');
    expect(t.text).toContain('Deleted 4 of 5 log files.');
    expect(t.text).toContain('could not be deleted');
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
  });
  it('export disk-full / permission', () => {
    expect(exportErrorText('no space left on device')).toMatch(/Not enough space/);
    expect(exportErrorText('permission denied')).toMatch(/isn't allowed to write there/);
  });
});
