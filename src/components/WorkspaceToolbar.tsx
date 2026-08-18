import { useRef, useState } from 'react';
import { minutesLabel } from './workspaceUtils';
import { ContextMenu } from './ContextMenu';
import type { ContextMenuItem } from './ContextMenu';
import { FolderOpenIcon } from './menuIcons';
import type { BranchInfo, JobStatus } from '../ipc';
import { shortcutLabel } from '../utils/platform';

export interface WorkspaceToolbarProps {
  remoteOp: 'fetch' | 'pull' | 'push' | null;
  refreshing: boolean;
  mutating: boolean;
  statusLoading: boolean;
  graphLoading: boolean;
  canPullPush: boolean;
  /** P37b: force-push is enabled only when the current branch has an upstream. */
  canForcePush: boolean;
  aiEligible: boolean;
  aiPanelLoading: boolean;
  headBranch: BranchInfo | null;
  jobStatus: JobStatus[];
  jobNow: number;
  onFetch(): void;
  onPull(): void;
  onPush(): void;
  /** P37b: opens the force-push-with-lease confirm dialog. */
  onForcePush(): void;
  onWhatChanged(): void;
  /** P55c: opens the "Ask Bonsai to…" natural-language input (gated aiEligible).
   *  Read-only until the proposed op's own confirm dialog. */
  onAskBonsai(): void;
  /** P60c: one-click undo — describes the last op, then opens the UndoDialog.
   *  Enabled on a born repo (the dialog explains when there's nothing to undo). */
  onUndo(): void;
  /** P38: opens the HEAD reflog overlay (recovery net). Enabled on a born repo. */
  onViewHeadReflog(): void;
  /** True once HEAD points at a commit (not unborn) — gates the reflog button. */
  headBorn: boolean;
  onRefresh(): void;
  /** P49b: the shared "Open externally" trio for the current repo path, already
   *  bound to RepoWorkspace's launch handlers. Rendered behind an always-enabled
   *  dropdown button (external launches never touch git state, so no op-gating). */
  externalItems: ContextMenuItem[];
}

/** P3e: the top workspace toolbar (fetch/pull/push + auto-fetch readout + AI
 *  digest entry + refresh). Presentational — all handlers/state are threaded in
 *  from RepoWorkspace so behavior is identical to the inline block it replaced. */
