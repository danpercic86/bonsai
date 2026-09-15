// P69g — the "General" category page: background activity, committing, and the
// external tools Bonsai launches.
//
// P112 sub-increment 4: this is the CONTAINER for the external-tools group — it
// owns the scan hook and the page's one `useOutcomeNotes` instance and threads
// `notes` + `announce` + handlers down; `SettingsExternalToolsSection` stays a
// leaf that renders them (§2.3). The group that was deleted in sub-increment 1
// is back as a detected-tool PICKER rather than the free-text program fields it
// used to be: the settings are catalog ids, so a text box would let the user
// type a non-id, which the backend's `coerce_tool_id` turns into `''` — a
// control that silently discards input.
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

import { useMemo } from 'react';

import { NumberSlider } from '../../NumberSlider';
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
import { SettingsExternalToolsSection } from '../SettingsExternalToolsSection';
import { useExternalToolScan } from '../useExternalToolScan';
import { useOutcomeNotes } from '../useOutcomeNotes';
import { useOutcomeScrollCorrection } from '../useOutcomeScrollCorrection';
import { useSettingsActions, useSettingsValues } from '../SettingsContext';
import type { PrimaryCommitAction } from '../../../ipc';

const AUTO_FETCH = 'general.auto-fetch';
const FETCH_INTERVAL = 'general.fetch-interval';
const AUTO_REFRESH = 'general.auto-refresh';
const REFRESH_INTERVAL = 'general.refresh-interval';
const PRIMARY_COMMIT_ACTION = 'general.primary-commit-action';

export function GeneralCategory() {
  const { autoFetch, healthRefresh, primaryCommitAction, terminalTool, editorTool } =
    useSettingsValues();
  const { change, adoptToolSelection } = useSettingsActions();

  // ONE `useOutcomeNotes` for the whole page, and one announcer — rendered by
  // the section, as its last child (§16.4). Seven outcomes A–G key into it by
  // row id; none of them is a toast being relocated, they are new outcomes
  // routed into the channel P113 built.
  const { notes, announce, begin, report, announceOnly } = useOutcomeNotes();
  // §16.12: extended from the Dev slots to these three. `general.rescan-tools`
  // is the LAST row on this page, structurally identical to `dev.delete-logs`,
  // which is where the clipping failure was found — and focus is still on the
  // acting control, because Browse and Rescan are `aria-disabled`, not
  // `disabled`. All four of the hook's conditions are unchanged.
  useOutcomeScrollCorrection(notes);

  // The hook's whole permitted surface: the outcome channel, App's narrow
  // non-writing adopt (§16.16-5 — one field, never the whole struct) and the
  // ordinary settings write for the two picker keys. No launcher callback, no
  // `pushToast` (§16.5 / UA17).
  //
  // `change` is threaded IN rather than called from here because the hook owns
  // the order (its rule 6): an explicit selection has to clear that kind's owed
  // adopt, and an `onChangeTool` defined in this file could patch without
  // clearing — which is exactly the divergence the rule closes. BOTH of each
  // row's write controls therefore go through `tools.changeTool`: the list and
  // the `↺`, the latter through `SettingsRow`'s `reset` override rather than
  // the generic `resetRow`, which cannot see the owed adopt (rule 6 enumerates
  // the closed set; `catalog/reset.ts` makes the generic path refuse these keys).
  //
  // Memoised for the reader's sake, not the hook's: the hook destructures these
  // five and depends on them individually, so a fresh wrapper object per render
  // would be harmless. One bag with a stable identity is still the honest shape
  // for "this is everything the hook may do to the page".
  const toolOps = useMemo(
    () => ({ begin, report, announceOnly, adoptToolSelection, changeToolSelection: change }),
    [begin, report, announceOnly, adoptToolSelection, change],
  );
  const tools = useExternalToolScan(toolOps);

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
        scan={tools.scan}
        scanning={tools.scanning}
        scanFailedCold={tools.scanFailedCold}
        browsing={tools.browsing}
        terminalTool={terminalTool}
        editorTool={editorTool}
        notes={notes}
        announce={announce}
        onChangeTool={tools.changeTool}
        onBrowse={tools.browse}
        onRescan={tools.refresh}
      />
    </>
  );
}
