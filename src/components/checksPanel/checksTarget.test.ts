import { describe, it, expect } from 'vitest';
import type { BranchesSnapshot } from '../../ipc';
import { resolveChecksTarget } from './checksTarget';

const SNAP: BranchesSnapshot = {
  local: [
    { name: 'main', isHead: true, upstream: 'origin/main', ahead: 0, behind: 0, tip: 'a'.repeat(40) },
    { name: 'wip', isHead: false, upstream: null, ahead: null, behind: null, tip: 'b'.repeat(40) },
  ],
  remote: [{ name: 'origin/main', tip: 'a'.repeat(40) }],
  tags: ['v1.0'],
  head: { branchName: 'main', oid: 'a'.repeat(40), detached: false, unborn: false },
};

describe('resolveChecksTarget', () => {
  it('resolves a local branch with an upstream', () => {
    const t = resolveChecksTarget({ kind: 'ref', name: 'main' }, SNAP);
    expect(t).toEqual({ name: 'main', tip: 'a'.repeat(40), hasUpstream: true });
  });

  it('marks a local branch with no upstream as hasUpstream:false', () => {
    const t = resolveChecksTarget({ kind: 'ref', name: 'wip' }, SNAP);
    expect(t).toEqual({ name: 'wip', tip: 'b'.repeat(40), hasUpstream: false });
  });

  it('resolves a remote-tracking branch (always hasUpstream:true)', () => {
    const t = resolveChecksTarget({ kind: 'ref', name: 'origin/main' }, SNAP);
    expect(t).toEqual({ name: 'origin/main', tip: 'a'.repeat(40), hasUpstream: true });
  });

  it('returns null for a tag name (present in neither branch list)', () => {
    expect(resolveChecksTarget({ kind: 'ref', name: 'v1.0' }, SNAP)).toBeNull();
  });

  it('returns null for an oid reveal (stash / raw commit)', () => {
    expect(resolveChecksTarget({ kind: 'oid', oid: 'c'.repeat(40) }, SNAP)).toBeNull();
  });

  it('returns null when the snapshot is null', () => {
    expect(resolveChecksTarget({ kind: 'ref', name: 'main' }, null)).toBeNull();
  });
});
