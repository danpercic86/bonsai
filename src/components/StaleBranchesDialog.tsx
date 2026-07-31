// P25d §6: the B4 stale-branch cleanup UI. Opening lists (read-only) the local
// branches merged into the base or with a gone upstream; the merged rows are
// pre-checked, gone-upstream rows are unchecked (force-delete opt-in). The
// "Delete selected" button routes through a nested ConfirmDialog that lists the
// EXACT branch names — deletion is destructive and only that confirm path calls
// `deleteBranches`. Cancel writes nothing. Rust owns all safety; this renders.

import { useEffect, useMemo, useState } from 'react';
import { ipc } from '../ipc';
import type { BranchDeleteResult, StaleBranch, StaleReport } from '../ipc';
import { ConfirmDialog } from './ConfirmDialog';
import { relativeDate } from '../graph/draw';
import { usePushToast } from '../ToastContext';
import { errorMessage } from '../utils/errors';

export interface StaleBranchesDialogProps {
  open: boolean;
  onClose(): void;
  repoId: string;
  /** Fired after ≥1 successful delete; the parent refetches its branches snapshot. */
  onDeleted(): void;
}

function shortOid(oid: string): string {
  return oid.slice(0, 7);
}

/** Ahead/behind vs the base (best-effort; hidden when both are 0/null). */
function AheadBehind({ branch }: { branch: StaleBranch }) {
  const ahead = branch.ahead ?? 0;
  const behind = branch.behind ?? 0;
  if (ahead === 0 && behind === 0) return null;
  const parts: string[] = [];
  if (ahead > 0) parts.push(`↑${ahead}`);
  if (behind > 0) parts.push(`↓${behind}`);
  return (
    <span className="branch-badge" title={`vs base`}>
      {parts.join(' ')}
    </span>
  );
}

