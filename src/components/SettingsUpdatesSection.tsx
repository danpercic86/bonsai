// P42b: presentational Settings "Updates" block (mirrors SettingsMcpSection).
// Shows the current app version, a manual "Check for updates" button with inline
// result text (checking / up to date / vX available / error), and the
// auto-check-on-launch toggle. All state + IPC live in App/useUpdateController;
// this component only renders and calls back.

import type { UpdateUiState } from '../hooks/useUpdateController';

export interface SettingsUpdatesSectionProps {
  /** App version from the last check; `null` until one resolves. */
  currentVersion: string | null;
  autoCheckUpdates: boolean;
  onToggleAutoCheck(value: boolean): void;
  /** Shared update state — drives the inline result line. */
  checkState: UpdateUiState;
  /** Run a manual (non-silent) check. */
  onCheck(): void;
  /** Open the UpdateDialog (release notes + download flow). */
  onOpenDialog(): void;
}

function ResultLine({
  state,
  onOpenDialog,
}: {
  state: UpdateUiState;
  onOpenDialog(): void;
}) {
  switch (state.status) {
    case 'checking':
      return <p className="settings-ai-status">Checking for updates…</p>;
    case 'upToDate':
      return <p className="settings-ai-status settings-ai-status-ok">You&apos;re up to date.</p>;
    case 'available':
    case 'downloading':
    case 'readyToRestart':
      return (
        <p className="settings-ai-status settings-ai-status-ok">
          Version {state.info.version} is available.{' '}
          <button type="button" className="settings-update-link" onClick={onOpenDialog}>
            What&apos;s new
          </button>
        </p>
      );
    case 'error':
      return (
        <p className="settings-ai-status settings-ai-status-warn" role="alert">
          {state.message}
        </p>
      );
    default:
      return null;
  }
}

export function SettingsUpdatesSection({
  currentVersion,
  autoCheckUpdates,
  onToggleAutoCheck,
  checkState,
  onCheck,
  onOpenDialog,
}: SettingsUpdatesSectionProps) {
  return (
    <section className="settings-section">
      <h3 className="settings-section-title">Updates</h3>
      <p className="settings-section-desc">
        Bonsai can check for new releases and download them for you. Updates are never installed
        without your confirmation.
      </p>
      <div className="settings-row">
        <span className="settings-control-label">Current version</span>
        <span className="mono">{currentVersion ?? '—'}</span>
      </div>
      <div className="settings-row">
        <span className="settings-control-label">Check for updates</span>
        <button
          type="button"
          className="btn-secondary settings-toggle-btn"
          disabled={checkState.status === 'checking'}
          onClick={onCheck}
        >
          {checkState.status === 'checking' ? 'Checking…' : 'Check for updates'}
        </button>
      </div>
      <ResultLine state={checkState} onOpenDialog={onOpenDialog} />
      <label className="settings-checkbox">
        <input
          type="checkbox"
          checked={autoCheckUpdates}
          onChange={(e) => onToggleAutoCheck(e.target.checked)}
        />
        <span>Automatically check for updates on launch</span>
      </label>
    </section>
  );
}
