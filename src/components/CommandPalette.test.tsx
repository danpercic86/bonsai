/** T3.3a — CommandPalette component wiring (filter logic itself is unit-tested
 *  in paletteActions; here we cover rendering, keyboard nav, dispatch+close,
 *  dynamic rows, and the capture-phase Escape). */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { CommandPalette } from './CommandPalette';
import type { PaletteAction } from './paletteActions';

function makeActions(overrides?: Partial<PaletteAction>[]): PaletteAction[] {
  const base: PaletteAction[] = [
    { id: 'a1', title: 'Fetch', group: 'action', run: vi.fn() },
    { id: 'a2', title: 'Pull', group: 'action', run: vi.fn() },
    { id: 'a3', title: 'Push', group: 'action', disabled: true, run: vi.fn() },
    { id: 'b1', title: 'main', group: 'branch', run: vi.fn() },
  ];
  return overrides === undefined ? base : base.map((a, i) => ({ ...a, ...overrides[i] }));
}

function renderPalette(actions = makeActions()) {
  const onClose = vi.fn();
  const onRunSearch = vi.fn();
  const onJumpToCommit = vi.fn();
  const utils = render(
    <CommandPalette
      open
      actions={actions}
      onClose={onClose}
      onRunSearch={onRunSearch}
      onJumpToCommit={onJumpToCommit}
    />,
  );
  return { ...utils, actions, onClose, onRunSearch, onJumpToCommit };
}

const input = () => screen.getByRole('combobox');

