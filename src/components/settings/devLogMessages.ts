// P91 §8.5.5 / §16.4 — pure message builders for the Developer-page log actions.
// Kept separate from the container so the toast/announce copy is unit-testable
// without a DOM. Error text is always a MAPPED sentence, never raw OS/libgit2
// text (§8.3, §8.5.5).

import type { LogsDeleteResult } from '../../ipc';
import { formatBytes } from '../../utils/format';

const NUM = new Intl.NumberFormat();

export interface DevToast {
  tone: 'success' | 'error';
  text: string;
  /** The polite-live-region announcement for the same outcome (§8.5.6). */
  announce: string;
}

/** §16.4 success / partial / — the delete outcome copy. `failedFiles > 0` is the
 *  externally-locked path; the success strings differentiate exports.
 *
 *  §F6: the usage-count clause is driven by `metricsCleared`, NEVER by
 *  `deletedMetrics`. On a launch younger than the first 60 s flush the aggregate
 *  is live in memory with nothing on disk yet, so `deletedMetrics === 0` while the
 *  clear fully succeeded — "0 files deleted" is not proof of anything here. */
export function deleteResultToast(r: LogsDeleteResult): DevToast {
  const freed = formatBytes(r.deletedBytes);
  const usageLead = r.metricsCleared
    ? 'Usage counts cleared.'
    : 'Usage counts were not cleared.';
  const usage = ` ${usageLead}`;
  if (r.failedFiles > 0) {
    const total = r.deletedFiles + r.failedFiles;
    // §6.8 R5 NIT: an error says what to do next. The remedy belongs to the
    // counts clause and to the PARTIAL-failure path only — a retry is what
    // actually clears them, and the log half already names its own cause.
    const partialUsage = r.metricsCleared ? usage : `${usage} Try again.`;
    return {
      tone: 'error',
      text: `Deleted ${NUM.format(r.deletedFiles)} of ${NUM.format(total)} log files. ${NUM.format(
        r.failedFiles,
      )} could not be deleted — they may be open in another program.${partialUsage}`,
      announce: `Deleted ${NUM.format(r.deletedFiles)} of ${NUM.format(total)} log files. ${NUM.format(
        r.failedFiles,
      )} could not be deleted.${partialUsage}`,
    };
  }
  const exports = r.deletedExports ?? 0;
  // `deletedFiles` is the TOTAL (log parts + export zips + metrics files), so both
  // of the other two come off it or the log-file count overstates itself.
  const logParts = Math.max(r.deletedFiles - exports - r.deletedMetrics, 0);
  const exportsClause = exports > 0 ? ` and ${NUM.format(exports)} export${exports === 1 ? '' : 's'}` : '';
  // §6.8 R5 — `logParts === 0` leads with the counts instead of "Deleted 0 log
  // files", which reads as a bug in the exact state §F6 made the row serve (never
  // turned Dev mode on, so nothing but usage counts to delete). Exports are NOT
  // assumed away: a saved zip outlives the logs it came from, so zero logs WITH
  // exports is reachable and keeps its clause. The lead is `usageLead`, not a
  // hard-coded "cleared", so an unreadable `metrics/` (no failed files, directory
  // still there) cannot make this sentence claim a clear that did not happen.
  let text: string;
  let announce: string;
  if (logParts === 0) {
    text =
      exports > 0
        ? `Deleted ${NUM.format(exports)} export${exports === 1 ? '' : 's'}. ${freed} freed. ${usageLead}`
        : `${usageLead} ${freed} freed.`;
    announce = usageLead;
  } else {
    text = `Deleted ${NUM.format(logParts)} log files${exportsClause}. ${freed} freed.${usage}`;
    announce = `Deleted ${NUM.format(logParts)} log files. ${freed} freed.${usage}`;
  }
  if (r.rolled) {
    text += ' Still recording — Bonsai started a new log file.';
    announce += ' Still recording in a new log file.';
  }
  return { tone: 'success', text, announce };
}

/** §8.5.5 total-failure mapping — permission / folder-missing / generic. */
export function deleteErrorText(raw: string): string {
  const m = raw.toLowerCase();
  if (m.includes('permission') || m.includes('allowed') || m.includes('denied')) {
    return "Bonsai isn't allowed to delete files in that folder.";
  }
  if (m.includes('no longer') || m.includes('not found') || m.includes('missing')) {
    return 'The logs folder is no longer there.';
  }
  // §6.8 R5: the fallback must name the action's FULL scope — the zero-log user
  // asked for the usage counts and the old string told them nothing about those.
  // The permission and folder-missing branches above stay: both are true
  // statements about a real cause.
  return "Couldn't delete the logs and usage counts.";
}

/** §8.3 export-failure mapping. */
export function exportErrorText(raw: string): string {
  const m = raw.toLowerCase();
  if (m.includes('space')) {
    return 'Not enough space to write the export. Free some space and try again.';
  }
  if (m.includes('permission') || m.includes('allowed') || m.includes('denied')) {
    return "Bonsai isn't allowed to write to its exports folder. Check the folder's permissions and try again.";
  }
  if (m.includes('no log') || m.includes('no longer') || m.includes('not found')) {
    return 'Those log files are no longer there. Turn on Dev mode and reproduce the problem again.';
  }
  return "Couldn't export the log.";
}
