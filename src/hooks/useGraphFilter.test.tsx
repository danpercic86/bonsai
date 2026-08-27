/** Spec-003 — useGraphFilter: intent → wire derivation (solo / hide / hide-all /
 *  clear), the chip summary copy, and the persist-patch actions. */
import { describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

import {
  deriveGraphFilter,
  graphFilterSummary,
  knownFullRefs,
  shortRefName,
  useGraphFilter,
} from './useGraphFilter';
import type { BranchesSnapshot } from '../ipc';

const snapshot: BranchesSnapshot = {
  local: [
    { name: 'main', isHead: true, upstream: null, ahead: null, behind: null, tip: 'a'.repeat(40) },
    { name: 'feature/x', isHead: false, upstream: null, ahead: null, behind: null, tip: 'b'.repeat(40) },
  ],
  remote: [{ name: 'origin/main', tip: 'c'.repeat(40) }],
  tags: ['v1.0'],
  head: { branchName: 'main', detached: false, unborn: false, oid: 'a'.repeat(40) },
};

const KNOWN = [
  'refs/heads/main',
  'refs/heads/feature/x',
  'refs/remotes/origin/main',
  'refs/tags/v1.0',
];

describe('knownFullRefs / shortRefName', () => {
  it('derives full ref names from the sidebar snapshot', () => {
    expect(knownFullRefs(snapshot)).toEqual(KNOWN);
    expect(knownFullRefs(null)).toEqual([]);
  });

  it('short names strip the standard prefixes', () => {
    expect(shortRefName('refs/heads/feature/x')).toBe('feature/x');
    expect(shortRefName('refs/remotes/origin/main')).toBe('origin/main');
    expect(shortRefName('refs/tags/v1.0')).toBe('v1.0');
    expect(shortRefName('weird')).toBe('weird');
  });
});

describe('deriveGraphFilter', () => {
  it('null intent → null filter (default walk)', () => {
    expect(deriveGraphFilter(false, null, KNOWN)).toBeNull();
    // refs: [] behaves like null (contract §1).
    expect(deriveGraphFilter(false, { mode: 'solo', refs: [] }, KNOWN)).toBeNull();
  });

  it('first-parent only → seedRefs stays null', () => {
    expect(deriveGraphFilter(true, null, KNOWN)).toEqual({ firstParent: true, seedRefs: null, foldLinear: false });
  });

  it('solo → refs verbatim', () => {
    expect(
      deriveGraphFilter(false, { mode: 'solo', refs: ['refs/heads/feature/x'] }, KNOWN),
    ).toEqual({ firstParent: false, seedRefs: ['refs/heads/feature/x'], foldLinear: false });
  });

  it('hide → known refs minus refs (new branches appear)', () => {
    expect(
      deriveGraphFilter(false, { mode: 'hide', refs: ['refs/heads/feature/x'] }, KNOWN),
    ).toEqual({
      firstParent: false,
      seedRefs: ['refs/heads/main', 'refs/remotes/origin/main', 'refs/tags/v1.0'],
      foldLinear: false,
    });
  });

  it('hiding every known ref → seedRefs [] (hide-all, HEAD-only seed)', () => {
    expect(deriveGraphFilter(false, { mode: 'hide', refs: KNOWN }, KNOWN)).toEqual({
      firstParent: false,
      seedRefs: [],
      foldLinear: false,
    });
  });

  // Spec-004: foldLinear alone requests a filter; it rides every derivation.
  it('foldLinear alone -> non-null filter carrying foldLinear', () => {
    expect(deriveGraphFilter(false, null, KNOWN, true)).toEqual({
      firstParent: false,
      seedRefs: null,
      foldLinear: true,
    });
    expect(deriveGraphFilter(true, null, KNOWN, true)).toEqual({
      firstParent: true,
      seedRefs: null,
      foldLinear: true,
    });
  });
});

describe('graphFilterSummary (chip copy §2)', () => {
  it('covers the label matrix', () => {
    expect(graphFilterSummary(true, null)).toBe('First-parent');
    expect(graphFilterSummary(false, { mode: 'solo', refs: ['refs/heads/main'] })).toBe(
      'Solo: main',
    );
    expect(
      graphFilterSummary(false, { mode: 'solo', refs: ['refs/heads/main', 'refs/tags/v1.0'] }),
    ).toBe('Solo: main +1');
    expect(graphFilterSummary(false, { mode: 'hide', refs: ['refs/heads/main'] })).toBe(
      '1 branch hidden',
    );
    expect(
      graphFilterSummary(false, { mode: 'hide', refs: ['refs/heads/main', 'refs/tags/v1.0'] }),
    ).toBe('2 branches hidden');
    expect(graphFilterSummary(true, { mode: 'solo', refs: ['refs/heads/main'] })).toBe(
      'First-parent · Solo: main',
    );
    expect(graphFilterSummary(false, null)).toBe('');
  });

  // Spec-004 §4.2: the 'Folded' segment ORs in the LOCAL toggle, ordered after
  // First-parent and before the ref summary.
  it('Folded segment placement (spec-004)', () => {
    expect(graphFilterSummary(false, null, true)).toBe('Folded');
    expect(graphFilterSummary(true, null, true)).toBe('First-parent · Folded');
    expect(graphFilterSummary(true, { mode: 'solo', refs: ['refs/heads/main'] }, true)).toBe(
      'First-parent · Folded · Solo: main',
    );
  });
});

describe('useGraphFilter actions', () => {
  function mount(over: Partial<Parameters<typeof useGraphFilter>[0]> = {}) {
    const onSettingsChange = vi.fn();
    const { result } = renderHook(() =>
      useGraphFilter({
        graphFirstParent: false,
        graphFoldLinear: false,
        graphRefFilter: null,
        onSettingsChange,
        branches: snapshot,
        ...over,
      }),
    );
    return { result, onSettingsChange };
  }

  it('toggleFirstParent patches the flipped value', () => {
    const { result, onSettingsChange } = mount({ graphFirstParent: true });
    result.current.toggleFirstParent();
    expect(onSettingsChange).toHaveBeenCalledWith({ graphFirstParent: false });
  });

  it('soloRef starts a fresh solo set (replacing a hide set)', () => {
    const { result, onSettingsChange } = mount({
      graphRefFilter: { mode: 'hide', refs: ['refs/tags/v1.0'] },
    });
    result.current.soloRef('refs/heads/main');
    expect(onSettingsChange).toHaveBeenCalledWith({
      graphRefFilter: { mode: 'solo', refs: ['refs/heads/main'] },
    });
  });

  it('addToSolo appends; unfilterRef removes; last removal clears to null', () => {
    const solo = { mode: 'solo' as const, refs: ['refs/heads/main'] };
    const a = mount({ graphRefFilter: solo });
    a.result.current.addToSolo('refs/tags/v1.0');
    expect(a.onSettingsChange).toHaveBeenCalledWith({
      graphRefFilter: { mode: 'solo', refs: ['refs/heads/main', 'refs/tags/v1.0'] },
    });
    const b = mount({ graphRefFilter: solo });
    b.result.current.unfilterRef('refs/heads/main');
    expect(b.onSettingsChange).toHaveBeenCalledWith({ graphRefFilter: null });
  });

  it('hideRef appends to an existing hide set', () => {
    const { result, onSettingsChange } = mount({
      graphRefFilter: { mode: 'hide', refs: ['refs/heads/main'] },
    });
    result.current.hideRef('refs/tags/v1.0');
    expect(onSettingsChange).toHaveBeenCalledWith({
      graphRefFilter: { mode: 'hide', refs: ['refs/heads/main', 'refs/tags/v1.0'] },
    });
  });

  it('clearAll clears every declutter knob in one patch (fold included)', () => {
    const { result, onSettingsChange } = mount({ graphFirstParent: true, graphFoldLinear: true });
    result.current.clearAll();
    expect(onSettingsChange).toHaveBeenCalledWith({
      graphFirstParent: false,
      graphFoldLinear: false,
      graphRefFilter: null,
    });
  });

  // Spec-004: fold intent — toggle patch, requested state, walkKey stability.
  it('toggleFoldLinear patches the flipped value', () => {
    const { result, onSettingsChange } = mount({ graphFoldLinear: true });
    result.current.toggleFoldLinear();
    expect(onSettingsChange).toHaveBeenCalledWith({ graphFoldLinear: false });
  });

  it('fold alone requests a filter but never changes the walkKey', () => {
    const off = mount();
    const on = mount({ graphFoldLinear: true });
    expect(off.result.current.requested).toBe(false);
    expect(on.result.current.requested).toBe(true);
    expect(on.result.current.filter).toEqual({
      firstParent: false,
      seedRefs: null,
      foldLinear: true,
    });
    // The walk subset is identical → fold toggles are cache Hits, no refetch.
    expect(on.result.current.walkKey).toBe(off.result.current.walkKey);
    expect(on.result.current.filterKey).not.toBe(off.result.current.filterKey);
    expect(on.result.current.activeSummary).toBe('Folded');
  });

  it('hide mode before the branches snapshot lands → full graph, not hide-all', () => {
    const { result } = mount({
      branches: null,
      graphRefFilter: { mode: 'hide', refs: ['refs/heads/feature/x'] },
    });
    // knownRefs is [] pre-snapshot; deriving would yield seedRefs [] (hide-all).
    expect(result.current.filter).toBeNull();
  });

  it('markerFor reports solo/hidden membership', () => {
    const { result } = mount({ graphRefFilter: { mode: 'solo', refs: ['refs/heads/main'] } });
    expect(result.current.markerFor('refs/heads/main')).toBe('solo');
    expect(result.current.markerFor('refs/tags/v1.0')).toBeNull();
  });
});
