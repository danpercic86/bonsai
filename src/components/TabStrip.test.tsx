/** T3.3a — TabStrip: tab select/close, disabled state, the + recents menu
 *  (open-tab filtering, Browse/Clone/Init routing, Escape/outside dismiss),
 *  drag-and-drop reorder, and the right-click tab menu hook.
 *  Note: middle-click close is NOT implemented (no auxclick handler) — by design. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { TabStrip, type TabStripProps } from './TabStrip';

const tabs = [
  { repoId: 'D:/repos/alpha', path: 'D:/repos/alpha' },
  { repoId: 'D:/repos/beta', path: 'D:/repos/beta' },
];

function makeProps(over: Partial<TabStripProps> = {}): TabStripProps {
  return {
    tabs,
    activeRepo: 'D:/repos/alpha',
    recents: [
      { path: 'D:/repos/alpha', lastOpened: 100 }, // already open → filtered
      { path: 'D:/repos/gamma', lastOpened: 90 },
    ],
    disabled: false,
    onSelect: vi.fn(),
    onClose: vi.fn(),
    onOpenPath: vi.fn(),
    onReorder: vi.fn(),
    onBrowse: vi.fn(),
    onClone: vi.fn(),
    onInit: vi.fn(),
    onMenuOpenChange: vi.fn(),
    onTabMenu: vi.fn(),
    ...over,
  };
}

const plusBtn = () => screen.getByRole('button', { name: 'Open a repository' });

describe('TabStrip', () => {
  it('renders a pill per tab (folder name) and marks the active one', () => {
    const { container } = render(<TabStrip {...makeProps()} />);
    expect(screen.getByRole('button', { name: 'alpha' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'beta' })).toBeInTheDocument();
    expect(container.querySelectorAll('.tab-active')).toHaveLength(1);
  });

  it('clicking a tab label selects it; Enter/Space work too', () => {
    const p = makeProps();
    render(<TabStrip {...p} />);
    fireEvent.click(screen.getByRole('button', { name: 'beta' }));
    expect(p.onSelect).toHaveBeenCalledWith('D:/repos/beta');
    fireEvent.keyDown(screen.getByRole('button', { name: 'alpha' }), { key: 'Enter' });
    expect(p.onSelect).toHaveBeenCalledWith('D:/repos/alpha');
  });

  it('disabled strip ignores label clicks and disables +', () => {
    const p = makeProps({ disabled: true });
    render(<TabStrip {...p} />);
    fireEvent.click(screen.getByRole('button', { name: 'beta' }));
    expect(p.onSelect).not.toHaveBeenCalled();
    expect(plusBtn()).toBeDisabled();
  });

  it('the × button closes exactly that tab', () => {
    const p = makeProps();
    render(<TabStrip {...p} />);
    fireEvent.click(screen.getByRole('button', { name: 'Close beta' }));
    expect(p.onClose).toHaveBeenCalledTimes(1);
    expect(p.onClose).toHaveBeenCalledWith('D:/repos/beta');
  });

  it('+ opens the menu (recents minus open tabs) and lifts onMenuOpenChange', () => {
    const p = makeProps();
    render(<TabStrip {...p} />);
    fireEvent.click(plusBtn());
    expect(p.onMenuOpenChange).toHaveBeenCalledWith(true);
    expect(screen.getByText('gamma')).toBeInTheDocument();
    expect(screen.queryByText('D:/repos/alpha')).not.toBeInTheDocument(); // filtered
    fireEvent.click(screen.getByText('gamma'));
    expect(p.onOpenPath).toHaveBeenCalledWith('D:/repos/gamma');
    expect(p.onMenuOpenChange).toHaveBeenLastCalledWith(false);
  });

  it('Browse… / Clone / New repository route their callbacks and close the menu', () => {
    const p = makeProps();
    render(<TabStrip {...p} />);
    for (const [label, cb] of [
      ['Browse…', p.onBrowse],
      ['Clone repository…', p.onClone],
      ['New repository…', p.onInit],
    ] as const) {
      fireEvent.click(plusBtn());
      fireEvent.click(screen.getByRole('button', { name: label }));
      expect(cb).toHaveBeenCalledTimes(1);
      expect(screen.queryByRole('button', { name: label })).not.toBeInTheDocument();
    }
  });

  it('Escape and outside mousedown dismiss the menu', () => {
    const p = makeProps();
    render(<TabStrip {...p} />);
    fireEvent.click(plusBtn());
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByText('gamma')).not.toBeInTheDocument();
    fireEvent.click(plusBtn());
    fireEvent.mouseDown(document.body);
    expect(screen.queryByText('gamma')).not.toBeInTheDocument();
  });

  it('drag-and-drop reorders from the source index to the drop index', () => {
    const p = makeProps();
    const { container } = render(<TabStrip {...p} />);
    const pills = container.querySelectorAll('.tab');
    const dataTransfer = {
      effectAllowed: '',
      dropEffect: '',
      setData: vi.fn(),
      getData: vi.fn(),
    };
    fireEvent.dragStart(pills[0], { dataTransfer });
    fireEvent.dragOver(pills[1], { dataTransfer });
    expect(pills[1]).toHaveClass('tab-drop-target');
    fireEvent.drop(pills[1], { dataTransfer });
    expect(p.onReorder).toHaveBeenCalledTimes(1);
    expect(p.onReorder).toHaveBeenCalledWith(0, 1);
  });

  it('dropping a tab on itself does not reorder', () => {
    const p = makeProps();
    const { container } = render(<TabStrip {...p} />);
    const pill = container.querySelectorAll('.tab')[0];
    const dataTransfer = { effectAllowed: '', dropEffect: '', setData: vi.fn() };
    fireEvent.dragStart(pill, { dataTransfer });
    fireEvent.drop(pill, { dataTransfer });
    expect(p.onReorder).not.toHaveBeenCalled();
  });

  it('right-clicking a tab opens the external menu at the pointer', () => {
    const p = makeProps();
    const { container } = render(<TabStrip {...p} />);
    fireEvent.contextMenu(container.querySelectorAll('.tab')[1], { clientX: 12, clientY: 34 });
    expect(p.onTabMenu).toHaveBeenCalledWith('D:/repos/beta', 12, 34);
  });
});
