// P91 §4.3 / §8.3 / §8.5.4 — the three Developer-page confirm dialogs, split out
// of the container. All three are the existing `ConfirmDialog`; default focus is
// Cancel in every one. The raw-names dialog is `primary` (reversible, destroys
// nothing); the delete dialog is `danger` (data loss, irreversible).

import type { LogSessionInfo } from '../../ipc';
import { formatBytes } from '../../utils/format';
import { ConfirmDialog } from '../ConfirmDialog';

const NUM = new Intl.NumberFormat();

function archives(n: number): string {
  return `${NUM.format(n)} exported log archive${n === 1 ? '' : 's'}`;
}

export function RawNamesConfirmDialog({
  open,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  onConfirm(): void;
  onCancel(): void;
}) {
  return (
    <ConfirmDialog
      open={open}
      title="Include raw repository names in logs?"
      confirmLabel="Include raw names"
      confirmVariant="primary"
      busy={false}
      onConfirm={onConfirm}
      onCancel={onCancel}
    >
      <p>
        New log files will contain the real names of your branches, tags, files and repository —
        instead of placeholders like <span className="mono">ref#3</span> and{' '}
        <span className="mono">path#7</span>.
      </p>
      <p>
        Commit messages, file contents, author names and email addresses are still never written,
        and passwords and access tokens are never written in any mode.
      </p>
      <p>
        Bonsai starts a new log file now, so the file you are recording into does not mix the two
        settings. Read an exported log before sending it to anyone.
      </p>
    </ConfirmDialog>
  );
}

export function ExportConfirmDialog({
  open,
  info,
  busy,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  info: LogSessionInfo | null;
  busy: boolean;
  onConfirm(): void;
  onCancel(): void;
}) {
  const n = info?.totalFiles ?? 0;
  const size = formatBytes(info?.totalBytes ?? 0);
  const raw = info?.redaction === 'raw';
  return (
    <ConfirmDialog
      open={open}
      title="Export this session's log"
      confirmLabel="Choose location…"
      confirmVariant="primary"
      busy={busy}
      onConfirm={onConfirm}
      onCancel={onCancel}
    >
      <p>{`You will get a .zip containing this session's log files (${NUM.format(n)} files, ${size}).`}</p>
      {raw ? (
        <p className="dev-warning-bar">
          <span className="dev-warning-glyph" aria-hidden="true">
            ⚠
          </span>
          Raw names are on: this log contains your real branch, tag, file and repository names.
          Commit messages, file contents, names and email addresses, passwords and tokens are still
          not in the file.
        </p>
      ) : (
        <p>
          Branch, file and repository names are replaced with placeholders. Commit messages, file
          contents, names and email addresses, passwords and tokens are not in the file.
        </p>
      )}
      <p>Open the file and read it before sending it to anyone.</p>
      <p>Exports you saved elsewhere are not removed by &ldquo;Delete all log files&rdquo;.</p>
    </ConfirmDialog>
  );
}

export function DeleteLogsConfirmDialog({
  open,
  info,
  devEnabled,
  busy,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  /** Re-read when the dialog opened, so the count consented to is the count deleted. */
  info: LogSessionInfo | null;
  devEnabled: boolean;
  busy: boolean;
  onConfirm(): void;
  onCancel(): void;
}) {
  const n = info?.totalFiles ?? 0;
  const size = formatBytes(info?.totalBytes ?? 0);
  const exportFiles = info?.exportFiles ?? 0;
  return (
    <ConfirmDialog
      open={open}
      title="Delete all log files?"
      confirmLabel="Delete logs"
      confirmVariant="danger"
      busy={busy}
      onConfirm={onConfirm}
      onCancel={onCancel}
    >
      <p>{`Delete ${NUM.format(n)} log files (${size})? This cannot be undone.`}</p>
      {exportFiles > 0 && <p>{`This includes ${archives(exportFiles)}.`}</p>}
      {devEnabled && (
        <p>
          This includes the session being recorded right now. Bonsai starts a new, empty log file
          and keeps recording.
        </p>
      )}
      <p>Exports you saved elsewhere are not removed.</p>
      <p>Deleting logs does not change any of your repositories.</p>
    </ConfirmDialog>
  );
}
