/** T3.2b — useWorkspaceKeyboard: Esc peel order, shortcut dispatch, typing/dialog
 *  guards, platform (ctrlKey vs metaKey) handling, and a sync check against the
 *  ShortcutOverlay documented table. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, renderHook, screen } from '@testing-library/react';

import { useWorkspaceKeyboard } from './useWorkspaceKeyboard';
import { ShortcutOverlay } from '../ShortcutOverlay';
import type { GraphLayout, GraphNode } from '../../ipc';
import type { GraphCanvasHandle } from '../../graph/GraphCanvas';
import type { DiffSlot } from '../StatusPanel';

afterEach(() => vi.restoreAllMocks());

type Deps = Parameters<typeof useWorkspaceKeyboard>[0];

function node(id: string): GraphNode {
  return { id, lane: 0, parents: [], summary: '', author: '', ts: 0, committerTs: 0 };
}
function graph(n: number): GraphLayout {
  return {
    nodes: Array.from({ length: n }, (_, i) => node(`c${i}`)),
    edges: [],
    laneCount: 1,
    headIndex: null,
    truncated: false,
  };
}

/** All-closed, all-idle deps with fresh spies; override per test. */
function makeDeps(over: Partial<Deps> = {}): Deps {
  return {
    active: true,
    globalModalOpen: false,
    collapseDiffSlot: vi.fn(),
    clearCompare: vi.fn(),
    closeAiPanel: vi.fn(),
    closeBlame: vi.fn(),
    closeHistory: vi.fn(),
    closeReflog: vi.fn(),
    aiPanelOpenRef: { current: false },
    blameOpenRef: { current: false },
    historyOpenRef: { current: false },
    reflogOpenRef: { current: false },
    commitBrowserOpenRef: { current: false },
    composerOpenRef: { current: false },
    closeComposer: vi.fn(),
    composerOpen: false,
    searchOpenRef: { current: false },
    closeSearch: vi.fn(),
    historySearchOpenRef: { current: false },
    closeHistorySearch: vi.fn(),
    paletteOpenRef: { current: false },
    closePalette: vi.fn(),
    diffSlotRef: { current: null },
    compareRef: { current: null },
    setSelectedIndex: vi.fn(),
    setCommitBrowserOpen: vi.fn(),
    searchOpen: false,
    openSearch: vi.fn(),
    historySearchOpen: false,
    paletteOpen: false,
    togglePalette: vi.fn(),
    refreshing: false,
    statusLoading: false,
    graphLoading: false,
    mutating: false,
    canPullPush: true,
    dialogOpen: false,
    abortConfirmOpen: false,
    selectedIndex: null,
    graph: null,
    graphRef: { current: null },
    handleRefresh: vi.fn(),
    handleFetch: vi.fn(),
    handlePull: vi.fn(),
    handlePush: vi.fn(),
    ...over,
  };
}

function mount(deps: Deps) {
  return renderHook((d: Deps) => useWorkspaceKeyboard(d), { initialProps: deps });
}

/** Dispatch a keydown from `target` (default: window) and return the event so
 *  tests can assert defaultPrevented. */
function press(key: string, opts: KeyboardEventInit = {}, target: EventTarget = window) {
  const ev = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...opts });
  target.dispatchEvent(ev);
  return ev;
}

/** A focused-ish input attached to the document so e.target is a real INPUT. */
function attachedInput(tag: 'input' | 'textarea' | 'select' = 'input') {
  const el = document.createElement(tag);
  document.body.appendChild(el);
  return el;
}

// ---------------------------------------------------------------------------
// Esc layering
// ---------------------------------------------------------------------------

