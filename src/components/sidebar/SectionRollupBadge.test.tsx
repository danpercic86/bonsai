/** P77 §2.5 — SectionRollupBadge: the collapsed-section rollup pill. Priority
 *  order is busy → count>0 (+ the P123 refreshing cue) → nothing; the ⚠ glyph
 *  and count are aria-hidden while the accessible name lives on the pill. Presentational: props in, DOM out. */
import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { SectionRollupBadge } from './SectionRollupBadge';

describe('SectionRollupBadge', () => {
  it('renders a ⚠ count pill named by ariaLabel when count > 0', () => {
    const { container } = render(
      <SectionRollupBadge count={2} busy={false} label="checking…" ariaLabel="2 tags out of sync on origin" />,
    );
    const pill = screen.getByLabelText('2 tags out of sync on origin');
    expect(pill).toHaveClass('submodule-badge-warn');
    // The count digit is present but aria-hidden (the count is a non-colour carrier,
    // yet the screen-reader name comes from ariaLabel to disambiguate the ⚠).
    expect(container.textContent).toContain('2');
    expect(container.querySelector('.submodule-badge-glyph')).toHaveAttribute('aria-hidden', 'true');
    // Default title falls back to the ariaLabel when no title prop is given.
    expect(pill).toHaveAttribute('title', '2 tags out of sync on origin');
  });

  it('prefers an explicit title over the ariaLabel', () => {
    render(
      <SectionRollupBadge
        count={3}
        busy={false}
        label="checking…"
        ariaLabel="3 tags out of sync on origin"
        title="3 tags differ from origin. Expand to resolve."
      />,
    );
    const pill = screen.getByLabelText('3 tags out of sync on origin');
    expect(pill).toHaveAttribute('title', '3 tags differ from origin. Expand to resolve.');
  });

  it('busy wins over count: shows the muted checking pill with aria-busy', () => {
    const { container } = render(
      <SectionRollupBadge count={5} busy label="checking…" ariaLabel="checking origin" />,
    );
    const pill = container.querySelector('span.branch-badge');
    expect(pill).toHaveClass('submodule-badge-muted');
    expect(pill).toHaveAttribute('aria-busy', 'true');
    expect(container.textContent).toContain('checking…');
    // No ⚠ verdict while busy.
    expect(container.querySelector('.submodule-badge-warn')).toBeNull();
  });

  it('the warn pill is a named image (P123 §1.5 role="img")', () => {
    render(<SectionRollupBadge count={1} busy={false} label="checking…" ariaLabel="1 tag out of sync on origin" />);
    const pill = screen.getByRole('img', { name: '1 tag out of sync on origin' });
    expect(pill).not.toHaveAttribute('aria-busy');
  });

  it('refreshing + count > 0: the SAME warn pill, plus aria-busy and the re-check name/title', () => {
    const { container } = render(
      <SectionRollupBadge
        count={2}
        busy={false}
        refreshing
        label="checking…"
        ariaLabel="2 tags out of sync on origin, checking again"
        title="2 tags differ from origin — checking again…"
      />,
    );
    const pill = screen.getByRole('img', { name: '2 tags out of sync on origin, checking again' });
    expect(pill).toHaveClass('submodule-badge-warn');
    expect(pill).toHaveAttribute('aria-busy', 'true');
    expect(pill).toHaveAttribute('title', '2 tags differ from origin — checking again…');
    expect(container.textContent).not.toContain('checking…');
  });

  it('refreshing + count 0 renders nothing', () => {
    const { container } = render(
      <SectionRollupBadge count={0} busy={false} refreshing label="checking…" ariaLabel="x" />,
    );
    expect(container.firstChild).toBeNull();
  });

  it('busy wins over refreshing: the hueless label pill', () => {
    const { container } = render(
      <SectionRollupBadge count={4} busy refreshing label="checking…" ariaLabel="x" />,
    );
    const pill = container.querySelector('span.branch-badge');
    expect(pill).toHaveClass('submodule-badge-muted');
    expect(pill).toHaveAttribute('aria-busy', 'true');
    expect(container.querySelector('.submodule-badge-warn')).toBeNull();
  });

  it('renders nothing for a clean section (count 0, not busy)', () => {
    const { container } = render(
      <SectionRollupBadge count={0} busy={false} label="checking…" ariaLabel="all clear" />,
    );
    expect(container.firstChild).toBeNull();
  });
});
