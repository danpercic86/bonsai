// P69g — the "General" category page: background activity + external tools.
//
// Re-skinned onto the canonical row (UI §5.1): the two checkboxes are
// `SettingsSwitchRow` (the row+switch pairing, including the `{rowId}-input` id
// and the `{rowId}-help` description — spelling it out here was verbatim what
// that component does), the two intervals are `NumberSlider` inside a row that
// owns the label/help/reset cells, and the section-level paragraph is gone — its
// content is now per-row help, which is what `aria-describedby` can point at.
//
// Every label, help string and `↺` descriptor comes from the CATALOG via the row
// id; nothing here restates them.

import { NumberSlider } from '../../NumberSlider';
import { SettingsExternalToolsSection } from '../../SettingsExternalToolsSection';
import {
  AUTO_FETCH_INTERVAL_MAX,
  AUTO_FETCH_INTERVAL_MIN,
  HEALTH_REFRESH_INTERVAL_MAX,
  HEALTH_REFRESH_INTERVAL_MIN,
} from '../../../settings/ranges';
import { settingsRowHelpId, settingsRowLabelId } from '../settingsCatalog';
import { SettingsGroup } from '../SettingsGroup';
import { SettingsRow } from '../SettingsRow';
import { SettingsSegmented } from '../SettingsSegmented';
import { SettingsSwitchRow } from '../SettingsSwitchRow';
import { useSettingsActions, useSettingsValues } from '../SettingsContext';
import type { PrimaryCommitAction } from '../../../ipc';

const AUTO_FETCH = 'general.auto-fetch';
const FETCH_INTERVAL = 'general.fetch-interval';
const AUTO_REFRESH = 'general.auto-refresh';
const REFRESH_INTERVAL = 'general.refresh-interval';
const PRIMARY_COMMIT_ACTION = 'general.primary-commit-action';

export function GeneralCategory() {
  const { autoFetch, healthRefresh, terminalCommand, editorCommand, primaryCommitAction } =
    useSettingsValues();
  const { change } = useSettingsActions();

  return (
    <>
      <SettingsGroup id="general-background" title="Background activity">
        <SettingsSwitchRow
          id={AUTO_FETCH}
          checked={autoFetch.enabled}
          onChange={(enabled) => change({ autoFetch: { ...autoFetch, enabled } })}
        />

        <SettingsRow
          id={FETCH_INTERVAL}
          controlId="settings-auto-fetch-interval"
          disabled={!autoFetch.enabled}
        >
          <NumberSlider
            id="settings-auto-fetch-interval"
            /* P69d / UI §5.3.7: two rows both labelled "Interval" gave two controls
               in one dialog the SAME accessible name. Ids are unchanged. */
            label="Fetch every"
            value={autoFetch.intervalMinutes}
            min={AUTO_FETCH_INTERVAL_MIN}
            max={AUTO_FETCH_INTERVAL_MAX}
            unit="minutes"
            disabled={!autoFetch.enabled}
            describedBy={settingsRowHelpId(FETCH_INTERVAL)}
            onChange={(intervalMinutes) => change({ autoFetch: { ...autoFetch, intervalMinutes } })}
          />
        </SettingsRow>

        <SettingsSwitchRow
          id={AUTO_REFRESH}
          checked={healthRefresh.enabled}
          onChange={(enabled) => change({ healthRefresh: { ...healthRefresh, enabled } })}
        />

        <SettingsRow
          id={REFRESH_INTERVAL}
          controlId="settings-health-refresh-interval"
          disabled={!healthRefresh.enabled}
        >
          <NumberSlider
            id="settings-health-refresh-interval"
            label="Refresh every"
            value={healthRefresh.intervalMinutes}
            min={HEALTH_REFRESH_INTERVAL_MIN}
            max={HEALTH_REFRESH_INTERVAL_MAX}
            unit="minutes"
            disabled={!healthRefresh.enabled}
            describedBy={settingsRowHelpId(REFRESH_INTERVAL)}
            onChange={(intervalMinutes) =>
              change({ healthRefresh: { ...healthRefresh, intervalMinutes } })
            }
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup id="general-committing" title="Committing">
        <SettingsRow id={PRIMARY_COMMIT_ACTION}>
          <SettingsSegmented<PrimaryCommitAction>
            name={PRIMARY_COMMIT_ACTION}
            value={primaryCommitAction}
            labelledBy={settingsRowLabelId(PRIMARY_COMMIT_ACTION)}
            describedBy={settingsRowHelpId(PRIMARY_COMMIT_ACTION)}
            options={[
              { value: 'commit', label: 'Commit' },
              { value: 'commitPush', label: 'Commit & Push' },
            ]}
            onChange={(next) => change({ primaryCommitAction: next })}
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsExternalToolsSection
        terminalCommand={terminalCommand}
        editorCommand={editorCommand}
        onChange={change}
      />
    </>
  );
}
