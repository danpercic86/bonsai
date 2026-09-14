/**
 * P91 §6.11.3 — the SIGNED delete-outcome strings, asserted verbatim.
 *
 * Its own file because it is a signed-copy table, not logic: the nine resolved
 * rows of §6.11.3 with the fixture each is derived from, so a reviewer can diff
 * the table against the contract without reading around it. `dev.test.tsx`
 * keeps the per-rule tests (what each branch must and must not say);
 * `obsDeleteCounts.test.ts` proves the harness can actually reach these states.
 *
 * Rows 6-9 are error-toned: §6.11.2 R13a made tone follow the words, so the
 * `metricsCleared === false` rows are errors even when nothing failed.
 *
 * §6.11.1 R12: every row asserts `announce === text`. One deletion, one string.
 */
import { describe, expect, it } from 'vitest';

import type { LogsDeleteResult } from '../../ipc';
import { deleteResultToast } from './devLogMessages';

/** `deletedFiles` is the CATEGORY TOTAL (log parts + export zips + metrics
 *  files), so each row states it the way the backend would. */
function result(over: Partial<LogsDeleteResult>): LogsDeleteResult {
  return {
    deletedFiles: 0,
    deletedBytes: 0,
    failedFiles: 0,
    activeFile: null,
    rolled: false,
    deletedExports: 0,
    deletedMetrics: 0,
    metricsCleared: true,
    ...over,
  };
}

interface Row {
  state: string;
  r: LogsDeleteResult;
  text: string;
  tone: 'success' | 'error';
}

const ROWS: Row[] = [
  {
    state: 'Dev ON, 1 log file, counts cleared',
    r: result({
      deletedFiles: 2,
      deletedBytes: 5_222,
      deletedMetrics: 1,
      rolled: true,
      activeFile: 'bonsai-next.jsonl',
    }),
    text: 'Deleted 1 log file. 5.1 KiB freed. Usage counts cleared. Still recording — Bonsai started a new log file.',
    tone: 'success',
  },
  {
    state: 'Dev ON, 4 log files + 2 exports',
    r: result({
      deletedFiles: 7,
      deletedBytes: 3_565_158,
      deletedExports: 2,
      deletedMetrics: 1,
      rolled: true,
      activeFile: 'bonsai-next.jsonl',
    }),
    text: 'Deleted 4 log files and 2 exports. 3.4 MiB freed. Usage counts cleared. Still recording — Bonsai started a new log file.',
    tone: 'success',
  },
  {
    state: 'Dev OFF, no logs, counts cleared, bytes on disk',
    r: result({ deletedFiles: 1, deletedBytes: 4_096, deletedMetrics: 1 }),
    text: 'Usage counts cleared. 4.0 KiB freed.',
    tone: 'success',
  },
  {
    // §F6 / R13b: a launch younger than the first 60 s flush — the aggregate is
    // live in memory with nothing on disk, so nothing is reclaimed.
    state: 'Dev OFF, no logs, counts cleared, nothing on disk yet',
    r: result({}),
    text: 'Usage counts cleared.',
    tone: 'success',
  },
  {
    // A saved zip outlives the logs it came from, so zero logs WITH exports is
    // reachable.
    state: 'no logs, 2 exports removed',
    r: result({
      deletedFiles: 3,
      deletedBytes: 1_258_291,
      deletedExports: 2,
      deletedMetrics: 1,
    }),
    text: 'Deleted 2 exports. 1.2 MiB freed. Usage counts cleared.',
    tone: 'success',
  },
  {
    // R13a: `metrics_purge.rs` reports `dir_removed: false` with
    // `failed_files: 0` for a present-but-unreadable `metrics/`. This row is why
    // tone may not key on `failedFiles` alone.
    state: 'unreadable metrics/, nothing else to do',
    r: result({ metricsCleared: false }),
    text: 'Usage counts were not cleared. Try again.',
    tone: 'error',
  },
  {
    state: 'Dev ON, 4 log files deleted, 1 file failed, counts not cleared',
    r: result({
      deletedFiles: 4,
      deletedBytes: 3_565_158,
      failedFiles: 1,
      metricsCleared: false,
      rolled: true,
      activeFile: 'bonsai-next.jsonl',
    }),
    text: 'Deleted 4 log files. 3.4 MiB freed. 1 file could not be deleted — it may be open in another program. Usage counts were not cleared. Try again. Still recording — Bonsai started a new log file.',
    tone: 'error',
  },
  {
    state: 'Dev OFF, nothing deleted, 1 file failed, counts not cleared',
    r: result({ failedFiles: 1, metricsCleared: false }),
    text: '1 file could not be deleted — it may be open in another program. Usage counts were not cleared. Try again.',
    tone: 'error',
  },
  {
    // The bytes are real here (the usage file went) and the freed clause is
    // STILL omitted: §6.11.3 never places one beside a bare failure sentence.
    state: 'metrics deleted, active log held open (Dev ON)',
    r: result({
      deletedFiles: 1,
      deletedBytes: 4_096,
      failedFiles: 1,
      deletedMetrics: 1,
      rolled: true,
      activeFile: 'bonsai-next.jsonl',
    }),
    text: '1 file could not be deleted — it may be open in another program. Usage counts cleared. Still recording — Bonsai started a new log file.',
    tone: 'error',
  },
];

describe('deleteResultToast — §6.11.3 signed rows', () => {
  for (const row of ROWS) {
    it(row.state, () => {
      const t = deleteResultToast(row.r);
      expect(t.text).toBe(row.text);
      expect(t.announce).toBe(row.text);
      expect(t.tone).toBe(row.tone);
    });
  }

  // Nothing here may contain a path or a file name: `activeFile` is set on four
  // of the rows above and is never interpolated into copy (§6.11.3).
  it('never names a file or a path', () => {
    for (const row of ROWS) {
      expect(deleteResultToast(row.r).text).not.toContain('.jsonl');
    }
  });
});
