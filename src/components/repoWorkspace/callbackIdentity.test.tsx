/**
 * Render-storm fix (P91 follow-up) — the IDENTITY guard for the three hooks that
 * exist only to give the sidebar stable props.
 *
 * `Sidebar.churn.test.tsx` measures what the sidebar costs when its callbacks
 * are stable, but it CANNOT see them becoming unstable: it builds them itself as
 * module constants. So re-inlining `onAddRemote={() => …}` at the RepoWorkspace
 * call site, or restoring `useReveal`'s `[revealIndex, graph, …]` dep array,
 * brings the whole storm back with that file green. This is that missing half:
 * render each hook twice with CHANGED deps and assert every returned callback is
 * `toBe`-equal across the two renders.
 *
 * The key-count assertions are deliberate — they make a tenth sidebar callback
 * or an eighth context-menu opener a test failure until it is covered here.
 */
import { renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { GraphLayout } from '../../ipc';
import type { GraphFilterController } from '../../hooks/useGraphFilter';
import type { WorkspaceMenus } from '../workspaceMenus';
import { useContextMenuOpeners, type ContextMenuOpenerDeps } from './contextMenuOpeners';
import { useSidebarCallbacks, type SidebarCallbackDeps } from './useSidebarCallbacks';
import { useReveal } from './useReveal';

/** Every dep FRESH — the point is that the hook's output survives it. */
function sidebarDeps(): SidebarCallbackDeps {
  return {
    setBranchesError: vi.fn(),
    handleCheckoutBranch: vi.fn(),
    handleCreateBranch: vi.fn(async () => {}),
    handleCreateStash: vi.fn(),
    refetchTagSync: vi.fn(),
    setPendingAddSubmodule: vi.fn(),
    setNewWorktreeOpen: vi.fn(),
    setPendingAddRemote: vi.fn(),
    setStaleCleanupOpen: vi.fn(),
  };
}

/** `menus` / `graphFilter` are only ever read when an opener RUNS, so an empty
 *  stub is enough for an identity assertion (nothing here invokes them). */
function openerDeps(head: string): ContextMenuOpenerDeps {
  return {
    setMenu: vi.fn(),
    menus: {} as WorkspaceMenus,
    submodules: [],
    worktrees: [],
    headBranch: { name: head, isHead: true, upstream: null, ahead: null, behind: null, tip: 'a'.repeat(40) },
    graphFilter: {} as GraphFilterController,
  };
}

function layout(oid: string): GraphLayout {
  return {
    nodes: [
      {
        id: oid,
        lane: 0,
        parents: [],
        refs: [{ name: 'main', kind: 'localBranch', isHead: true }],
        summary: 's',
        author: 'a',
        ts: 1,
        committerTs: 1,
      },
    ],
    edges: [],
    laneCount: 1,
    headIndex: 0,
    truncated: false,
  };
}

/** Assert every key of `a` is `toBe`-identical in `b`, and that there are `n`. */
function expectSameCallbacks<T extends object>(a: T, b: T, n: number): void {
  const keys = Object.keys(a) as (keyof T)[];
  expect(keys).toHaveLength(n);
  for (const key of keys) expect(b[key]).toBe(a[key]);
}

describe('useSidebarCallbacks — stable identities across a deps change', () => {
  it('returns the same nine callbacks after a re-render with all-new deps', () => {
    const { result, rerender } = renderHook((deps: SidebarCallbackDeps) => useSidebarCallbacks(deps), {
      initialProps: sidebarDeps(),
    });
    const first = result.current;
    rerender(sidebarDeps());
    expectSameCallbacks(first, result.current, 9);
  });
});

describe('useContextMenuOpeners — stable identities across a deps change', () => {
  it('returns the same seven openers after a re-render with all-new deps', () => {
    const { result, rerender } = renderHook(
      (deps: ContextMenuOpenerDeps) => useContextMenuOpeners(deps),
      { initialProps: openerDeps('main') },
    );
    const first = result.current;
    rerender(openerDeps('dev'));
    expectSameCallbacks(first, result.current, 7);
  });
});

describe('useReveal — handleReveal survives a new graph', () => {
  /** THE regression this guards: `graph` is re-streamed (brand-new node arrays)
   *  on every refresh round whose scope includes the graph, so a dep-array
   *  `handleReveal` changes identity on `git branch foo` and re-renders all 500
   *  BranchRows through `rowPropsEqual`'s `Object.is` on `onReveal`. */
  it('keeps one identity when the layout is replaced', () => {
    const { result, rerender } = renderHook((graph: GraphLayout | null) => useReveal({
      graph,
      setSelectedIndex: vi.fn(),
      revealBranch: vi.fn(),
      pushToast: vi.fn(),
    }), { initialProps: null as GraphLayout | null });
    const first = result.current.handleReveal;
    rerender(layout('b'.repeat(40)));
    expect(result.current.handleReveal).toBe(first);
    rerender(layout('c'.repeat(40)));
    expect(result.current.handleReveal).toBe(first);
  });

  it('still reads the CURRENT graph through the latch', () => {
    const setSelectedIndex = vi.fn();
    const { result, rerender } = renderHook((graph: GraphLayout | null) => useReveal({
      graph,
      setSelectedIndex,
      revealBranch: vi.fn(),
      pushToast: vi.fn(),
    }), { initialProps: null as GraphLayout | null });
    const reveal = result.current.handleReveal;
    rerender(layout('d'.repeat(40)));
    // Same function object, new graph: the ref must supply the fresh index.
    reveal({ kind: 'ref', name: 'main' });
    expect(setSelectedIndex).toHaveBeenCalledWith(0);
  });
});
