/**
 * P68g §1 — the "Limits" group of Settings → AI → Runs, in its own file because it
 * owns all of the section's LOGIC: the three sentinel switches and their resume
 * values.
 *
 * `0` is a MODE, not a number (V2). `aiHardCapSecs: 0` (a run has no deadline —
 * Cancel in the dock is the designed stop) and `aiMaxBudgetUsd: 0` (no spend cap)
 * are LOCKED user decisions; `aiIdleTimeoutSecs: 0` is reachable by hand-editing
 * the settings file (audit L4). Each is therefore an unchecked switch plus a
 * DISABLED numeric row that still shows what re-enabling would restore — a field
 * reading `0` looks broken, and a row that unmounts reads as "this feature does
 * not exist". None of these six rows carries a `↺`: their `0` is a documented mode
 * sentinel, so a reset would silently turn the feature on or off.
 *
 * P69j: re-skinned onto the canonical row (UI §5.1). The three sentinel hints are
 * STATEFUL — they restate the live field value and read differently while the
 * sentinel is off — so they ride the switch row's hint slot and are the switch's
 * whole description: the catalog's static `help` for those three switches was
 * deleted rather than stacked above them, because one row gets one help line and
 * the stateful sentence is the one that tells the truth. They live on the SWITCH
 * row, which is never dimmed by its own sentinel, so the sentence explaining a
 * disabled field is always at full contrast. Each number row keeps its OWN catalog
 * help and merely APPENDS the switch's note, so it is never described by somebody
 * else's sentence alone.
 *
 * The parent owns the single `<fieldset>` that makes the whole section inert when
 * AI is off (§1.2); `disabled` here is only the row-level dim that pairs with it.
 */
import { useState } from 'react';

import type { UiSettingsPatch } from '../ipc';
import { NumberSlider } from './NumberSlider';
import { SettingsGroup } from './settings/SettingsGroup';
import { SettingsRow } from './settings/SettingsRow';
import { SettingsSwitchRow } from './settings/SettingsSwitchRow';
import { settingsRowHelpId } from './settings/settingsCatalog';
import {
  AI_HARD_CAP_MAX,
  AI_HARD_CAP_MIN,
  AI_IDLE_TIMEOUT_MAX,
  AI_IDLE_TIMEOUT_MIN,
  AI_MAX_BUDGET_USD_MAX,
  AI_MAX_TURNS_MAX,
  AI_MAX_TURNS_MIN,
} from '../settings/ranges';

/** What a sentinel restores when it is switched back on and the user has not typed
 *  a value during this Settings open (§1.3). */
const IDLE_RESUME = 300;
const CAP_RESUME = 1800;
const BUDGET_RESUME = 5;

/** The budget field's own floor and its two grains. Rust clamps the budget to `0`
 *  (off) or `(0, AI_MAX_BUDGET_USD_MAX]`, so any positive value is legal there;
 *  the floor keeps `0` reachable only via the switch.
 *
 *  The SLIDER moves in 0.50 notches — a money cap is tuned by feel in half-dollars,
 *  and a cent-grained slider would be ~20 000 notches wide. What the user TYPES is
 *  kept to the cent: snapping typed input to 0.50 would round a $2.75 cap UP to
 *  $3.00, i.e. silently raise a limit the user set deliberately. */
const BUDGET_MIN = 0.5;
const BUDGET_STEP = 0.5;
const BUDGET_TYPED_STEP = 0.01;

const IDLE_ON = 'ai.idle-timeout-enabled';
const IDLE_SECS = 'ai.idle-timeout-secs';
const CAP_ON = 'ai.hard-cap-enabled';
const CAP_SECS = 'ai.hard-cap-secs';
const TURNS = 'ai.max-turns';
const BUDGET_ON = 'ai.budget-enabled';
const BUDGET_USD = 'ai.budget-usd';

export interface SettingsAiLimitsProps {
  aiIdleTimeoutSecs: number;
  aiHardCapSecs: number;
  aiMaxTurns: number;
  aiMaxBudgetUsd: number;
  /** `!aiActive` — the row-level dim that pairs with the parent's `<fieldset>`. */
  disabled: boolean;
  /** Every handler patches EXACTLY one field (App owns the debounced persist). */
  onChange(patch: UiSettingsPatch): void;
}

