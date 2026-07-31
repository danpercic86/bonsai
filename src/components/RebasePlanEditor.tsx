import { useEffect, useState } from 'react';
import type { RebaseAction, RebaseTodoOp } from '../ipc';

export interface RebasePlanEditorProps {
  open: boolean;
  /** Human label for the base the plan replays onto (short oid or ref name). */
  ontoLabel: string;
  ontoOid: string;
  /** Seed rows from getInteractivePlan (all `pick`, oldest-first). */
  initialTodos: RebaseTodoOp[];
  /** Per-row commit summaries for display (keyed by oid). */
  summaries: Record<string, string>;
  mutating: boolean;
  /** Backend error from a failed Start, shown in-dialog (dialog stays open). */
  error: string | null;
  onCancel(): void;
  /** Fired with the FINAL edited plan (order + actions + messages). */
  onStart(todos: RebaseTodoOp[]): void;
}

/** Local draft row: action + the (reword/squash) message textarea contents. */
interface DraftRow {
  oid: string;
  action: RebaseAction;
  message: string;
}

const ACTIONS: RebaseAction[] = ['pick', 'reword', 'squash', 'fixup', 'drop'];

function shortOid(oid: string): string {
  return oid.slice(0, 7);
}

/**
 * P23b §8.1: interactive-rebase plan editor. Presentation-only — holds a local
 * draft (order + per-row action + reword/squash message) and emits the final
 * `todos` array via `onStart`. No IPC inside the component. Rows are listed
 * OLDEST → NEWEST (== the backend's execution order). Reorder is Up/Down buttons
 * (drag deferred to Polish, OPEN #3). Client-side validation mirrors the Rust
 * `validate_todos` (§2.6): a non-empty, not-all-drop plan whose first applied row
 * is pick/reword and whose reword rows carry a message.
 */
