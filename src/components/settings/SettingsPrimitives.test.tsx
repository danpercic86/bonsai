/**
 * P69g — the three control primitives, in isolation.
 *
 * The load-bearing claim is UI D4: a switch is a NATIVE `<input type="checkbox">`
 * and a segmented control is NATIVE `<input type="radio">`, so the ~30
 * `getByRole('checkbox'|'radio', {name})` assertions across the suite survive the
 * re-skin and AT gets the semantics for free. If any of these ever needs
 * `role="switch"`, that decision has to be made deliberately — not discovered.
 */
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';

import { SettingsRow } from './SettingsRow';
import { SettingsSwitch } from './SettingsSwitch';
import { SettingsSegmented } from './SettingsSegmented';
import { settingsRowHelpId, settingsRowLabelId } from './settingsCatalog';

const SWITCH_ROW = 'general.auto-fetch'; // label "Auto-fetch from remotes", has reset
const SEG_ROW = 'appearance.theme'; // label "Theme", no reset
const TEXT_ROW = 'general.terminal-command'; // label "Terminal command", has reset

function renderSwitch(over: { checked?: boolean; disabled?: boolean } = {}) {
  const onChange = vi.fn();
  render(
    <SettingsRow id={SWITCH_ROW} controlId="sw">
      <SettingsSwitch
        id="sw"
        checked={over.checked ?? false}
        disabled={over.disabled}
        describedBy={settingsRowHelpId(SWITCH_ROW)}
        onChange={onChange}
      />
    </SettingsRow>,
  );
  return onChange;
}

describe('SettingsSwitch', () => {
  it('is a native checkbox named by the row label', () => {
    renderSwitch();
    const box = screen.getByRole('checkbox', { name: 'Auto-fetch from remotes' });
    expect(box).toHaveAttribute('type', 'checkbox');
    expect(box).not.toHaveAttribute('role');
    expect(box).not.toBeChecked();
  });

  it('reflects checked/disabled and reports the new value', () => {
    const onChange = renderSwitch({ checked: true });
    expect(screen.getByRole('checkbox')).toBeChecked();
    fireEvent.click(screen.getByRole('checkbox'));
    expect(onChange).toHaveBeenCalledWith(false);
  });

  it('is describedby the row help paragraph, and that id resolves', () => {
    renderSwitch();
    const box = screen.getByRole('checkbox');
    const id = box.getAttribute('aria-describedby');
    expect(id).toBe(settingsRowHelpId(SWITCH_ROW));
    expect(document.getElementById(id ?? '')).not.toBeNull();
  });

  it('disabled is the native attribute, so it also leaves the tab order', () => {
    // Asserted on the attribute, not by firing a click: `fireEvent.click`
    // dispatches straight at the node and bypasses the disabled check that a real
    // pointer/keyboard would hit.
    renderSwitch({ disabled: true });
    expect(screen.getByRole('checkbox')).toBeDisabled();
  });
});

describe('SettingsSegmented', () => {
  function renderSegmented(value: 'dark' | 'light' = 'dark') {
    const onChange = vi.fn();
    render(
      <SettingsRow id={SEG_ROW}>
        <SettingsSegmented<'dark' | 'light'>
          name={SEG_ROW}
          value={value}
          labelledBy={settingsRowLabelId(SEG_ROW)}
          options={[
            { value: 'dark', label: 'Dark' },
            { value: 'light', label: 'Light' },
          ]}
          onChange={onChange}
        />
      </SettingsRow>,
    );
    return onChange;
  }

  it('is a radiogroup named by the row label, holding native radios', () => {
    renderSegmented();
    const group = screen.getByRole('radiogroup', { name: 'Theme' });
    expect(within(group).getAllByRole('radio')).toHaveLength(2);
    expect(screen.getByRole('radio', { name: 'Dark' })).toBeChecked();
    expect(screen.getByRole('radio', { name: 'Light' })).not.toBeChecked();
  });

  it('selecting the other option reports it; re-selecting the current one is inert', () => {
    const onChange = renderSegmented();
    fireEvent.click(screen.getByRole('radio', { name: 'Dark' }));
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('radio', { name: 'Light' }));
    expect(onChange).toHaveBeenCalledWith('light');
  });

  it('warns in DEV past three segments (UI §5.6 — beyond that it is a Combobox)', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    render(
      <SettingsSegmented<string>
        name="too-many"
        value="a"
        labelledBy="nope"
        options={['a', 'b', 'c', 'd'].map((v) => ({ value: v, label: v }))}
        onChange={vi.fn()}
      />,
    );
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
  });
});