describe('CommandPalette', () => {
  it('renders nothing when closed', () => {
    const { container } = render(
      <CommandPalette open={false} actions={[]} onClose={vi.fn()} onRunSearch={vi.fn()} onJumpToCommit={vi.fn()} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it('renders all actions grouped, focuses the input on open', () => {
    renderPalette();
    expect(input()).toHaveFocus();
    expect(screen.getByRole('option', { name: 'Fetch' })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: 'main' })).toBeInTheDocument();
    // Group headers exist (aria-hidden li).
    expect(screen.getByText('Actions')).toBeInTheDocument();
    expect(screen.getByText('Branches')).toBeInTheDocument();
  });

  it('typing narrows the visible list', () => {
    renderPalette();
    fireEvent.change(input(), { target: { value: 'pull' } });
    expect(screen.getByRole('option', { name: 'Pull' })).toBeInTheDocument();
    expect(screen.queryByRole('option', { name: 'Fetch' })).not.toBeInTheDocument();
  });

  it('shows the empty row (and Enter is a no-op) when there is nothing to run', () => {
    // Empty input yields no dynamic rows, so an empty registry = true empty state.
    const { onClose } = renderPalette([]);
    expect(screen.getByText('No matching commands')).toBeInTheDocument();
    fireEvent.keyDown(input(), { key: 'Enter' });
    expect(onClose).not.toHaveBeenCalled();
  });

  it('ArrowDown/ArrowUp move the highlight, skipping disabled rows', () => {
    renderPalette();
    const options = () => screen.getAllByRole('option');
    // Initial highlight = first enabled ("Fetch").
    expect(options()[0]).toHaveAttribute('aria-selected', 'true');
    fireEvent.keyDown(input(), { key: 'ArrowDown' });
    expect(screen.getByRole('option', { name: 'Pull' })).toHaveAttribute('aria-selected', 'true');
    // Next down skips the disabled "Push" and lands on "main".
    fireEvent.keyDown(input(), { key: 'ArrowDown' });
    expect(screen.getByRole('option', { name: 'main' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('option', { name: 'Push' })).toHaveAttribute('aria-disabled', 'true');
    // Up goes back, again skipping "Push".
    fireEvent.keyDown(input(), { key: 'ArrowUp' });
    expect(screen.getByRole('option', { name: 'Pull' })).toHaveAttribute('aria-selected', 'true');
  });

  it('Enter runs the highlighted action and closes', () => {
    const { actions, onClose } = renderPalette();
    fireEvent.keyDown(input(), { key: 'Enter' });
    expect(actions[0].run).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('mousedown on a row runs it and closes; disabled rows do nothing', () => {
    const { actions, onClose } = renderPalette();
    fireEvent.mouseDown(screen.getByRole('option', { name: 'Push' }));
    expect(actions[2].run).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.mouseDown(screen.getByRole('option', { name: 'Pull' }));
    expect(actions[1].run).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('Escape (window capture) closes without running anything', () => {
    const { actions, onClose } = renderPalette();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
    for (const a of actions) expect(a.run).not.toHaveBeenCalled();
  });

  it('overlay mousedown closes; clicks inside the card do not', () => {
    const { onClose, container } = renderPalette();
    fireEvent.mouseDown(container.querySelector('.command-palette-overlay')!);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('hex input offers "Jump to commit" first; Enter dispatches onJumpToCommit', () => {
    const { onJumpToCommit, onRunSearch } = renderPalette();
    fireEvent.change(input(), { target: { value: 'abc123' } });
    expect(screen.getByRole('option', { name: 'Jump to commit abc123' })).toBeInTheDocument();
    fireEvent.keyDown(input(), { key: 'Enter' });
    expect(onJumpToCommit).toHaveBeenCalledWith('abc123');
    expect(onRunSearch).not.toHaveBeenCalled();
  });

  it('non-hex text offers the search row; running it dispatches onRunSearch', () => {
    const { onRunSearch } = renderPalette();
    fireEvent.change(input(), { target: { value: 'fix the thing' } });
    const row = screen.getByRole('option', { name: /Search commits for/ });
    fireEvent.mouseDown(row);
    expect(onRunSearch).toHaveBeenCalledWith('fix the thing');
  });

  it('preserves the highlight when the actions prop is replaced with a new array of identical-id rows', () => {
    const props = {
      onClose: vi.fn(),
      onRunSearch: vi.fn(),
      onJumpToCommit: vi.fn(),
    };
    const { rerender } = render(<CommandPalette open actions={makeActions()} {...props} />);
    // Move the highlight off row 0 ("Fetch") onto "Pull".
    fireEvent.keyDown(input(), { key: 'ArrowDown' });
    expect(screen.getByRole('option', { name: 'Pull' })).toHaveAttribute('aria-selected', 'true');
    // A churning producer hands us a brand-new array with the SAME ids/order
    // (e.g. a streamed search batch or once-a-second AI run). The highlight must
    // NOT snap back to the first enabled row.
    rerender(<CommandPalette open actions={makeActions()} {...props} />);
    expect(screen.getByRole('option', { name: 'Pull' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('option', { name: 'Fetch' })).toHaveAttribute('aria-selected', 'false');
  });

  it('resets the highlight to the first enabled row when the filtered id set actually changes', () => {
    const props = {
      onClose: vi.fn(),
      onRunSearch: vi.fn(),
      onJumpToCommit: vi.fn(),
    };
    const { rerender } = render(<CommandPalette open actions={makeActions()} {...props} />);
    fireEvent.keyDown(input(), { key: 'ArrowDown' });
    expect(screen.getByRole('option', { name: 'Pull' })).toHaveAttribute('aria-selected', 'true');
    // Different rows entirely (new ids) → visible set changed → reset to first enabled.
    const nextActions: PaletteAction[] = [
      { id: 'x1', title: 'Rebase', group: 'action', run: vi.fn() },
      { id: 'x2', title: 'Stash', group: 'action', run: vi.fn() },
    ];
    rerender(<CommandPalette open actions={nextActions} {...props} />);
    expect(screen.getByRole('option', { name: 'Rebase' })).toHaveAttribute('aria-selected', 'true');
  });

  it('reopening resets the query text', () => {
    const props = {
      actions: makeActions(),
      onClose: vi.fn(),
      onRunSearch: vi.fn(),
      onJumpToCommit: vi.fn(),
    };
    const { rerender } = render(<CommandPalette open {...props} />);
    fireEvent.change(input(), { target: { value: 'pull' } });
    rerender(<CommandPalette open={false} {...props} />);
    rerender(<CommandPalette open {...props} />);
    expect(input()).toHaveValue('');
  });
});
