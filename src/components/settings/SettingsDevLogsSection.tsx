// P91 §8 — group 4 "Log files": reveal, export, delete. NONE is gated by the
// master switch — the workflow's last three steps (find, send, erase) happen
// after Dev mode is turned off, and the files deliberately persist. Delete in
// particular must work WHILE Dev mode is on (§8.5.4).
//
// The two benign actions and the destructive one sit in SEPARATE rows, fenced by
// the standard 1px border (§8.1) — never shoulder-to-shoulder. Disabled buttons
// use `aria-disabled` (not `disabled`) so a keyboard user can reach them and hear
// why (§8.2).

import type { DevSettings, LogSessionInfo } from '../../ipc';
import { formatBytes } from '../../utils/format';
import { SettingsGroup } from './SettingsGroup';
import { SettingsOutcomeNote, type SettingsOutcome } from './SettingsOutcomeNote';
import { SettingsRow } from './SettingsRow';

const NUM = new Intl.NumberFormat();

/** §6.8 R6 — the delete row's hint, wired as the danger button's accessible
 *  description. A literal because this row exists exactly once; a repeating row
 *  would have to interpolate its instance (see `settingsRowHelpId`). */
const DELETE_HINT_ID = 'dev-delete-logs-hint';

/** P113 §6 — TWO outcome slots, not five. `dev.logs` owns `Show in folder` AND
 *  `Export session…`; `dev.delete-logs` owns `Delete all…`. All three actions are
 *  gated by `anyBusy`, so at most one is in flight and the newest result replaces
 *  its slot — a slot per BUTTON would put two notes 6px apart reporting the two
 *  outcomes of the same button. The logs row's state note had no id at all before
 *  this, and neither benign button had any description. */
const LOGS_NOTE_ID = 'dev-logs-note';
const LOGS_OUTCOME_ID = 'dev-logs-outcome';
const DELETE_OUTCOME_ID = 'dev-delete-logs-outcome';
const LOGS_DESCRIBED_BY = `${LOGS_NOTE_ID} ${LOGS_OUTCOME_ID}`;

export interface DevLogsBusy {
  reveal: boolean;
  export: boolean;
  delete: boolean;
}

/** §F6 §6.4 — three branches, because the delete now clears the usage counts too
 *  and the no-logs branch is a REACHABLE, ACTIONABLE state rather than a dead end.
 *  It used to read 'No log files to delete.', which after §F6 is both false and
 *  the reason the row would look dead to the user who most needs it.
 *
 *  §6.10 R8a — `count` is `number | null`, where null means NOT KNOWN (the
 *  `logSessionInfo` read failed), never a known zero. §6.8 R3 removed the
 *  understating copy from the dialog; collapsing null to 0 HERE put it straight
 *  back, because §6.8 R6 wired this hint up as the danger button's
 *  `aria-describedby`: a screen-reader user was told "No log files yet" as the
 *  description of a control about to delete however many exist. The caveat LEADS
 *  rather than trails — it is the exceptional fact, and the scope uncertainty has
 *  to arrive before the detail. "all of them" binds to "the log files" before it.
 *  No "first" here, unlike the dialog's line: a standing row hint has no
 *  "before asking you" moment to be first of. */
function deleteNote(count: number | null, enabled: boolean, size: string): string {
  if (count === 0) return "No log files yet. Removes Bonsai's usage counts.";
  const lead = count === null ? 'Bonsai could not count the log files. ' : '';
  const target =
    count === null ? 'of them' : `${NUM.format(count)} log file${count === 1 ? '' : 's'} (${size})`;
  return enabled
    ? `${lead}Removes all ${target} and Bonsai's usage counts, including the log being recorded now. Recording continues in a new file.`
    : `${lead}Removes all ${target} and Bonsai's usage counts from this computer.`;
}

