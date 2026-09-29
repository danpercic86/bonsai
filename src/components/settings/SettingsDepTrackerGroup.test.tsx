// P125-ui §10 AC7 — the switch opens the consent dialog and flips only on
// confirm; the interval row is disabled while off; turning off shows the §7
// outcome note beside the help.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, screen, within } from '@testing-library/react';

import { renderPanel } from '../../test/settingsPanelKit';
import {
  CONSENT_CONFIRM,
  CONSENT_TITLE,
  SETTINGS_GROUP_TITLE,
  SETTINGS_LEAD,
  SETTINGS_OFF_NOTE,
} from '../deps/depsCopy';
import { DEP_OFF_NOTE_ID } from './SettingsDepTrackerGroup';

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const toggle = () => screen.getByRole('checkbox', { name: 'Check dependencies online' });

describe('SettingsDepTrackerGroup', () => {
  it('renders the lead and disables the interval while off', () => {
    renderPanel({ initialCategory: 'general' });
    expect(screen.getByText(SETTINGS_LEAD)).toBeTruthy();
    expect((screen.getByRole('slider', { name: 'Check dependencies every' }) as HTMLInputElement).disabled).toBe(true);
  });

  it('turning on opens the consent dialog; nothing is patched until confirm', () => {
    const { props } = renderPanel({ initialCategory: 'general' });
    fireEvent.click(toggle());
    const dialog = screen.getByRole('dialog', { name: CONSENT_TITLE });
    expect(props.onChange).not.toHaveBeenCalled();
    expect(within(dialog).getByText(/api\.deps\.dev, for the latest versions/)).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: CONSENT_CONFIRM }));
    expect(props.onChange).toHaveBeenCalledWith({ depTracker: { enabled: true, intervalHours: 12 } });
    expect(screen.queryByRole('dialog', { name: CONSENT_TITLE })).toBeNull();
  });

  it('cancel leaves the setting off', () => {
    const { props } = renderPanel({ initialCategory: 'general' });
    fireEvent.click(toggle());
    fireEvent.click(within(screen.getByRole('dialog', { name: CONSENT_TITLE })).getByRole('button', { name: 'Cancel' }));
    expect(props.onChange).not.toHaveBeenCalled();
  });

  it('turning off needs no confirm and reports the outcome note beside the help', () => {
    const { props } = renderPanel({
      initialCategory: 'general',
      depTracker: { enabled: true, intervalHours: 6 },
    });
    expect(toggle().getAttribute('aria-describedby')).toBe(`general.dep-tracker-help ${DEP_OFF_NOTE_ID}`);
    expect(document.getElementById(DEP_OFF_NOTE_ID)?.textContent).toBe('');
    fireEvent.click(toggle());
    expect(screen.queryByRole('dialog', { name: CONSENT_TITLE })).toBeNull();
    expect(props.onChange).toHaveBeenCalledWith({ depTracker: { enabled: false, intervalHours: 6 } });
    expect(document.getElementById(DEP_OFF_NOTE_ID)?.textContent).toBe(SETTINGS_OFF_NOTE);
  });

  it("the dialog's deep link scrolls the group into view and focuses its switch", () => {
    const scroll = vi.spyOn(Element.prototype, 'scrollIntoView');
    renderPanel({ initialCategory: 'general', configInitialFocus: 'dep-tracker' });
    const group = screen.getByRole('region', { name: SETTINGS_GROUP_TITLE });
    expect(scroll.mock.contexts).toContain(group);
    expect(document.activeElement).toBe(toggle());
  });

  it('no deep link (or a different target): no scroll, no focus grab', () => {
    const scroll = vi.spyOn(Element.prototype, 'scrollIntoView');
    renderPanel({ initialCategory: 'general', configInitialFocus: 'identity' });
    expect(scroll.mock.contexts).not.toContain(screen.getByRole('region', { name: SETTINGS_GROUP_TITLE }));
    expect(document.activeElement).not.toBe(toggle());
  });
});
