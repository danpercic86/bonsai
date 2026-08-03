// P31 §7: the worktree × AI-context matrix. One row per worktree (main first,
// backend order) showing branch, status badges, the active profile, and the
// per-worktree drift/missing chips. Activation is gated through the shared
// ProfileActivateDialog SAFETY GATE — the per-target diff preview is shown and
// nothing is written without explicit confirm. Rust owns all logic; this
// component only renders + calls ipc.

import { useCallback, useEffect, useRef, useState } from 'react';
import { ipc } from '../ipc';
import type { ProfileActivation, ProfileStore, WorktreeContextStatus } from '../ipc';
import { errorMessage } from '../utils/errors';
import { ProfileActivateDialog } from './ProfileActivateDialog';

export interface WorktreeContextDialogProps {
  open: boolean;
  repoId: string;
  onClose(): void;
  /** Fired after every successful activation (the matrix refetches itself; the
   *  parent refreshes any other open views, e.g. the AI Assets panel). */
  onActivated?(activation: ProfileActivation): void;
}

/** P27 §6.2 badge set, derived from the matrix row (same intent classes as the
 *  sidebar worktree rows). A row may show more than one. */
function rowBadges(row: WorktreeContextStatus): { label: string; intent: string; title?: string }[] {
  const out: { label: string; intent: string; title?: string }[] = [];
  if (row.isCurrent) out.push({ label: 'current', intent: 'submodule-badge-ok' });
  if (row.isMain) out.push({ label: 'main', intent: 'submodule-badge-muted' });
  if (row.locked)
    out.push({ label: 'locked', intent: 'submodule-badge-warn', title: row.blockedReason ?? 'locked' });
  if (row.prunable || !row.valid)
    out.push({ label: 'stale', intent: 'submodule-badge-warn', title: row.blockedReason ?? 'stale' });
  return out;
}

