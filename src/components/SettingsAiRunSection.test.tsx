/**
 * P68g §6.1 — the eight AI-run controls. Covers `SettingsAiRunSection` AND the
 * `SettingsAiLimits` child it renders (the section is the only public entry point;
 * splitting the file must not split the behaviour).
 *
 * Two things this suite is really guarding:
 *   * `0` is a MODE, not a number. `aiHardCapSecs` / `aiMaxBudgetUsd` ship as 0 by
 *     LOCKED user decision (no deadline, no spend cap), so unchecking must patch
 *     exactly `0`, re-checking must restore a usable value, and the numeric row must
 *     stay on screen (disabled) rather than vanishing.
 *   * every patch carries EXACTLY one key — these knobs are independent in Rust, and
 *     a bundled patch would silently overwrite a field the user did not touch.
 */
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';

import { SettingsAiRunSection } from './SettingsAiRunSection';
import type { AiRunPrefs } from '../settings/aiRunPrefs';
import type { UiSettingsPatch } from '../ipc';

const DEFAULTS: AiRunPrefs = {
  aiConflictTools: 'readOnly',
  aiStreamLog: true,
  aiIncludePartialMessages: false,
  aiIdleTimeoutSecs: 300,
  aiHardCapSecs: 0,
  aiMaxTurns: 6,
  aiMaxBudgetUsd: 0,
  aiBulkMaxBytes: 400_000,
};

/** A live harness: patches are applied back onto the props, exactly as App does, so
 *  multi-step flows (type → uncheck → re-check) behave like the real panel. */
function Harness({
  initial,
  aiActive,
  onPatch,
}: {
  initial: AiRunPrefs;
  aiActive: boolean;
  onPatch(patch: UiSettingsPatch): void;
}) {
  const [prefs, setPrefs] = useState(initial);
  return (
    <SettingsAiRunSection
      aiRun={prefs}
      aiActive={aiActive}
      onChange={(patch) => {
        onPatch(patch);
        setPrefs((cur) => ({ ...cur, ...patch }));
      }}
    />
  );
}

function mount(over: Partial<AiRunPrefs> = {}, aiActive = true) {
  const onPatch = vi.fn();
  const view = render(
    <Harness initial={{ ...DEFAULTS, ...over }} aiActive={aiActive} onPatch={onPatch} />,
  );
  return { ...view, onPatch };
}

/** The number input of a `NumberSlider` (its id), never the range twin. */
function num(id: string): HTMLInputElement {
  return document.getElementById(id) as HTMLInputElement;
}

/** The repository-access segmented control (UI §5.3 item 4 — it was a
 *  self-labelling button, the riskiest place for that defect: it names a
 *  permission level). Native radios inside a `role="radiogroup"`. */
function accessGroup(): HTMLElement {
  return screen.getByRole('radiogroup', { name: 'Repository access' });
}

function accessOption(name: 'Read-only' | 'No file access'): HTMLElement {
  return within(accessGroup()).getByRole('radio', { name });
}

describe('SettingsAiRunSection — one field per patch', () => {
  it('each of the eight controls patches exactly its own key', () => {
    const { onPatch } = mount({ aiHardCapSecs: 1800, aiMaxBudgetUsd: 5 });

    fireEvent.click(accessOption('No file access'));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Stream AI output' }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Stream partial replies' }));
    fireEvent.change(num('settings-ai-idle'), { target: { value: '600' } });
    fireEvent.change(num('settings-ai-cap'), { target: { value: '3600' } });
    fireEvent.change(num('settings-ai-turns'), { target: { value: '9' } });
    fireEvent.change(num('settings-ai-budget'), { target: { value: '7.5' } });
    fireEvent.change(num('settings-ai-bulk'), { target: { value: '800' } });

    expect(onPatch.mock.calls.map((c) => c[0])).toEqual([
      { aiConflictTools: 'none' },
      { aiStreamLog: false },
      { aiIncludePartialMessages: true },
      { aiIdleTimeoutSecs: 600 },
      { aiHardCapSecs: 3600 },
      { aiMaxTurns: 9 },
      { aiMaxBudgetUsd: 7.5 },
      { aiBulkMaxBytes: 800_000 },
    ]);
  });

  // D10: the grant is read-only or nothing. There is no write/edit/bash option to
  // pick, and none may appear as one.
  it('repository access offers exactly two values, and never a write grant', () => {
    mount();
    const options = within(accessGroup()).getAllByRole('radio');
    const seen = options.map((o) => o.parentElement?.textContent ?? '');
    // No third value is reachable, the CURRENT one is shown as selected rather
    // than as the button's label, and none of the offered ones grants writing.
    expect(seen).toEqual(['Read-only', 'No file access']);
    expect(accessOption('Read-only')).toBeChecked();
    for (const label of seen) {
      expect(label.toLowerCase()).not.toMatch(/write|edit|bash/);
    }
    fireEvent.click(accessOption('No file access'));
    expect(accessOption('No file access')).toBeChecked();
    expect(within(accessGroup()).getAllByRole('radio')).toHaveLength(2);
  });
});

