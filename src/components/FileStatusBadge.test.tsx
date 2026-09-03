/** P109 §10 — FileStatusBadge: the single owner of the status glyph + its name.
 *  Covers all 8 BadgeStatus values on BOTH channels (visible letter and computed
 *  accessible name), the `added` vs `untracked` distinction the contract exists
 *  to restore, and the fact that the label REPLACES the glyph in the
 *  accessibility tree rather than prefixing it (so `textContent` is unchanged). */
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { FileStatusBadge, type BadgeStatus } from './FileStatusBadge';

const CASES: ReadonlyArray<readonly [BadgeStatus, string, string]> = [
  ['added', 'A', 'Added'],
  ['modified', 'M', 'Modified'],
  ['deleted', 'D', 'Deleted'],
  ['renamed', 'R', 'Renamed'],
  ['typechange', 'T', 'Type changed'],
  ['conflicted', 'C', 'Conflicted'],
  ['untracked', 'U', 'Untracked'],
  ['unknown', '?', 'Status unknown'],
];

describe('FileStatusBadge', () => {
  it.each(CASES)('%s renders %s named "%s"', (status, letter, name) => {
    const { container } = render(<FileStatusBadge status={status} />);
    const badge = screen.getByRole('img', { name });
    expect(badge.textContent).toBe(letter);
    // The glyph stays the visible text: no hidden sibling, so a row's
    // textContent-based queries keep working (contract §4.1, AC7).
    expect(container.textContent).toBe(letter);
  });

  it('distinguishes added from untracked on both channels', () => {
    const added = render(<FileStatusBadge status="added" />).container
      .firstElementChild as HTMLElement;
    const untracked = render(<FileStatusBadge status="untracked" />).container
      .firstElementChild as HTMLElement;
    expect(added.textContent).not.toBe(untracked.textContent);
    expect(added.getAttribute('aria-label')).not.toBe(untracked.getAttribute('aria-label'));
  });

  it('keeps the shared badge class so the status-class ink rules still apply', () => {
    const { container } = render(<FileStatusBadge status="modified" />);
    expect(container.firstElementChild?.className).toBe('file-badge mono');
  });
});
