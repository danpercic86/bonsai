/**
 * `docs/contracts/settings-ai-autonomy-disabled-ui.md` §6 — the AUTONOMY row's
 * "why is this disabled" hint. Two distinct causes collapse into the same
 * `!aiActive` boolean and must each get their own copy (§2), composed onto the
 * existing per-option `aria-describedby` rather than replacing it (§4), and must
 * disappear entirely — copy AND aria wiring — once `aiActive` is true.
 *
 * Copy is asserted verbatim (curly quotes included) per this file's own frozen-
 * strings convention (see the file banner in SettingsAiSection.tsx).
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import { SettingsAiSection } from './SettingsAiSection';
import type { SettingsAiSectionProps } from './SettingsAiSection';

const CASE_A_HINT = 'Turn on “Enable AI features” above to change this.';
const CASE_B_HINT = 'Turn “Enable AI features” off and on again to confirm access.';

function mount(over: Partial<SettingsAiSectionProps> = {}) {
  const props: SettingsAiSectionProps = {
    aiEnabled: false,
    aiConflictAutonomy: 'proposeReview',
    aiActive: false,
    aiAvailability: null,
    onToggleEnabled: vi.fn(),
    onChange: vi.fn(),
    ...over,
  };
  return render(<SettingsAiSection {...props} />);
}

function proposeRadio(): HTMLElement {
  return screen.getByRole('radio', { name: 'Propose & review' });
}

function autoRadio(): HTMLElement {
  return screen.getByRole('radio', { name: 'Resolve automatically' });
}

describe('SettingsAiSection — autonomy disabled-hint (case a: !aiEnabled)', () => {
  it('renders the case-(a) copy verbatim', () => {
    mount({ aiEnabled: false, aiActive: false });
    expect(screen.getByText(CASE_A_HINT)).toBeInTheDocument();
    expect(document.getElementById('ai-autonomy-disabled-hint')).toHaveTextContent(CASE_A_HINT);
    expect(screen.queryByText(CASE_B_HINT)).toBeNull();
  });

  it('composes the disabled-hint id onto each radio\'s own per-option hint id', () => {
    mount({ aiEnabled: false, aiActive: false });
    expect(proposeRadio()).toHaveAttribute(
      'aria-describedby',
      'ai-autonomy-propose-hint ai-autonomy-disabled-hint',
    );
    expect(autoRadio()).toHaveAttribute(
      'aria-describedby',
      'ai-autonomy-auto-hint ai-autonomy-disabled-hint',
    );
  });

  it('both radios are actually disabled', () => {
    mount({ aiEnabled: false, aiActive: false });
    expect(proposeRadio()).toBeDisabled();
    expect(autoRadio()).toBeDisabled();
  });
});

describe('SettingsAiSection — autonomy disabled-hint (case b: aiEnabled && !aiConsented)', () => {
  it('renders the case-(b) copy verbatim', () => {
    mount({ aiEnabled: true, aiActive: false });
    expect(screen.getByText(CASE_B_HINT)).toBeInTheDocument();
    expect(document.getElementById('ai-autonomy-disabled-hint')).toHaveTextContent(CASE_B_HINT);
    expect(screen.queryByText(CASE_A_HINT)).toBeNull();
  });

  it('composes the disabled-hint id onto each radio\'s own per-option hint id', () => {
    mount({ aiEnabled: true, aiActive: false });
    expect(proposeRadio()).toHaveAttribute(
      'aria-describedby',
      'ai-autonomy-propose-hint ai-autonomy-disabled-hint',
    );
    expect(autoRadio()).toHaveAttribute(
      'aria-describedby',
      'ai-autonomy-auto-hint ai-autonomy-disabled-hint',
    );
  });

  it('both radios are actually disabled', () => {
    mount({ aiEnabled: true, aiActive: false });
    expect(proposeRadio()).toBeDisabled();
    expect(autoRadio()).toBeDisabled();
  });
});

describe('SettingsAiSection — autonomy active (aiActive=true): no hint at all', () => {
  it('mounts neither hint paragraph', () => {
    mount({ aiEnabled: true, aiActive: true });
    expect(screen.queryByText(CASE_A_HINT)).toBeNull();
    expect(screen.queryByText(CASE_B_HINT)).toBeNull();
    expect(document.getElementById('ai-autonomy-disabled-hint')).toBeNull();
  });

  it('aria-describedby reverts to only the per-option hint id, on both radios', () => {
    mount({ aiEnabled: true, aiActive: true });
    expect(proposeRadio()).toHaveAttribute('aria-describedby', 'ai-autonomy-propose-hint');
    expect(autoRadio()).toHaveAttribute('aria-describedby', 'ai-autonomy-auto-hint');
  });

  it('both radios are enabled', () => {
    mount({ aiEnabled: true, aiActive: true });
    expect(proposeRadio()).not.toBeDisabled();
    expect(autoRadio()).not.toBeDisabled();
  });
});