describe('Esc peel order', () => {
  it('peels exactly the topmost open layer, in contract order', () => {
    // Full stack open: palette > composer > aiPanel > blame > history > reflog
    // > commitBrowser > search > historySearch > diffSlot > compare > deselect.
    const deps = makeDeps({
      paletteOpenRef: { current: true },
      composerOpenRef: { current: true },
      aiPanelOpenRef: { current: true },
      blameOpenRef: { current: true },
      historyOpenRef: { current: true },
      reflogOpenRef: { current: true },
      commitBrowserOpenRef: { current: true },
      searchOpenRef: { current: true },
      historySearchOpenRef: { current: true },
      diffSlotRef: { current: {} as DiffSlot },
      compareRef: { current: { oid: 'x' } },
    });
    mount(deps);

    const order: [() => void, ReturnType<typeof vi.fn>][] = [
      [() => (deps.paletteOpenRef.current = false), deps.closePalette as never],
      [() => (deps.composerOpenRef.current = false), deps.closeComposer as never],
      [() => (deps.aiPanelOpenRef.current = false), deps.closeAiPanel as never],
      [() => (deps.blameOpenRef.current = false), deps.closeBlame as never],
      [() => (deps.historyOpenRef.current = false), deps.closeHistory as never],
      [() => (deps.reflogOpenRef.current = false), deps.closeReflog as never],
      [() => (deps.commitBrowserOpenRef.current = false), deps.setCommitBrowserOpen as never],
      [() => (deps.searchOpenRef.current = false), deps.closeSearch as never],
      [() => (deps.historySearchOpenRef.current = false), deps.closeHistorySearch as never],
      [() => (deps.diffSlotRef.current = null), deps.collapseDiffSlot as never],
      [() => (deps.compareRef.current = null), deps.clearCompare as never],
    ];
    for (const [closeLayer, spy] of order) {
      press('Escape');
      expect(spy).toHaveBeenCalledTimes(1);
      // Nothing below the topmost layer was touched this press.
      expect(deps.setSelectedIndex).not.toHaveBeenCalled();
      closeLayer();
    }
    // Everything closed → Esc deselects (functional update).
    press('Escape');
    expect(deps.setSelectedIndex).toHaveBeenCalledTimes(1);
    const updater = (deps.setSelectedIndex as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(updater(3)).toBeNull();
    expect(updater(null)).toBeNull();
  });

  it('commitBrowser closes via setCommitBrowserOpen(false)', () => {
    const deps = makeDeps({ commitBrowserOpenRef: { current: true } });
    mount(deps);
    press('Escape');
    expect(deps.setCommitBrowserOpen).toHaveBeenCalledWith(false);
  });

  it('is fully suppressed by globalModalOpen', () => {
    const deps = makeDeps({
      globalModalOpen: true,
      paletteOpenRef: { current: true },
      diffSlotRef: { current: {} as DiffSlot },
    });
    mount(deps);
    press('Escape');
    expect(deps.closePalette).not.toHaveBeenCalled();
    expect(deps.collapseDiffSlot).not.toHaveBeenCalled();
    expect(deps.setSelectedIndex).not.toHaveBeenCalled();
  });

  it('does nothing when the tab is inactive', () => {
    const deps = makeDeps({ active: false, paletteOpenRef: { current: true } });
    mount(deps);
    press('Escape');
    expect(deps.closePalette).not.toHaveBeenCalled();
  });

  it('typing bail: Esc from an input/textarea skips the lower layers…', () => {
    const deps = makeDeps({ diffSlotRef: { current: {} as DiffSlot } });
    mount(deps);
    for (const tag of ['input', 'textarea'] as const) {
      const el = attachedInput(tag);
      press('Escape', {}, el);
      el.remove();
    }
    expect(deps.collapseDiffSlot).not.toHaveBeenCalled();
    expect(deps.setSelectedIndex).not.toHaveBeenCalled();
  });

  it('…but the palette and composer still close from a text field (above the bail)', () => {
    const deps = makeDeps({ composerOpenRef: { current: true } });
    mount(deps);
    const el = attachedInput('textarea');
    press('Escape', {}, el);
    expect(deps.closeComposer).toHaveBeenCalledTimes(1);
    el.remove();
  });

  it('unmount removes the listener', () => {
    const deps = makeDeps({ paletteOpenRef: { current: true } });
    const h = mount(deps);
    h.unmount();
    press('Escape');
    expect(deps.closePalette).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// Shortcut effect: refresh / search / palette / fetch / pull / push / nav
// ---------------------------------------------------------------------------

describe('refresh (Ctrl/Cmd-R, F5)', () => {
  it('Ctrl-R refreshes and preventDefaults; Cmd-R (metaKey) works too; F5 works', () => {
    const deps = makeDeps();
    mount(deps);
    expect(press('r', { ctrlKey: true }).defaultPrevented).toBe(true);
    press('r', { metaKey: true });
    press('F5');
    expect(deps.handleRefresh).toHaveBeenCalledTimes(3);
  });

  it('uppercase R (shifted) still matches via toLowerCase', () => {
    const deps = makeDeps();
    mount(deps);
    press('R', { ctrlKey: true });
    expect(deps.handleRefresh).toHaveBeenCalledTimes(1);
  });

  it('plain r without Ctrl/Cmd does nothing and is not default-prevented', () => {
    const deps = makeDeps();
    mount(deps);
    expect(press('r').defaultPrevented).toBe(false);
    expect(deps.handleRefresh).not.toHaveBeenCalled();
  });

  it.each([
    ['refreshing', { refreshing: true }],
    ['statusLoading', { statusLoading: true }],
    ['graphLoading', { graphLoading: true }],
    ['mutating', { mutating: true }],
    ['dialogOpen', { dialogOpen: true }],
    ['abortConfirmOpen', { abortConfirmOpen: true }],
  ] as const)('suppressed while %s (but still preventDefaults the webview reload)', (_n, over) => {
    const deps = makeDeps(over);
    mount(deps);
    expect(press('r', { ctrlKey: true }).defaultPrevented).toBe(true);
    expect(deps.handleRefresh).not.toHaveBeenCalled();
  });
});

describe('Ctrl/Cmd-F search + Ctrl/Cmd-K palette', () => {
  it('Ctrl-F opens search (preventDefault) even from a text input', () => {
    const deps = makeDeps();
    mount(deps);
    const el = attachedInput();
    expect(press('f', { ctrlKey: true }, el).defaultPrevented).toBe(true);
    expect(deps.openSearch).toHaveBeenCalledTimes(1);
    el.remove();
  });

  it('Ctrl-F is suppressed under a dialog / abort confirm / composer', () => {
    for (const over of [
      { dialogOpen: true },
      { abortConfirmOpen: true },
      { composerOpen: true },
    ]) {
      const deps = makeDeps(over);
      const h = mount(deps);
      press('f', { ctrlKey: true });
      expect(deps.openSearch).not.toHaveBeenCalled();
      h.unmount();
    }
  });

  it('Ctrl-K toggles the palette — a second press closes it (not gated on paletteOpen)', () => {
    const deps = makeDeps({ paletteOpen: true });
    mount(deps);
    press('k', { metaKey: true });
    expect(deps.togglePalette).toHaveBeenCalledTimes(1);
  });

  it('Ctrl-Shift-K does NOT toggle the palette (shift excluded)', () => {
    const deps = makeDeps();
    mount(deps);
    press('k', { ctrlKey: true, shiftKey: true });
    expect(deps.togglePalette).not.toHaveBeenCalled();
  });
});

describe('fetch / pull / push (Ctrl/Cmd-Shift-F/P/U)', () => {
  it('dispatches the right handler and preventDefaults', () => {
    const deps = makeDeps();
    mount(deps);
    expect(press('F', { ctrlKey: true, shiftKey: true }).defaultPrevented).toBe(true);
    press('P', { ctrlKey: true, shiftKey: true });
    press('U', { metaKey: true, shiftKey: true });
    expect(deps.handleFetch).toHaveBeenCalledTimes(1);
    expect(deps.handlePull).toHaveBeenCalledTimes(1);
    expect(deps.handlePush).toHaveBeenCalledTimes(1);
  });

  it('pull/push are gated on canPullPush; fetch is not', () => {
    const deps = makeDeps({ canPullPush: false });
    mount(deps);
    press('F', { ctrlKey: true, shiftKey: true });
    press('P', { ctrlKey: true, shiftKey: true });
    press('U', { ctrlKey: true, shiftKey: true });
    expect(deps.handleFetch).toHaveBeenCalledTimes(1);
    expect(deps.handlePull).not.toHaveBeenCalled();
    expect(deps.handlePush).not.toHaveBeenCalled();
  });

  it('typing guard blocks fetch from an input/textarea/select/contenteditable', () => {
    const deps = makeDeps();
    mount(deps);
    for (const tag of ['input', 'textarea', 'select'] as const) {
      const el = attachedInput(tag);
      press('F', { ctrlKey: true, shiftKey: true }, el);
      el.remove();
    }
    const div = document.createElement('div');
    // jsdom does not compute isContentEditable from the attribute — stub the
    // property the guard actually reads.
    Object.defineProperty(div, 'isContentEditable', { value: true });
    document.body.appendChild(div);
    press('F', { ctrlKey: true, shiftKey: true }, div);
    div.remove();
    expect(deps.handleFetch).not.toHaveBeenCalled();
  });

  it.each([
    ['dialogOpen', { dialogOpen: true }],
    ['abortConfirmOpen', { abortConfirmOpen: true }],
    ['searchOpen', { searchOpen: true }],
    ['paletteOpen', { paletteOpen: true }],
    ['composerOpen', { composerOpen: true }],
    ['historySearchOpen', { historySearchOpen: true }],
  ] as const)('inert while %s', (_n, over) => {
    const deps = makeDeps(over);
    mount(deps);
    press('F', { ctrlKey: true, shiftKey: true });
    press('P', { ctrlKey: true, shiftKey: true });
    press('U', { ctrlKey: true, shiftKey: true });
    expect(deps.handleFetch).not.toHaveBeenCalled();
    expect(deps.handlePull).not.toHaveBeenCalled();
    expect(deps.handlePush).not.toHaveBeenCalled();
  });

  it('fetch suppressed while refreshing/mutating', () => {
    const deps = makeDeps({ mutating: true });
    mount(deps);
    press('F', { ctrlKey: true, shiftKey: true });
    expect(deps.handleFetch).not.toHaveBeenCalled();
  });
});

describe('graph navigation', () => {
  function navDeps(over: Partial<Deps> = {}) {
    return makeDeps({ selectedIndex: 2, graph: graph(10), ...over });
  }
  function lastNext(deps: Deps, prev: number): number | null {
    const calls = (deps.setSelectedIndex as ReturnType<typeof vi.fn>).mock.calls;
    const arg = calls[calls.length - 1][0];
    return typeof arg === 'function' ? arg(prev) : arg;
  }

  it('ArrowDown/ArrowUp step by one, clamped to [0, n-1]', () => {
    const deps = navDeps();
    mount(deps);
    expect(press('ArrowDown').defaultPrevented).toBe(true);
    expect(lastNext(deps, 2)).toBe(3);
    press('ArrowUp');
    expect(lastNext(deps, 0)).toBe(0); // clamp at top
    press('ArrowDown');
    expect(lastNext(deps, 9)).toBe(9); // clamp at bottom
  });

  it('PageDown/PageUp step by the visible row count (graphRef), default 10', () => {
    const deps = navDeps({
      graphRef: { current: { getVisibleRowCount: () => 4 } as unknown as GraphCanvasHandle },
    });
    mount(deps);
    press('PageDown');
    expect(lastNext(deps, 2)).toBe(6);
    press('PageUp');
    expect(lastNext(deps, 2)).toBe(0); // 2-4 clamped

    const noRef = navDeps();
    mount(noRef);
    press('PageDown');
    expect(lastNext(noRef, 2)).toBe(9); // 2+10 clamped to n-1 (fallback n=10)
  });

  it('Home/End jump to first/last', () => {
    const deps = navDeps();
    mount(deps);
    press('Home');
    expect(deps.setSelectedIndex).toHaveBeenLastCalledWith(0);
    press('End');
    expect(deps.setSelectedIndex).toHaveBeenLastCalledWith(9);
  });

  it('nav is inert (and not default-prevented) with no selection or no graph', () => {
    for (const over of [{ selectedIndex: null }, { graph: null }] as const) {
      const deps = navDeps(over);
      const h = mount(deps);
      expect(press('ArrowDown').defaultPrevented).toBe(false);
      expect(press('Home').defaultPrevented).toBe(false);
      expect(deps.setSelectedIndex).not.toHaveBeenCalled();
      h.unmount();
    }
  });

  it('nav is inert while typing in an input', () => {
    const deps = navDeps();
    mount(deps);
    const el = attachedInput();
    press('ArrowDown', {}, el);
    expect(deps.setSelectedIndex).not.toHaveBeenCalled();
    el.remove();
  });
});

// ---------------------------------------------------------------------------
// ShortcutOverlay sync — the documented table vs the actual bindings. Both
// tables are hardcoded separately (SHORTCUTS in ShortcutOverlay.tsx is not
// exported; the handler is imperative code), so this duplicates minimal key
// literals — fragile by construction (finding candidate, see FINDINGS T3.2b).
// ---------------------------------------------------------------------------

describe('ShortcutOverlay sync', () => {
  function overlayText(): string {
    render(<ShortcutOverlay open onClose={() => {}} />);
    return screen.getByRole('dialog').textContent ?? '';
  }

  it('documents every workspace binding it has historically covered', () => {
    const text = overlayText();
    for (const needle of [
      'Ctrl+R',
      'F5',
      'Ctrl+Shift+F',
      'Ctrl+Shift+P',
      'Ctrl+Shift+U',
      'Esc',
      'Home',
      'End',
      'Page Up+Page Down',
      '↑+↓',
    ]) {
      expect(text.includes(needle), `overlay should document ${needle}`).toBe(true);
    }
  });

  // FINDING [T3.2b] F-T32b-1 (FIXED): useWorkspaceKeyboard binds Ctrl/Cmd-F
  // (commit search, P50b) and Ctrl/Cmd-K (command palette, P50c); the
  // ShortcutOverlay §6.1 table was stale until the campaign fix added both rows.
  it('documents Ctrl+F (search) and Ctrl+K (palette) in the overlay table', () => {
    const text = overlayText();
    expect(text).toContain('Ctrl+F');
    expect(text).toContain('Ctrl+K');
  });
});
