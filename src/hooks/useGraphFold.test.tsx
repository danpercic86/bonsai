/** Spec-004 — useGraphFold: AC6 selection pinning at spans arrival, transient
 *  expansion (toggle / expandFor / collapseRunContaining), and the reset on
 *  repo/filter change. jsdom via renderHook. */
import { act } from 'react';
import { describe, expect, it } from 'vitest';
import { renderHook } from '@testing-library/react';

import { useGraphFold } from './useGraphFold';
import type { FoldSpan } from '../ipc';

const SPANS: FoldSpan[] = [
  { start: 2, count: 5, lane: 0 },
  { start: 10, count: 6, lane: 1 },
];

function mount(over: { selected?: number | null; filterKey?: string; repoId?: string } = {}) {
  const selectedIndexRef = { current: over.selected ?? null };
  const initial = {
    filterKey: over.filterKey ?? 'k1',
    repoId: over.repoId ?? 'r1',
    selectedIndex: over.selected ?? null,
  };
  const hook = renderHook(
    (p: { filterKey: string; repoId: string; selectedIndex: number | null }) =>
      useGraphFold({
        graphFoldLinear: true,
        totalRows: 30,
        filterKey: p.filterKey,
        repoId: p.repoId,
        selectedIndexRef,
        selectedIndex: p.selectedIndex,
      }),
    { initialProps: initial },
  );
  return { ...hook, selectedIndexRef, initial };
}

describe('useGraphFold', () => {
  it('setSpans builds the model; toggleSpan expands and re-collapses', () => {
    const { result } = mount();
    expect(result.current.model).toBeNull();
    act(() => result.current.setSpans(SPANS));
    expect(result.current.model?.collapsed).toHaveLength(2);
    act(() => result.current.toggleSpan(2));
    expect(result.current.expandedSpans.map((s) => s.start)).toEqual([2]);
    expect(result.current.model?.collapsed.map((s) => s.start)).toEqual([10]);
    act(() => result.current.toggleSpan(2));
    expect(result.current.expandedSpans).toEqual([]);
  });

  it('AC6: spans arrival pins the run containing the selection and resets prior expansion', () => {
    const { result, selectedIndexRef } = mount({ selected: 12 });
    act(() => result.current.setSpans(SPANS));
    expect(result.current.expandedSpans.map((s) => s.start)).toEqual([10]); // pinned
    // A later arrival with a different selection replaces the transient set.
    act(() => result.current.toggleSpan(2));
    selectedIndexRef.current = 0;
    act(() => result.current.setSpans(SPANS));
    expect(result.current.expandedSpans).toEqual([]); // prior expansion cleared
  });

  it('expandFor expands the collapsed run containing a hidden row (reveal §5)', () => {
    const { result } = mount();
    act(() => result.current.setSpans(SPANS));
    act(() => result.current.expandFor(4));
    expect(result.current.expandedSpans.map((s) => s.start)).toEqual([2]);
    // Rows outside any span are a no-op.
    act(() => result.current.expandFor(8));
    expect(result.current.expandedSpans.map((s) => s.start)).toEqual([2]);
  });

  it('collapseRunContaining collapses an EXPANDED run and actives its pill', () => {
    const { result } = mount();
    act(() => result.current.setSpans(SPANS));
    expect(result.current.collapseRunContaining(4)).toBe(false); // collapsed run → no-op
    act(() => result.current.toggleSpan(2));
    let did = false;
    act(() => {
      did = result.current.collapseRunContaining(4);
    });
    expect(did).toBe(true);
    expect(result.current.expandedSpans).toEqual([]);
    expect(result.current.activePillStart).toBe(2);
  });

  it('mouse selection after ArrowLeft clears the stale active pill (§3 regression)', () => {
    // Repro: expand span → ArrowLeft collapses it and actives the pill →
    // mouse-click another commit row (selection change) must clear the pill,
    // or the next ArrowDown navigates from the stale pill index.
    const { result, rerender, initial } = mount({ selected: 1 });
    act(() => result.current.setSpans(SPANS));
    act(() => result.current.toggleSpan(2));
    act(() => {
      result.current.collapseRunContaining(4);
    });
    expect(result.current.activePillStart).toBe(2);
    rerender({ ...initial, selectedIndex: 8 }); // the mouse click
    expect(result.current.activePillStart).toBeNull();
    // A pill landing with an UNCHANGED selection survives (keyboard §3 path).
    act(() => result.current.setActivePill(2));
    rerender({ ...initial, selectedIndex: 8 });
    expect(result.current.activePillStart).toBe(2);
  });

  it('filter/repo change resets spans, expansion, and the active pill', () => {
    const { result, rerender } = mount();
    act(() => {
      result.current.setSpans(SPANS);
    });
    act(() => {
      result.current.toggleSpan(2);
      result.current.setActivePill(10);
    });
    rerender({ filterKey: 'k2', repoId: 'r1', selectedIndex: null });
    expect(result.current.spans).toEqual([]);
    expect(result.current.model).toBeNull();
    expect(result.current.expandedSpans).toEqual([]);
    expect(result.current.activePillStart).toBeNull();
  });
});