describe('SettingsAiRunSection — 0 is a mode', () => {
  it('unchecking the fixed-time limit patches 0; re-checking restores 1800', () => {
    const { onPatch } = mount({ aiHardCapSecs: 1800 });
    const box = screen.getByRole('checkbox', { name: 'Stop a run after a fixed time' });
    expect(box).toBeChecked();

    fireEvent.click(box);
    expect(onPatch).toHaveBeenLastCalledWith({ aiHardCapSecs: 0 });
    // The row stays mounted, disabled, still showing what re-checking restores.
    expect(num('settings-ai-cap')).toBeDisabled();
    expect(num('settings-ai-cap').value).toBe('1800');

    fireEvent.click(box);
    expect(onPatch).toHaveBeenLastCalledWith({ aiHardCapSecs: 1800 });
  });

  it('re-checking restores the value the user last typed, not the default', () => {
    const { onPatch } = mount({ aiHardCapSecs: 1800 });
    fireEvent.change(num('settings-ai-cap'), { target: { value: '600' } });
    const box = screen.getByRole('checkbox', { name: 'Stop a run after a fixed time' });
    fireEvent.click(box); // off
    fireEvent.click(box); // on again
    expect(onPatch).toHaveBeenLastCalledWith({ aiHardCapSecs: 600 });
  });

  it('the spend limit is off by default and restores 5.00 when switched on', () => {
    const { onPatch } = mount();
    const box = screen.getByRole('checkbox', { name: 'Set a spend limit per run' });
    expect(box).not.toBeChecked();
    expect(num('settings-ai-budget')).toBeDisabled();
    expect(num('settings-ai-budget').value).toBe('5');
    fireEvent.click(box);
    expect(onPatch).toHaveBeenLastCalledWith({ aiMaxBudgetUsd: 5 });
  });

  // Audit L4: reachable on purpose, and stated rather than refused (OQ-3).
  it('states the no-limit-at-all case only when BOTH limits are off', () => {
    const line = 'With neither limit on, a run continues until it finishes or you cancel it.';
    const both = mount({ aiIdleTimeoutSecs: 0, aiHardCapSecs: 0 });
    expect(screen.getByText(line)).toBeInTheDocument();
    both.unmount();

    mount({ aiIdleTimeoutSecs: 300, aiHardCapSecs: 0 });
    expect(screen.queryByText(line)).toBeNull();
  });

  it('the quiet-stop sentinel switches its own hint copy', () => {
    const { onPatch } = mount();
    const box = screen.getByRole('checkbox', { name: 'Stop a run that goes quiet' });
    expect(screen.getByText(/300 seconds is five minutes/)).toBeInTheDocument();
    fireEvent.click(box);
    expect(onPatch).toHaveBeenLastCalledWith({ aiIdleTimeoutSecs: 0 });
    expect(
      screen.getByText(
        'A run that stops printing is left alone. Cancel in the AI activity dock is how you end it.',
      ),
    ).toBeInTheDocument();
  });

  /**
   * The contract hard-coded "300 seconds is five minutes" into a hint describing a
   * user-editable field, so at any other value it named a number the control was not
   * showing. The parenthetical now tracks the field.
   */
  it('the idle hint restates the value actually in the field, not a fixed 300', () => {
    const at600 = mount({ aiIdleTimeoutSecs: 600 });
    expect(screen.getByText(/600 seconds is 10 minutes/)).toBeInTheDocument();
    expect(screen.queryByText(/300 seconds/)).toBeNull();
    at600.unmount();

    // A non-whole-minute value stays in seconds rather than rounding to a figure the
    // field does not show.
    mount({ aiIdleTimeoutSecs: 90 });
    expect(screen.getByText(/nothing for this long — 90 seconds\./)).toBeInTheDocument();
    expect(screen.queryByText(/minute/)).toBeNull();
  });
});

describe('SettingsAiRunSection — clamping and units', () => {
  it('turns: out-of-range snaps to the clamp; a blank field patches nothing', () => {
    const { onPatch } = mount();
    fireEvent.change(num('settings-ai-turns'), { target: { value: '99999' } });
    expect(onPatch).toHaveBeenLastCalledWith({ aiMaxTurns: 20 });
    fireEvent.change(num('settings-ai-turns'), { target: { value: '0' } });
    expect(onPatch).toHaveBeenLastCalledWith({ aiMaxTurns: 1 });
    const before = onPatch.mock.calls.length;
    // A number input blanks itself on non-numeric input, and a blank field is not a
    // request to set the minimum.
    fireEvent.change(num('settings-ai-turns'), { target: { value: 'abc' } });
    expect(onPatch.mock.calls).toHaveLength(before);
  });

  it('bulk size is shown in KB and patched in bytes', () => {
    const { onPatch } = mount();
    expect(num('settings-ai-bulk').value).toBe('400');
    fireEvent.change(num('settings-ai-bulk'), { target: { value: '1200' } });
    expect(onPatch).toHaveBeenLastCalledWith({ aiBulkMaxBytes: 1_200_000 });
    fireEvent.change(num('settings-ai-bulk'), { target: { value: '99999' } });
    expect(onPatch).toHaveBeenLastCalledWith({ aiBulkMaxBytes: 4_000_000 });
  });

  it('the spend limit keeps two decimals and clamps to 100', () => {
    const { onPatch } = mount({ aiMaxBudgetUsd: 5 });
    fireEvent.change(num('settings-ai-budget'), { target: { value: '12.5' } });
    expect(onPatch).toHaveBeenLastCalledWith({ aiMaxBudgetUsd: 12.5 });
    fireEvent.change(num('settings-ai-budget'), { target: { value: '500' } });
    expect(onPatch).toHaveBeenLastCalledWith({ aiMaxBudgetUsd: 100 });
    // `0` can only arrive from the checkbox — never from this field.
    fireEvent.change(num('settings-ai-budget'), { target: { value: '0' } });
    expect(onPatch).toHaveBeenLastCalledWith({ aiMaxBudgetUsd: 0.5 });
  });
});

