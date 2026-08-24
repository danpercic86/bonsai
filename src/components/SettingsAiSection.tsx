/**
 * P68g §2.3 — Settings → AI → "Assistance": the enable switch, the autonomy
 * choice and the three CLI-status branches.
 *
 * The autonomy disclosure sits AT THE POINT OF CHOICE (V7): both radios carry an
 * always-visible hint, because "Resolve automatically" writes to the user's files
 * and stages them with NO review step (audit H1/M2) and a consent dialog seen once
 * months ago is not where that belongs.
 *
 * P69j: re-skinned onto the canonical row (UI §5.1). Two changes only, both
 * structural — the checkbox is a switch (a CSS skin over the SAME native
 * checkbox, so the consent flow and every `getByRole('checkbox')` are untouched),
 * and the autonomy fieldset became a `role="radiogroup"` named by the row label.
 * It STAYS a radio group by contract (§5.2): each option needs a sentence.
 * **Every string in this file is frozen (UI §8) — layout moved, words did not.**
 */
import type { AiAutonomy, AiAvailability, UiSettingsPatch } from '../ipc';
import { SettingsGroup } from './settings/SettingsGroup';
import { SettingsRow } from './settings/SettingsRow';
import { SettingsSwitchRow } from './settings/SettingsSwitchRow';
import { settingsRowLabelId } from './settings/settingsCatalog';

const ENABLED = 'ai.enabled';
const AUTONOMY = 'ai.conflict-resolution';

export interface SettingsAiSectionProps {
  aiEnabled: boolean;
  aiConflictAutonomy: AiAutonomy;
  /** `aiEnabled && aiConsented` — gates the autonomy choice. */
  aiActive: boolean;
  /** CLI health status; `null` while App is probing (never a dead control). */
  aiAvailability: AiAvailability | null;
  /** Enable/disable, already consent-gated by the container. */
  onToggleEnabled(checked: boolean): void;
  onChange(patch: UiSettingsPatch): void;
}

export function SettingsAiSection({
  aiEnabled,
  aiConflictAutonomy,
  aiActive,
  aiAvailability,
  onToggleEnabled,
  onChange,
}: SettingsAiSectionProps) {
  return (
    <SettingsGroup id="ai-assistance" title="Assistance">
      <p className="settings-group-lead">
        Resolve merge conflicts with the local Claude Code CLI, under your Claude subscription.
        Claude can read files in this repository while it works — see Repository access below.
      </p>

      <SettingsSwitchRow id={ENABLED} checked={aiEnabled} onChange={onToggleEnabled} />

      {/* Stacked (UI §5.1): each option carries a sentence, so the choice needs
          the full row width rather than the 200px control cell. */}
      <SettingsRow id={AUTONOMY} stacked disabled={!aiActive}>
        <div
          className="settings-radio-choices"
          role="radiogroup"
          aria-labelledby={settingsRowLabelId(AUTONOMY)}
        >
          <label className="settings-radio">
            <input
              type="radio"
              name="ai-autonomy"
              checked={aiConflictAutonomy === 'proposeReview'}
              disabled={!aiActive}
              aria-describedby={
                aiActive
                  ? 'ai-autonomy-propose-hint'
                  : 'ai-autonomy-propose-hint ai-autonomy-disabled-hint'
              }
              onChange={() => onChange({ aiConflictAutonomy: 'proposeReview' })}
            />
            <span>Propose &amp; review</span>
          </label>
          {/* Both hints are ALWAYS visible, not switched on the selected value: the
              consequence has to be readable BEFORE the choice is made (V7). */}
          <p className="settings-radio-hint" id="ai-autonomy-propose-hint">
            Each result opens as a proposal. Nothing is written to your files or staged until you
            apply it.
          </p>
          <label className="settings-radio">
            <input
              type="radio"
              name="ai-autonomy"
              checked={aiConflictAutonomy === 'autoResolve'}
              disabled={!aiActive}
              aria-describedby={
                aiActive
                  ? 'ai-autonomy-auto-hint'
                  : 'ai-autonomy-auto-hint ai-autonomy-disabled-hint'
              }
              onChange={() => onChange({ aiConflictAutonomy: 'autoResolve' })}
            />
            <span>Resolve automatically</span>
          </label>
          <p className="settings-radio-hint" id="ai-autonomy-auto-hint">
            Marker-free results are written to your files and staged for you, with no review step.
            Results that still contain conflict markers open as proposals instead.
          </p>
        </div>
      </SettingsRow>

      {!aiActive && (
        <p className="settings-group-lead" id="ai-autonomy-disabled-hint">
          {aiEnabled
            ? 'Turn “Enable AI features” off and on again to confirm access.'
            : 'Turn on “Enable AI features” above to change this.'}
        </p>
      )}

      {/* UI §1.3 #43: unchanged. */}
      {aiAvailability === null ? (
        <p className="settings-ai-status">Checking for the Claude Code CLI…</p>
      ) : aiAvailability.installed ? (
        <p className="settings-ai-status settings-ai-status-ok">{aiAvailability.detail}</p>
      ) : (
        <p className="settings-ai-status settings-ai-status-warn" role="note">
          Claude Code CLI not found on PATH — install it and log in to use AI features
        </p>
      )}
    </SettingsGroup>
  );
}