export function StaleBranchesDialog({ open, onClose, repoId, onDeleted }: StaleBranchesDialogProps) {
  const pushToast = usePushToast();
  const [report, setReport] = useState<StaleReport | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  // §6.2: per-row outcomes from the last delete, keyed by branch name (only the
  // non-`deleted` ones — skipped/failed). Deleted rows just vanish from the
  // re-fetched list; these annotate the rows that legitimately came back.
  const [outcomes, setOutcomes] = useState<Record<string, BranchDeleteResult>>({});

  // Load the stale report each time the dialog opens (and on repoId change). A
  // cancelled guard keeps a stale async response from clobbering a newer one
  // (mirrors ProfileActivateDialog / AiAssetsPanel).
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setReport(null);
    setLoadError(null);
    setOutcomes({});
    (async () => {
      try {
        const r = await ipc.listStaleBranches(repoId);
        if (!cancelled) setReport(r);
      } catch (e) {
        if (!cancelled) setLoadError(errorMessage(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, repoId]);

  // Whenever a fresh report lands (initial load or post-delete re-fetch), reset
  // the selection to the merged rows (pre-checked); gone-upstream rows stay off.
  useEffect(() => {
    if (report === null) return;
    setSelected(new Set(report.branches.filter((b) => b.merged).map((b) => b.name)));
  }, [report]);

  // Esc closes (capture phase + stopPropagation, mirroring ConfirmDialog). While
  // the nested confirm is up we bow out so IT handles its own Escape.
  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (confirmOpen) return;
      e.stopPropagation();
      if (!busy) onClose();
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [open, busy, confirmOpen, onClose]);

  const now = useMemo(() => Math.floor(Date.now() / 1000), [report]);

  const selectedNames = useMemo(
    () => (report === null ? [] : report.branches.filter((b) => selected.has(b.name)).map((b) => b.name)),
    [report, selected],
  );

  if (!open) return null;

  const branches = report?.branches ?? [];
  const allSelected = branches.length > 0 && selectedNames.length === branches.length;

  function toggle(name: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  }

  function toggleAll() {
    if (allSelected) setSelected(new Set());
    else setSelected(new Set(branches.map((b) => b.name)));
  }

  const doDelete = async (): Promise<void> => {
    setBusy(true);
    try {
      const res = await ipc.deleteBranches(repoId, selectedNames);
      const deleted = res.filter((r) => r.status === 'deleted').length;
      const failed = res.filter((r) => r.status === 'failed').length;
      const skipped = res.filter((r) => r.status.startsWith('skipped')).length;

      const segs = [`Deleted ${deleted} branch${deleted === 1 ? '' : 'es'}`];
      if (skipped > 0) segs.push(`skipped ${skipped}`);
      if (failed > 0) segs.push(`failed ${failed}`);
      const summary = segs.join(', ');
      const level = failed > 0 ? 'error' : deleted === 0 ? 'info' : 'success';
      pushToast(level, summary);

      // Retain the non-deleted per-branch results so the re-fetched list can
      // annotate each row the backend skipped/failed (§6.2).
      const kept: Record<string, BranchDeleteResult> = {};
      for (const r of res) if (r.status !== 'deleted') kept[r.name] = r;
      setOutcomes(kept);

      // Notify the parent (branches snapshot refetch) then re-fetch the stale
      // list so the deleted rows disappear; close if nothing remains.
      onDeleted();
      const next = await ipc.listStaleBranches(repoId);
      setReport(next);
      if (next.branches.length === 0) onClose();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setBusy(false);
      setConfirmOpen(false);
    }
  };

  return (
    <>
      <div
        className="dialog-overlay"
        onMouseDown={(e) => {
          if (e.target === e.currentTarget && !busy && !confirmOpen) onClose();
        }}
      >
        <div
          className="dialog-card stale-card"
          role="dialog"
          aria-modal="true"
          aria-label="Clean up branches"
        >
          <div className="shortcut-header">
            <h2 className="dialog-title shortcut-title">Clean up branches</h2>
            <button
              type="button"
              className="btn-icon shortcut-close"
              aria-label="Close"
              title="Close"
              disabled={busy}
              onClick={onClose}
            >
              {'×'}
            </button>
          </div>

          {loadError !== null ? (
            <div className="error-banner" role="alert">
              {loadError}
            </div>
          ) : report === null ? (
            <p className="settings-ai-status">Loading…</p>
          ) : branches.length === 0 ? (
            <p className="settings-ai-status">No stale branches — nothing to clean up.</p>
          ) : (
            <>
              <div className="stale-subhead">
                <p className="settings-section-desc">
                  Branches merged into <span className="mono">{report.base}</span> or with a gone
                  upstream.
                </p>
                <button type="button" className="stale-selectall" onClick={toggleAll}>
                  {allSelected ? 'Select none' : 'Select all'}
                </button>
              </div>

              <ul className="stale-list">
                {branches.map((b) => {
                  const gone = b.reason === 'goneUpstream';
                  const outcome = outcomes[b.name];
                  return (
                    <li key={b.name} className="stale-row">
                      <label className="stale-row-main">
                        <input
                          type="checkbox"
                          checked={selected.has(b.name)}
                          disabled={busy}
                          onChange={() => toggle(b.name)}
                        />
                        <span className="stale-name mono" title={b.name}>
                          {b.name}
                        </span>
                        <span
                          className={`asset-chip ${gone ? 'asset-chip-drifted' : 'asset-chip-sync'}`}
                        >
                          {gone ? 'gone upstream' : 'merged'}
                        </span>
                        <span className="stale-summary" title={b.lastCommitSummary}>
                          {b.lastCommitSummary}
                        </span>
                        <AheadBehind branch={b} />
                        <span className="stale-time mono" title={shortOid(b.tip)}>
                          {relativeDate(b.lastCommitTime, now)}
                        </span>
                      </label>
                      {gone && outcome === undefined && (
                        <div className="stale-force-hint">
                          gone upstream — force delete (unchecked by default)
                        </div>
                      )}
                      {outcome !== undefined && (
                        <div
                          className={`stale-outcome ${
                            outcome.status === 'failed'
                              ? 'stale-outcome-error'
                              : 'stale-outcome-warn'
                          }`}
                        >
                          {outcome.status === 'failed' ? 'delete failed' : 'skipped'}
                          {outcome.message !== null ? `: ${outcome.message}` : ''}
                        </div>
                      )}
                    </li>
                  );
                })}
              </ul>
            </>
          )}

          <div className="dialog-buttons">
            <button type="button" className="btn-secondary" disabled={busy} onClick={onClose}>
              Close
            </button>
            <button
              type="button"
              className="btn-danger"
              disabled={busy || selectedNames.length === 0}
              onClick={() => setConfirmOpen(true)}
            >
              Delete selected ({selectedNames.length})
            </button>
          </div>
        </div>
      </div>

      {/* SAFETY GATE: the only path that deletes. Lists the exact names. */}
      <ConfirmDialog
        open={confirmOpen}
        title="Delete branches"
        confirmLabel={busy ? 'Deleting…' : `Delete ${selectedNames.length}`}
        busy={busy}
        onConfirm={() => void doDelete()}
        onCancel={() => setConfirmOpen(false)}
      >
        <div>
          Delete {selectedNames.length} local branch{selectedNames.length === 1 ? '' : 'es'}? This
          cannot be undone.
        </div>
        <ul className="confirm-name-list">
          {selectedNames.map((name) => (
            <li key={name} className="mono">
              {name}
            </li>
          ))}
        </ul>
      </ConfirmDialog>
    </>
  );
}
