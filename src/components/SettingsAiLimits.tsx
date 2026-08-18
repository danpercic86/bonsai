/**
 * P68g §1 — the "Limits" group of Settings → AI runs, in its own file because it owns
 * all of the section's LOGIC: the three sentinel checkboxes, their resume values, and
 * the one numeric field `NumberSlider` cannot carry (USD, two decimals).
 *
 * `0` is a MODE, not a number (V2). `aiHardCapSecs: 0` (a run has no deadline — Cancel
 * in the dock is the designed stop) and `aiMaxBudgetUsd: 0` (no spend cap) are LOCKED
 * user decisions; `aiIdleTimeoutSecs: 0` is reachable by hand-editing the settings
 * file (audit L4). Each is therefore an unchecked checkbox plus a DISABLED numeric row
 * that still shows what re-checking would restore — a field reading `0` looks broken,
 * and a row that unmounts reads as "this feature does not exist".
 *
 * Renders a fragment: the parent owns the single `<fieldset>` that makes the whole
 * section inert when AI is off (§1.2), and nothing here re-implements that.
 */
import { useState } from 'react';

import type { UiSettingsPatch } from '../ipc';
import { NumberSlider } from './NumberSlider';
import {
  AI_HARD_CAP_MAX,
  AI_HARD_CAP_MIN,
  AI_IDLE_TIMEOUT_MAX,
  AI_IDLE_TIMEOUT_MIN,
  AI_MAX_BUDGET_USD_MAX,
  AI_MAX_TURNS_MAX,
  AI_MAX_TURNS_MIN,
} from '../settings/ranges';

/** What a sentinel checkbox restores when it is re-checked and the user has not typed
 *  a value during this Settings open (§1.3). */
const IDLE_RESUME = 300;
const CAP_RESUME = 1800;
const BUDGET_RESUME = 5;

/** The budget field's own floor. Rust clamps the budget to `0` (off) or
 *  `(0, AI_MAX_BUDGET_USD_MAX]`, so any positive value is legal there; 0.5 is the
 *  smallest step worth typing, and it keeps `0` reachable only via the checkbox. */
const BUDGET_MIN = 0.5;
const BUDGET_STEP = 0.5;

export interface SettingsAiLimitsProps {
  aiIdleTimeoutSecs: number;
  aiHardCapSecs: number;
  aiMaxTurns: number;
  aiMaxBudgetUsd: number;
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
  // The USD field keeps a draft string while it is being typed: a controlled number
  // input re-rendered from `Number('12.')` would delete the "." just typed. Cleared
  // on blur, when the clamped value takes over.
  const [budgetDraft, setBudgetDraft] = useState<string | null>(null);

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
  // Two decimals, never an integer round: `NumberSlider` rounds to integers, so this
  // one field owns its own commit (§1.3). `0` can never arrive through it — that is
  // the checkbox's job.
  const commitBudget = (raw: string): void => {
    setBudgetDraft(raw);
    if (raw.trim() === '') return;
    const n = Number(raw);
    if (!Number.isFinite(n)) return;
    const clamped =
      Math.round(Math.min(AI_MAX_BUDGET_USD_MAX, Math.max(BUDGET_MIN, n)) * 100) / 100;
    setBudgetResume(clamped);
    onChange({ aiMaxBudgetUsd: clamped });
  };