export function SettingsDevLogsSection({
  dev,
  info,
  busy,
  onReveal,
  onExport,
  onRequestDelete,
  logsOutcome,
  deleteOutcome,
}: {
  dev: DevSettings;
  info: LogSessionInfo | null;
  busy: DevLogsBusy;
  onReveal(): void;
  onExport(): void;
  onRequestDelete(): void;
  /** P113 rows 1-3: reveal / export outcome, shared by both benign buttons. */
  logsOutcome: SettingsOutcome | null;
  /** P113 rows 4-5: the delete outcome, whose tone varies at runtime. */
  deleteOutcome: SettingsOutcome | null;
}) {
  // §6.10 R8a: `hasLogs` STAYS collapsed for the two benign actions below —
  // treating an unknown count as "no logs" only disables reveal/export, which is
  // the safe direction. The destructive row gets the honest `number | null`.
  const count = info?.totalFiles ?? 0;
  const hasLogs = count > 0;
  const deleteCount = info === null ? null : info.totalFiles;
  const size = formatBytes(info?.totalBytes ?? 0);
  const dir = info?.dir ?? '';
  const anyBusy = busy.reveal || busy.export || busy.delete;

  const logsNote = hasLogs ? (
    <p className="settings-row-note" id={LOGS_NOTE_ID}>
      Kept in{' '}
      <span className="mono" title={dir}>
        {dir}
      </span>{' '}
      after you turn Dev mode off. Bonsai keeps the 10 most recent sessions and deletes older ones
      automatically.
    </p>
  ) : (
    <p className="settings-row-note" id={LOGS_NOTE_ID}>
      No logs yet. Turn on Dev mode, reproduce the problem, then export the session.
    </p>
  );

  const deleteText = busy.delete ? 'Deleting…' : deleteNote(deleteCount, dev.enabled, size);

  return (
    <SettingsGroup id="dev-logs" title="Log files">
      <SettingsRow
        id="dev.logs"
        rowLabel="Log files"
        stacked
        hint={
          <>
            {logsNote}
            {/* P113 §6.1: the SECOND tenant of the help slot — the state note
                above stays visible, it is never replaced. */}
            <SettingsOutcomeNote slot="dev.logs" id={LOGS_OUTCOME_ID} outcome={logsOutcome} />
          </>
        }
      >
        <div className="dev-logs-actions">
          <button
            type="button"
            className="btn-secondary"
            aria-disabled={!hasLogs || anyBusy}
            aria-busy={busy.reveal}
            title={hasLogs ? undefined : 'No logs yet.'}
            aria-describedby={LOGS_DESCRIBED_BY}
            onClick={() => {
              if (hasLogs && !anyBusy) onReveal();
            }}
          >
            Show in folder
          </button>
          <button
            type="button"
            className="btn-secondary"
            aria-disabled={!hasLogs || anyBusy}
            aria-busy={busy.export}
            title={hasLogs ? undefined : 'No logs yet.'}
            aria-describedby={LOGS_DESCRIBED_BY}
            onClick={() => {
              if (hasLogs && !anyBusy) onExport();
            }}
          >
            Export session…
          </button>
        </div>
      </SettingsRow>

      {/* §F6 §6.4 — the delete row is ALWAYS ENABLED, unlike the two rows above.
          It used to be gated on `hasLogs`, which left the user who never turned
          Dev mode on — zero log files, up to 90 days of usage counts — looking at
          a permanently disabled button: exactly the user for whom deletability
          was granted. `metrics.rs` bumps `sessions` at init on EVERY launch, so by
          the time anyone can see this row there is always something to clear;
          there is no reachable "nothing to delete" state. `anyBusy` still gates it
          during an in-flight operation, and stays `aria-disabled` (not `disabled`)
          so focus survives the operation's own completion. */}
      <SettingsRow
        id="dev.delete-logs"
        rowLabel="Delete logs and usage counts"
        hint={
          <>
            <p className="settings-row-note" id={DELETE_HINT_ID}>
              {deleteText}
            </p>
            <SettingsOutcomeNote
              slot="dev.delete-logs"
              id={DELETE_OUTCOME_ID}
              outcome={deleteOutcome}
            />
          </>
        }
      >
        <button
          type="button"
          className="btn-danger"
          aria-disabled={anyBusy}
          aria-busy={busy.delete}
          /* §6.8 R6 — `SettingsRow` renders `hint` as a bare node and wires no
             `aria-describedby`, and the row title is a <span>, not a label, so
             this button's whole accessible name was `Delete all…`: "all" of WHAT
             reached sighted users only, by proximity. Pointing at the hint is
             additive, changes nothing visually, and leaves the accessible NAME
             (which the DOM↔catalog guard asserts) untouched.

             P113 §9 — the outcome id is COMPOSED onto the hint, never replaces
             it, and both ids are always present: an id resolving to an empty
             element reads as nothing, while mutating `aria-describedby` can
             itself trigger a re-announcement. */
          aria-describedby={`${DELETE_HINT_ID} ${DELETE_OUTCOME_ID}`}
          onClick={() => {
            if (!anyBusy) onRequestDelete();
          }}
        >
          Delete all…
        </button>
      </SettingsRow>
    </SettingsGroup>
  );
}
