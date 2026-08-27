/** Spec-005 — useRailInput: rings-channel pick + the currentMatch trap.
 *  Plan §Testing gap: `deriveMatchRows` drops oids absent from the layout, so
 *  `matchRows[currentMatch]` is NOT valid — the rail's rows/jump pairs must be
 *  rebuilt aligned with the ORIGINAL results.matches indices, and the current
 *  row resolved from the current match's oid. */
import { describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

import { useRailInput } from './railProps';
import type { UseRailInputDeps } from './railProps';
import type { GraphLayout } from '../../ipc';
import type { UseCommitSearch } from './useCommitSearch';
import type { UseHistorySearch } from './useHistorySearch';

function layoutOf(count: number): GraphLayout {
  return {
    nodes: Array.from({ length: count }, (_, i) => ({
      id: `oid${i}`,
      lane: 0,
      parents: [],
      summary: `c${i}`,
      author: 'a',
      ts: 0,
      committerTs: 0,
    })),
    edges: [],
    laneCount: 1,
    headIndex: 0,
    truncated: false,
  };
}

const CLOSED_SEARCH = { open: false, results: null, currentMatch: -1, goToMatch: vi.fn() };
const CLOSED_HISTORY = { open: false, matchRows: [] as number[] };

function makeDeps(overrides: Partial<UseRailInputDeps> = {}): UseRailInputDeps {
  return {
    search: CLOSED_SEARCH as unknown as UseCommitSearch,
    historySearch: CLOSED_HISTORY as unknown as UseHistorySearch,
    graph: layoutOf(5),
    revealCommitByOid: vi.fn(),
    generation: 1,
    alwaysShow: false,
    ...overrides,
  };
}

function searchWith(matchOids: string[], currentMatch: number, goToMatch = vi.fn()) {
  return {
    open: true,
    results: { matches: matchOids.map((oid) => ({ oid })) },
    currentMatch,
    goToMatch,
  } as unknown as UseCommitSearch;
}

describe('useRailInput — commit-search channel', () => {
  it('maps match oids to model rows, dropping oids absent from the layout', () => {
    const deps = makeDeps({ search: searchWith(['missing', 'oid2', 'oid0'], -1) });
    const { result } = renderHook(() => useRailInput(deps));
    expect(result.current.ringsLive).toBe(true);
    expect(result.current.matchRows).toEqual([2, 0]);
  });

  it('onJumpToMatch resolves through the ORIGINAL results.matches index', () => {
    const goToMatch = vi.fn();
    const deps = makeDeps({ search: searchWith(['missing', 'oid2', 'oid0'], -1, goToMatch) });
    const { result } = renderHook(() => useRailInput(deps));
    // Tick index 1 = the SECOND surviving row (oid0), which is results index 2.
    result.current.onJumpToMatch(1);
    expect(goToMatch).toHaveBeenCalledExactlyOnceWith(2);
    result.current.onJumpToMatch(0);
    expect(goToMatch).toHaveBeenLastCalledWith(1);
  });

  it('currentMatchRow resolves via the current oid, NOT matchRows[currentMatch]', () => {
    // currentMatch = 2 (oid0). Naive matchRows[2] would be undefined after the
    // drop; the oid lookup must yield row 0.
    const deps = makeDeps({ search: searchWith(['missing', 'oid2', 'oid0'], 2) });
    const { result } = renderHook(() => useRailInput(deps));
    expect(result.current.currentMatchRow).toBe(0);
  });

  it('current match whose oid fell out of the layout yields null', () => {
    const deps = makeDeps({ search: searchWith(['missing', 'oid2'], 0) });
    const { result } = renderHook(() => useRailInput(deps));
    expect(result.current.currentMatchRow).toBeNull();
  });

  it('currentMatch -1 (no current) yields null', () => {
    const deps = makeDeps({ search: searchWith(['oid1'], -1) });
    const { result } = renderHook(() => useRailInput(deps));
    expect(result.current.currentMatchRow).toBeNull();
  });

  it('zero surviving matches → ringsLive false', () => {
    const deps = makeDeps({ search: searchWith(['missing'], -1) });
    const { result } = renderHook(() => useRailInput(deps));
    expect(result.current.ringsLive).toBe(false);
    expect(result.current.matchRows).toEqual([]);
  });
});

describe('useRailInput — channel pick and idle states', () => {
  it('search closed and history closed → inert bundle', () => {
    const { result } = renderHook(() => useRailInput(makeDeps()));
    expect(result.current.ringsLive).toBe(false);
    expect(result.current.matchRows).toEqual([]);
    expect(result.current.currentMatchRow).toBeNull();
    expect(() => result.current.onJumpToMatch(0)).not.toThrow();
  });

  it('null graph with search open → inert (no oid index to build)', () => {
    const deps = makeDeps({ graph: null, search: searchWith(['oid1'], 0) });
    const { result } = renderHook(() => useRailInput(deps));
    expect(result.current.ringsLive).toBe(false);
  });

  it('historySearch open WINS over an open commit search; no current tick (UI §3.6)', () => {
    const reveal = vi.fn();
    const goToMatch = vi.fn();
    const deps = makeDeps({
      search: searchWith(['oid1'], 0, goToMatch),
      historySearch: { open: true, matchRows: [3, 1] } as unknown as UseHistorySearch,
      revealCommitByOid: reveal,
    });
    const { result } = renderHook(() => useRailInput(deps));
    expect(result.current.ringsLive).toBe(true);
    expect(result.current.matchRows).toEqual([3, 1]);
    expect(result.current.currentMatchRow).toBeNull();
    // Jump resolves by oid through the shared reveal path, not goToMatch.
    result.current.onJumpToMatch(0);
    expect(reveal).toHaveBeenCalledExactlyOnceWith('oid3');
    expect(goToMatch).not.toHaveBeenCalled();
  });

  it('historySearch open with no matches → mounted channel but ringsLive false', () => {
    const deps = makeDeps({
      historySearch: { open: true, matchRows: [] } as unknown as UseHistorySearch,
    });
    const { result } = renderHook(() => useRailInput(deps));
    expect(result.current.ringsLive).toBe(false);
  });

  it('threads generation and alwaysShow through unchanged', () => {
    const { result } = renderHook(() => useRailInput(makeDeps({ generation: 7, alwaysShow: true })));
    expect(result.current.generation).toBe(7);
    expect(result.current.alwaysShow).toBe(true);
  });
});
