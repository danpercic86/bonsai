// P42b: presentational Settings "Updates" block (mirrors SettingsMcpSection).
// Shows the current app version, a manual "Check for updates" button with inline
// result text (checking / up to date / vX available / error), and the
// auto-check-on-launch toggle. All state + IPC live in App/useUpdateController;
// this component only renders and calls back.
//
// P69g: re-skinned onto the canonical row (UI §5.1) inside the About category's
// "Version" group — the group title replaces the old section heading, and the
// section paragraph is gone in favour of per-row help from the catalog. Like the
// external-tools section it keeps its own props (§2.3 leaf boundary) and supplies
// its own reset source, so it still renders standalone in its unit suite.

import { DEFAULT_UI_SETTINGS } from '../settings/defaults';
import { SettingsGroup } from './settings/SettingsGroup';
import { SettingsRow } from './settings/SettingsRow';
import { settingsRowHelpId } from './settings/settingsCatalog';
import type { UpdateUiState } from '../hooks/useUpdateController';
import { SettingsSwitch } from './settings/SettingsSwitch';

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

const AUTO_CHECK = 'about.auto-check-updates';

export function SettingsUpdatesSection({
  currentVersion,
  autoCheckUpdates,
  onToggleAutoCheck,
  checkState,
  onCheck,
  onOpenDialog,
}: SettingsUpdatesSectionProps) {
  return (
    <SettingsGroup id="about-version" title="Version">
      <SettingsRow id="about.version">
        <span className="mono">{currentVersion ?? '—'}</span>
      </SettingsRow>

      <SettingsRow id="about.check-updates">
        <button
          type="button"
          className="btn-secondary"
          disabled={checkState.status === 'checking'}
          onClick={onCheck}
        >
          {checkState.status === 'checking' ? 'Checking…' : 'Check for updates'}
        </button>
      </SettingsRow>
      <ResultLine state={checkState} onOpenDialog={onOpenDialog} />

      <SettingsRow
        id={AUTO_CHECK}
        controlId={`${AUTO_CHECK}-input`}
        reset={{
          isDefault: autoCheckUpdates === DEFAULT_UI_SETTINGS.autoCheckUpdates,
          onReset: () => onToggleAutoCheck(DEFAULT_UI_SETTINGS.autoCheckUpdates),
        }}
      >
        <SettingsSwitch
          id={`${AUTO_CHECK}-input`}
          checked={autoCheckUpdates}
          describedBy={settingsRowHelpId(AUTO_CHECK)}
          onChange={onToggleAutoCheck}
        />
      </SettingsRow>
    </SettingsGroup>
  );
}
