/**
 * P112 §4.1 / §4.2 — the option list, one builder for both picker rows.
 *
 * Its own module because it is shared by the row and its tests and because a
 * component file that also exports a helper breaks fast refresh. Pure: it reads
 * the scan the backend sent and nothing else. In particular it does NOT
 * synthesise the `custom` entry — the remembered browsed tool arrives inside
 * `scan.terminals` / `scan.editors` as the last element, already built, with a
 * backend-derived label and a sanitized `detail` (§16.1). Nothing here derives a
 * label from a path.
 */
import { ToolPathLabel } from './ToolPathLabel';
import {
  OPT_AUTO_DETAIL,
  OPT_AUTO_LABEL,
  OPT_BUILTIN_DETAIL,
  OPT_MISSING_DETAIL,
} from './toolPickerCopy';
import type { ComboboxOption } from '../Combobox';
import type { DetectedTool } from '../../ipc';

/** The architect froze `detail` to exactly two forms: the literal `'built in'`,
 *  or a resolved path. One `===` compare, rather than a DTO change, maps it to
 *  the sentence-case option detail (§4.2). */
export const BUILT_IN = 'built in';

/**
 * Order: `Auto-detect` first (the default and the ↺ target), then this kind's
 * rows in scan order — catalog order, so the list never reshuffles between
 * mounts — then the stale entry, when the stored id is in neither.
 *
 * No separator: `Combobox` has no separator concept, and adding one for a 2–8
 * row list is chrome for nothing.
 */
export function buildToolOptions(
  rows: readonly DetectedTool[],
  labels: Readonly<Record<string, string>>,
  value: string,
  /** A scan has LANDED. Without this the stale branch below fires during the
   *  cold window, where `rows` is empty for a reason that proves nothing — and
   *  the empty label map then renders the option as the RAW ID, which is the one
   *  thing the label map exists to prevent (`common.ts:209-211`). The cold input
   *  is deliberately blank-with-a-placeholder instead (§5 state 7a). */
  hasScan: boolean,
): ComboboxOption[] {
  const options: ComboboxOption[] = [{ value: '', label: OPT_AUTO_LABEL, hint: OPT_AUTO_DETAIL }];
  for (const row of rows) {
    options.push({ value: row.id, label: row.label, hint: optionHint(row) });
  }
  // §4.2: the stale entry stays ENABLED. Disabling it greys the row the user is
  // currently ON, which reads as a broken control, and it would make the
  // selection unrecoverable after the tool is reinstalled but before a Rescan.
  // `Not installed` is a WORD, so the marker survives greyscale and needs no hue.
  //
  // `labels[value] ?? value` is the AMEND-1 label map doing its job: without it
  // a selection synced from another machine renders as the raw id. The fallback
  // is unreachable while the map stays all-OS, and a raw id still beats a blank.
  if (hasScan && value !== '' && !rows.some((r) => r.id === value)) {
    options.push({ value, label: labels[value] ?? value, hint: OPT_MISSING_DETAIL });
  }
  return options;
}

function optionHint(row: DetectedTool) {
  // A remembered browsed row whose path is gone is LISTED (AMEND-3) so the UI can
  // explain itself, and says so in a word rather than showing a dead path.
  if (!row.present) return OPT_MISSING_DETAIL;
  if (row.detail === BUILT_IN) return OPT_BUILTIN_DETAIL;
  return <ToolPathLabel value={row.detail} />;
}
