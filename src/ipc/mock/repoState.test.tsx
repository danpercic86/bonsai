/** T3.4 — repoState.ts pure helpers + per-repo seeding: canonicalization,
 *  repoId dedupe, branch-name validation, conflict-marker stripping, stale
 *  report, repo-kind/fixture routing, buildInfo shapes, requireRepo guard. */
import { describe, expect, it } from 'vitest';

import {
  buildInfo,
  buildStaleReport,
  createRepoState,
  isInvalidBranchName,
  isRefTip,
  mockCanonical,
  repoGraphFixture,
  repoKind,
  repoOp,
  repos,
  requireRepo,
  resolveRepoId,
  stripConflictMarkers,
} from './repoState';
import { MOCK_OID } from '../fixtures/branches';

describe('mockCanonical / resolveRepoId', () => {
  it('normalizes separators and strips trailing slashes, preserving case', () => {
    expect(mockCanonical('C:\\Repos\\Bonsai\\')).toBe('C:/Repos/Bonsai');
    expect(mockCanonical('/a//b///c/')).toBe('/a/b/c');
    expect(mockCanonical('/MiXeD/Case')).toBe('/MiXeD/Case');
  });

  it('reuses an existing repo key case-insensitively (backend dedupe scan)', () => {
    const key = '/Mock/T34-Dedupe';
    repos.set(key, createRepoState(key));
    try {
      expect(resolveRepoId('/mock/t34-dedupe/')).toBe(key);
      expect(resolveRepoId('/mock/t34-other')).toBe('/mock/t34-other');
    } finally {
      repos.delete(key);
    }
  });
});

describe('isInvalidBranchName (documented ref-format simplification)', () => {
  it.each([
    '',
    '   ',
    'has space',
    'a..b',
    'a~b',
    'a^b',
    'a:b',
    'a?b',
    'a*b',
    'a[b',
    'a\\b',
    'a@{b',
    '-leading-dash',
    '/leading-slash',
    'trailing-slash/',
    'name.lock',
  ])('rejects %j', (name) => {
    expect(isInvalidBranchName(name)).toBe(true);
  });

  it.each(['main', 'feature/x', 'fix-1.2', 'a_b', 'hotfix/UPPER'])('accepts %j', (name) => {
    expect(isInvalidBranchName(name)).toBe(false);
  });
});

describe('stripConflictMarkers', () => {
  it('drops all three marker kinds, keeps both bodies', () => {
    const text = [
      'keep 1',
      '<<<<<<< HEAD',
      'ours line',
      '=======',
      'theirs line',
      '>>>>>>> feature/login',
      'keep 2',
    ].join('\n');
    expect(stripConflictMarkers(text)).toBe('keep 1\nours line\ntheirs line\nkeep 2');
  });

  it('handles indented markers and marker-free text', () => {
    expect(stripConflictMarkers('  <<<<<<< x\nbody\n  >>>>>>> y')).toBe('body');
    expect(stripConflictMarkers('plain\ntext')).toBe('plain\ntext');
  });
});

describe('repo seeding routers (path substrings win over query params)', () => {
  it('repoOp: path substrings merge/rebase; else null without a query', () => {
    expect(repoOp('/mock/with-merge-state')).toBe('merge');
    expect(repoOp('/mock/with-rebase-state')).toBe('rebase');
    expect(repoOp('/mock/plain')).toBeNull();
  });

  it('repoGraphFixture + repoKind route by path substring', () => {
    expect(repoGraphFixture('/mock/detached-tab')).toBe('detached');
    expect(repoGraphFixture('/mock/plain')).toBe('default');
    expect(repoKind('/mock/unborn-tab', 'default')).toBe('unborn');
    expect(repoKind('/mock/plain', 'detached')).toBe('detached');
    expect(repoKind('/mock/plain', 'default')).toBe('default');
  });
});

