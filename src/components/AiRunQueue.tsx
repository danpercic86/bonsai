/**
 * P68e §5 — the per-file queue of a bulk run.
 *
 * §5.1 makes discoverability a HARD REQUIREMENT, not a nicety: the reported bug
 * ("Propose & review does nothing") was caused by results landing in the center pane
 * while the button lived in the right panel. These per-row `Review` buttons are the
 * only place a user can reach proposal #2 and #3 at all.
 *
 * Rows are deliberately NOT clickable as a whole — the button is the target, so
 * there is no ambiguous hit area beside it.
 */
import { splitPath } from './StatusFileRow';
import type { AiActivityFile } from './aiDockFormat';

const GLYPH: Record<AiActivityFile['status'], string> = {
  pending: '…',
  ready: '✓',
  failed: '⚠',
};

const WORD: Record<AiActivityFile['status'], string> = {
  pending: 'Working…',
  ready: 'Ready',
  failed: 'Failed',
};

export interface AiRunQueueProps {
  files: AiActivityFile[];
  onReviewFile(path: string): void;
  onRetryFile(path: string): void;
}

export function AiRunQueue({ files, onReviewFile, onRetryFile }: AiRunQueueProps) {
  return (
    <ul className="ai-run-queue" aria-label="Files in this AI run">
      {files.map((file) => {
        const { dir, name } = splitPath(file.path);
        return (
          <li key={file.path} className="ai-run-queue-row" data-status={file.status}>
            <span className="ai-run-queue-glyph" aria-hidden="true">
              {GLYPH[file.status]}
            </span>
            <span className="ai-run-queue-path mono" title={file.path}>
              {dir !== null && <span className="ai-dock-dir">{dir}</span>}
              <span className="ai-dock-name">{name}</span>
            </span>
            <span className="ai-run-queue-word">{WORD[file.status]}</span>
            {file.status === 'failed' && file.error !== null && (
              <span className="ai-run-queue-reason" title={file.error}>
                {file.error}
              </span>
            )}
            {file.status === 'ready' && (
              <button
                type="button"
                className="btn-secondary ai-run-queue-action"
                aria-label={`Review AI proposal for ${file.path}`}
                title="Open the proposal in the center pane"
                onClick={() => onReviewFile(file.path)}
              >
                Review
              </button>
            )}
            {file.status === 'failed' && (
              <button
                type="button"
                className="btn-secondary ai-run-queue-action"
                aria-label={`Retry AI resolution for ${file.path}`}
                onClick={() => onRetryFile(file.path)}
              >
                Retry
              </button>
            )}
          </li>
        );
      })}
    </ul>
  );
}