export function WorktreeContextDialog({
  open,
  repoId,
  onClose,
  onActivated,
}: WorktreeContextDialogProps) {
  const [rows, setRows] = useState<WorktreeContextStatus[] | null>(null);
  const [store, setStore] = useState<ProfileStore | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  // Per-row profile pick (keyed by worktreeKey), seeded from the fresh matrix.
  const [selection, setSelection] = useState<Record<string, string>>({});
  // The activation in flight through the safety gate (null => gate closed).
  const [activateTarget, setActivateTarget] = useState<{
    worktreeKey: string;
    profile: string;
  } | null>(null);

  // Monotonic request id: a fetch whose id no longer matches the latest issued
  // one is stale (repoId change / newer Refresh) and must NOT write state.
  const fetchIdRef = useRef(0);

  const refresh = useCallback(async (): Promise<void> => {
    const id = (fetchIdRef.current += 1);
    setLoading(true);
    setError(null);
    try {
      const [matrix, profiles] = await Promise.all([
        ipc.listWorktreeContexts(repoId),
        ipc.listProfiles(repoId),
      ]);
      if (fetchIdRef.current !== id) return;
      setRows(matrix);
      setStore(profiles);
      // Seed each row's pick: its active profile when it still exists in the
      // store, else the first profile. Fresh seed on every fetch keeps the
      // selects honest after activations elsewhere.
      const names = profiles.profiles.map((p) => p.name);
      const seeded: Record<string, string> = {};
      for (const row of matrix) {
        seeded[row.worktreeKey] =
          row.activeProfile !== null && names.includes(row.activeProfile)
            ? row.activeProfile
            : names[0] ?? '';
      }
      setSelection(seeded);
    } catch (e) {
      if (fetchIdRef.current !== id) return;
      setError(errorMessage(e));
    } finally {
      if (fetchIdRef.current === id) setLoading(false);
    }
  }, [repoId]);

  // Fetch on open + whenever the repo changes while open.
  useEffect(() => {
    if (open) void refresh();
  }, [open, refresh]);

  // Esc closes (capture phase + stopPropagation, mirroring ConfirmDialog).
  // While the activation gate is up we bow out so IT handles its own Escape.
  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (activateTarget !== null) return;
      e.stopPropagation();
      onClose();
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [open, activateTarget, onClose]);

  if (!open) return null;

  const profileNames = store?.profiles.map((p) => p.name) ?? [];

  const handleActivated = (activation: ProfileActivation): void => {
    void refresh();
    onActivated?.(activation);
  };

  return (
    <div
      className="dialog-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="dialog-card ai-assets-card" role="dialog" aria-label="Worktree AI contexts">
        <div className="shortcut-header">
          <h2 className="dialog-title shortcut-title">Worktree AI contexts</h2>
          <div className="asset-header-actions">
            <button
              type="button"
              className="btn-secondary settings-toggle-btn"
              disabled={loading}
              onClick={() => void refresh()}
            >
              {loading ? 'Refreshing…' : 'Refresh'}
            </button>
            <button
              type="button"
              className="btn-icon shortcut-close"
              aria-label="Close"
              title="Close"
              onClick={onClose}
            >
              {'×'}
            </button>
          </div>
        </div>

        <p className="settings-section-desc">
          Each worktree carries its own instruction files. Activating a profile previews the
          per-file changes before anything is written.
        </p>

        {error !== null && (
          <div className="error-banner" role="alert">
            {error}
          </div>
        )}

        {rows === null ? (
          <p className="settings-ai-status">Loading worktrees…</p>
        ) : rows.length === 0 ? (
          <p className="settings-ai-status">No worktrees.</p>
        ) : (
          <ul className="asset-list">
            {rows.map((row) => (
              <li className="asset-row wtctx-row" key={row.worktreeKey}>
                <div className="asset-row-main">
                  <div className="asset-row-head">
                    <span className="asset-row-label">{row.name}</span>
                    {row.valid && (
                      <span className="wtctx-branch mono" title={row.branch ?? 'detached HEAD'}>
                        {row.branch ?? 'detached'}
                      </span>
                    )}
                    {rowBadges(row).map((b) => (
                      <span
                        key={b.label}
                        className={`branch-badge ${b.intent}`}
                        title={b.title ?? b.label}
                      >
                        {b.label}
                      </span>
                    ))}
                  </div>
                  <span className="asset-row-path mono">{row.absPath}</span>
                  <div className="asset-row-head wtctx-status">
                    {row.activeProfile !== null ? (
                      <span className="asset-chip asset-chip-active">
                        active: {row.activeProfile}
                      </span>
                    ) : (
                      <span className="asset-chip asset-chip-muted">no profile</span>
                    )}
                    {row.activatable && row.driftedCount > 0 && (
                      <span className="asset-chip asset-chip-drifted">
                        {row.driftedCount} drifted
                      </span>
                    )}
                    {row.activatable && row.missingCount > 0 && (
                      <span className="asset-chip asset-chip-missing">
                        {row.missingCount} missing
                      </span>
                    )}
                  </div>
                  {!row.activatable && row.blockedReason !== null && (
                    <span className="wtctx-blocked" title={row.blockedReason}>
                      {row.blockedReason}
                    </span>
                  )}
                </div>
                <div className="asset-row-actions wtctx-actions">
                  <select
                    className="dialog-input wtctx-select"
                    aria-label={`Profile for ${row.name}`}
                    disabled={!row.activatable || profileNames.length === 0}
                    value={selection[row.worktreeKey] ?? ''}
                    onChange={(e) =>
                      setSelection((prev) => ({ ...prev, [row.worktreeKey]: e.target.value }))
                    }
                  >
                    {profileNames.length === 0 && (
                      <option value="" disabled>
                        No profiles
                      </option>
                    )}
                    {profileNames.map((n) => (
                      <option key={n} value={n}>
                        {n}
                      </option>
                    ))}
                  </select>
                  <button
                    type="button"
                    className="btn-secondary settings-toggle-btn"
                    disabled={
                      !row.activatable ||
                      profileNames.length === 0 ||
                      (selection[row.worktreeKey] ?? '') === ''
                    }
                    title={row.activatable ? undefined : row.blockedReason ?? undefined}
                    onClick={() => {
                      const profile = selection[row.worktreeKey] ?? '';
                      if (profile === '') return;
                      setActivateTarget({ worktreeKey: row.worktreeKey, profile });
                    }}
                  >
                    Activate…
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}

        {profileNames.length === 0 && store !== null && (
          <p className="settings-ai-status">
            No context profiles yet — create one in the AI Assets panel first.
          </p>
        )}
      </div>

      {/* P31 SAFETY GATE: the per-target diff preview + explicit confirm is the
          ONLY path that writes files into the target worktree. */}
      <ProfileActivateDialog
        open={activateTarget !== null}
        repoId={repoId}
        name={activateTarget?.profile ?? null}
        worktreeName={activateTarget?.worktreeKey ?? null}
        onClose={() => setActivateTarget(null)}
        onActivated={handleActivated}
      />
    </div>
  );
}
