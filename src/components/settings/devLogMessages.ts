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

/** §16.4 / §6.11.3 — the delete outcome copy: three templates, exhaustive.
 *
 *  T1 (something was deleted)   `{deleted}{freed?}{failure?}{usage}{rolled?}`
 *  T2 (nothing deleted or failed) `{usage}{freed?}{rolled?}`
 *  T3 (nothing deleted, something failed) `{failure}{usage}{rolled?}`
 *
 *  §6.11.1 R12: **`announce` is byte-identical to `text`.** The success
 *  announcement used to drop the exports clause while the failure announcement
 *  kept it, and the zero-log rows dropped the freed clause too — one deletion,
 *  three shapes. An announcement that omits a count of files actually removed
 *  states a NARROWER blast radius than the operation had, to the one user who
 *  cannot read the toast. The house criterion for trimming a live-region string
 *  is "not actionable by voice" (`DevCategory.tsx` drops the export FOLDER on
 *  exactly that ground, and it is the only sanctioned trim here); nothing in a
 *  delete outcome meets it — the counts are the scope, `— it may be open in
 *  another program` is the most actionable sentence in the message, and
 *  `Try again.` is the remedy. `announce: text` also cannot drift, where §6.8
 *  R5, §6.10 R9 and R10 each fixed one twin and left the other.
 *  `DevToast.announce` stays on the interface: the EXPORT path's divergence is
 *  legitimate under the criterion above.
 *
 *  §6.11.2 R13a: `tone` is `error` whenever something the row promised did not
 *  happen — `failedFiles > 0` **or** `metricsCleared === false`. Keying it on
 *  `failedFiles` alone shipped a GREEN toast reading "Usage counts were not
 *  cleared.": `metrics_purge.rs` reports `dir_removed: false` with
 *  `failed_files: 0` for a present-but-unreadable `metrics/`. Consequently
 *  ` Try again.` attaches wherever the clear failed, not only on the
 *  partial-failure path (§6.8 R5's NIT, amended).
 *
 *  §6.10 R10: BOTH branches discriminate on the same `logParts`/`exports` pair,
 *  NEVER on `deletedFiles`. `deletedFiles` is a CATEGORY TOTAL (log parts + export
 *  zips + metrics files), and the failure branch used to feed it raw into the noun
 *  "log files": a failed `metrics/usage.json` was reported as a log file that would
 *  not delete, and with Dev mode off — zero logs on disk — the toast invented one
 *  outright ("Deleted 0 of 1 log files."). The `X of Y` framing cannot be salvaged
 *  because `LogsDeleteResult` carries no per-category failure attribution, so `Y`
 *  is unknowable per category; the failed count therefore gets the only noun that
 *  is certainly true — `file`.
 *
 *  §F6: the usage-count clause is driven by `metricsCleared`, NEVER by
 *  `deletedMetrics`. On a launch younger than the first 60 s flush the aggregate
 *  is live in memory with nothing on disk yet, so `deletedMetrics === 0` while the
 *  clear fully succeeded — "0 files deleted" is not proof of anything here. */
export function deleteResultToast(r: LogsDeleteResult): DevToast {
  // `deletedFiles` is the TOTAL (log parts + export zips + metrics files), so both
  // of the other two come off it or the log-file count overstates itself.
  const exports = r.deletedExports ?? 0;
  const logParts = Math.max(r.deletedFiles - exports - r.deletedMetrics, 0);
  const failed = r.failedFiles > 0;
  // §6.11.2 R13a — an unkept promise is an error toast even with nothing failed.
  const tone: DevToast['tone'] = failed || !r.metricsCleared ? 'error' : 'success';
  // Every optional clause carries its own single leading space, so the templates
  // below are pure concatenation: exactly one space between sentences, none
  // trailing.
  const usage = r.metricsCleared
    ? ' Usage counts cleared.'
    : ' Usage counts were not cleared. Try again.';
  // §6.11.2 R13b — `formatBytes(0)` is `0 B`, and a clear before the first 60 s
  // flush reclaims nothing while succeeding. "0 B freed." advertises a benefit
  // that did not occur, so the clause is omitted at zero bytes — and it never
  // accompanies a bare failure sentence (T3), where a byte count is noise.
  const freed = r.deletedBytes > 0 ? ` ${formatBytes(r.deletedBytes)} freed.` : '';
  // §6.11.2 R13c — on BOTH branches. The roll happens before the purge
  // (`obs_delete.rs` sets `rolled: true` first), so it is true on a failure too,
  // and that user is the one most likely to conclude logging has stopped.
  const rolled = r.rolled ? ' Still recording — Bonsai started a new log file.' : '';
  // Singular agreement: the shipped string said "1 could not be deleted — THEY
  // may be open".
  const failure = failed
    ? ` ${NUM.format(r.failedFiles)} file${r.failedFiles === 1 ? '' : 's'} could not be deleted — ${
        r.failedFiles === 1 ? 'it' : 'they'
      } may be open in another program.`
    : '';
  let text: string;
  if (logParts > 0 || exports > 0) {
    // T1. §6.10 R9: `Deleted 1 log files` was the DEFAULT Dev-ON success toast.
    // Each count pluralises off itself (R4/R9).
    const exportsClause =
      exports > 0 ? ` and ${NUM.format(exports)} export${exports === 1 ? '' : 's'}` : '';
    const deleted =
      logParts > 0
        ? `Deleted ${NUM.format(logParts)} log file${logParts === 1 ? '' : 's'}${exportsClause}.`
        : `Deleted ${NUM.format(exports)} export${exports === 1 ? '' : 's'}.`;
    text = `${deleted}${freed}${failure}${usage}${rolled}`;
  } else if (failed) {
    // T3. Keyed on the `logParts`/`exports` PAIR, not on `deletedFiles === 0`:
    // metrics deleted fine while the active log file was held open gives
    // `deletedFiles > 0` with `logParts === 0 && exports === 0`, and keying it the
    // other way would leave that state with no string at all.
    text = `${failure.trimStart()}${usage}${rolled}`;
  } else {
    // T2. §6.8 R5 — leading with the counts beats "Deleted 0 log files", which
    // reads as a bug in the exact state §F6 made this row serve (never turned Dev
    // mode on, so nothing but usage counts to delete). The lead is the usage
    // clause itself, not a hard-coded "cleared", so an unreadable `metrics/` (no
    // failed files, directory still there) cannot claim a clear that did not
    // happen. The freed clause TRAILS it here, and is dropped with the
    // not-cleared form (§6.11.2 R13b).
    text = `${usage.trimStart()}${r.metricsCleared ? freed : ''}${rolled}`;
  }
  return { tone, text, announce: text };
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
