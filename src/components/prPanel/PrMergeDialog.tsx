import { useEffect, useId, useRef, useState } from 'react';
import type { ForgeKind, MergeMethod, MergePrInput } from '../../ipc';
import { SettingsSegmented } from '../settings/SettingsSegmented';
import {
  MERGE_METHOD_DESC,
  MERGE_METHOD_LABEL,
  MERGE_METHOD_WORD,
  methodTakesCommitFields,
} from './mergeMethods';

// P83 — merge form dialog (UI contract §2). Built on the shared `dialog-card`
// chrome; IS the confirmation (no second modal). Local form state only; no IPC.
// Initial focus lands on Cancel so a stray Enter never fires the merge.

export interface PrMergeDialogProps {
  open: boolean;
  number: number;
  kind: ForgeKind;
  host: string;
  sourceBranch: string;
  targetBranch: string;
  supportedMethods: MergeMethod[];
  busy: boolean;
  onConfirm(input: MergePrInput): void;
  onCancel(): void;
}

export function PrMergeDialog({
  open,
  number,
  kind,
  host,
  sourceBranch,
  targetBranch,
  supportedMethods,
  busy,
  onConfirm,
  onCancel,
}: PrMergeDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  const triggerRef = useRef<HTMLElement | null>(null);
  const methodLabelId = useId();

  // Capture the trigger on open and restore focus to it on close/unmount, so
  // keyboard users land back where they were (guard for null / removed node).
  useEffect(() => {
    if (!open) return;
    triggerRef.current = document.activeElement as HTMLElement | null;
    return () => {
      const el = triggerRef.current;
      triggerRef.current = null;
      if (el && el.isConnected && typeof el.focus === 'function') el.focus();
    };
  }, [open]);

  const defaultMethod = supportedMethods[0] ?? 'merge';
  const [method, setMethod] = useState<MergeMethod>(defaultMethod);
  const [commitTitle, setCommitTitle] = useState('');
  const [commitMessage, setCommitMessage] = useState('');
  const [deleteSourceBranch, setDeleteSourceBranch] = useState(false);

  // Reset form each time the dialog (re)opens so a prior edit never leaks.
  useEffect(() => {
    if (open) {
      setMethod(defaultMethod);
      setCommitTitle('');
      setCommitMessage('');
      setDeleteSourceBranch(false);
      cancelRef.current?.focus();
    }
    // defaultMethod is derived from supportedMethods, stable per PR.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.stopPropagation();
      onCancel();
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [open, onCancel]);

  if (!open) return null;

  const title = `Merge pull request #${number}?`;
  const showCommitFields = methodTakesCommitFields(method);
  // Orchestrator decision (b): GitHub's merge API ignores delete-source-branch,
  // so don't render a dead control there; show it for the other forges.
  const showDeleteBranch = kind !== 'gitHub';

  const methodOptions = supportedMethods.map((m) => ({
    value: m,
    label: MERGE_METHOD_LABEL[m],
  }));

  function handleConfirm() {
    const input: MergePrInput = {
      method,
      commitTitle: showCommitFields && commitTitle.trim() !== '' ? commitTitle : null,
      commitMessage: showCommitFields && commitMessage.trim() !== '' ? commitMessage : null,
      deleteSourceBranch: showDeleteBranch && deleteSourceBranch,
      headSha: null, // filled backend-side (Azure only)
    };
    onConfirm(input);
  }

  return (
    <div className="dialog-overlay" onClick={onCancel}>
      <div
        className="dialog-card pr-merge-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby={methodLabelId + '-title'}
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="dialog-title" id={methodLabelId + '-title'}>
          {title}
        </h2>
        <div className="dialog-body">
          <p>
            This merges{' '}
            <span className="mono" title={sourceBranch}>
              {sourceBranch}
            </span>{' '}
            into{' '}
            <span className="mono" title={targetBranch}>
              {targetBranch}
            </span>{' '}
            on {host} using a {MERGE_METHOD_WORD[method]}. This can’t be undone from Bonsai.
          </p>

          <div className="pr-field">
            <span className="pr-field-label" id={methodLabelId}>
              Merge method
            </span>
            <SettingsSegmented<MergeMethod>
              name={`pr-merge-method-${number}`}
              value={method}
              options={methodOptions}
              labelledBy={methodLabelId}
              disabled={busy}
              onChange={setMethod}
            />
            <p className="pr-merge-method-desc">{MERGE_METHOD_DESC[method]}</p>
          </div>

          {showCommitFields && (
            <>
              <label className="pr-field">
                <span className="pr-field-label">Commit title</span>
                <input
                  className="pr-input"
                  type="text"
                  placeholder="Leave blank to use the forge default"
                  value={commitTitle}
                  disabled={busy}
                  onChange={(e) => setCommitTitle(e.target.value)}
                />
              </label>
              <label className="pr-field">
                <span className="pr-field-label">Commit message</span>
                <textarea
                  className="pr-input pr-textarea"
                  placeholder="Leave blank to use the forge default"
                  value={commitMessage}
                  disabled={busy}
                  rows={4}
                  onChange={(e) => setCommitMessage(e.target.value)}
                />
              </label>
            </>
          )}

          {showDeleteBranch && (
            <label className="pr-draft-toggle">
              <input
                type="checkbox"
                checked={deleteSourceBranch}
                disabled={busy}
                onChange={(e) => setDeleteSourceBranch(e.target.checked)}
              />
              <span>
                Delete{' '}
                <span className="mono" title={sourceBranch}>
                  {sourceBranch}
                </span>{' '}
                after merging
              </span>
            </label>
          )}
        </div>
        <div className="dialog-buttons">
          <button type="button" className="btn-secondary" ref={cancelRef} onClick={onCancel}>
            Cancel
          </button>
          <button type="button" className="btn-primary" disabled={busy} onClick={handleConfirm}>
            {busy ? 'Merging…' : 'Merge pull request'}
          </button>
        </div>
      </div>
    </div>
  );
}