export function RebasePlanEditor({
  open,
  ontoLabel,
  ontoOid,
  initialTodos,
  summaries,
  mutating,
  error,
  onCancel,
  onStart,
}: RebasePlanEditorProps) {
  const [rows, setRows] = useState<DraftRow[]>([]);

  // Re-seed the draft whenever the editor opens on a new target.
  useEffect(() => {
    if (!open) return;
    setRows(initialTodos.map((t) => ({ oid: t.oid, action: t.action, message: '' })));
  }, [open, ontoOid, initialTodos]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      // Capture phase + stopPropagation so App's global Esc-deselect never fires.
      e.stopPropagation();
      onCancel();
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [open, onCancel]);

  if (!open) return null;

  const summaryOf = (oid: string): string => summaries[oid] ?? shortOid(oid);

  function move(index: number, dir: -1 | 1): void {
    setRows((prev) => {
      const target = index + dir;
      if (target < 0 || target >= prev.length) return prev;
      const next = prev.slice();
      const [item] = next.splice(index, 1);
      next.splice(target, 0, item);
      return next;
    });
  }

  function setAction(index: number, action: RebaseAction): void {
    setRows((prev) =>
      prev.map((r, i) => {
        if (i !== index) return r;
        // Prefill the message when switching INTO reword/squash and it is empty.
        let message = r.message;
        if (message.trim() === '') {
          if (action === 'reword') {
            message = summaryOf(r.oid);
          } else if (action === 'squash') {
            // Predecessor = the previous non-drop row (its message, else summary).
            let predSummary = '';
            for (let j = i - 1; j >= 0; j--) {
              if (prev[j].action === 'drop') continue;
              predSummary =
                prev[j].message.trim() !== '' ? prev[j].message : summaryOf(prev[j].oid);
              break;
            }
            message = predSummary !== '' ? `${predSummary}\n\n${summaryOf(r.oid)}` : summaryOf(r.oid);
          }
        }
        return { ...r, action, message };
      }),
    );
  }

  function setMessage(index: number, message: string): void {
    setRows((prev) => prev.map((r, i) => (i === index ? { ...r, message } : r)));
  }

  const kept = rows.filter((r) => r.action !== 'drop');
  let validationError: string | null = null;
  if (kept.length === 0) {
    validationError = 'The plan drops every commit — nothing to rebase.';
  } else if (kept[0].action === 'squash' || kept[0].action === 'fixup') {
    validationError =
      'The first applied commit must be “pick” or “reword” — a squash/fixup needs a preceding commit to combine into.';
  } else if (kept.some((r) => r.action === 'reword' && r.message.trim() === '')) {
    validationError = 'A “reword” commit needs a non-empty message.';
  }

  // No-op guard: unchanged order AND every row still a plain pick.
  const isNoOp =
    validationError === null &&
    rows.length === initialTodos.length &&
    rows.every((r, i) => r.action === 'pick' && r.oid === initialTodos[i]?.oid);
  const noOpHint = isNoOp
    ? 'No changes to apply yet — reorder, reword, squash, fixup or drop a commit to rebase.'
    : null;

  const canStart = !mutating && validationError === null && !isNoOp;

  function buildTodos(): RebaseTodoOp[] {
    return rows.map((r) => ({
      oid: r.oid,
      action: r.action,
      newMessage:
        r.action === 'reword'
          ? r.message.trim()
          : r.action === 'squash'
            ? r.message.trim() === ''
              ? null
              : r.message
            : null,
    }));
  }

  return (
    <div className="dialog-overlay" onClick={onCancel}>
      <div
        className="dialog-card rebase-plan-card"
        role="dialog"
        aria-modal="true"
        aria-label="Interactive rebase"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="dialog-title">Interactive rebase onto {ontoLabel}</h2>
        <p className="rebase-plan-hint">
          Commits are listed oldest → newest (execution order). Reorder with the arrows; pick an
          action per commit.
        </p>
        <div className="rebase-plan-list">
          {rows.map((r, i) => {
            const dropped = r.action === 'drop';
            const showMessage = r.action === 'reword' || r.action === 'squash';
            return (
              <div className="rebase-plan-row" key={`${r.oid}:${i}`}>
                <div className="rebase-plan-reorder">
                  <button
                    type="button"
                    className="rebase-plan-icon-btn"
                    aria-label="Move up"
                    title="Move up"
                    disabled={i === 0 || mutating}
                    onClick={() => move(i, -1)}
                  >
                    ↑
                  </button>
                  <button
                    type="button"
                    className="rebase-plan-icon-btn"
                    aria-label="Move down"
                    title="Move down"
                    disabled={i === rows.length - 1 || mutating}
                    onClick={() => move(i, 1)}
                  >
                    ↓
                  </button>
                </div>
                <select
                  className="rebase-plan-action"
                  aria-label="Action"
                  value={r.action}
                  disabled={mutating}
                  onChange={(e) => setAction(i, e.target.value as RebaseAction)}
                >
                  {ACTIONS.map((a) => (
                    <option key={a} value={a}>
                      {a}
                    </option>
                  ))}
                </select>
                <div className="rebase-plan-main">
                  <div className={dropped ? 'rebase-plan-commit dropped' : 'rebase-plan-commit'}>
                    <span className="mono">{shortOid(r.oid)}</span> {summaryOf(r.oid)}
                  </div>
                  {showMessage && (
                    <textarea
                      className="dialog-input dialog-textarea rebase-plan-msg"
                      rows={2}
                      placeholder={
                        r.action === 'reword'
                          ? 'New commit message (required)'
                          : 'Combined commit message (optional)'
                      }
                      value={r.message}
                      disabled={mutating}
                      onChange={(e) => setMessage(i, e.target.value)}
                    />
                  )}
                </div>
              </div>
            );
          })}
        </div>
        {validationError !== null && <p className="dialog-error">{validationError}</p>}
        {noOpHint !== null && <p className="rebase-plan-hint">{noOpHint}</p>}
        {error !== null && <p className="dialog-error">{error}</p>}
        <div className="dialog-buttons">
          <button type="button" className="btn-secondary" onClick={onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className="btn-primary"
            disabled={!canStart}
            onClick={() => onStart(buildTodos())}
          >
            Start rebase
          </button>
        </div>
      </div>
    </div>
  );
}
