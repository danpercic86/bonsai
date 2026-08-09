/** T3.3a — ContextMenu primitive: item activation + close, danger tone,
 *  disabled rows, keyboard nav, submenu open (keyboard), and dismiss paths
 *  (Escape, outside pointerdown, scroll). */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ContextMenu, type ContextMenuItem } from './ContextMenu';

function makeItems() {
  const onCheckout = vi.fn();
  const onDelete = vi.fn();
  const onSoft = vi.fn();
  const items: ContextMenuItem[] = [
    { label: 'Checkout', onSelect: onCheckout },
    { label: 'Reset', children: [{ label: 'Soft', onSelect: onSoft }] },
    { label: 'Blocked', onSelect: vi.fn(), disabled: true },
    { label: 'Delete branch', onSelect: onDelete, tone: 'danger' },
  ];
  return { items, onCheckout, onDelete, onSoft };
}

function renderMenu() {
  const fixtures = makeItems();
  const onClose = vi.fn();
  const utils = render(<ContextMenu x={40} y={40} items={fixtures.items} onClose={onClose} />);
  return { ...utils, ...fixtures, onClose };
}

describe('ContextMenu', () => {
  it('renders all rows as menuitems and focuses the first enabled one', () => {
    renderMenu();
    expect(screen.getAllByRole('menuitem')).toHaveLength(4);
    expect(screen.getByRole('menuitem', { name: 'Checkout' })).toHaveFocus();
  });

  it('clicking an item runs onSelect then onClose', () => {
    const { onCheckout, onClose } = renderMenu();
    fireEvent.click(screen.getByRole('menuitem', { name: 'Checkout' }));
    expect(onCheckout).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('danger items carry data-tone="danger"; plain items do not', () => {
    renderMenu();
    expect(screen.getByRole('menuitem', { name: 'Delete branch' })).toHaveAttribute(
      'data-tone',
      'danger',
    );
    expect(screen.getByRole('menuitem', { name: 'Checkout' })).not.toHaveAttribute('data-tone');
  });

  it('disabled rows are inert', () => {
    const { items, onClose } = renderMenu();
    const blocked = screen.getByRole('menuitem', { name: 'Blocked' });
    expect(blocked).toBeDisabled();
    fireEvent.click(blocked);
    expect(items[2].onSelect).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });

  it('ArrowDown/ArrowUp move focus between rows, skipping disabled', () => {
    renderMenu();
    const checkout = screen.getByRole('menuitem', { name: 'Checkout' });
    fireEvent.keyDown(checkout, { key: 'ArrowDown' });
    const reset = screen.getByRole('menuitem', { name: 'Reset' });
    expect(reset).toHaveFocus();
    // Down again skips the disabled "Blocked" row.
    fireEvent.keyDown(reset, { key: 'ArrowDown' });
    expect(screen.getByRole('menuitem', { name: 'Delete branch' })).toHaveFocus();
  });

  it('ArrowRight opens the submenu; its item activates and closes the whole menu', () => {
    const { onSoft, onClose } = renderMenu();
    const reset = screen.getByRole('menuitem', { name: 'Reset' });
    expect(reset).toHaveAttribute('aria-haspopup', 'menu');
    expect(reset).toHaveAttribute('aria-expanded', 'false');
    fireEvent.keyDown(reset, { key: 'ArrowRight' });
    expect(reset).toHaveAttribute('aria-expanded', 'true');
    const soft = screen.getByRole('menuitem', { name: 'Soft' });
    fireEvent.click(soft);
    expect(onSoft).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('Enter on a submenu parent toggles it open (no onSelect defined)', () => {
    renderMenu();
    const reset = screen.getByRole('menuitem', { name: 'Reset' });
    fireEvent.keyDown(reset, { key: 'Enter' });
    expect(screen.getByRole('menuitem', { name: 'Soft' })).toBeInTheDocument();
  });

  it('Escape closes without running anything', () => {
    const { onCheckout, onClose } = renderMenu();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onCheckout).not.toHaveBeenCalled();
  });

  it('pointerdown outside closes; inside does not', () => {
    const { onClose } = renderMenu();
    fireEvent.pointerDown(screen.getByRole('menuitem', { name: 'Checkout' }));
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.pointerDown(document.body);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('scroll anywhere (capture) closes the menu', () => {
    const { onClose } = renderMenu();
    fireEvent.scroll(window);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
