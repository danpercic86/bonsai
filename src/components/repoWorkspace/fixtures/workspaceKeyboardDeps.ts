/** Shared test fixture for `useWorkspaceKeyboard`: an all-closed / all-idle dep
 *  bag plus tiny graph builders. Used by `useWorkspaceKeyboard.test.tsx` and
 *  `useWorkspaceKeyboard.p95.test.tsx` so neither file re-declares the ~50-line
 *  dependency object. */
import { vi } from 'vitest';

import type { useWorkspaceKeyboard } from '../useWorkspaceKeyboard';
import type { GraphLayout, GraphNode } from '../../../ipc';
import type { GraphCanvasHandle } from '../../../graph/GraphCanvas';

export type WorkspaceKeyboardDeps = Parameters<typeof useWorkspaceKeyboard>[0];

export function graphNode(id: string): GraphNode {
  return {
    id,
    lane: 0,
    parents: [],
    summary: '',
    author: '',
    ts: 0,
    committerTs: 0,
  };
}

export function graphOf(n: number): GraphLayout {
  return {
    nodes: Array.from({ length: n }, (_, i) => graphNode(`c${i}`)),
    edges: [],
    laneCount: 1,
    headIndex: null,
    truncated: false,
  };
}

/** A spy-backed `GraphCanvasHandle` — `focusScroller` is the P95 §2 assertion point. */
export interface SpyGraphHandle extends GraphCanvasHandle {
  focusScroller: GraphCanvasHandle['focusScroller'] & { mock: unknown };
}
export function graphHandle(visibleRows = 5): SpyGraphHandle {
  return {
    getVisibleRowCount: () => visibleRows,
    focusScroller: vi.fn(),
  } as unknown as SpyGraphHandle;
}

/** All-closed, all-idle deps with fresh spies; override per test. */
export function makeKeyboardDeps(
  over: Partial<WorkspaceKeyboardDeps> = {},
): WorkspaceKeyboardDeps {
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
    onAiActivity: vi.fn(),
    onGitActivity: vi.fn(),
    handleRefresh: vi.fn(),
    handleFetch: vi.fn(),
    handlePull: vi.fn(),
    handlePush: vi.fn(),
    ...over,
  };
}

/** Dispatch a keydown from `target` (default: window); returns the event so
 *  tests can assert `defaultPrevented`. */
export function pressKey(
  key: string,
  opts: KeyboardEventInit = {},
  target: EventTarget = window,
): KeyboardEvent {
  const ev = new KeyboardEvent('keydown', {
    key,
    bubbles: true,
    cancelable: true,
    ...opts,
  });
  target.dispatchEvent(ev);
  return ev;
}
