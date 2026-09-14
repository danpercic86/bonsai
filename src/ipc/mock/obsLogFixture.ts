/**
 * P91 §6.11.5 — the Dev-page log/export fixture state and its harness seams.
 *
 * Split out of `handlers/obs.ts` so the delete arithmetic the §6.11.3 copy is
 * asserted against reads as one thing, and so neither file grows into a
 * god-file.
 *
 * **RULE (§6.8 R7, restated by §6.11.5): every count here DERIVES from fixture
 * state.** The hard-coded `deletedFiles: 5` this lineage replaces rendered
 * "Deleted 3 log files and 1 export" for a harness user with none, and that
 * fiction is what hid a whole toast branch for an entire increment. A mock that
 * reports deletions the fixture does not contain is worse than no mock.
 *
 * Before §6.11.5 the fixture was frozen at one log file (Dev ON) / zero
 * (Dev OFF) and `exportFiles: 0` in both states, so the partial-failure row and
 * EVERY export-bearing string — including the confirm dialog's
 * `This includes {N} exported log archive{s}.` — had never been rendered in a
 * browser. `?obsLogFiles=N` / `?obsExports=N` are what make them reachable.
 */
import type { LogsDeleteResult } from '../types';
import { query } from './repoState';

/** On-disk size of the fixture's single `usage.json` (§6.8 R7), so the delete
 *  toast's "N freed" traces to fixture state instead of a magic number. */
export const MOCK_USAGE_BYTES = 4_096;

/** A closed (rolled) log part. Every part but the active one measures this, so
 *  `?obsLogFiles=10010` reaches the `GiB` branch of `formatBytes` and the
 *  `Intl.NumberFormat` grouping §6.11.5's pathological state asks for. */
export const MOCK_ROLLED_PART_BYTES = 512 * 1_024;

/** One exported zip. Uniform, so a deleted-export byte count is exact rather
 *  than apportioned. */
export const MOCK_EXPORT_ZIP_BYTES = 1_310_720;

/** A non-negative integer harness flag, or `null` when absent/unparseable.
 *  Unparseable is treated as absent: a typo in a URL must fall back to the
 *  default fixture, never to `NaN` counts. */
function countFlag(name: string): number | null {
  const raw = query(name);
  if (raw === null || raw.trim() === '') return null;
  const n = Number.parseInt(raw, 10);
  if (!Number.isFinite(n) || n < 0) return null;
  return n;
}

/** What the fixture has on disk. `logSessionInfo` reports it and
 *  `logsDeleteAll` deletes exactly it — one source, so the two can never
 *  disagree. */
export interface LogFixtureState {
  /** Log parts across ALL sessions (`logSessionInfo.totalFiles`). */
  logFiles: number;
  /** Sum of every part's size (`logSessionInfo.totalBytes`). */
  logBytes: number;
  /** True when one of `logFiles` is the part logging is appending to. A part
   *  exists from session start, so it is independent of how many records the
   *  ring holds. */
  hasActivePart: boolean;
  /** Size of that part, 0 when there is none (or nothing is in it yet).
   *  Counted inside `logBytes`. */
  activePartBytes: number;
  exportZips: number;
  exportBytes: number;
}

/**
 * §6.11.5 — `?obsLogFiles=N` and `?obsExports=N`.
 *
 * `logFiles` is deliberately independent of the Dev-mode toggle: log files
 * outlive it, so `?obsLogFiles=3` with Dev OFF and `?obsLogFiles=0` with Dev ON
 * are both real states — the second is §6.11.8 AC5's "no log files on disk,
 * which is not the same as Dev mode off".
 *
 * `sessionBytes` is what the ring holds (the active part's size). With Dev ON
 * and no flag the state is exactly what shipped before: one part, `sessionBytes`
 * long.
 */
export function logFixtureState(devEnabled: boolean, sessionBytes: number): LogFixtureState {
  const logFiles = countFlag('obsLogFiles') ?? (devEnabled ? 1 : 0);
  // Dev ON means one of the parts is being appended to; the rest are closed.
  const hasActivePart = devEnabled && logFiles > 0;
  const activePartBytes = hasActivePart ? sessionBytes : 0;
  const closedParts = Math.max(logFiles - (hasActivePart ? 1 : 0), 0);
  const exportZips = countFlag('obsExports') ?? 0;
  return {
    logFiles,
    logBytes: activePartBytes + closedParts * MOCK_ROLLED_PART_BYTES,
    hasActivePart,
    activePartBytes,
    exportZips,
    exportBytes: exportZips * MOCK_EXPORT_ZIP_BYTES,
  };
}

/** Which half of the delete fails, and how. §6.11.5's seams, plus the `'1'`
 *  alias the shipped tests and the §6.1 mock spec already use. */
export type DeleteFailMode = 'none' | 'metrics' | 'logs' | 'exports' | 'partial' | 'all' | 'throw';

/** §6.11.5 — `?obsDeleteFail=logs|exports|all|partial|throw`, with `=1` kept as
 *  an alias for the metrics-only failure it has always meant (back-compat with
 *  `obsDeleteCounts.test.ts` and `DevCategory.test.tsx`). An unrecognised value
 *  is "no failure", never a thrown mock. */
export function deleteFailMode(): DeleteFailMode {
  switch (query('obsDeleteFail')) {
    case '1':
      return 'metrics';
    case 'logs':
      return 'logs';
    case 'exports':
      return 'exports';
    case 'partial':
      return 'partial';
    case 'all':
      return 'all';
    case 'throw':
      return 'throw';
    default:
      return 'none';
  }
}