  return (
    <>
      <h4 className="settings-subsection-title">Limits</h4>
      {/* 4 — idle watchdog. `0` disables it; the numeric row stays mounted and
          disabled, showing what re-checking would restore. */}
      <label className="settings-checkbox">
        <input
          type="checkbox"
          checked={idleOn}
          aria-describedby="settings-ai-idle-hint"
          onChange={(e) => onChange({ aiIdleTimeoutSecs: e.target.checked ? idleResume : 0 })}
        />
        <span>Stop a run that goes quiet</span>
      </label>
      <div className="settings-indent">
        <NumberSlider
          id="settings-ai-idle"
          label="After"
          value={idleOn ? aiIdleTimeoutSecs : idleResume}
          min={AI_IDLE_TIMEOUT_MIN}
          max={AI_IDLE_TIMEOUT_MAX}
          unit="seconds"
          disabled={!idleOn}
          describedBy="settings-ai-idle-hint"
          onChange={setIdle}
        />
        <p className="settings-hint" id="settings-ai-idle-hint">
          {idleOn
            ? // P68g §1.4 hint 4, with its parenthetical made to track the field. The
              // contract hard-coded "300 seconds is five minutes"; the field is
              // user-editable, so at any other value that sentence stated a number the
              // control was not showing.
              `Ends a run that has printed nothing for this long — ${describeSecs(aiIdleTimeoutSecs)}. The clock pauses while Claude is waiting for your answer.`
            : 'A run that stops printing is left alone. Cancel in the AI activity dock is how you end it.'}
        </p>
      </div>

      {/* 5 — a LOCKED default: off, i.e. a run has no deadline. */}
      <label className="settings-checkbox">
        <input
          type="checkbox"
          checked={capOn}
          aria-describedby={capDescribedBy}
          onChange={(e) => onChange({ aiHardCapSecs: e.target.checked ? capResume : 0 })}
        />
        <span>Stop a run after a fixed time</span>
      </label>
      <div className="settings-indent">
        <NumberSlider
          id="settings-ai-cap"
          label="Limit"
          value={capOn ? aiHardCapSecs : capResume}
          min={AI_HARD_CAP_MIN}
          max={AI_HARD_CAP_MAX}
          unit="seconds"
          disabled={!capOn}
          describedBy={capDescribedBy}
          onChange={setCap}
        />
        <p className="settings-hint" id="settings-ai-cap-hint">
          {capOn
            ? 'Ends the run at this point whatever it is doing. The clock pauses while Claude is waiting for your answer.'
            : 'Off by default: a run has no deadline, and Cancel in the AI activity dock is how you stop one. Turn this on if you would rather have a hard limit.'}
        </p>
        {/* Audit L4, reachable on purpose and stated in one factual sentence rather
            than refused (OQ-3). */}
        {noLimits && (
          <p className="settings-hint" id="settings-ai-nolimit-hint">
            With neither limit on, a run continues until it finishes or you cancel it.
          </p>
        )}
      </div>

      {/* 6 — turns. Gated by neither sentinel, so it is not indented under one. */}
      <NumberSlider
        id="settings-ai-turns"
        label="Replies per run"
        value={aiMaxTurns}
        min={AI_MAX_TURNS_MIN}
        max={AI_MAX_TURNS_MAX}
        unit="turns"
        describedBy="settings-ai-turns-hint"
        onChange={(v) => onChange({ aiMaxTurns: v })}
      />
      <p className="settings-hint" id="settings-ai-turns-hint">
        How many times Claude may answer inside one run, including its answers to your questions. A
        run still asking after this many turns is ended.
      </p>

      {/* 7 — the other LOCKED default: no spend cap. */}
      <label className="settings-checkbox">
        <input
          type="checkbox"
          checked={budgetOn}
          aria-describedby="settings-ai-budget-hint"
          onChange={(e) => onChange({ aiMaxBudgetUsd: e.target.checked ? budgetResume : 0 })}
        />
        <span>Set a spend limit per run</span>
      </label>
      <div className="settings-indent">
        <div className={`settings-control${budgetOn ? '' : ' is-disabled'}`}>
          <label className="settings-control-label" htmlFor="settings-ai-budget">
            Limit
          </label>
          <div className="settings-control-inputs">
            <input
              id="settings-ai-budget"
              className="settings-number settings-number-wide"
              type="number"
              min={BUDGET_MIN}
              max={AI_MAX_BUDGET_USD_MAX}
              step={BUDGET_STEP}
              value={budgetDraft ?? String(budgetOn ? aiMaxBudgetUsd : budgetResume)}
              disabled={!budgetOn}
              aria-describedby="settings-ai-budget-hint"
              onChange={(e) => commitBudget(e.target.value)}
              onBlur={() => setBudgetDraft(null)}
            />
            <span className="settings-unit">USD</span>
          </div>
        </div>
        <p className="settings-hint" id="settings-ai-budget-hint">
          {budgetOn
            ? 'Passed to the Claude CLI as a budget for each run separately — not a total for the day.'
            : 'Off by default: Bonsai does not cap what a run may spend. The AI activity dock shows the running cost after each turn, and Cancel stops it.'}
        </p>
      </div>
    </>
  );
}
