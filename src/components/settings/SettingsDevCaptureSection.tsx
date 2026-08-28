// P91 §2/§4 — group 2 "What is captured".
//
// One `<fieldset disabled>` around the whole group (§2.2): meaningless with
// capture off, so it is DISABLED (not hidden — no layout jump, preview before
// commit). The lead sentence explains the dim and the fieldset points
// `aria-describedby` at it ONLY while disabled. The sensitive raw-names row
// (§4.2) carries a leading `--warning` bar; enabling it is confirm-gated (§4.3),
// turning it off is silent.

import { useId } from 'react';

import type { DevSettings, LogLevel } from '../../ipc';
import { settingsRowHelpId, settingsRowLabelId } from './settingsCatalog';
import { SettingsGroup } from './SettingsGroup';
import { SettingsRow } from './SettingsRow';
import { SettingsSegmented } from './SettingsSegmented';
import { SettingsSwitch } from './SettingsSwitch';
import { SettingsSwitchRow } from './SettingsSwitchRow';

const LEVEL = 'dev.level';
const RAW = 'dev.include-raw-names';
const RAW_NOTE = 'dev.include-raw-names-note';

const LEVEL_OPTIONS: readonly { value: LogLevel; label: string }[] = [
  { value: 'info', label: 'Info' },
  { value: 'debug', label: 'Debug' },
  { value: 'trace', label: 'Trace' },
];

export function SettingsDevCaptureSection({
  dev,
  onPatch,
  onRequestRawNames,
}: {
  dev: DevSettings;
  /** Patch one field of the whole `dev` struct. */
  onPatch(patch: Partial<DevSettings>): void;
  /** Turning raw names ON is confirm-gated — the container owns the dialog. */
  onRequestRawNames(): void;
}) {
  const enabled = dev.enabled;
  const leadId = useId();
  // Architect §5: Trace force-enables frame capture. The switch must not lie —
  // render it checked + disabled and swap its help for a note.
  const framesForced = dev.level === 'trace';
  const rawOn = dev.includeRawNames;

  return (
    <SettingsGroup id="dev-capture" title="What is captured">
      {!enabled && (
        <p className="settings-group-lead" id={leadId}>
          Turn on Dev mode to change what is recorded.
        </p>
      )}
      <fieldset
        className="settings-fieldset"
        disabled={!enabled}
        aria-describedby={!enabled ? leadId : undefined}
      >
        <SettingsRow id={LEVEL} disabled={!enabled}>
          <SettingsSegmented<LogLevel>
            name={LEVEL}
            value={dev.level}
            options={LEVEL_OPTIONS}
            labelledBy={settingsRowLabelId(LEVEL)}
            describedBy={settingsRowHelpId(LEVEL)}
            disabled={!enabled}
            onChange={(level) => onPatch({ level })}
          />
        </SettingsRow>

        <SettingsSwitchRow
          id="dev.capture-ipc"
          checked={dev.captureIpc}
          disabled={!enabled}
          onChange={(captureIpc) => onPatch({ captureIpc })}
        />

        <SettingsSwitchRow
          id="dev.capture-react"
          checked={dev.captureReact}
          disabled={!enabled}
          onChange={(captureReact) => onPatch({ captureReact })}
        />

        <SettingsSwitchRow
          id="dev.capture-frames"
          checked={framesForced ? true : dev.captureFrames}
          disabled={!enabled || framesForced}
          hint={
            framesForced ? (
              <p className="settings-row-note">Always on at the Trace detail level.</p>
            ) : undefined
          }
          onChange={(captureFrames) => onPatch({ captureFrames })}
        />

        {/* §4.2 — the sensitive row: leading --warning bar, value-tracking note,
            NO catalog help. Enabling is confirm-gated; turning it off is silent. */}
        <SettingsRow
          id={RAW}
          controlId={`${RAW}-input`}
          disabled={!enabled}
          className="settings-row--sensitive"
          hint={
            <p className="settings-row-note" id={RAW_NOTE}>
              {rawOn
                ? 'On: new log files contain your real branch, tag, file and repository names.'
                : 'Off: names appear as ref#3, path#7, repo#1.'}
            </p>
          }
        >
          <SettingsSwitch
            id={`${RAW}-input`}
            checked={rawOn}
            disabled={!enabled}
            describedBy={RAW_NOTE}
            onChange={(next) => {
              if (next) onRequestRawNames();
              else onPatch({ includeRawNames: false });
            }}
          />
        </SettingsRow>

        {rawOn && (
          <p className="settings-group-note dev-warning-bar">
            <span className="dev-warning-glyph" aria-hidden="true">
              ⚠
            </span>
            Raw names are on. Read an exported log before sending it to anyone.
          </p>
        )}
      </fieldset>
    </SettingsGroup>
  );
}
