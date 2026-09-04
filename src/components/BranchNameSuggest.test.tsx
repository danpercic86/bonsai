/** P111 §5 — the load-bearing accessibility claim for the suggestion chips:
 *  truncation is CSS-only, so a chip's ACCESSIBLE NAME is always the exact,
 *  complete branch name. A truncated branch name would be a wrong branch name,
 *  and `RefLabel`'s two spans make name-from-content ambiguous (it can join the
 *  siblings with a space), so the chip carries an explicit `aria-label`. */
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { BranchNameSuggest } from './BranchNameSuggest';
import { BRANCH_NAMES_FROM_WORKING } from '../ipc/fixtures/branchNames';

const LONG = BRANCH_NAMES_FROM_WORKING[3];

function renderSuggest(onPick = vi.fn()) {
  const suggest = vi.fn().mockResolvedValue({ names: BRANCH_NAMES_FROM_WORKING, costUsd: 0.003 });
  render(<BranchNameSuggest aiEligible workingDirty onPick={onPick} suggest={suggest} />);
  return { onPick };
}

describe('BranchNameSuggest chips', () => {
  it('names every chip by its complete branch name, however long', async () => {
    const user = userEvent.setup();
    renderSuggest();
    await user.click(screen.getByRole('button', { name: 'Suggest name' }));
    // getByRole matches on the ACCESSIBLE NAME, so this asserts the a11y tree,
    // not the DOM text: the whole 90-char ref, unsliced, no separating space.
    for (const name of BRANCH_NAMES_FROM_WORKING) {
      expect(await screen.findByRole('button', { name })).toBeInTheDocument();
    }
    const chip = screen.getByRole('button', { name: LONG });
    expect(chip.textContent).toBe(LONG);
    expect(chip).toHaveAttribute('title', `Use "${LONG}"`);
  });

  it('picks the complete name, not the visible fragment', async () => {
    const user = userEvent.setup();
    const { onPick } = renderSuggest();
    await user.click(screen.getByRole('button', { name: 'Suggest name' }));
    await user.click(await screen.findByRole('button', { name: LONG }));
    expect(onPick).toHaveBeenCalledWith(LONG);
  });
});
