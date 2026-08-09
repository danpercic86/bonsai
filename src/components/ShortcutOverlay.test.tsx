/** T3.5 — ShortcutOverlay: renders the full binding table (incl. the P50
 *  Ctrl+F / Ctrl+K rows) and closes on backdrop click / ✕. Esc is handled by
 *  App's global overlay handler, not this component. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ShortcutOverlay } from './ShortcutOverlay';

describe('ShortcutOverlay', () => {
  it('renders nothing when closed', () => {
    const { container } = render(<ShortcutOverlay open={false} onClose={vi.fn()} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('renders every binding row, including search and palette', () => {
    const { container } = render(<ShortcutOverlay open onClose={vi.fn()} />);
    expect(screen.getByRole('dialog', { name: 'Keyboard shortcuts' })).toBeInTheDocument();
    const rows = container.querySelectorAll('.shortcut-row');
    expect(rows).toHaveLength(15);
    const actions = [
      'Commit staged changes',
      'Deselect commit / close dialog',
      'Open repository',
      'Search commits',
      'Open command palette',
      'Fetch all remotes',
      'Pull (fast-forward only)',
      'Push current branch',
      'Move commit selection',
      'Move commit selection by one screenful',
      'Select the topmost commit',
      'Select the last commit',
      'Toggle this overlay',
    ];
    for (const a of actions) expect(screen.getByText(a)).toBeInTheDocument();
    expect(screen.getAllByText('Refresh')).toHaveLength(2); // Ctrl+R and F5
    // Key caps for the two P50 rows ('F' also appears in Ctrl+Shift+F).
    expect(screen.getAllByText('F').length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText('K')).toBeInTheDocument();
  });

  it('✕ closes; backdrop mousedown closes; clicks inside the card do not', () => {
    const onClose = vi.fn();
    const { container } = render(<ShortcutOverlay open onClose={onClose} />);
    fireEvent.mouseDown(screen.getByRole('dialog'));
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(onClose).toHaveBeenCalledTimes(1);
    fireEvent.mouseDown(container.querySelector('.dialog-overlay')!);
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
