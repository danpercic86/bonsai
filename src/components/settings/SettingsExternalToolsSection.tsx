/**
 * P112 §2 / §7 / §16.4 — General → "External tools": two picker rows, one
 * Rescan row, the group lead and note, and the section's ONE live region.
 *
 * A NEW file, not a rewrite: sub-increment 1 deleted the old group (its two rows
 * were free-text PROGRAM fields, which a catalog-id setting cannot be), and
 * every settings component written since P113 lives in `settings/`.
 *
 * Presentational. Props only — no hooks, no IPC (the §2.3 leaf boundary), and no
 * launcher callback or `pushToast` anywhere in the surface below (§16.5 / UA17).
 *
 * **General's live-region count is 1**, in every state: the `sr-only
 * role="status"` at the bottom of this group. Every `[data-outcome-note]` above
 * it is description-only. When settings-search filters the group out
 * `SettingsGroup` returns `null` and the announcer unmounts with it — correct,
 * since the only controls that can write it are inside the group, and that is
 * P113 §7's "also clears on unmount" one level up.
 */
import { SettingsGroup } from './SettingsGroup';
import { SettingsOutcomeNote, type SettingsOutcome } from './SettingsOutcomeNote';
import { SettingsRow } from './SettingsRow';
import { ToolPickerRow } from './ToolPickerRow';
import {
  BTN_RESCAN,
  EDITOR_NOTE_ID,
  EDITOR_OUTCOME_ID,
  EDITOR_SLOT,
  GROUP_NOTE,
  LEAD,
  RESCAN_DESCRIBED_BY,
  RESCAN_NOTE_ID,
  RESCAN_OUTCOME_ID,
  RESCAN_SLOT,
  SCAN_SCANNING,
  TERMINAL_NOTE_ID,
  TERMINAL_OUTCOME_ID,
  TERMINAL_SLOT,
  scanCounts,
} from './toolPickerCopy';
import type { ExternalToolKind, ExternalToolScan } from '../../ipc';

export interface SettingsExternalToolsSectionProps {
  /** The last good scan, or `null` while none has landed. */
  scan: ExternalToolScan | null;
  scanning: boolean;
  scanFailedCold: boolean;
  browsing: ExternalToolKind | null;
  terminalTool: string;
  editorTool: string;
  /** Slot key → its newest outcome, from the category's one `useOutcomeNotes`. */
  notes: ReadonlyMap<string, SettingsOutcome>;
  /** The section's one announcement. */
  announce: string;
  onChangeTool(kind: ExternalToolKind, next: string): void;
  onBrowse(kind: ExternalToolKind): void;
  onRescan(): void;
}

export function SettingsExternalToolsSection({
  scan,
  scanning,
  scanFailedCold,
  browsing,
  terminalTool,
  editorTool,
  notes,
  announce,
  onChangeTool,
  onBrowse,
  onRescan,
}: SettingsExternalToolsSectionProps) {
  const hasScan = scan !== null;

  // §16.4 R4: the Rescan row's counts are STATE, not an outcome — they are true
  // whether or not anyone pressed the button. In flight they become the `…`
  // line, on P113 §10.1's precedent (`aria-busy` on the button, `Deleting…` in
  // the row's state note). Its outcome slot carries `SCAN_ERR` and nothing else.
  //
  // The empty string is §16.4 R5's deliberate one state with NO state note: on a
  // cold failure `SCAN_NONE` ("No terminals or editors found on this computer.")
  // would be a lie, so `SCAN_ERR` in the outcome slot is the row's only sentence.
  const rescanNote = scanning
    ? SCAN_SCANNING
    : scan === null
      ? scanFailedCold
        ? ''
        : SCAN_SCANNING
      : scanCounts(scan.terminals.length, scan.editors.length);

  return (
    <SettingsGroup id="general-external-tools" title="External tools">
      <p className="settings-group-lead">{LEAD}</p>

      <ToolPickerRow
        kind="terminal"
        rowId={TERMINAL_SLOT}
        controlId="settings-terminal-tool"
        noteId={TERMINAL_NOTE_ID}
        outcomeId={TERMINAL_OUTCOME_ID}
        value={terminalTool}
        rows={scan?.terminals ?? []}
        labels={scan?.terminalLabels ?? {}}
        hasScan={hasScan}
        scanning={scanning}
        scanFailedCold={scanFailedCold}
        browsing={browsing === 'terminal'}
        outcome={notes.get(TERMINAL_SLOT) ?? null}
        onChange={(next) => onChangeTool('terminal', next)}
        onBrowse={() => onBrowse('terminal')}
      />

      <ToolPickerRow
        kind="editor"
        rowId={EDITOR_SLOT}
        controlId="settings-editor-tool"
        noteId={EDITOR_NOTE_ID}
        outcomeId={EDITOR_OUTCOME_ID}
        value={editorTool}
        rows={scan?.editors ?? []}
        labels={scan?.editorLabels ?? {}}
        hasScan={hasScan}
        scanning={scanning}
        scanFailedCold={scanFailedCold}
        browsing={browsing === 'editor'}
        outcome={notes.get(EDITOR_SLOT) ?? null}
        onChange={(next) => onChangeTool('editor', next)}
        onBrowse={() => onBrowse('editor')}
      />

      {/* §7: ONE Rescan for the section — `list_external_tools` returns both
          lists in a single round trip, so two buttons would be two names for one
          action. `rowLabel`, never `controlId`: `SettingsRow` emits
          `<label for>`, and `<button>` is a labelable element, so a `controlId`
          would rename the button `Detected tools` and contradict the catalog. */}
      <SettingsRow
        id={RESCAN_SLOT}
        rowLabel="Detected tools"
        hint={
          <>
            <p className="settings-row-note" id={RESCAN_NOTE_ID}>
              {rescanNote}
            </p>
            <SettingsOutcomeNote
              slot={RESCAN_SLOT}
              id={RESCAN_OUTCOME_ID}
              outcome={notes.get(RESCAN_SLOT) ?? null}
            />
          </>
        }
      >
        <button
          type="button"
          className="btn-secondary settings-toggle-btn"
          aria-disabled={scanning}
          aria-busy={scanning}
          aria-describedby={RESCAN_DESCRIBED_BY}
          onClick={() => {
            if (!scanning) onRescan();
          }}
        >
          {/* Label unchanged while scanning — never `Rescanning…`, same rule as
              `Browse…`. */}
          {BTN_RESCAN}
        </button>
      </SettingsRow>

      <p className="settings-group-note">{GROUP_NOTE}</p>

      {/* The section's ONE live region, always mounted so its text always
          arrives in a LATER commit than its mount (the condition for being
          announced at all). General's live-region count: 1. */}
      <p className="sr-only" role="status" aria-live="polite">
        {announce}
      </p>
    </SettingsGroup>
  );
}
