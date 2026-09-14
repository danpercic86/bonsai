/**
 * REGRESSION (design review §6.8 R7) — the harness mock may not invent files.
 *
 * `logsDeleteAll` used to return `deletedFiles: 5, deletedExports: 1` no matter
 * what `logSessionInfo` had just reported, so in the zero-log state the harness
 * showed **"Deleted 3 log files and 1 export. 1.2 MiB freed."** to a user who had
 * none. The browser harness is the only way this UI is seen before a native run,
 * so a mock that contradicts the copy it exists to verify is worse than no mock:
 * that fiction is precisely what hid the `logParts === 0` toast branch for a
 * whole increment.
 *
 * These assertions are on the ARITHMETIC RELATION between the two handlers, not
 * on frozen numbers, so the guard survives a fixture edit.
 *
 * The mock handler module reads `window.location` at import time (harness query
 * flags), so this `.test.ts` opts into jsdom rather than the node project.
 *
 * @vitest-environment jsdom
 */
import { beforeEach, describe, expect, it } from 'vitest';

import { deleteErrorText, deleteResultToast } from '../../components/settings/devLogMessages';
import { obsHandlers } from './handlers/obs';
import { ringClear } from './obsRing';

const UI_SETTINGS_KEY = 'bonsai.mockUiSettings';

function setDevEnabled(enabled: boolean): void {
  window.localStorage.setItem(
    UI_SETTINGS_KEY,
    JSON.stringify({ dev: { enabled, includeRawNames: false } }),
  );
}

beforeEach(() => {
  window.localStorage.clear();
  window.history.replaceState(null, '', '/');
  ringClear();
});