describe('SettingsRow', () => {
  it('stamps its catalog id and renders label + help from the catalog', () => {
    const { container } = render(
      <SettingsRow id={SWITCH_ROW} controlId="sw">
        <input id="sw" type="checkbox" readOnly />
      </SettingsRow>,
    );
    const row = container.querySelector('[data-setting-id]');
    expect(row).toHaveAttribute('data-setting-id', SWITCH_ROW);
    expect(row?.querySelector('[data-setting-control]')).not.toBeNull();
    expect(screen.getByText('Auto-fetch from remotes')).toHaveAttribute(
      'id',
      settingsRowLabelId(SWITCH_ROW),
    );
    expect(document.getElementById(settingsRowHelpId(SWITCH_ROW))).not.toBeNull();
  });

  it('renders ↺ only off-default, with the catalog default in its title', () => {
    const onReset = vi.fn();
    const { rerender } = render(
      <SettingsRow id={TEXT_ROW} controlId="t" reset={{ isDefault: true, onReset }}>
        <input id="t" />
      </SettingsRow>,
    );
    expect(screen.queryByRole('button', { name: /^Reset/ })).toBeNull();

    rerender(
      <SettingsRow id={TEXT_ROW} controlId="t" reset={{ isDefault: false, onReset }}>
        <input id="t" />
      </SettingsRow>,
    );
    const reset = screen.getByRole('button', { name: 'Reset Terminal command to default' });
    expect(reset).toHaveAttribute('title', 'Reset to default (auto-detect)');
    fireEvent.click(reset);
    expect(onReset).toHaveBeenCalledTimes(1);
  });

  it('a disabled row disables its ↺ — a dimmed control may not stay clickable', () => {
    const onReset = vi.fn();
    render(
      <SettingsRow id={TEXT_ROW} controlId="t" disabled reset={{ isDefault: false, onReset }}>
        <input id="t" disabled />
      </SettingsRow>,
    );
    expect(screen.getByRole('button', { name: 'Reset Terminal command to default' })).toBeDisabled();
  });

  it('never renders ↺ for a row the catalog gives no reset descriptor', () => {
    render(
      <SettingsRow id={SEG_ROW} reset={{ isDefault: false, onReset: vi.fn() }}>
        <span />
      </SettingsRow>,
    );
    expect(screen.queryByRole('button', { name: /^Reset/ })).toBeNull();
  });

  it('the stacked variant and the slider help reservation are class-driven', () => {
    const { container } = render(
      <SettingsRow id={TEXT_ROW} controlId="t" stacked>
        <input id="t" />
      </SettingsRow>,
    );
    expect(container.firstElementChild).toHaveClass('settings-row--stacked');

    const slider = render(
      <SettingsRow id="general.fetch-interval" controlId="n">
        <input id="n" />
      </SettingsRow>,
    );
    // P69c §13.2.1: the reserved 18px help line lives on this modifier.
    expect(slider.container.firstElementChild).toHaveClass('settings-row--slider');
  });

  it('shouts in DEV when a row has no catalog entry', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    render(
      <SettingsRow id="general.not-a-real-row">
        <span />
      </SettingsRow>,
    );
    expect(spy).toHaveBeenCalledWith(expect.stringContaining('general.not-a-real-row'));
    spy.mockRestore();
  });
});
