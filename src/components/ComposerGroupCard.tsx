import type { FileStatus } from '../ipc';
import type { MoveTarget } from './repoWorkspace/useCommitComposer';

// P54c: one presentational card in the composer review dialog — either a
// commit group (editable message + file rows + drop/merge actions) or the
// read-only "Unassigned" bucket (no message, no drop/merge). ALL state arrives
// via props; this component owns no IPC and no local state.

const BADGES: Record<FileStatus, string> = {
  added: 'A',
  modified: 'M',
  deleted: 'D',
  renamed: 'R',
  typechange: 'T',
  untracked: 'A',
  conflicted: 'C',
};

/** A move-to option: another group (by index) or the unassigned bucket. */
export interface MoveOption {
  value: MoveTarget;
  label: string;
}

export interface ComposerGroupCardProps {
  variant: 'group' | 'unassigned';
  title: string;
  /** Commit message (group variant only). */
  message: string;
  files: string[];
  /** Status badge letter per path (from the working-dir snapshot). */
  statusByPath: Map<string, FileStatus>;
  /** Move-to targets, EXCLUDING this card itself. */
  moveTargets: MoveOption[];
  /** Group variant only: enabled when a following group exists. */
  canMergeIntoNext: boolean;
  onEditMessage(message: string): void;
  onMoveFile(path: string, target: MoveTarget): void;
  /** Group variant only: send this row back to the unassigned bucket. */
  onRemoveFile(path: string): void;
  onPreviewFile(path: string): void;
  /** Group variant only. */
  onDropGroup(): void;
  onMergeIntoNext(): void;
}

function badgeFor(path: string, statusByPath: Map<string, FileStatus>): string {
  const s = statusByPath.get(path);
  return s !== undefined ? BADGES[s] : '?';
}

export function ComposerGroupCard({
  variant,
  title,
  message,
  files,
  statusByPath,
  moveTargets,
  canMergeIntoNext,
  onEditMessage,
  onMoveFile,
  onRemoveFile,
  onPreviewFile,
  onDropGroup,
  onMergeIntoNext,
}: ComposerGroupCardProps) {
  const isGroup = variant === 'group';
  const emptyMessage = isGroup && message.trim() === '';

  return (
    <section className={`composer-card composer-card--${variant}`}>
      <div className="composer-card-head">
        <span className="composer-card-title">
          {title}
          <span className="composer-card-count"> ({files.length})</span>
        </span>
        {isGroup && (
          <div className="composer-card-actions">
            <button
              type="button"
              className="btn-secondary composer-card-action"
              disabled={!canMergeIntoNext}
              title={
                canMergeIntoNext
                  ? 'Merge this commit into the next one'
                  : 'No following commit to merge into'
              }
              onClick={onMergeIntoNext}
            >
              Merge into next
            </button>
            <button
              type="button"
              className="btn-secondary composer-card-action"
              title="Drop this commit (its files return to Unassigned)"
              onClick={onDropGroup}
            >
              Drop group
            </button>
          </div>
        )}
      </div>

      {isGroup && (
        <>
          <textarea
            className="composer-message"
            rows={3}
            placeholder="Commit message (summary, then optional body)"
            value={message}
            onChange={(e) => onEditMessage(e.target.value)}
          />
          {emptyMessage && (
            <div className="composer-message-hint" role="note">
              Add a message to include this commit.
            </div>
          )}
        </>
      )}

      {files.length === 0 ? (
        <p className="composer-empty">
          {isGroup ? 'No files — assign changes or drop this commit.' : 'No unassigned files.'}
        </p>
      ) : (
        <ul className="composer-files">
          {files.map((path) => (
            <li key={path} className="composer-file-row" title={path}>
              <span className="file-badge mono">{badgeFor(path, statusByPath)}</span>
              <span className="composer-file-path mono">{path}</span>
              <select
                className="composer-move-select"
                aria-label={`Move ${path} to another commit`}
                value=""
                onChange={(e) => {
                  const v = e.target.value;
                  if (v === '') return;
                  onMoveFile(path, v === 'unassigned' ? 'unassigned' : Number(v));
                }}
              >
                <option value="">Move to…</option>
                {moveTargets.map((t) => (
                  <option key={String(t.value)} value={String(t.value)}>
                    {t.label}
                  </option>
                ))}
              </select>
              <button
                type="button"
                className="row-action composer-file-preview"
                title="Preview this file's changes (HEAD → working tree)"
                aria-label={`Preview changes to ${path}`}
                onClick={() => onPreviewFile(path)}
              >
                Preview
              </button>
              {isGroup && (
                <button
                  type="button"
                  className="row-action composer-file-remove"
                  title="Remove from this commit (back to Unassigned)"
                  aria-label={`Remove ${path} from ${title}`}
                  onClick={() => onRemoveFile(path)}
                >
                  {'×'}
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
