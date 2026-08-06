// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import { fixtureOid } from './oids';
import type { GraphFixture, RepoKind } from '../mock/repoState';
import type { WorktreeInfo } from '../types';

/** Seed the DEFAULT repo's worktrees so the sidebar section shows every badge:
 *  main, a clean linked, a locked linked, a stale/prunable linked (P27 §5).
 *  Stored rows carry `isCurrent: false`; the flag is computed per viewing repo
 *  at list time (listWorktrees) since all tabs share one repository. */
export function seedWorktrees(kind: RepoKind, graphFixture: GraphFixture): WorktreeInfo[] {
  if (kind !== 'default' || graphFixture !== 'default') return [];
  return [
    {
      name: 'repo',
      absPath: '/mock/repo',
      relPath: null,
      branch: 'main',
      headOid: fixtureOid(1),
      locked: false,
      lockReason: null,
      isMain: true,
      isCurrent: false,
      prunable: false,
      valid: true,
    },
    {
      name: 'feature-login',
      absPath: '/mock/.worktrees/repo/feature-login',
      relPath: null,
      branch: 'feature/login',
      headOid: fixtureOid(3),
      locked: false,
      lockReason: null,
      isMain: false,
      isCurrent: false,
      prunable: false,
      valid: true,
    },
    {
      name: 'release-1.2',
      absPath: '/mock/.worktrees/repo/release-1.2',
      relPath: null,
      branch: 'release/1.2',
      headOid: fixtureOid(4),
      locked: true,
      lockReason: 'pinned for QA',
      isMain: false,
      isCurrent: false,
      prunable: false,
      valid: true,
    },
    {
      name: 'hotfix-stale',
      absPath: '/mock/.worktrees/repo/hotfix-stale',
      relPath: null,
      branch: null,
      headOid: null,
      locked: false,
      lockReason: null,
      isMain: false,
      isCurrent: false,
      prunable: true,
      valid: false,
    },
  ];
}
