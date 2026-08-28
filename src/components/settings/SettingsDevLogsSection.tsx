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
import { SettingsRow } from './SettingsRow';

const NUM = new Intl.NumberFormat();

export interface DevLogsBusy {
  reveal: boolean;
  export: boolean;
  delete: boolean;
}

function deleteNote(hasLogs: boolean, enabled: boolean, count: number, size: string): string {
  if (!hasLogs) return 'No log files to delete.';
  const files = `${NUM.format(count)} log file${count === 1 ? '' : 's'} (${size})`;
  return enabled
    ? `Removes all ${files}, including the one being recorded now. Recording continues in a new file.`
    : `Removes all ${files} from this computer.`;
}

export function SettingsDevLogsSection({
  dev,
  info,
  busy,
  onReveal,
  onExport,
  onRequestDelete,
}: {
  dev: DevSettings;
  info: LogSessionInfo | null;
  busy: DevLogsBusy;
  onReveal(): void;
  onExport(): void;
  onRequestDelete(): void;
}) {
  const count = info?.totalFiles ?? 0;
  const hasLogs = count > 0;
  const size = formatBytes(info?.totalBytes ?? 0);
  const dir = info?.dir ?? '';
  const anyBusy = busy.reveal || busy.export || busy.delete;

  const logsNote = hasLogs ? (
    <p className="settings-row-note">
      Kept in{' '}
      <span className="mono" title={dir}>
        {dir}
      </span>{' '}
      after you turn Dev mode off. Bonsai keeps the 10 most recent sessions and deletes older ones
      automatically.
    </p>
  ) : (
    <p className="settings-row-note">
      No logs yet. Turn on Dev mode, reproduce the problem, then export the session.
    </p>
  );

  const deleteText = busy.delete
    ? 'Deleting…'
    : deleteNote(hasLogs, dev.enabled, count, size);

  return (
    <SettingsGroup id="dev-logs" title="Log files">
      <SettingsRow id="dev.logs" rowLabel="Log files" stacked hint={logsNote}>
        <div className="dev-logs-actions">
          <button
            type="button"
            className="btn-secondary"
            aria-disabled={!hasLogs || anyBusy}
            aria-busy={busy.reveal}
            title={hasLogs ? undefined : 'No logs yet.'}
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
            onClick={() => {
              if (hasLogs && !anyBusy) onExport();
            }}
          >
            Export session…
          </button>
        </div>
      </SettingsRow>

      <SettingsRow
        id="dev.delete-logs"
        rowLabel="Delete all log files"
        hint={<p className="settings-row-note">{deleteText}</p>}
      >
        <button
          type="button"
          className="btn-danger"
          aria-disabled={!hasLogs || anyBusy}
          aria-busy={busy.delete}
          title={hasLogs ? undefined : 'No logs yet.'}
          onClick={() => {
            if (hasLogs && !anyBusy) onRequestDelete();
          }}
        >
          Delete logs…
        </button>
      </SettingsRow>
    </SettingsGroup>
  );
}
