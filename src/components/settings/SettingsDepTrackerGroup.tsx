// P125-ui §7 — General → "Dependency checks": the tracker switch + its
// dependent interval slider (the Auto-fetch pair's shape).
//
// Turning ON never flips the switch directly: `DepConsentDialog` opens and the
// patch lands only on confirm (focus returns to the switch via ConfirmDialog's
// `useDialogFocus`). Turning OFF needs no confirmation (only cached results are
// deleted, and they rebuild) and reports a §12.14 success outcome BESIDE the
// row help. The outcome goes through General's ONE `useOutcomeNotes` channel,
// so the page keeps exactly one live region (P112 UA12) — the announcer the
// External tools group renders.
import { useEffect, useRef, useState } from 'react';

import { NumberSlider } from '../NumberSlider';
import { DEP_TRACKER_INTERVAL_MAX, DEP_TRACKER_INTERVAL_MIN } from '../../settings/ranges';
import type { DepTrackerSettings } from '../../ipc/types/depTracker';
import { DepConsentDialog } from '../deps/DepConsentDialog';
import { SETTINGS_GROUP_TITLE, SETTINGS_LEAD, SETTINGS_OFF_NOTE } from '../deps/depsCopy';
import { settingsRowHelpId } from './settingsCatalog';
import { SettingsGroup } from './SettingsGroup';
import { SettingsOutcomeNote } from './SettingsOutcomeNote';
import type { OutcomeNotes } from './useOutcomeNotes';
import { SettingsRow } from './SettingsRow';
import { SettingsSwitch } from './SettingsSwitch';
import { useSettingsActions, useSettingsValues } from './SettingsContext';
import { useSettingsSearch } from './SettingsSearchContext';

const DEP_TRACKER = 'general.dep-tracker';
const DEP_INTERVAL = 'general.dep-interval';
const INTERVAL_ID = 'settings-dep-interval';
/** The `{rowId}-input` convention `SettingsSwitchRow` uses. */
const SWITCH_ID = `${DEP_TRACKER}-input`;
export const DEP_OFF_NOTE_ID = 'general-dep-tracker-outcome';

export function SettingsDepTrackerGroup({
  notes,
  begin,
  report,
}: Pick<OutcomeNotes, 'notes' | 'begin' | 'report'>) {
  const { depTracker, configInitialFocus } = useSettingsValues();
  const { change } = useSettingsActions();
  const [consentOpen, setConsentOpen] = useState(false);
  // A search result is never a focus target (the GitConfigCategory HAZARD).
  const searching = useSettingsSearch() !== null;
  const sectionRef = useRef<HTMLElement>(null);
  const focusedOnce = useRef(false);

  // P125 deep link (the dialog's "Open Settings"): bring the group into view
  // and put focus on its switch, once per mount — SettingsShell remounts the
  // page per request, so a second deep link lands again.
  useEffect(() => {
    if (configInitialFocus !== 'dep-tracker' || searching || focusedOnce.current) return;
    focusedOnce.current = true;
    sectionRef.current?.scrollIntoView({ block: 'start' });
    document.getElementById(SWITCH_ID)?.focus({ preventScroll: true });
  }, [configInitialFocus, searching]);

  const patch = (next: DepTrackerSettings) => change({ depTracker: next });

  const onToggle = (enabled: boolean) => {
    // Turning on: the confirm opens and nothing starts yet — cancelling keeps
    // the previous outcome (`begin` runs on confirm, per useOutcomeNotes).
    if (enabled) {
      setConsentOpen(true);
      return;
    }
    begin(DEP_TRACKER);
    patch({ ...depTracker, enabled: false });
    report(DEP_TRACKER, 'success', SETTINGS_OFF_NOTE);
  };

  return (
    <SettingsGroup id="general-deps" title={SETTINGS_GROUP_TITLE} innerRef={sectionRef}>
      <p className="settings-group-lead">{SETTINGS_LEAD}</p>
      {/* Spelled out rather than `SettingsSwitchRow` so the ↺ (reset → Off)
          goes through `onToggle` too and reports the same outcome note. */}
      <SettingsRow
        id={DEP_TRACKER}
        controlId={SWITCH_ID}
        reset={{ isDefault: !depTracker.enabled, onReset: () => onToggle(false) }}
        hint={<SettingsOutcomeNote slot={DEP_TRACKER} id={DEP_OFF_NOTE_ID} outcome={notes.get(DEP_TRACKER) ?? null} />}
      >
        <SettingsSwitch
          id={SWITCH_ID}
          checked={depTracker.enabled}
          describedBy={`${settingsRowHelpId(DEP_TRACKER)} ${DEP_OFF_NOTE_ID}`}
          onChange={onToggle}
        />
      </SettingsRow>
      <SettingsRow id={DEP_INTERVAL} controlId={INTERVAL_ID} disabled={!depTracker.enabled}>
        <NumberSlider
          id={INTERVAL_ID}
          label="Check dependencies every"
          value={depTracker.intervalHours}
          min={DEP_TRACKER_INTERVAL_MIN}
          max={DEP_TRACKER_INTERVAL_MAX}
          unit="hours"
          disabled={!depTracker.enabled}
          describedBy={settingsRowHelpId(DEP_INTERVAL)}
          onChange={(intervalHours) => patch({ ...depTracker, intervalHours })}
        />
      </SettingsRow>
      <DepConsentDialog
        open={consentOpen}
        intervalHours={depTracker.intervalHours}
        onConfirm={() => {
          setConsentOpen(false);
          // Turning back on clears the "off" note and the announcer.
          begin(DEP_TRACKER);
          patch({ ...depTracker, enabled: true });
        }}
        onCancel={() => setConsentOpen(false)}
      />
    </SettingsGroup>
  );
}