describe('createRepoState / buildInfo', () => {
  it('default repo: seeded status, branches, stashes, remotes, no op', () => {
    const s = createRepoState('/mock/t34-seed');
    expect(s.kind).toBe('default');
    expect(s.opState).toEqual({ kind: 'none' });
    expect(s.conflicts).toEqual([]);
    expect(s.headBranch).toBe('main');
    expect(s.headOid).toBe(MOCK_OID);
    expect(s.status.staged.length).toBeGreaterThan(0);
    expect(s.stashes).toHaveLength(3);
    expect(s.remotes).toEqual([{ name: 'origin', url: 'https://example.com/repo.git' }]);
    const info = buildInfo(s, '/mock/t34-seed');
    expect(info).toMatchObject({
      isRepo: true,
      bare: false,
      head: { branchName: 'main', oid: MOCK_OID, detached: false, unborn: false },
    });
  });

  it('a path containing "merge" seeds a paused conflicted merge', () => {
    const s = createRepoState('/mock/t34-merge-tab');
    expect(s.opState.kind).toBe('merge');
    expect(s.conflicts.map((c) => c.path)).toEqual(['README.md', 'src/auth.ts']);
    expect(s.conflictTexts.get('src/auth.ts')?.kind).toBe('bothModified');
    expect(s.status.conflicted).toHaveLength(2);
    // README.md is conflicted, not plain-modified, while paused.
    expect(s.status.unstaged.some((e) => e.path === 'README.md')).toBe(false);
  });

  it('a path containing "rebase" seeds a paused rebase at step 2/3', () => {
    const s = createRepoState('/mock/t34-rebase-tab');
    expect(s.opState).toMatchObject({ kind: 'rebase', currentStep: 2, totalSteps: 3 });
    expect(s.conflicts.map((c) => c.path)).toEqual(['src/auth.ts']);
  });

  it('unborn / detached kinds shape buildInfo accordingly (no stashes/submodules)', () => {
    const unborn = createRepoState('/mock/t34-unborn-tab');
    expect(buildInfo(unborn, 'p').head).toEqual({
      branchName: 'main',
      oid: '',
      detached: false,
      unborn: true,
    });
    expect(unborn.stashes).toEqual([]);
    const detached = createRepoState('/mock/t34-detached-tab');
    expect(buildInfo(detached, 'p').head).toMatchObject({ branchName: null, detached: true });
    expect(detached.graphFixture).toBe('detached');
  });
});

describe('requireRepo / isRefTip', () => {
  it('requireRepo throws the backend noRepo shape for unknown ids', () => {
    expect(() => requireRepo('/nope/never-opened')).toThrowError(
      expect.objectContaining({ kind: 'noRepo' }),
    );
  });

  it('isRefTip resolves HEAD, local tips, and remote tips; not random oids', () => {
    const s = createRepoState('/mock/t34-reftip');
    expect(isRefTip(s, s.branches.head.oid)).toBe(true);
    expect(isRefTip(s, 'a'.repeat(40))).toBe(true); // feature/sidebar tip
    expect(isRefTip(s, 'f0'.repeat(20))).toBe(false);
  });
});

describe('buildStaleReport', () => {
  it('classifies only STALE_SEED branches, excluding base + current HEAD', () => {
    const s = createRepoState('/mock/t34-stale');
    const report = buildStaleReport(s);
    expect(report.base).toBe('main');
    const names = report.branches.map((b) => b.name);
    expect(names).toEqual(['feature/gone', 'feature/merged-a', 'feature/merged-b']);
    expect(names).not.toContain('experiment-unmerged');
    const gone = report.branches.find((b) => b.name === 'feature/gone');
    expect(gone).toMatchObject({ goneUpstream: true, merged: false });
    const merged = report.branches.find((b) => b.name === 'feature/merged-a');
    expect(merged).toMatchObject({ merged: true, ahead: 0 });
  });

  it('shrinks naturally after a local branch is removed', () => {
    const s = createRepoState('/mock/t34-stale-2');
    s.branches.local = s.branches.local.filter((b) => b.name !== 'feature/merged-a');
    expect(buildStaleReport(s).branches.map((b) => b.name)).toEqual([
      'feature/gone',
      'feature/merged-b',
    ]);
  });
});
