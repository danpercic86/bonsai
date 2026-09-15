/**
 * P112 §3 / §4 / §5 — one detected-tool picker row: the strict `Combobox`, the
 * `Browse…` button, the row's stateful note and its outcome slot.
 *
 * Both rows are this component with different props (§3): their copy differs
 * only by two interpolated nouns, so one file, not two.
 *
 * **Strict mode is the security property, not a preference.** `allowFreeInput`
 * is omitted ⇒ `false`, so `onChange` can only ever fire with an option's value.
 * The backend coerces an unknown id to `''` (`coerce_tool_id`), which means a
 * free-text control would SILENTLY DISCARD what the user typed — that is why the
 * old text rows were deleted rather than rewired, and why this control must
 * never accept a typed value.
 *
 * A browsed path is DISPLAYED and never trusted: `detail` is display-only,
 * sanitized backend-side, and the label is backend-derived. Nothing here
 * re-derives a label from a path (§16.1), and `ToolPathLabel` does not transform
 * the path it renders (§16.9).
 *
 * Takes no launcher callback and no `pushToast` (§16.5 / UA17): it configures a
 * launcher, it never invokes one.
 */
import { Combobox } from '../Combobox';
import { SettingsOutcomeNote, type SettingsOutcome } from './SettingsOutcomeNote';
import { SettingsRow } from './SettingsRow';
import { BUILT_IN, buildToolOptions } from './toolPickerOptions';
import {
  BTN_BROWSE,
  PLACEHOLDER,
  browseLabel,
  noteAuto,
  noteBuiltIn,
  noteNone,
  noteScanning,
  noteStale,
  noteStaleCustomTail,
} from './toolPickerCopy';
import type { ReactNode } from 'react';
import type { DetectedTool, ExternalToolKind } from '../../ipc';
import type { SettingsRowId } from './types';

export interface ToolPickerRowProps {
  kind: ExternalToolKind;
  rowId: SettingsRowId;
  /** DOM id of the `Combobox` input — `SettingsRow` emits `<label for>` for it,
   *  which is the control's ONLY naming source (never also `ariaLabel`: two
   *  naming sources on one control is how the `Git config , repository` defect
   *  happened). */
  controlId: string;
  /** Hand-written ids for the stateful note and the outcome slot, composed into
   *  `aria-describedby` — state note FIRST (P113 §9). */
  noteId: string;
  outcomeId: string;
  /** The persisted selection: `''`, a catalog id, or `'custom'`. */
  value: string;
  /** This kind's rows from the last good scan, in scan order, with the
   *  remembered browsed row last (it arrives built; nothing here synthesises it). */
  rows: readonly DetectedTool[];
  /** `id -> label` for every catalog entry of this kind on ANY os (AMEND-1) —
   *  without it a kept-but-undetected selection would render as the raw id. */
  labels: Readonly<Record<string, string>>;
  /** A scan has landed at least once. */
  hasScan: boolean;
  /** A scan is in flight (first mount or a Rescan). */
  scanning: boolean;
  /** No scan has ever landed AND the last one failed. */
  scanFailedCold: boolean;
  /** This row's native dialog is open. */
  browsing: boolean;
  outcome: SettingsOutcome | null;
  onChange(next: string): void;
  onBrowse(): void;
}

/** The inline path, `.mono` + `title` — the `SettingsDevLogsSection.tsx:56-58`
 *  precedent. Never sliced in JS; the title is what keeps an elided path
 *  recoverable. */
function mono(path: string): ReactNode {
  return (
    <span className="mono" title={path}>
      {path}
    </span>
  );
}

/**
 * §5's state table, as the row's ONE state note.
 *
 * The empty return is §16.4 R5's shape one row over: with no scan and a failed
 * one behind us there is no label map, so `NOTE_STALE` cannot name the stored
 * tool and `NOTE_SCANNING` would claim we are still looking. The Rescan row's
 * `SCAN_ERR` is the sentence that explains it, one row below, and it names the
 * action. (§16 specifies this for the Rescan row only; extending it to the
 * picker rows is the only option that does not either lie or invent copy.)
 */
