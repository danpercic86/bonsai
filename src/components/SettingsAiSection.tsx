/**
 * P68g §2.3 — Settings "AI assistance" section (own file, mirroring the other
 * extracted settings sections). The enable checkbox, the autonomy fieldset and the
 * three CLI-status branches moved here verbatim out of `SettingsPanel.tsx`; the
 * container got SHORTER rather than growing (V1).
 *
 * What is new here is the autonomy disclosure AT THE POINT OF CHOICE (V7): both
 * radios carry an always-visible hint, because "Resolve automatically" writes to the
 * user's files and stages them with NO review step (audit H1/M2) and a consent dialog
 * seen once months ago is not where that belongs. The radio is also renamed from
 * "Auto-resolve, then review" — which promised a review that does not happen — to
 * "Resolve automatically", which is what the bulk confirm dialog already calls it.
 */
import type { AiAutonomy, AiAvailability, UiSettingsPatch } from '../ipc';

export interface SettingsAiSectionProps {
  aiEnabled: boolean;
  aiConflictAutonomy: AiAutonomy;
  /** `aiEnabled && aiConsented` — gates the autonomy fieldset. */
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
    <section className="settings-section">
      <h3 className="settings-section-title">AI assistance</h3>
      <p className="settings-section-desc">
        Resolve merge conflicts with the local Claude Code CLI, under your Claude subscription.
        Claude can read files in this repository while it works — see Repository access below.
      </p>
      <label className="settings-checkbox">
        <input
          type="checkbox"
          checked={aiEnabled}
          onChange={(e) => onToggleEnabled(e.target.checked)}
        />
        <span>Enable AI features</span>
      </label>

      <fieldset className="settings-radio-group" disabled={!aiActive}>
        <legend className="settings-radio-legend">Conflict resolution</legend>
        <label className="settings-radio">
          <input
            type="radio"
            name="ai-autonomy"
            checked={aiConflictAutonomy === 'proposeReview'}
            disabled={!aiActive}
            aria-describedby="ai-autonomy-propose-hint"
            onChange={() => onChange({ aiConflictAutonomy: 'proposeReview' })}
          />
          <span>Propose &amp; review</span>
        </label>
        {/* Both hints are ALWAYS visible, not switched on the selected value: the
            consequence has to be readable BEFORE the choice is made (V7). */}
        <p className="settings-radio-hint" id="ai-autonomy-propose-hint">
          Each result opens as a proposal. Nothing is written to your files or staged until you apply
          it.
        </p>
        <label className="settings-radio">
          <input
            type="radio"
            name="ai-autonomy"
            checked={aiConflictAutonomy === 'autoResolve'}
            disabled={!aiActive}
            aria-describedby="ai-autonomy-auto-hint"
            onChange={() => onChange({ aiConflictAutonomy: 'autoResolve' })}
          />
          <span>Resolve automatically</span>
        </label>
        <p className="settings-radio-hint" id="ai-autonomy-auto-hint">
          Marker-free results are written to your files and staged for you, with no review step.
          Results that still contain conflict markers open as proposals instead.
        </p>
      </fieldset>

      {aiAvailability === null ? (
        <p className="settings-ai-status">Checking for the Claude Code CLI…</p>
      ) : aiAvailability.installed ? (
        <p className="settings-ai-status settings-ai-status-ok">{aiAvailability.detail}</p>
      ) : (
        <p className="settings-ai-status settings-ai-status-warn" role="note">
          Claude Code CLI not found on PATH — install it and log in to use AI features
        </p>
      )}
    </section>
  );
}
