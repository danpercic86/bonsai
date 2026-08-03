import { minutesLabel } from './workspaceUtils';
import type { BranchInfo, JobStatus } from '../ipc';

export interface WorkspaceToolbarProps {
  remoteOp: 'fetch' | 'pull' | 'push' | null;
  refreshing: boolean;
  mutating: boolean;
  statusLoading: boolean;
  graphLoading: boolean;
  canPullPush: boolean;
  aiEligible: boolean;
  aiPanelLoading: boolean;
  headBranch: BranchInfo | null;
  jobStatus: JobStatus[];
  jobNow: number;
  onFetch(): void;
  onPull(): void;
  onPush(): void;
  onWhatChanged(): void;
  onRefresh(): void;
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
  aiEligible,
  aiPanelLoading,
  headBranch,
  jobStatus,
  jobNow,
  onFetch,
  onPull,
  onPush,
  onWhatChanged,
  onRefresh,
}: WorkspaceToolbarProps) {
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
            disabled={refreshing || mutating}
            onClick={() => onFetch()}
            title="Fetch all remotes (Ctrl+Shift+F)"
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
            title="Pull (fast-forward only) (Ctrl+Shift+P)"
          >
            {remoteOp === 'pull' ? 'Pulling…' : '⇣ Pull'}
          </button>
          <button
            type="button"
            className="toolbar-btn"
            disabled={refreshing || mutating || !canPullPush}
            onClick={() => onPush()}
            title={`${pushTitle} (Ctrl+Shift+U)`}
          >
            {remoteOp === 'push' ? 'Pushing…' : '↑ Push'}
          </button>
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
        </div>
        <button
          type="button"
          className="btn-icon toolbar-refresh"
          disabled={refreshing || statusLoading || graphLoading || mutating}
          onClick={() => onRefresh()}
          title="Refresh (Ctrl+R)"
          aria-label="Refresh"
        >
          {'⟳'}
        </button>
      </div>
      {(remoteOp !== null || refreshing) && <div className="header-progress" aria-hidden="true" />}
    </>
  );
}