function stateNote(props: ToolPickerRowProps): ReactNode {
  const { kind, value, rows, labels, hasScan, scanning, scanFailedCold } = props;
  if (scanning || (!hasScan && !scanFailedCold)) return noteScanning(kind);
  // Cold failure. At `''` the input reads `Auto-detect` — a synchronous option —
  // and `NOTE_AUTO` is true whether or not a scan ever landed, so the row keeps
  // its sentence. At any other value there is no label map to name the stored
  // tool with, and `NOTE_NONE` would be a lie (nothing was searched), so the row
  // has no state note: `SCAN_ERR` one row below is the only honest sentence.
  if (!hasScan) return value === '' ? noteAuto(kind) : '';
  if (value === '') return rows.length === 0 ? noteNone(kind) : noteAuto(kind);
  const selected = rows.find((r) => r.id === value);
  if (selected === undefined) return noteStale(kind, labels[value] ?? value);
  if (!selected.present) {
    return (
      <>
        {'Nothing is at '}
        {mono(selected.detail)}
        {noteStaleCustomTail(kind)}
      </>
    );
  }
  if (selected.detail === BUILT_IN) return noteBuiltIn(selected.label);
  return (
    <>
      {'Runs '}
      {mono(selected.detail)}
      {'.'}
    </>
  );
}

export function ToolPickerRow(props: ToolPickerRowProps) {
  const {
    kind,
    rowId,
    controlId,
    noteId,
    outcomeId,
    value,
    rows,
    labels,
    hasScan,
    scanFailedCold,
    browsing,
    outcome,
    onChange,
    onBrowse,
  } = props;

  const options = buildToolOptions(rows, labels, value, hasScan);
  const selectedLabel = options.find((o) => o.value === value)?.label;
  const describedBy = `${noteId} ${outcomeId}`;

  return (
    <SettingsRow
      id={rowId}
      controlId={controlId}
      stacked
      hint={
        <>
          <p className="settings-row-note" id={noteId}>
            {stateNote(props)}
          </p>
          {/* The SECOND tenant of the help slot: the state note above stays
              visible and is never replaced — it is still true, a DIFFERENT file
              was rejected. Description-only: no `aria-live`, no `role`; the
              section's one announcer speaks (§16.4). */}
          <SettingsOutcomeNote slot={rowId} id={outcomeId} outcome={outcome} />
        </>
      }
    >
      <div className="settings-value-copy">
        <div className="tool-picker">
          <Combobox
            id={controlId}
            options={options}
            value={value}
            describedBy={describedBy}
            title={selectedLabel}
            /* §5 state 7a / §16.6: only while NO scan has landed AND one may
               still land. On a RESCAN the previous options are in hand, so the
               input keeps its label — blanking a control that already knows its
               own value for 2.1 s would be a regression introduced by a refresh
               button. After a COLD FAILURE it is dropped too: nothing is being
               looked for any more, and `SCAN_ERR` one row below is the sentence
               that explains the blank and names the action. */
            placeholder={hasScan || scanFailedCold ? undefined : PLACEHOLDER}
            onChange={(next) => {
              // Re-picking the current value would fire a pointless
              // `set_ui_settings` round trip on every row.
              if (next !== value) onChange(next);
            }}
          />
        </div>
        <button
          type="button"
          className="btn-secondary tool-picker-browse"
          aria-label={browseLabel(kind)}
          /* `aria-disabled`, never `disabled`: a disabled button loses focus to
             `<body>`, and keeping it focusable is what makes focus still be on
             `Browse…` when the native dialog closes — so cancel needs no
             focus-restore logic at all. The handler is then what refuses. */
          aria-disabled={browsing}
          aria-busy={browsing}
          aria-describedby={describedBy}
          onClick={() => {
            if (!browsing) onBrowse();
          }}
        >
          {/* The visible label does NOT change while busy. Swapping a button's
              text mid-press re-announces it and changes its width under the
              cursor, and the native dialog is itself the feedback. */}
          {BTN_BROWSE}
        </button>
      </div>
    </SettingsRow>
  );
}
