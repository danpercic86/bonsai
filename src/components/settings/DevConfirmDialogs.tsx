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
        New log files will contain the real names in your repository: the folder it lives in, your
        branches, tags and files, your remote addresses, and full commit IDs — instead of
        placeholders like <span className="mono">ref#3</span> and{' '}
        <span className="mono">path#7</span>. A folder path may include your computer account name.
      </p>
      <p>
        It never adds anything you typed. Commit messages, search text and other text you write
        stay out of the log in every mode — as do the contents of your files, the name and email
        address you commit under, and any password, access token or key.
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
      confirmLabel="Export"
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
          Raw names are on: this log contains the folder your repository lives in, your real
          branch, tag and file names, your remote addresses and full commit IDs. Commit messages,
          search text and anything else you typed are not in the file, and neither are file
          contents, the name and email address you commit under, or any password or access token.
        </p>
      ) : (
        <p>
          Branch, file, remote and repository names are replaced with placeholders, and commit IDs
          are shortened. Commit messages, search text and anything else you typed are not in the
          file, and neither are file contents, the name and email address you commit under, or any
          password or access token.
        </p>
      )}
      <p>Open the file and read it before sending it to anyone.</p>
      {/* The quoted label is part of §6.2's byte-identical pair — it must name the
          control that exists, or this dialog points at one that does not. The rest
          of §4's export copy is unchanged: usage counts are not in an export, so
          neither of its content statements became false. */}
      <p>
        Exports you saved elsewhere are not removed by &ldquo;Delete logs and usage
        counts&rdquo;.
      </p>
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
  // §6.8 R3: `null` means NOT KNOWN (the `logSessionInfo` read failed), which is a
  // different state from a known zero and MUST NOT collapse into it — `?? 0` here
  // is what made a failed read promise "usage counts only" and then delete every
  // log file on disk.
  const n = info === null ? null : info.totalFiles;
  const size = formatBytes(info?.totalBytes ?? 0);
  const exportFiles = info?.exportFiles ?? 0;
  return (
    <ConfirmDialog
      open={open}
      title="Delete logs and usage counts?"
      confirmLabel="Delete all"
      confirmVariant="danger"
      busy={busy}
      onConfirm={onConfirm}
      onCancel={onCancel}
    >
      {/* §F6 §6.4 + §6.8 R3 — THREE lead lines, not two:
            • n > 0    — the count is known and nonzero;
            • n === 0  — known zero: the row is always enabled now, so this is a
                         REACHABLE state and must not promise "0 log files";
            • n === null — NOT KNOWN (the count read failed). It states the scope
                         at its WIDEST, which is the only safe direction for a
                         destructive confirm: reusing the `n === 0` line here
                         would name a smaller target than the command destroys.
          The `n > 0` line pluralises `log file` off the same count as the row
          hint 8px above it (§6.8 R4) — `Delete 1 log files` was the bug. */}
      <p>
        {n === null
          ? "Delete all log files and clear Bonsai's usage counts? Bonsai could not count them first. This cannot be undone."
          : n === 0
            ? "Clear Bonsai's usage counts? This cannot be undone."
            : `Delete ${NUM.format(n)} log file${n === 1 ? '' : 's'} (${size}) and clear Bonsai's usage counts? This cannot be undone.`}
      </p>
      {/* Names the exact target, per the destructive-action rule: a confirmation
          that names a smaller target than it destroys is the defect §F6 fixes. The
          FOLDER, never `usage.json` — deleting that file alone is undone by its
          `.bak`. There is no undo and none is offered: unlike logs, usage counts
          are not exported, so nothing recovers them from another surface. */}
      <p>
        Usage counts are how often Bonsai&apos;s own actions ran. They live in a{' '}
        <span className="mono">metrics</span> folder beside your log files, and Bonsai removes
        that whole folder.
      </p>
      {exportFiles > 0 && <p>{`This includes ${archives(exportFiles)}.`}</p>}
      {devEnabled && (
        <p>
          This includes the session being recorded right now. Bonsai starts a new, empty log file
          and keeps recording.
        </p>
      )}
      <p>Exports you saved elsewhere are not removed.</p>
      <p>Deleting these does not change any of your repositories.</p>
    </ConfirmDialog>
  );
}
