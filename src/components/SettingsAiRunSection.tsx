/**
 * P68g §1 — Settings → AI → "Runs": the eight AI-run knobs that shipped with NO UI
 * at all (settings-file-only), including `aiStreamLog`, whose absence left the AI
 * activity log's empty state pointing at a control that did not exist (V8).
 *
 * This file owns the frame (the single inert-when-off `<fieldset>`, the Runs and
 * Bulk resolve groups) and the four stateless controls: the repository-access
 * grant, the two output toggles, and the bulk batch size. The four limit controls
 * carry all the sentinel logic and live in `SettingsAiLimits.tsx`.
 *
 * Clamping uses `settings/ranges.ts`, the same numbers Rust clamps with, so the
 * value shown, the value App holds and the value Rust stores are ONE value — there
 * is deliberately no second clamp implementation in here (§1.5).
 *
 * P69j / UI §5.4 — the whole-fieldset-disabled pattern, kept and re-presented:
 *   * the `<fieldset disabled>` STAYS. It is the only mechanism that removes ten
 *     controls from the tab order in one place, and it spans all three groups;
 *   * the gate note LEADS the Runs group, directly under its title, and is the
 *     fieldset's `aria-describedby` target, so the reason the group is inert is
 *     announced on entry rather than found afterwards;
 *   * the `.55` dim lives on the individual ROWS, never on the `<fieldset>` —
 *     `opacity` is a group property, so dimming the fieldset would drag the very
 *     note that explains the state down with it. Hue never carries the state, and
 *     the switch knob POSITION still reads on-vs-off while disabled.
 *   * copy is unchanged: the §5.4 rewording is still pending the A3 sign-off.
 *
 * DENSITY: one geometry in both `cozy` and `compact` (UI §5.1 / D10) — do not add
 * a density variant to these rows.
 */
import type { AiConflictTools, UiSettingsPatch } from '../ipc';
import type { AiRunPrefs } from '../settings/aiRunPrefs';
import { NumberSlider } from './NumberSlider';
import { SettingsAiLimits } from './SettingsAiLimits';
import { SettingsGroup } from './settings/SettingsGroup';
import { SettingsRow } from './settings/SettingsRow';
import { SettingsSegmented } from './settings/SettingsSegmented';
import { SettingsSwitchRow } from './settings/SettingsSwitchRow';
import { settingsRowHelpId, settingsRowLabelId } from './settings/settingsCatalog';
import { AI_BULK_MAX_BYTES_MAX, AI_BULK_MAX_BYTES_MIN } from '../settings/ranges';

/** 1 KB = 1000 B exactly, so the byte clamps map to whole KB with no drift. */
const BULK_KB_MIN = AI_BULK_MAX_BYTES_MIN / 1000;
const BULK_KB_MAX = AI_BULK_MAX_BYTES_MAX / 1000;

const ACCESS = 'ai.repository-access';
const STREAM_LOG = 'ai.stream-output';
const STREAM_PARTIAL = 'ai.stream-partial';
const BULK = 'ai.bulk-batch-size';

export interface SettingsAiRunSectionProps {
  /** The eight values, read-only (see `AiRunPrefs`). */
  aiRun: AiRunPrefs;
  /** `aiEnabled && aiConsented`. One disabled `<fieldset>` makes every descendant
   *  inert and unfocusable, so nothing in here is half-live (§1.2). */
  aiActive: boolean;
  /** Every handler patches EXACTLY one field (App owns the debounced persist). */
  onChange(patch: UiSettingsPatch): void;
}

export function SettingsAiRunSection({ aiRun, aiActive, onChange }: SettingsAiRunSectionProps) {
  const {
    aiConflictTools,
    aiStreamLog,
    aiIncludePartialMessages,
    aiIdleTimeoutSecs,
    aiHardCapSecs,
    aiMaxTurns,
    aiMaxBudgetUsd,
    aiBulkMaxBytes,
  } = aiRun;

  const off = !aiActive;

  return (
    <fieldset
      className="settings-fieldset"
      disabled={off}
      /* Only while the note EXISTS — a dangling idref is worse than none. */
      aria-describedby={off ? 'ai-run-gate-note' : undefined}
    >
      <SettingsGroup id="ai-runs" title="Runs">
        {off && (
          <p className="settings-group-lead" id="ai-run-gate-note">
            {'Turn on “Enable AI features” above to change these.'}
          </p>
        )}

        {/* 1 — repository access. It leads the group: it is the grant the user was
            never told about (audit M2), and its hint states the grant, not the
            flag. UI §5.3 item 4: it was a self-labelling button reading its own
            current value, which is the riskiest place for that defect — it names a
            permission level. Now segmented, so the current value is SHOWN and the
            other value is the affordance. */}
        <SettingsRow
          id={ACCESS}
          disabled={off}
          hint={
            <p className="settings-row-note" id="settings-ai-tools-hint">
              {aiConflictTools === 'readOnly'
                ? 'Claude can read, search and list files in this repository while it resolves a conflict — that is what lets it match your surrounding code. Anything it reads is sent to Anthropic. It cannot write files, stage anything, or run commands, and reads outside this repository are refused.'
                : "Claude sees only the conflicting versions of each file and nothing else in your repository. Resolutions are noticeably less accurate — this was Bonsai's behaviour before repository reads existed."}
            </p>
          }
        >
          <SettingsSegmented<AiConflictTools>
            name="ai-conflict-tools"
            value={aiConflictTools}
            labelledBy={settingsRowLabelId(ACCESS)}
            describedBy="settings-ai-tools-hint"
            options={[
              { value: 'readOnly', label: 'Read-only' },
              { value: 'none', label: 'No file access' },
            ]}
            onChange={(v) => onChange({ aiConflictTools: v })}
          />
        </SettingsRow>

        {/* 2-3 — output. Switch 2 is the control the AI activity log's empty state
            already points at ("turn on \"Stream AI output\" in Settings"). */}
        <SettingsSwitchRow
          id={STREAM_LOG}
          checked={aiStreamLog}
          disabled={off}
          onChange={(v) => onChange({ aiStreamLog: v })}
        />
        <SettingsSwitchRow
          id={STREAM_PARTIAL}
          checked={aiIncludePartialMessages}
          disabled={off}
          onChange={(v) => onChange({ aiIncludePartialMessages: v })}
        />
      </SettingsGroup>

      {/* 4-7 — the limits, and the two LOCKED "off" defaults. */}
      <SettingsAiLimits
        aiIdleTimeoutSecs={aiIdleTimeoutSecs}
        aiHardCapSecs={aiHardCapSecs}
        aiMaxTurns={aiMaxTurns}
        aiMaxBudgetUsd={aiMaxBudgetUsd}
        disabled={off}
        onChange={onChange}
      />

      <SettingsGroup id="ai-bulk" title="Bulk resolve">
        {/* 8 — bytes shown as KB (1 KB = 1000 B), so the clamps land on whole KB. */}
        <SettingsRow id={BULK} controlId="settings-ai-bulk" disabled={off}>
          <NumberSlider
            bare
            id="settings-ai-bulk"
            label="Batch size"
            value={Math.round(aiBulkMaxBytes / 1000)}
            min={BULK_KB_MIN}
            max={BULK_KB_MAX}
            unit="KB"
            disabled={off}
            describedBy={settingsRowHelpId(BULK)}
            onChange={(v) => onChange({ aiBulkMaxBytes: v * 1000 })}
          />
        </SettingsRow>
      </SettingsGroup>
    </fieldset>
  );
}