describe('SettingsAiRunSection — states and a11y', () => {
  it('AI off: the whole section is inert and says how to turn it on', () => {
    const { container } = mount({}, false);
    for (const el of container.querySelectorAll('input, button')) {
      expect(el).toBeDisabled();
    }
    expect(
      screen.getByText('Turn on “Enable AI features” above to change these.'),
    ).toBeInTheDocument();
  });

  // P69d (UI §5.4): the gate note is the fieldset's description, so the REASON the
  // group is inert is announced on entry instead of being an orphaned paragraph. The
  // copy is byte-identical to the assertion above — only the wiring is new.
  it('AI off: the gate note leads the group and describes the fieldset', () => {
    const { container } = mount({}, false);
    const fieldset = container.querySelector('fieldset');
    expect(fieldset).toHaveAttribute('aria-describedby', 'ai-run-gate-note');
    const note = document.getElementById('ai-run-gate-note');
    expect(note).toHaveTextContent('Turn on “Enable AI features” above to change these.');
    // P69j / UI §5.4: the note now LEADS the group — inside the fieldset (so the
    // fieldset can point at it, and so the .55 row dim never touches it), but
    // before every row it explains (DOCUMENT_POSITION_FOLLOWING = 4).
    expect(fieldset!.contains(note!)).toBe(true);
    const firstRow = container.querySelector('[data-setting-id]');
    expect(note!.compareDocumentPosition(firstRow!) & Node.DOCUMENT_POSITION_FOLLOWING).toBe(4);
  });

  it('AI on: the fieldset has no dangling describedby (the note is gone)', () => {
    const { container } = mount();
    expect(container.querySelector('fieldset')).not.toHaveAttribute('aria-describedby');
    expect(document.getElementById('ai-run-gate-note')).toBeNull();
  });

  it('AI on: the "turn it on" line is gone', () => {
    mount();
    expect(screen.queryByText(/Turn on “Enable AI features” above/)).toBeNull();
  });

  it('every aria-describedby target exists in the DOM', () => {
    const { container } = mount({ aiIdleTimeoutSecs: 0, aiHardCapSecs: 0 });
    const referenced = new Set<string>();
    for (const el of container.querySelectorAll('[aria-describedby]')) {
      for (const id of (el.getAttribute('aria-describedby') ?? '').split(/\s+/)) {
        if (id !== '') referenced.add(id);
      }
    }
    expect(referenced.size).toBeGreaterThan(7);
    for (const id of referenced) expect(document.getElementById(id)).not.toBeNull();
    // The guard line's id is announced FIRST for the fixed-time control while the
    // "no limits at all" sentence is on screen.
    expect(
      screen.getByRole('checkbox', { name: 'Stop a run after a fixed time' }),
    ).toHaveAttribute('aria-describedby', 'settings-ai-cap-hint settings-ai-nolimit-hint');

    // Same guard in the GATED state, where the fieldset's own describedby appears.
    cleanup();
    const off = mount({}, false).container;
    const offRefs = new Set<string>();
    for (const el of off.querySelectorAll('[aria-describedby]')) {
      for (const id of (el.getAttribute('aria-describedby') ?? '').split(/\s+/)) {
        if (id !== '') offRefs.add(id);
      }
    }
    expect(offRefs.has('ai-run-gate-note')).toBe(true);
    for (const id of offRefs) expect(document.getElementById(id)).not.toBeNull();
  });

  it('the access group is named by its row, and each option names its own value', () => {
    mount();
    // The group carries the SETTING's name and the options carry the VALUES —
    // which is what the old self-labelling button could not express.
    expect(accessGroup()).toHaveAttribute('aria-labelledby', 'ai.repository-access-label');
    for (const option of within(accessGroup()).getAllByRole('radio')) {
      expect(option).toHaveAttribute('aria-describedby', 'settings-ai-tools-hint');
    }
  });

  it('the read grant is disclosed in words, and switched with the value', () => {
    mount();
    expect(
      screen.getByText(/Anything it reads is sent to Anthropic\./),
    ).toBeInTheDocument();
    fireEvent.click(accessOption('No file access'));
    expect(
      screen.getByText(/Claude sees only the conflicting versions of each file/),
    ).toBeInTheDocument();
  });
});