/**
 * The message `?obsDeleteFail=throw` rejects with — mirroring the backend
 * VERBATIM in shape.
 *
 * `logs_delete_all` has exactly one class of reachable in-thread failure before
 * the purge: `writer::open_part`'s `cannot open log file: {io error}` raised by
 * the `roll(true)` that `roll_and_purge` runs FIRST (`obs_delete.rs`'s
 * all-or-nothing comment). So on every reachable rejection zero files were
 * removed and the usage counts were never touched, which is what makes
 * `deleteErrorText`'s mapped sentence and `DevCategory`'s `Nothing was deleted.`
 * literally true. This text lands in the permission branch.
 */
export const DELETE_THROW_MESSAGE = 'cannot open log file: permission denied (os error 13)';

/** How many files of a category a partial failure holds open: ~10%, at least
 *  one whenever the category is non-empty. Deterministic and derived, so
 *  `?obsLogFiles=10010&obsExports=129&obsDeleteFail=partial` exercises
 *  `Intl.NumberFormat` grouping in the deleted AND the failed count, while the
 *  default one-file fixture still reaches the singular "1 file … it may be
 *  open". */
function partialFailures(n: number): number {
  return n === 0 ? 0 : Math.max(1, Math.floor(n / 10));
}

/** The metrics half of the outcome. Kept separate from the log half because the
 *  backend runs it separately (`obs_delete.rs`: always AFTER the log purge, on
 *  its own `spawn_blocking`, and a `clear` failure never discards the log
 *  counts already earned). */
interface MetricsOutcome {
  deletedMetrics: number;
  deletedBytes: number;
  failedFiles: number;
  metricsCleared: boolean;
}

function metricsOutcome(fail: DeleteFailMode): MetricsOutcome {
  // §6.11.5 — `?obsMetricsUnreadable=1`: `metrics/` is present but `read_dir`
  // fails, so `purge_metrics_dir` counts NOTHING, fails nothing, and reports
  // `dir_removed: !dir.exists()` = false (`metrics_purge.rs`). That is the
  // `metricsCleared === false` WITH `failedFiles === 0` state — the one §6.11.2
  // R13a exists for, and the only state that made the toast read "Usage counts
  // were not cleared." in success green.
  if (query('obsMetricsUnreadable') === '1') {
    return {
      deletedMetrics: 0,
      deletedBytes: 0,
      failedFiles: 0,
      metricsCleared: false,
    };
  }
  if (fail === 'metrics' || fail === 'all') {
    // A held-open `usage.json`: one failure, the directory survives.
    return {
      deletedMetrics: 0,
      deletedBytes: 0,
      failedFiles: 1,
      metricsCleared: false,
    };
  }
  // §6.11.5 — `?obsMetricsFresh=1`: a launch younger than the first 60 s flush.
  // The aggregate is live in memory with nothing on disk, so the clear fully
  // succeeds while deleting no file and reclaiming no bytes. Combined with a
  // zero-log fixture this is the `deletedBytes === 0` success §6.11.2 R13b
  // needs — it is what made the toast say "Usage counts cleared. 0 B freed.".
  if (query('obsMetricsFresh') === '1') {
    return {
      deletedMetrics: 0,
      deletedBytes: 0,
      failedFiles: 0,
      metricsCleared: true,
    };
  }
  return {
    deletedMetrics: 1,
    deletedBytes: MOCK_USAGE_BYTES,
    failedFiles: 0,
    metricsCleared: true,
  };
}

/**
 * The whole delete outcome, derived from `state` and the harness flags.
 *
 * Byte attribution: the files another program holds open are CLOSED parts (the
 * active one Bonsai holds itself, and rolls before purging), so the failed
 * bytes come off the closed parts first and the active part is only counted as
 * failed once every closed part already is. Export zips are uniform, so their
 * byte count is exact.
 *
 * `rolled` tracks Dev mode, as the backend does: `roll_and_purge` runs only
 * when a sink exists, and it sets `rolled: true` BEFORE the purge — which is
 * why §6.11.2 R13c's "Still recording" clause is true even on a failure.
 */
export function deleteOutcome(state: LogFixtureState, devEnabled: boolean): LogsDeleteResult {
  const fail = deleteFailMode();
  const failedLogs =
    fail === 'logs' || fail === 'all'
      ? state.logFiles
      : fail === 'partial'
        ? partialFailures(state.logFiles)
        : 0;
  const failedZips =
    fail === 'exports' || fail === 'all'
      ? state.exportZips
      : fail === 'partial'
        ? partialFailures(state.exportZips)
        : 0;
  const closedParts = Math.max(state.logFiles - (state.hasActivePart ? 1 : 0), 0);
  const failedLogBytes =
    Math.min(failedLogs, closedParts) * MOCK_ROLLED_PART_BYTES +
    (failedLogs > closedParts ? state.activePartBytes : 0);
  const metrics = metricsOutcome(fail);
  const deletedLogs = state.logFiles - failedLogs;
  const deletedZips = state.exportZips - failedZips;
  return {
    // Log parts + export zips + metrics files, exactly as the backend counts.
    deletedFiles: deletedLogs + deletedZips + metrics.deletedMetrics,
    deletedBytes:
      state.logBytes - failedLogBytes + deletedZips * MOCK_EXPORT_ZIP_BYTES + metrics.deletedBytes,
    failedFiles: failedLogs + failedZips + metrics.failedFiles,
    activeFile: devEnabled ? 'bonsai-2026-08-27T14-05-52-smock0001.jsonl' : null,
    rolled: devEnabled,
    deletedExports: deletedZips,
    // One `usage.json`, counted INSIDE `deletedFiles` like an export zip.
    deletedMetrics: metrics.deletedMetrics,
    metricsCleared: metrics.metricsCleared,
  };
}