export function WorkspaceToolbar({
  remoteOp,
  refreshing,
  mutating,
  statusLoading,
  graphLoading,
  canPullPush,
  canForcePush,
  aiEligible,
  aiPanelLoading,
  headBranch,
  jobStatus,
  jobNow,
  onFetch,
  onPull,
  onPush,
  onForcePush,
  onWhatChanged,
  onAskBonsai,
  onUndo,
  onViewHeadReflog,
  headBorn,
  onRefresh,
  externalItems,
}: WorkspaceToolbarProps) {
  // P37b: anchor for the Push caret dropdown (positioned at the caret's rect).
  const caretRef = useRef<HTMLButtonElement>(null);
  const [pushMenu, setPushMenu] = useState<{ x: number; y: number } | null>(null);
  // P49b: anchor + open-state for the "Open externally" dropdown (same idiom).
  const externalRef = useRef<HTMLButtonElement>(null);
  const [externalMenu, setExternalMenu] = useState<{ x: number; y: number } | null>(null);

  const openPushMenu = () => {
    const rect = caretRef.current?.getBoundingClientRect();
    if (rect === undefined) return;
    setPushMenu({ x: rect.left, y: rect.bottom + 2 });
  };

  const openExternalMenu = () => {
    const rect = externalRef.current?.getBoundingClientRect();
    if (rect === undefined) return;
    setExternalMenu({ x: rect.right, y: rect.bottom + 2 });
  };

  const pushTitle =
    headBranch === null
      ? 'Push'
      : headBranch.upstream !== null
        ? `Push ${headBranch.name} to ${headBranch.upstream}`
        : `Push ${headBranch.name} to origin/${headBranch.name} and set upstream`;

  // P30 D11: small muted auto-fetch readout next to the Fetch control —
  // "Fetched Xm ago", or the backoff notice with the retry estimate.
  const autoFetchStatus = jobStatus.find((s) => s.job === 'autoFetch');
  let autoFetchReadout: { text: string; title: string } | null = null;
  if (autoFetchStatus !== undefined && autoFetchStatus.enabled) {
    if (autoFetchStatus.inBackoff) {
      const retry =
        autoFetchStatus.nextRunMs !== null
          ? ` — retrying in ${minutesLabel(autoFetchStatus.nextRunMs - jobNow)}`
          : '';
      autoFetchReadout = {
        text: `Auto-fetch paused${retry}`,
        title: autoFetchStatus.lastError ?? 'Auto-fetch is failing; retries are backed off',
      };
    } else if (autoFetchStatus.lastRunMs !== null && autoFetchStatus.lastOutcome !== null) {
      autoFetchReadout = {
        text: `Fetched ${minutesLabel(jobNow - autoFetchStatus.lastRunMs)} ago`,
        title: `Background auto-fetch — last outcome: ${autoFetchStatus.lastOutcome}`,
      };
    }
  }

  return (
    <>
      <div className="workspace-toolbar">
        <div className="toolbar-center">
          <button
            type="button"
            className="toolbar-btn"
            disabled={refreshing || mutating || !headBorn}
            onClick={() => onUndo()}
            title="Undo the last operation (commit, merge, rebase, reset…)"
          >
            ↶ Undo
          </button>
          <button
            type="button"
            className="toolbar-btn"
            disabled={refreshing || mutating}
            onClick={() => onFetch()}
            title={`Fetch all remotes (${shortcutLabel('Mod+Shift+F')})`}
          >
            {remoteOp === 'fetch' ? 'Fetching…' : '↓ Fetch'}
          </button>
          {autoFetchReadout !== null && (
            <span className="toolbar-job-status" title={autoFetchReadout.title}>
              {autoFetchReadout.text}
            </span>
          )}
          <button
            type="button"
            className="toolbar-btn"
            disabled={refreshing || mutating || !canPullPush}
            onClick={() => onPull()}
            title={`Pull (fast-forward only) (${shortcutLabel('Mod+Shift+P')})`}
          >
            {remoteOp === 'pull' ? 'Pulling…' : '⇣ Pull'}
          </button>
          <span className="toolbar-split">
            <button
              type="button"
              className="toolbar-btn toolbar-split-main"
              disabled={refreshing || mutating || !canPullPush}
              onClick={() => onPush()}
              title={`${pushTitle} (${shortcutLabel('Mod+Shift+U')})`}
            >
              {remoteOp === 'push' ? 'Pushing…' : '↑ Push'}
            </button>
            <button
              ref={caretRef}
              type="button"
              className="toolbar-btn toolbar-caret"
              disabled={refreshing || mutating || !canForcePush}
              onClick={() => openPushMenu()}
              aria-label="More push actions"
              aria-haspopup="menu"
              title={
                canForcePush
                  ? 'More push actions'
                  : "Force-push needs a branch with an upstream."
              }
            >
              ▾
            </button>
          </span>
          {aiEligible && (
            <button
              type="button"
              className="toolbar-btn"
              disabled={aiPanelLoading}
              onClick={() => onWhatChanged()}
              title="AI digest of what changed over a range (read-only)"
            >
              ✨ What changed…
            </button>
          )}
          {aiEligible && (
            <button
              type="button"
              className="toolbar-btn"
              onClick={() => onAskBonsai()}
              title="Ask Bonsai to perform a safe git operation from a natural-language request (previewed + confirmed)"
            >
              ✨ Ask…
            </button>
          )}
          <button
            type="button"
            className="toolbar-btn"
            disabled={!headBorn}
            onClick={() => onViewHeadReflog()}
            title="View the HEAD reflog (recover prior positions after reset/rebase/amend)"
          >
            ↺ Reflog
          </button>
        </div>
        <div className="toolbar-right">
          <button
            ref={externalRef}
            type="button"
            className="btn-icon toolbar-external"
            onClick={() => openExternalMenu()}
            title="Open this repository externally (terminal / file manager / editor)"
            aria-label="Open externally"
            aria-haspopup="menu"
          >
            <span className="toolbar-external-icon" aria-hidden="true">
              <FolderOpenIcon />
            </span>
          </button>
          <button
            type="button"
            className="btn-icon toolbar-refresh"
            disabled={refreshing || statusLoading || graphLoading || mutating}
            onClick={() => onRefresh()}
            title={`Refresh (${shortcutLabel('Mod+R')})`}
            aria-label="Refresh"
          >
            {'⟳'}
          </button>
        </div>
      </div>
      {(remoteOp !== null || refreshing) && <div className="header-progress" aria-hidden="true" />}
      {pushMenu !== null && (
        <ContextMenu
          x={pushMenu.x}
          y={pushMenu.y}
          items={[
            {
              label: 'Force-push with lease…',
              onSelect: () => onForcePush(),
            },
          ]}
          onClose={() => setPushMenu(null)}
        />
      )}
      {externalMenu !== null && (
        <ContextMenu
          x={externalMenu.x}
          y={externalMenu.y}
          items={externalItems}
          onClose={() => setExternalMenu(null)}
        />
      )}
    </>
  );
}
