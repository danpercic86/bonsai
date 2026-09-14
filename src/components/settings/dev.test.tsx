/**
 * P91 §12 row-7 — Developer-page states the browser harness reaches only behind
 * a fixture flag, or not at all. `?obsDeleteFail=1` covers partial failure and
 * §6.10's `?obsInfoFail=1` covers the unknown count, and §6.11.5's
 * `?obsLogFiles=N` / `?obsExports=N` / `?obsMetricsUnreadable=1` reach the
 * multi-file, export-bearing and unreadable-metrics states the old fixed-count
 * fixture could not. The pure builders are asserted here against stubbed shapes;
 * `obsDeleteCounts.test.ts` asserts the same copy over the real mock handler.
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

  // §6.11.2 R13a — nothing FAILED here (`failedFiles: 0`); the clear simply did
  // not happen, which `metrics_purge.rs` reaches with a present-but-unreadable
  // `metrics/`. The tone must follow the words, and the remedy attaches on this
  // branch too — a green toast reading "were not cleared", with no way forward,
  // is what shipped.
  it('says so plainly when the usage counts were NOT cleared', () => {
    const t = deleteResultToast({ ...base, deletedMetrics: 0, metricsCleared: false });
    expect(t.tone).toBe('error');
    expect(t.text).toContain('Usage counts were not cleared. Try again.');
    expect(t.announce).toBe(t.text);
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
    // §6.11.1 R12 — the announcement used to drop the freed clause here.
    expect(t.announce).toBe(t.text);
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
    // §6.11.1 R12 — the announcement used to name neither the exports nor the
    // bytes: a NARROWER blast radius than the operation had, told to the one user
    // who cannot read the toast.
    expect(t.announce).toBe(t.text);
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
    // §6.11.3 — the whole signed string for this state, verbatim: R13a adds the
    // remedy and the error tone, R13b drops the freed clause (`0 B freed.` beside
    // a bare failure sentence advertised a benefit that did not occur).
    expect(t.text).toBe('Usage counts were not cleared. Try again.');
    expect(t.announce).toBe(t.text);
    expect(t.tone).toBe('error');
  });

  // §6.11.2 R13b — reachable on a SUCCESS: a launch younger than the first 60 s
  // flush clears a live aggregate with nothing on disk (§F6), so `deletedBytes`
  // is 0 while `metricsCleared` is true. `Usage counts cleared. 0 B freed.` is
  // what shipped.
  it('omits the freed clause when nothing was reclaimed', () => {
    const t = deleteResultToast({
      ...base,
      deletedFiles: 0,
      deletedBytes: 0,
      deletedMetrics: 0,
      metricsCleared: true,
    });
    expect(t.tone).toBe('success');
    expect(t.text).toBe('Usage counts cleared.');
    expect(t.text).not.toContain('0 B');
    expect(t.announce).toBe(t.text);
  });

  it('partial failure is a danger toast', () => {
    const t = deleteResultToast({
      ...base,
      deletedFiles: 4,
      failedFiles: 1,
      metricsCleared: false,
    });
    expect(t.tone).toBe('error');
    // §6.10 R10: the successes are reported the way the success branch reports
    // them (`logParts`, here 4 — 4 deleted files, no exports, no metrics), and the
    // failed count gets the only noun that is certainly true. `4 of 5` claimed a
    // 5th LOG file that `LogsDeleteResult` cannot attribute to any category.
    expect(t.text).toContain('Deleted 4 log files.');
    // ...and the failed count agrees with its own pronoun: the shipped string
    // said "1 could not be deleted — THEY may be open".
    expect(t.text).toContain('1 file could not be deleted — it may be open in another program.');
    expect(t.text).not.toContain('of 5');
    // §6.8 R5 NIT, AMENDED by §6.11.2 R13a — an error says what to do next, and
    // a retry is what actually clears the counts, so the remedy attaches wherever
    // `metricsCleared` is false, not on the partial-failure path only. The THROWN
    // path is the one that maps to `deleteErrorText` instead: it returns before
    // the clear ever runs, so there is no usage clause to carry a remedy.
    expect(t.text).toContain('Usage counts were not cleared. Try again.');
    expect(t.announce).toContain('Usage counts were not cleared. Try again.');
  });

  // §6.10 R10 — the other two partial-failure rows. The exports-only row, and
  // row 1 keyed on the `logParts`/`exports` PAIR rather than on `deletedFiles`:
  // metrics deleted fine while the active log file was held open gives
  // `deletedFiles > 0` with `logParts === 0 && exports === 0`, and keying row 1 on
  // `deletedFiles === 0` would leave that state with no string at all.
  it('partial failure: the exports-only and metrics-only rows', () => {
    const withExports = deleteResultToast({
      ...base,
      deletedFiles: 2,
      deletedExports: 2,
      failedFiles: 1,
    });
    expect(withExports.text).toMatch(
      /^Deleted 2 exports\. .* freed\. 1 file could not be deleted — it may be open in another program\. Usage counts cleared\.$/,
    );
    expect(withExports.text).not.toContain('log file');
    // §6.11.8 AC2 from the other side: the clear SUCCEEDED here, so this tone can
    // only come from `failedFiles`. Pinning both sides keeps R13a's `||` honest.
    expect(withExports.tone).toBe('error');
    expect(withExports.announce).toBe(withExports.text);

    // Dev ON, the one log file held open by another program; the usage file went.
    const logHeldOpen = deleteResultToast({
      ...base,
      deletedFiles: 1,
      deletedMetrics: 1,
      failedFiles: 1,
    });
    expect(logHeldOpen.text).toBe(
      '1 file could not be deleted — it may be open in another program. Usage counts cleared.',
    );
    // §6.11.1 R12 — the announcement used to drop the " — it may be open in
    // another program" clause, which is the most actionable sentence in it.
    expect(logHeldOpen.announce).toBe(logHeldOpen.text);
    expect(logHeldOpen.text).not.toContain('Deleted 0');
  });

  // §6.11.2 R13c — the failure branch used to `return` before the `rolled`
  // suffix, so Dev ON plus one locked log file never heard that recording
  // continues. The roll genuinely happened: `obs_delete.rs` sets `rolled: true`
  // before the purge runs.
  it('names the new log file on a partial failure too', () => {
    const t = deleteResultToast({
      ...base,
      failedFiles: 1,
      rolled: true,
      activeFile: 'new.jsonl',
    });
    expect(t.text).toBe(
      'Deleted 4 log files. 1.2 MiB freed. 1 file could not be deleted — it may be open in another program. Usage counts cleared. Still recording — Bonsai started a new log file.',
    );
    expect(t.announce).toBe(t.text);
  });

  /** §6.11.8 AC1 — the invariant that closes the class it took R5, R9, R10 and
   *  R12 to chase, one fixed twin at a time. Every fixture in this file. */
  it('announces exactly what the toast says, for every outcome shape', () => {
    const fixtures: LogsDeleteResult[] = [
      base,
      { ...base, deletedMetrics: 0, metricsCleared: true },
      { ...base, deletedFiles: 6, deletedMetrics: 2 },
      { ...base, deletedMetrics: 0, metricsCleared: false },
      { ...base, rolled: true, activeFile: 'new.jsonl' },
      { ...base, deletedFiles: 5, deletedExports: 1 },
      { ...base, deletedFiles: 1, deletedMetrics: 1 },
      { ...base, deletedFiles: 3, deletedExports: 2, deletedMetrics: 1 },
      { ...base, deletedFiles: 0, deletedBytes: 0, deletedMetrics: 0, metricsCleared: false },
      { ...base, deletedFiles: 0, deletedBytes: 0, deletedMetrics: 0, metricsCleared: true },
      { ...base, deletedFiles: 4, failedFiles: 1, metricsCleared: false },
      { ...base, deletedFiles: 2, deletedExports: 2, failedFiles: 1 },
      { ...base, deletedFiles: 1, deletedMetrics: 1, failedFiles: 1 },
      { ...base, failedFiles: 1, rolled: true, activeFile: 'new.jsonl' },
      { ...base, deletedFiles: 10_010, deletedExports: 129, failedFiles: 1_013, rolled: true },
    ];
    for (const f of fixtures) {
      const t = deleteResultToast(f);
      expect(t.announce, t.text).toBe(t.text);
      // §6.11.8 AC3, pinned directly rather than by inference.
      expect(t.text).not.toContain('0 B freed');
      // No template may leave a doubled or a trailing space behind.
      expect(t.text).not.toMatch(/ {2}| $/);
    }
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
