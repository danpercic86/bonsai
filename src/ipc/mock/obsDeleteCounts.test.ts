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

import { deleteResultToast } from '../../components/settings/devLogMessages';
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
    // Prefix only: the toast's own pluralisation is not a ruled string, and this
    // guard is about the COUNT coming from the fixture, not about the wording.
    expect(t.text).toMatch(/^Deleted 1 log file/);
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
    expect(t.text).toContain('Deleted 1 of 2 log files.');
    expect(t.text).not.toContain('of 4');
    expect(t.text).toContain('Usage counts were not cleared. Try again.');
  });
});
