// P69f §1.1 — the "About" category page: the welcome tour row (P43a) and the
// updates section (P42b). Both blocks were moved VERBATIM out of
// SettingsPanel.tsx; only the value/callback sources changed.

import { SettingsUpdatesSection } from '../../SettingsUpdatesSection';
import { useSettingsActions, useSettingsValues } from '../SettingsContext';

export function AboutCategory() {
  const { updateCurrentVersion, autoCheckUpdates, updateState } = useSettingsValues();
  const { change, showOnboarding, checkUpdate, openUpdateDialog } = useSettingsActions();

  return (
    <>
      {/* --- Getting started (P43a) --- */}
      <section className="settings-section">
        <h3 className="settings-section-title">Getting started</h3>
        <div className="settings-row">
          <span className="settings-control-label">First-run tour</span>
          <button
            type="button"
            className="btn-secondary settings-toggle-btn"
            onClick={showOnboarding}
          >
            {'Show welcome tour'}
          </button>
        </div>
      </section>

      {/* --- Updates (P42b) --- */}
      <SettingsUpdatesSection
        currentVersion={updateCurrentVersion}
        autoCheckUpdates={autoCheckUpdates}
        onToggleAutoCheck={(v) => change({ autoCheckUpdates: v })}
        checkState={updateState}
        onCheck={checkUpdate}
        onOpenDialog={openUpdateDialog}
      />
    </>
  );
}