/** "300 seconds is five minutes" for the idle hint, tracking the live field value.
 *  Whole minutes get the friendly form; anything else stays in seconds rather than
 *  rounding to a figure the field does not show. */
function describeSecs(secs: number): string {
  if (secs % 60 !== 0) return `${secs} seconds`;
  const mins = secs / 60;
  const WORDS = ['zero', 'one', 'two', 'three', 'four', 'five', 'six', 'seven', 'eight', 'nine'];
  const spelled = mins < WORDS.length ? WORDS[mins] : String(mins);
  return `${secs} seconds is ${spelled} minute${mins === 1 ? '' : 's'}`;
}

export function SettingsAiLimits({
  aiIdleTimeoutSecs,
  aiHardCapSecs,
  aiMaxTurns,
  aiMaxBudgetUsd,
  disabled,
  onChange,
}: SettingsAiLimitsProps) {
  // Resume values: the last non-zero value held during THIS Settings open (the panel
  // unmounts on close, so that is exactly this component's lifetime).
  const [idleResume, setIdleResume] = useState(
    aiIdleTimeoutSecs === 0 ? IDLE_RESUME : aiIdleTimeoutSecs,
  );
  const [capResume, setCapResume] = useState(aiHardCapSecs === 0 ? CAP_RESUME : aiHardCapSecs);
  const [budgetResume, setBudgetResume] = useState(
    aiMaxBudgetUsd === 0 ? BUDGET_RESUME : aiMaxBudgetUsd,
  );

  const idleOn = aiIdleTimeoutSecs !== 0;
  const capOn = aiHardCapSecs !== 0;
  const budgetOn = aiMaxBudgetUsd !== 0;
  // The "no limit at all" combination is described, not refused (OQ-3), and the
  // sentence is announced with the limit control while it is on screen.
  const noLimits = !idleOn && !capOn;
  const capDescribedBy = noLimits
    ? 'settings-ai-cap-hint settings-ai-nolimit-hint'
    : 'settings-ai-cap-hint';

  const setIdle = (v: number): void => {
    setIdleResume(v);
    onChange({ aiIdleTimeoutSecs: v });
  };
  const setCap = (v: number): void => {
    setCapResume(v);
    onChange({ aiHardCapSecs: v });
  };
  // `0` can never arrive through this field — that is the switch's job, which is
  // why its floor is BUDGET_MIN rather than 0.
  const setBudget = (v: number): void => {
    setBudgetResume(v);
    onChange({ aiMaxBudgetUsd: v });
  };

  return (
    <SettingsGroup id="ai-limits" title="Limits">
      {/* 4 — idle watchdog. `0` disables it; the numeric row stays mounted and
          disabled, showing what re-enabling would restore. */}
      <SettingsSwitchRow
        id={IDLE_ON}
        checked={idleOn}
        disabled={disabled}
        describedBy="settings-ai-idle-hint"
        hint={
          <p className="settings-row-note" id="settings-ai-idle-hint">
            {idleOn
              ? // P68g §1.4 hint 4, with its parenthetical made to track the field. The
                // contract hard-coded "300 seconds is five minutes"; the field is
                // user-editable, so at any other value that sentence stated a number the
                // control was not showing.
                `Ends a run that has printed nothing for this long — ${describeSecs(aiIdleTimeoutSecs)}. The clock pauses while Claude is waiting for your answer.`
              : 'A run that stops printing is left alone. Cancel in the AI activity dock is how you end it.'}
          </p>
        }
        onChange={(on) => onChange({ aiIdleTimeoutSecs: on ? idleResume : 0 })}
      />
      <SettingsRow id={IDLE_SECS} controlId="settings-ai-idle" disabled={disabled || !idleOn}>
        <NumberSlider
          bare
          id="settings-ai-idle"
          label="After"
          value={idleOn ? aiIdleTimeoutSecs : idleResume}
          min={AI_IDLE_TIMEOUT_MIN}
          max={AI_IDLE_TIMEOUT_MAX}
          unit="seconds"
          disabled={disabled || !idleOn}
          // The row's own help line FIRST, then the switch's stateful note: the
          // note explains the sentinel, it does not describe this field, so it is
          // appended rather than substituted (P69j-1 review item 5).
          describedBy={`${settingsRowHelpId(IDLE_SECS)} settings-ai-idle-hint`}
          onChange={setIdle}
        />
      </SettingsRow>

      {/* 5 — a LOCKED default: off, i.e. a run has no deadline. */}
      <SettingsSwitchRow
        id={CAP_ON}
        checked={capOn}
        disabled={disabled}
        describedBy={capDescribedBy}
        hint={
          <>
            <p className="settings-row-note" id="settings-ai-cap-hint">
              {capOn
                ? 'Ends the run when it reaches this limit, whatever it is doing. The clock pauses while Claude is waiting for your answer.'
                : 'Off by default: a run has no deadline, and Cancel in the AI activity dock is how you stop one. Turn this on if you would rather have a hard limit.'}
            </p>
            {/* Audit L4, reachable on purpose and stated in one factual sentence
                rather than refused (OQ-3). */}
            {noLimits && (
              <p className="settings-row-note" id="settings-ai-nolimit-hint">
                With neither limit on, a run continues until it finishes or you cancel it.
              </p>
            )}
          </>
        }
        onChange={(on) => onChange({ aiHardCapSecs: on ? capResume : 0 })}
      />
      <SettingsRow id={CAP_SECS} controlId="settings-ai-cap" disabled={disabled || !capOn}>
        <NumberSlider
          bare
          id="settings-ai-cap"
          label="Time limit"
          value={capOn ? aiHardCapSecs : capResume}
          min={AI_HARD_CAP_MIN}
          max={AI_HARD_CAP_MAX}
          unit="seconds"
          disabled={disabled || !capOn}
          describedBy={`${settingsRowHelpId(CAP_SECS)} ${capDescribedBy}`}
          onChange={setCap}
        />
      </SettingsRow>

      {/* 6 — turns. Gated by neither sentinel. */}
      <SettingsRow id={TURNS} controlId="settings-ai-turns" disabled={disabled}>
        <NumberSlider
          bare
          id="settings-ai-turns"
          label="Replies per run"
          value={aiMaxTurns}
          min={AI_MAX_TURNS_MIN}
          max={AI_MAX_TURNS_MAX}
          unit="turns"
          disabled={disabled}
          describedBy={settingsRowHelpId(TURNS)}
          onChange={(v) => onChange({ aiMaxTurns: v })}
        />
      </SettingsRow>

      {/* 7 — the other LOCKED default: no spend cap. */}
      <SettingsSwitchRow
        id={BUDGET_ON}
        checked={budgetOn}
        disabled={disabled}
        describedBy="settings-ai-budget-hint"
        hint={
          <p className="settings-row-note" id="settings-ai-budget-hint">
            {budgetOn
              ? 'Passed to the Claude CLI as a budget for each run separately — not a total for the day.'
              : 'Off by default: Bonsai does not cap what a run may spend. The AI activity dock shows the running cost after each turn, and Cancel stops it.'}
          </p>
        }
        onChange={(on) => onChange({ aiMaxBudgetUsd: on ? budgetResume : 0 })}
      />
      <SettingsRow id={BUDGET_USD} controlId="settings-ai-budget" disabled={disabled || !budgetOn}>
        <NumberSlider
          bare
          id="settings-ai-budget"
          label="Spend limit"
          value={budgetOn ? aiMaxBudgetUsd : budgetResume}
          min={BUDGET_MIN}
          max={AI_MAX_BUDGET_USD_MAX}
          step={BUDGET_STEP}
          typedStep={BUDGET_TYPED_STEP}
          unit="USD"
          disabled={disabled || !budgetOn}
          describedBy={`${settingsRowHelpId(BUDGET_USD)} settings-ai-budget-hint`}
          onChange={setBudget}
        />
      </SettingsRow>
    </SettingsGroup>
  );
}
