/** P111 §3.3 + §5 — RefLabel splits at the LAST `/` so the identifying leaf is
 *  never the part that ellipsizes, and it never shortens the string: the DOM
 *  always holds the whole ref, which is what keeps the accessible name correct.
 *  (The trailing-`/` degenerate case lives here rather than in the branch-name
 *  suggestion fixture, because a suggested name must be a valid branch name.) */
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { RefLabel } from './RefLabel';

const LONG = 'feature/observability/rewrite-the-per-commit-blame-why-layer-behind-a-cached-lane-index-v2';

describe('RefLabel', () => {
  it('renders the head and the leaf as separate spans, holding the whole value', () => {
    const { container } = render(<RefLabel value={LONG} />);
    const head = container.querySelector('.ref-label-head');
    const leaf = container.querySelector('.ref-label-leaf');
    expect(head?.textContent).toBe('feature/observability/');
    expect(leaf?.textContent).toBe('rewrite-the-per-commit-blame-why-layer-behind-a-cached-lane-index-v2');
    // The whole string is in the DOM — truncation is CSS-only, never a JS slice.
    expect(container.textContent).toBe(LONG);
    expect(container.textContent).not.toContain('…');
  });

  it('a value with no separator renders one leaf span and no head', () => {
    const { container } = render(<RefLabel value="rewrite-the-cached-lane-index" />);
    expect(container.querySelector('.ref-label-head')).toBeNull();
    expect(container.querySelector('.ref-label-leaf')?.textContent).toBe('rewrite-the-cached-lane-index');
  });

  it('a value ending in a separator keeps the whole value in the head, not blank', () => {
    const { container } = render(<RefLabel value="feature/observability/cached-lane-index/" />);
    expect(container.querySelector('.ref-label-head')?.textContent).toBe(
      'feature/observability/cached-lane-index/',
    );
    expect(container.querySelector('.ref-label-leaf')?.textContent).toBe('');
    expect(container.textContent).toBe('feature/observability/cached-lane-index/');
  });

  it('adds the surface class and only opts into a title when asked', () => {
    const { container, rerender } = render(<RefLabel value={LONG} className="chip-ref" />);
    const wrapper = container.querySelector('.ref-label');
    expect(wrapper?.className).toBe('ref-label chip-ref');
    expect(wrapper?.getAttribute('title')).toBeNull();
    rerender(<RefLabel value={LONG} withTitle />);
    expect(screen.getByTitle(LONG)).toHaveClass('ref-label');
  });
});
