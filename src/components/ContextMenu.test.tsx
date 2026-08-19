/** T3.3a — ContextMenu primitive: item activation + close, danger tone,
 *  disabled rows, keyboard nav, submenu open (keyboard), and dismiss paths
 *  (Escape, outside pointerdown, scroll). */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/react';
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

/** P69i (UI §4.4) — the three additive fields. Each must be invisible when
 *  absent: every existing call site (branch menus, the push caret, the external
 *  tools dropdown) passes none of them and must render exactly as before. */
describe('ContextMenu — checked / detail / header (P69i)', () => {
  it('a row without `checked` stays a plain menuitem with no check column', () => {
    renderMenu();
    expect(screen.queryAllByRole('menuitemradio')).toHaveLength(0);
    expect(document.querySelector('.context-menu-check')).toBeNull();
    expect(document.querySelector('.context-menu-header')).toBeNull();
    expect(document.querySelector('.context-menu-detail')).toBeNull();
    // The label element is unchanged (no extra wrapper) when `detail` is absent.
    const row = screen.getByRole('menuitem', { name: 'Checkout' });
    expect(row.querySelector('.context-menu-lines')).toBeNull();
    expect(row.querySelector('.context-menu-label')?.textContent).toBe('Checkout');
  });

  it('`checked` renders menuitemradio + aria-checked and reserves the column', () => {
    const items: ContextMenuItem[] = [
      { label: 'Work', checked: true, onSelect: vi.fn() },
      { label: 'Personal', checked: false, onSelect: vi.fn() },
    ];
    render(<ContextMenu x={10} y={10} items={items} onClose={vi.fn()} />);

    const on = screen.getByRole('menuitemradio', { name: /Work/ });
    const off = screen.getByRole('menuitemradio', { name: /Personal/ });
    expect(on).toHaveAttribute('aria-checked', 'true');
    expect(off).toHaveAttribute('aria-checked', 'false');
    // The column exists on BOTH rows, so ticking one never shifts the other.
    expect(on.querySelector('.context-menu-check')?.textContent).toBe('✓');
    expect(off.querySelector('.context-menu-check')?.textContent).toBe('');
  });

  it('the check column is reserved on EVERY row of a checked list', () => {
    // The failure this pins: reserving the column per-ROW leaves the plain tail
    // rows 24px to the left of the profile labels — a ragged menu. Comparing two
    // radio rows to each other cannot see it; comparing a radio to a plain one can.
    const items: ContextMenuItem[] = [
      { label: 'Work', checked: true, onSelect: vi.fn() },
      { label: 'Manage identities…', onSelect: vi.fn() },
    ];
    render(<ContextMenu x={10} y={10} items={items} onClose={vi.fn()} />);

    const radio = screen.getByRole('menuitemradio', { name: /Work/ });
    const plain = screen.getByRole('menuitem', { name: 'Manage identities…' });
    expect(radio.querySelector('.context-menu-check')).not.toBeNull();
    expect(plain.querySelector('.context-menu-check')).not.toBeNull();
    expect(plain.querySelector('.context-menu-check')?.textContent).toBe('');
    // …and a list with no checked row at all still reserves nothing.
    cleanup();
    render(
      <ContextMenu x={10} y={10} items={[{ label: 'Plain', onSelect: vi.fn() }]} onClose={vi.fn()} />,
    );
    expect(document.querySelector('.context-menu-check')).toBeNull();
  });

  it('a row whose label mutates keeps its DOM node, and its focus', () => {
    // The identity menu stays open while a write settles and renames the active
    // row to "… — Applying…". A label-derived key would remount that button and
    // drop focus to <body> for the whole in-flight window.
    const items: ContextMenuItem[] = [
      { label: 'Work', checked: false, onSelect: vi.fn() },
      { label: 'Personal', checked: false, onSelect: vi.fn() },
    ];
    const { rerender } = render(<ContextMenu x={10} y={10} items={items} onClose={vi.fn()} />);
    const before = screen.getByRole('menuitemradio', { name: /Work/ });
    before.focus();

    rerender(
      <ContextMenu
        x={10}
        y={10}
        items={[{ ...items[0], label: 'Work — Applying…' }, items[1]]}
        onClose={vi.fn()}
      />,
    );

    const after = screen.getByRole('menuitemradio', { name: /Applying…/ });
    expect(after).toBe(before);
    expect(after).toHaveFocus();
  });

  it('menuitemradio rows are keyboard-navigable exactly like menuitems', () => {
    const items: ContextMenuItem[] = [
      { label: 'Work', checked: true, onSelect: vi.fn() },
      { label: 'Personal', checked: false, onSelect: vi.fn() },
      { label: 'Manage identities…', onSelect: vi.fn() },
    ];
    render(<ContextMenu x={10} y={10} items={items} onClose={vi.fn()} />);

    // Focus queries used to scope to [role="menuitem"] only — a radio row would
    // have been skipped by ArrowDown, which is the whole risk of this change.
    const first = screen.getByRole('menuitemradio', { name: /Work/ });
    expect(first).toHaveFocus();
    fireEvent.keyDown(first, { key: 'ArrowDown' });
    expect(screen.getByRole('menuitemradio', { name: /Personal/ })).toHaveFocus();
    fireEvent.keyDown(screen.getByRole('menuitemradio', { name: /Personal/ }), {
      key: 'ArrowDown',
    });
    expect(screen.getByRole('menuitem', { name: 'Manage identities…' })).toHaveFocus();
  });

  it('`detail` adds a second line inside the row and keeps it in the name', () => {
    const items: ContextMenuItem[] = [
      { label: 'Work', detail: 'Ada Lovelace · work@bonsai.dev', onSelect: vi.fn() },
    ];
    render(<ContextMenu x={10} y={10} items={items} onClose={vi.fn()} />);

    const row = screen.getByRole('menuitem', { name: /Work/ });
    expect(row.querySelector('.context-menu-detail')?.textContent).toBe(
      'Ada Lovelace · work@bonsai.dev',
    );
    // Long details ellipsise, so the full string has to be reachable.
    expect(row.querySelector('.context-menu-detail')).toHaveAttribute(
      'title',
      'Ada Lovelace · work@bonsai.dev',
    );
  });

  it('`header` renders above the list, is presentational, and is not focusable', () => {
    const items: ContextMenuItem[] = [{ label: 'Only', onSelect: vi.fn() }];
    render(
      <ContextMenu
        x={10}
        y={10}
        items={items}
        header={<p>{'Committing as'}</p>}
        busy
        onClose={vi.fn()}
      />,
    );

    const header = document.querySelector('.context-menu-header');
    expect(header).not.toBeNull();
    expect(header).toHaveAttribute('role', 'presentation');
    expect(screen.getByRole('menu')).toHaveAttribute('aria-busy', 'true');
    // Focus still lands on the first ROW, never on the header.
    expect(screen.getByRole('menuitem', { name: 'Only' })).toHaveFocus();
    expect(screen.getAllByRole('menuitem')).toHaveLength(1);
  });
});
