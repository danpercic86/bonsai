/** The WIP row summary's identity contract: it must only change when its VALUE
 *  changes. `wip` is a dependency of `useScrollSelectionIntoView`, so a fresh
 *  object per status refetch re-ran the scroll adjustment on every background
 *  refresh round (part of the "UI jumps on refresh" report). */

import { describe, it, expect } from 'vitest';
import { renderHook } from '@testing-library/react';

import { useWipSummary, wipFileCountOf } from './useWipSummary';
import type { HeadInfo, StatusSnapshot } from '../../ipc';

const HEAD: HeadInfo = { branchName: 'main', oid: 'a'.repeat(40), detached: false, unborn: false };
const UNBORN: HeadInfo = { branchName: 'main', oid: '', detached: false, unborn: true };

function snapshot(staged: string[], unstaged: string[] = []): StatusSnapshot {
  return {
    staged: staged.map((path) => ({ path, origPath: null, status: 'modified' as const })),
    unstaged: unstaged.map((path) => ({ path, origPath: null, status: 'modified' as const })),
    untracked: [],
    conflicted: [],
  };
}

describe('wipFileCountOf', () => {
  it('counts DISTINCT paths across sections', () => {
    expect(wipFileCountOf(snapshot(['a.ts'], ['a.ts', 'b.ts']), HEAD)).toBe(2);
  });

  it('is null with no snapshot, an unborn HEAD, or a clean tree', () => {
    expect(wipFileCountOf(null, HEAD)).toBeNull();
    expect(wipFileCountOf(snapshot(['a.ts']), UNBORN)).toBeNull();
    expect(wipFileCountOf(snapshot([]), HEAD)).toBeNull();
  });
});

describe('useWipSummary', () => {
  it('keeps the same object across a status refetch with an unchanged file count', () => {
    const { result, rerender } = renderHook(
      ({ s }: { s: StatusSnapshot | null }) => useWipSummary(s, HEAD),
      { initialProps: { s: snapshot(['a.ts', 'b.ts']) } },
    );
    const first = result.current;
    expect(first).toEqual({ fileCount: 2 });

    // A refresh round: a brand-new snapshot object, identical content.
    rerender({ s: snapshot(['a.ts', 'b.ts']) });
    expect(result.current).toBe(first);
  });

  it('produces a new object when the count changes', () => {
    const { result, rerender } = renderHook(
      ({ s }: { s: StatusSnapshot | null }) => useWipSummary(s, HEAD),
      { initialProps: { s: snapshot(['a.ts']) } },
    );
    const first = result.current;
    rerender({ s: snapshot(['a.ts', 'b.ts']) });
    expect(result.current).not.toBe(first);
    expect(result.current).toEqual({ fileCount: 2 });
  });

  it('clears to null when the tree goes clean', () => {
    const { result, rerender } = renderHook(
      ({ s }: { s: StatusSnapshot | null }) => useWipSummary(s, HEAD),
      { initialProps: { s: snapshot(['a.ts']) } },
    );
    rerender({ s: snapshot([]) });
    expect(result.current).toBeNull();
  });
});