describe('mock logsDeleteAll derives its counts from the fixture (§6.8 R7)', () => {
  it('deletes only the usage file when the fixture has no logs', async () => {
    setDevEnabled(false);
    const info = await obsHandlers.logSessionInfo();
    expect(info.totalFiles, 'precondition: the zero-log state').toBe(0);

    const r = await obsHandlers.logsDeleteAll();
    expect(r.deletedFiles).toBe(1); // the metrics file, nothing else
    expect(r.deletedMetrics).toBe(1);
    expect(r.deletedExports).toBe(0);
    expect(r.metricsCleared).toBe(true);
    expect(r.failedFiles).toBe(0);

    // The point of the fixture: the copy the harness renders is now true.
    const t = deleteResultToast(r);
    expect(t.text).not.toMatch(/log files/);
    expect(t.text).not.toMatch(/export/);
    expect(t.text).toMatch(/^Usage counts cleared\./);
  });

  it('counts the one log file the fixture reports while Dev mode is on', async () => {
    setDevEnabled(true);
    const info = await obsHandlers.logSessionInfo();
    expect(info.totalFiles, 'precondition: exactly one log file').toBe(1);
    // Optional on the wire (`exportFiles?: number`); the fixture always sets it,
    // and the precondition below fails loudly if that ever stops being true.
    const exportFiles = info.exportFiles ?? 0;
    expect(exportFiles, 'precondition: the fixture has no exports').toBe(0);

    const r = await obsHandlers.logsDeleteAll();
    // log parts + export zips + metrics files, exactly as the backend counts it.
    expect(r.deletedFiles).toBe(info.totalFiles + exportFiles + r.deletedMetrics);
    expect(r.deletedExports).toBe(exportFiles);
    expect(r.deletedBytes).toBeGreaterThanOrEqual(info.totalBytes);
    expect(r.rolled, 'Dev ON rolls into a fresh file').toBe(true);

    const t = deleteResultToast(r);
    // §6.10 R9: this is the DEFAULT Dev-ON success path, and it is where
    // `Deleted 1 log files.` shipped. The trailing `\. ` is the whole point of the
    // assertion — the prefix-only form this replaces matched the bug.
    expect(t.text).toMatch(/^Deleted 1 log file\. /);
    expect(t.text).not.toMatch(/export/);
  });

  it('?obsDeleteFail=1 fails the metrics file and leaves the log half honest', async () => {
    setDevEnabled(true);
    window.history.replaceState(null, '', '/?obsDeleteFail=1');
    const r = await obsHandlers.logsDeleteAll();

    // The F6 contract's failure shape, with the log counts still the fixture's.
    expect(r.failedFiles).toBe(1);
    expect(r.deletedMetrics).toBe(0);
    expect(r.metricsCleared).toBe(false);
    expect(r.deletedFiles).toBe(1); // the one log file went; the usage file did not

    const t = deleteResultToast(r);
    expect(t.tone).toBe('error');
    // §6.10 R10: `logParts` = 1 - 0 exports - 0 metrics = 1. `1 of 2 log files`
    // counted the failed `metrics/usage.json` as a second LOG file.
    expect(t.text).toContain('Deleted 1 log file.');
    expect(t.text).toContain('1 file could not be deleted');
    expect(t.text).toContain('Usage counts were not cleared. Try again.');
  });

  /** §6.10 R10 — this is the assertion that pins the defect. With Dev mode off
   *  the fixture has NO log files, so the only thing that can fail is the usage
   *  file; the old toast still said "Deleted 0 of 1 log files." and invented one.
   *  (The `not.toContain('of 4')` guard this replaces went vacuous with `X of Y`.) */
  it('?obsDeleteFail=1 with Dev OFF never claims a log file existed', async () => {
    setDevEnabled(false);
    window.history.replaceState(null, '', '/?obsDeleteFail=1');
    const r = await obsHandlers.logsDeleteAll();
    expect(r.deletedFiles, 'precondition: nothing deleted — no logs, and the usage file failed').toBe(
      0,
    );
    expect(r.failedFiles).toBe(1);

    const t = deleteResultToast(r);
    expect(t.text).not.toContain('log file');
    expect(t.text).toBe(
      '1 file could not be deleted — it may be open in another program. Usage counts were not cleared. Try again.',
    );
    // §6.11.1 R12 — the announcement used to drop the cause clause, which is the
    // most actionable sentence in the message.
    expect(t.announce).toBe(t.text);
  });

  /** §6.11.5 — `?obsMetricsUnreadable=1`. The ONLY way to reach §6.11.2 R13a:
   *  `metrics/` is present but unreadable, so `purge_metrics_dir` fails
   *  `read_dir`, counts nothing, fails nothing and reports `dir_removed: false`.
   *  Before this flag the state was unreachable in a browser, and it is the one
   *  that rendered "Usage counts were not cleared." in success GREEN. */
  it('?obsMetricsUnreadable=1 reaches a failed clear with nothing failed', async () => {
    setDevEnabled(false);
    window.history.replaceState(null, '', '/?obsMetricsUnreadable=1');
    const r = await obsHandlers.logsDeleteAll();
    expect(r.failedFiles, 'the read failed: nothing was even enumerated').toBe(0);
    expect(r.deletedMetrics).toBe(0);
    expect(r.metricsCleared).toBe(false);
    expect(r.deletedFiles).toBe(0);

    // §6.11.3's "unreadable `metrics/`, nothing else to do" row, verbatim.
    const t = deleteResultToast(r);
    expect(t.tone, 'AC2: a failed clear is never a success toast').toBe('error');
    expect(t.text).toBe('Usage counts were not cleared. Try again.');
    expect(t.announce).toBe(t.text);
    expect(t.text).not.toContain('0 B');
  });

  it('?obsMetricsUnreadable=1 leaves the log half succeeding', async () => {
    setDevEnabled(true);
    window.history.replaceState(null, '', '/?obsMetricsUnreadable=1');
    const info = await obsHandlers.logSessionInfo();
    const r = await obsHandlers.logsDeleteAll();
    expect(r.deletedFiles).toBe(info.totalFiles);
    expect(r.failedFiles).toBe(0);

    const t = deleteResultToast(r);
    expect(t.tone).toBe('error');
    expect(t.text).toMatch(/^Deleted 1 log file\./);
    expect(t.text).toContain('Usage counts were not cleared. Try again.');
    // §6.11.2 R13c — the roll happened before the purge, on this branch too.
    expect(t.text).toContain('Still recording — Bonsai started a new log file.');
    expect(t.announce).toBe(t.text);
  });

  /** §6.11.5 — `?obsMetricsFresh=1`: a launch younger than the first 60 s flush.
   *  The aggregate is live in memory with nothing on disk, so the clear succeeds
   *  having deleted no file and reclaimed no bytes. This is the state that made
   *  the toast say "Usage counts cleared. 0 B freed." (§6.11.2 R13b). */
  it('?obsMetricsFresh=1 is a zero-byte success with no freed clause', async () => {
    setDevEnabled(false);
    window.history.replaceState(null, '', '/?obsMetricsFresh=1');
    const r = await obsHandlers.logsDeleteAll();
    expect(r.metricsCleared).toBe(true);
    expect(r.deletedMetrics).toBe(0);
    expect(r.deletedBytes).toBe(0);

    const t = deleteResultToast(r);
    expect(t.tone).toBe('success');
    expect(t.text).toBe('Usage counts cleared.');
    expect(t.text).not.toContain('0 B');
    expect(t.announce).toBe(t.text);
  });

  /** §6.11.8 AC5 — no log files ON DISK, which is NOT the same as Dev mode off:
   *  logs outlive the toggle, so `?obsLogFiles=0` with Dev mode ON is the honest
   *  shape of that state and the flag is the only way to reach it. */
  it('?obsLogFiles=0 with Dev ON names no log file, failure or not', async () => {
    setDevEnabled(true);
    window.history.replaceState(null, '', '/?obsLogFiles=0');
    const info = await obsHandlers.logSessionInfo();
    expect(info.totalFiles, 'Dev ON, nothing flushed to disk yet').toBe(0);
    expect(info.files).toEqual([]);

    // AC5 asks that no string NAME a log file. Read as "no COUNTED log file":
    // the one occurrence below is the `rolled` reassurance clause, which is true
    // in this state (the delete rolled into a fresh part) and is not a count of
    // anything. What must be unreachable is the fabricated `Deleted 0 of 1 log
    // files.` — i.e. `{N} log file` in a deleted or a failed clause.
    const clean = deleteResultToast(await obsHandlers.logsDeleteAll());
    expect(clean.text).not.toMatch(/\d+ log file/);
    expect(clean.text).toBe(
      'Usage counts cleared. 4.0 KiB freed. Still recording — Bonsai started a new log file.',
    );

    // ...and with every category forced to fail, the failed count still cannot
    // become a log file it does not have.
    window.history.replaceState(null, '', '/?obsLogFiles=0&obsDeleteFail=all');
    const failed = await obsHandlers.logsDeleteAll();
    expect(failed.failedFiles, 'only the usage file could fail').toBe(1);
    const t = deleteResultToast(failed);
    expect(t.text).not.toMatch(/\d+ log file/);
    expect(t.text).toBe(
      '1 file could not be deleted — it may be open in another program. Usage counts were not cleared. Try again. Still recording — Bonsai started a new log file.',
    );
    expect(t.tone).toBe('error');
  });

  /** §6.11.5 — `?obsLogFiles=N` / `?obsExports=N`. Without them the partial row
   *  and EVERY export-bearing string were invisible in the harness: the fixture
   *  reported one log file and zero exports, so `Deleted 2 log files and 1
   *  export`, the plural failure sentence and the confirm dialog's `This includes
   *  2 exported log archives.` had never been rendered. */
  it('?obsLogFiles / ?obsExports drive both handlers, and =partial fails a share of each', async () => {
    setDevEnabled(false);
    window.history.replaceState(null, '', '/?obsLogFiles=3&obsExports=2&obsDeleteFail=partial');
    const info = await obsHandlers.logSessionInfo();
    expect(info.totalFiles, 'log files outlive the Dev-mode toggle').toBe(3);
    expect(info.exportFiles, 'what the confirm dialog counts archives from').toBe(2);

    const r = await obsHandlers.logsDeleteAll();
    // Derived, not invented: one part and one zip are held open (10%, min 1).
    expect(r.failedFiles).toBe(2);
    expect(r.deletedExports).toBe(1);
    expect(r.deletedFiles).toBe(2 + 1 + r.deletedMetrics);
    // `exportBytes` is optional on the wire; the fixture always sets it.
    expect(r.deletedBytes).toBeLessThan(info.totalBytes + (info.exportBytes ?? 0));

    const t = deleteResultToast(r);
    expect(t.text).toMatch(/^Deleted 2 log files and 1 export\. /);
    expect(t.text).toContain(
      '2 files could not be deleted — they may be open in another program.',
    );
    expect(t.text).toContain('Usage counts cleared.');
    expect(t.announce).toBe(t.text);
  });

  /** §6.11.5 — `?obsDeleteFail=throw`. The THROWN path (`deleteErrorText` plus
   *  `Nothing was deleted.`) was unexercisable in the harness. §6.11's constraint
   *  boundary: on every reachable rejection ZERO files were removed and the
   *  counts were never touched, which is what makes that announcement true — so
   *  the mock must reject before it touches either. */
  it('?obsDeleteFail=throw rejects with the backend text and deletes nothing', async () => {
    setDevEnabled(true);
    window.history.replaceState(null, '', '/?obsDeleteFail=throw');
    const before = await obsHandlers.metricsSnapshot();
    await expect(obsHandlers.logsDeleteAll()).rejects.toThrow(
      'cannot open log file: permission denied (os error 13)',
    );
    expect(await obsHandlers.metricsSnapshot(), 'the counts were never touched').toEqual(before);
    // The mapped sentence the page actually shows for it.
    expect(deleteErrorText('cannot open log file: permission denied (os error 13)')).toBe(
      "Bonsai isn't allowed to delete files in that folder.",
    );
  });
});

/** §6.10 harness seam — the unknown-count state (`info === null`) has to be
 *  reachable in the browser, or R8a/R8b/R11's copy can only ever be seen by the
 *  user. `?obsInfoFail=1` is the flag that reaches it. */
describe('mock logSessionInfo can fail on demand (§6.10)', () => {
  it("rejects with the backend's message under ?obsInfoFail=1, and not otherwise", async () => {
    setDevEnabled(true);
    await expect(obsHandlers.logSessionInfo()).resolves.toBeTruthy();

    window.history.replaceState(null, '', '/?obsInfoFail=1');
    await expect(obsHandlers.logSessionInfo()).rejects.toThrow(
      'cannot resolve app config dir: unknown path',
    );
  });
});
