// P69f §1.1 — the "General" category page: background activity + external tools.
//
// The Background-jobs markup was moved VERBATIM out of SettingsPanel.tsx; only
// the value/callback sources changed (props → SettingsContext). The external
// tools row keeps its existing leaf-section props (§2.3).

import { NumberSlider } from '../../NumberSlider';
import { SettingsExternalToolsSection } from '../../SettingsExternalToolsSection';
import {
  AUTO_FETCH_INTERVAL_MAX,
  AUTO_FETCH_INTERVAL_MIN,
  HEALTH_REFRESH_INTERVAL_MAX,
  HEALTH_REFRESH_INTERVAL_MIN,
} from '../../../settings/ranges';
import { useSettingsActions, useSettingsValues } from '../SettingsContext';

export function GeneralCategory() {
  const { autoFetch, healthRefresh, terminalCommand, editorCommand } = useSettingsValues();
  const { change } = useSettingsActions();

  return (
    <>
      {/* --- Background jobs (P30 §6) --- */}
      <section className="settings-section">
        <h3 className="settings-section-title">Background jobs</h3>
        <p className="settings-section-desc">
          Runs in the background for all open repositories. Auto-fetch never pulls, pushes, or
          prompts for credentials.
        </p>
        <label className="settings-checkbox">
          <input
            type="checkbox"
            checked={autoFetch.enabled}
            onChange={(e) => change({ autoFetch: { ...autoFetch, enabled: e.target.checked } })}
          />
          <span>Enable auto-fetch</span>
        </label>
        <NumberSlider
          id="settings-auto-fetch-interval"
          /* P69d / UI §5.3.7: two rows both labelled "Interval" gave two controls in
             one dialog the SAME accessible name. Ids are unchanged. */
          label="Fetch every"
          value={autoFetch.intervalMinutes}
          min={AUTO_FETCH_INTERVAL_MIN}
          max={AUTO_FETCH_INTERVAL_MAX}
          unit="minutes"
          disabled={!autoFetch.enabled}
          onChange={(v) => change({ autoFetch: { ...autoFetch, intervalMinutes: v } })}
        />
        <label className="settings-checkbox">
          <input
            type="checkbox"
            checked={healthRefresh.enabled}
            onChange={(e) =>
              change({ healthRefresh: { ...healthRefresh, enabled: e.target.checked } })
            }
          />
          <span>Refresh status &amp; health periodically</span>
        </label>
        <NumberSlider
          id="settings-health-refresh-interval"
          label="Refresh every"
          value={healthRefresh.intervalMinutes}
          min={HEALTH_REFRESH_INTERVAL_MIN}
          max={HEALTH_REFRESH_INTERVAL_MAX}
          unit="minutes"
          disabled={!healthRefresh.enabled}
          onChange={(v) => change({ healthRefresh: { ...healthRefresh, intervalMinutes: v } })}
        />
      </section>

      {/* --- External tools (P49b) --- */}
      <SettingsExternalToolsSection
        terminalCommand={terminalCommand}
        editorCommand={editorCommand}
        onChange={change}
      />
    </>
  );
}
