/**
 * P68g §1 — Settings → "AI runs": the eight AI-run knobs that shipped with NO UI at
 * all (settings-file-only), including `aiStreamLog`, whose absence left the AI
 * activity log's empty state pointing at a control that did not exist (V8).
 *
 * This file owns the frame (section, description, the single inert-when-off
 * `<fieldset>`) and the four stateless controls: the repository-access grant, the two
 * output toggles, and the bulk batch size. The four limit controls carry all the
 * sentinel logic and live in `SettingsAiLimits.tsx`.
 *
 * Clamping uses `settings/ranges.ts`, the same numbers Rust clamps with, so the value
 * shown, the value App holds and the value Rust stores are ONE value — there is
 * deliberately no second clamp implementation in here (§1.5).
 */
import type { UiSettingsPatch } from '../ipc';
import type { AiRunPrefs } from '../settings/aiRunPrefs';
import { NumberSlider } from './NumberSlider';
import { SettingsAiLimits } from './SettingsAiLimits';
import { AI_BULK_MAX_BYTES_MAX, AI_BULK_MAX_BYTES_MIN } from '../settings/ranges';

/** 1 KB = 1000 B exactly, so the byte clamps map to whole KB with no drift. */
const BULK_KB_MIN = AI_BULK_MAX_BYTES_MIN / 1000;
const BULK_KB_MAX = AI_BULK_MAX_BYTES_MAX / 1000;

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

  const readOnlyTools = aiConflictTools === 'readOnly';
  const toolsLabel = readOnlyTools ? 'Read-only' : 'No file access';
  const toolsOther = readOnlyTools ? 'No file access' : 'Read-only';
  const toolsName = `Repository access — currently ${toolsLabel}. Activate to switch to ${toolsOther}.`;

  return (
    <section className="settings-section">
      <h3 className="settings-section-title">AI runs</h3>
      <p className="settings-section-desc">
        Applies to conflict resolution with Claude. Changes take effect on the next run.
      </p>
      {/* P69d (UI §5.4): the gate note leads the group and is wired to the fieldset via
          aria-describedby, so the REASON the ten controls are inert is announced when
          focus reaches the group instead of being orphaned. It stays outside the
          fieldset so it is not dimmed by the 0.5 opacity (§1.2). The describedby is
          only set while the note exists — a dangling idref would be worse than none.
          Copy is unchanged pending the A3 sign-off. */}
      {!aiActive && (
        <p className="settings-hint" id="ai-run-gate-note">
          {'Turn on “Enable AI features” above to change these.'}
        </p>
      )}

      <fieldset
        className="settings-section-fields"
        disabled={!aiActive}
        aria-describedby={aiActive ? undefined : 'ai-run-gate-note'}
      >
        {/* 1 — repository access. It leads the section: it is the grant the user was
            never told about (audit M2), and its hint states the grant, not the flag. */}
        <div className="settings-row">
          <span className="settings-control-label">Repository access</span>
          <button
            type="button"
            className="btn-secondary settings-toggle-btn"
            aria-label={toolsName}
            title={toolsName}
            aria-describedby="settings-ai-tools-hint"
            onClick={() => onChange({ aiConflictTools: readOnlyTools ? 'none' : 'readOnly' })}
          >
            {toolsLabel}
          </button>
        </div>
        <p className="settings-hint" id="settings-ai-tools-hint">
          {readOnlyTools
            ? 'Claude can read, search and list files in this repository while it resolves a conflict — that is what lets it match your surrounding code. Anything it reads is sent to Anthropic. It cannot write files, stage anything, or run commands, and reads outside this repository are refused.'
            : "Claude sees only the conflicting versions of each file and nothing else in your repository. Resolutions are noticeably less accurate — this was Bonsai's behaviour before repository reads existed."}
        </p>

        {/* 2-3 — output. Checkbox 2 is the control the AI activity log's empty state
            already points at ("turn on \"Stream AI output\" in Settings"). */}
        <label className="settings-checkbox">
          <input
            type="checkbox"
            checked={aiStreamLog}
            aria-describedby="settings-ai-streamlog-hint"
            onChange={(e) => onChange({ aiStreamLog: e.target.checked })}
          />
          <span>Stream AI output</span>
        </label>
        <p className="settings-hint" id="settings-ai-streamlog-hint">
          Show every line the Claude CLI prints in the AI activity dock. With this off, the dock
          still shows status, cost, which files Claude read, and any refused read — just not the rest
          of the output.
        </p>
        <label className="settings-checkbox">
          <input
            type="checkbox"
            checked={aiIncludePartialMessages}
            aria-describedby="settings-ai-partial-hint"
            onChange={(e) => onChange({ aiIncludePartialMessages: e.target.checked })}
          />
          <span>Stream partial replies</span>
        </label>
        <p className="settings-hint" id="settings-ai-partial-hint">
          {
            "Show Claude's text as it is typed, instead of when each turn finishes. More output in the dock; partial text is never applied to a file."
          }
        </p>

        {/* 4-7 — the limits, and the two LOCKED "off" defaults. */}
        <SettingsAiLimits
          aiIdleTimeoutSecs={aiIdleTimeoutSecs}
          aiHardCapSecs={aiHardCapSecs}
          aiMaxTurns={aiMaxTurns}
          aiMaxBudgetUsd={aiMaxBudgetUsd}
          onChange={onChange}
        />

        <h4 className="settings-subsection-title">Bulk resolve</h4>
        {/* 8 — bytes shown as KB (1 KB = 1000 B), so the clamps land on whole KB. */}
        <NumberSlider
          id="settings-ai-bulk"
          label="Batch size"
          value={Math.round(aiBulkMaxBytes / 1000)}
          min={BULK_KB_MIN}
          max={BULK_KB_MAX}
          unit="KB"
          describedBy="settings-ai-bulk-hint"
          onChange={(v) => onChange({ aiBulkMaxBytes: v * 1000 })}
        />
        <p className="settings-hint" id="settings-ai-bulk-hint">
          The most text Bonsai puts into one bulk run. A larger merge is split into several runs, one
          after another — never truncated.
        </p>
      </fieldset>
    </section>
  );
}
