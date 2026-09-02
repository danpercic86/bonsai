/** P95 §2 — focus follows consumption + the `defaultPrevented` guard.
 *
 *  Covers AC5 (every graph-nav branch that calls `preventDefault` also calls
 *  `focusScroller`), AC7 (overlay/typing guards still win) and AC17 (a widget
 *  that already consumed the arrow key keeps both the selection and the focus). */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

import { useWorkspaceKeyboard } from './useWorkspaceKeyboard';
import {
  graphHandle,
  graphOf,
  makeKeyboardDeps,
  pressKey,
} from './fixtures/workspaceKeyboardDeps';
import type { WorkspaceKeyboardDeps } from './fixtures/workspaceKeyboardDeps';

afterEach(() => vi.restoreAllMocks());

/** A DOM node that mirrors GitActivityDock: `preventDefault`, no `stopPropagation`. */
function consumingRow(): HTMLDivElement {
  const el = document.createElement('div');
  document.body.appendChild(el);
  el.addEventListener('keydown', (e) => e.preventDefault());
  return el;
}

describe('P95 §2 focus follows consumption', () => {
  it('seeds the selection and focuses the scroller on the first nav key', () => {
    const h = graphHandle();
    const deps = makeKeyboardDeps({
      graph: graphOf(10),
      selectedIndex: null,
      graphRef: { current: h },
    });
    renderHook(() => useWorkspaceKeyboard(deps));

    const ev = pressKey('ArrowDown');
    expect(ev.defaultPrevented).toBe(true);
    expect(deps.setSelectedIndex).toHaveBeenCalledWith(0);
    expect(h.focusScroller).toHaveBeenCalledTimes(1);
  });

  it.each(['ArrowDown', 'ArrowUp', 'PageDown', 'PageUp', 'Home', 'End'])(
    'focuses the scroller when %s moves an existing selection',
    (key) => {
      const h = graphHandle();
      const deps = makeKeyboardDeps({
        graph: graphOf(10),
        selectedIndex: 3,
        graphRef: { current: h },
      });
      renderHook(() => useWorkspaceKeyboard(deps));

      const ev = pressKey(key);
      expect(ev.defaultPrevented).toBe(true);
      expect(deps.setSelectedIndex).toHaveBeenCalledTimes(1);
      expect(h.focusScroller).toHaveBeenCalledTimes(1);
    },
  );

  it('does not focus the scroller on the fetch/pull/push shortcuts', () => {
    const h = graphHandle();
    const deps = makeKeyboardDeps({
      graph: graphOf(10),
      selectedIndex: 3,
      graphRef: { current: h },
    });
    renderHook(() => useWorkspaceKeyboard(deps));

    pressKey('F', { ctrlKey: true, shiftKey: true });
    pressKey('P', { ctrlKey: true, shiftKey: true });
    pressKey('U', { ctrlKey: true, shiftKey: true });
    expect(h.focusScroller).not.toHaveBeenCalled();
  });

  it('tolerates a null graphRef (no handle mounted yet)', () => {
    const deps = makeKeyboardDeps({
      graph: graphOf(10),
      selectedIndex: 3,
      graphRef: { current: null },
    });
    renderHook(() => useWorkspaceKeyboard(deps));

    expect(() => pressKey('ArrowDown')).not.toThrow();
    expect(deps.setSelectedIndex).toHaveBeenCalledTimes(1);
  });
});

describe('P95 §2.1 defaultPrevented guard (AC17)', () => {
  it('leaves the selection and focus alone when another widget consumed the key', () => {
    const h = graphHandle();
    const deps = makeKeyboardDeps({
      graph: graphOf(10),
      selectedIndex: 3,
      graphRef: { current: h },
    });
    renderHook(() => useWorkspaceKeyboard(deps));

    const row = consumingRow();
    pressKey('ArrowDown', {}, row);
    expect(deps.setSelectedIndex).not.toHaveBeenCalled();
    expect(h.focusScroller).not.toHaveBeenCalled();
    row.remove();
  });

  it('seeds nothing when the key was consumed and there is no selection', () => {
    const h = graphHandle();
    const deps = makeKeyboardDeps({
      graph: graphOf(10),
      selectedIndex: null,
      graphRef: { current: h },
    });
    renderHook(() => useWorkspaceKeyboard(deps));

    const row = consumingRow();
    pressKey('ArrowUp', {}, row);
    expect(deps.setSelectedIndex).not.toHaveBeenCalled();
    expect(h.focusScroller).not.toHaveBeenCalled();
    row.remove();
  });
});

describe('P95 §2.2 item 4 — existing guards still run first (AC7)', () => {
  it('does not focus the scroller while typing in an input', () => {
    const h = graphHandle();
    const deps = makeKeyboardDeps({
      graph: graphOf(10),
      selectedIndex: 3,
      graphRef: { current: h },
    });
    renderHook(() => useWorkspaceKeyboard(deps));

    const input = document.createElement('input');
    document.body.appendChild(input);
    pressKey('ArrowDown', {}, input);
    expect(deps.setSelectedIndex).not.toHaveBeenCalled();
    expect(h.focusScroller).not.toHaveBeenCalled();
    input.remove();
  });

  it.each([
    ['dialogOpen', { dialogOpen: true }],
    ['abortConfirmOpen', { abortConfirmOpen: true }],
    ['searchOpen', { searchOpen: true }],
    ['paletteOpen', { paletteOpen: true }],
    ['composerOpen', { composerOpen: true }],
    ['historySearchOpen', { historySearchOpen: true }],
  ])('does not focus the scroller while %s', (_label, over) => {
    const h = graphHandle();
    const deps = makeKeyboardDeps({
      graph: graphOf(10),
      selectedIndex: 3,
      graphRef: { current: h },
      ...(over as Partial<WorkspaceKeyboardDeps>),
    });
    renderHook(() => useWorkspaceKeyboard(deps));

    pressKey('ArrowDown');
    expect(deps.setSelectedIndex).not.toHaveBeenCalled();
    expect(h.focusScroller).not.toHaveBeenCalled();
  });
});
