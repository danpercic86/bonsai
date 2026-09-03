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
 *  externally-locked path; the success strings differentiate exports. */
export function deleteResultToast(r: LogsDeleteResult): DevToast {
  const freed = formatBytes(r.deletedBytes);
  if (r.failedFiles > 0) {
    const total = r.deletedFiles + r.failedFiles;
    return {
      tone: 'error',
      text: `Deleted ${NUM.format(r.deletedFiles)} of ${NUM.format(total)} log files. ${NUM.format(
        r.failedFiles,
      )} could not be deleted — they may be open in another program.`,
      announce: `Deleted ${NUM.format(r.deletedFiles)} of ${NUM.format(total)} log files. ${NUM.format(
        r.failedFiles,
      )} could not be deleted.`,
    };
  }
  const exports = r.deletedExports ?? 0;
  const logParts = r.deletedFiles - exports;
  const exportsClause = exports > 0 ? ` and ${NUM.format(exports)} export${exports === 1 ? '' : 's'}` : '';
  let text = `Deleted ${NUM.format(logParts)} log files${exportsClause}. ${freed} freed.`;
  let announce = `Deleted ${NUM.format(logParts)} log files. ${freed} freed.`;
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
  return "Couldn't delete the log files.";
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
