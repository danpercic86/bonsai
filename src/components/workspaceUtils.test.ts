import { describe, expect, it } from 'vitest';

import type { RepoInfo, WorktreeInfo } from '../ipc';
import {
  MAX_HISTORY_UI,
  isUsableRepo,
  minutesLabel,
  shortOid,
  worktreeContainerPreview,
} from './workspaceUtils';

describe('shortOid', () => {
  it('takes the first 7 chars of a 40-hex oid', () => {
    expect(shortOid('0123456789abcdef0123456789abcdef01234567')).toBe('0123456');
  });

  it('shorter/empty input passes through unpadded', () => {
    expect(shortOid('abc')).toBe('abc');
    expect(shortOid('')).toBe('');
  });
});

describe('minutesLabel', () => {
  it('under half a minute → "<1m"', () => {
    expect(minutesLabel(0)).toBe('<1m');
    expect(minutesLabel(29_999)).toBe('<1m'); // rounds to 0
  });

  it('30s rounds up to "1m"', () => {
    expect(minutesLabel(30_000)).toBe('1m');
  });

  it('negative deltas clamp to "<1m" (clock skew)', () => {
    expect(minutesLabel(-5_000)).toBe('<1m');
    expect(minutesLabel(-999_999_999)).toBe('<1m');
  });

  it('minute range up to 59m', () => {
    expect(minutesLabel(5 * 60_000)).toBe('5m');
    expect(minutesLabel(59 * 60_000)).toBe('59m');
  });

  it('59.5 min rounds to 60 → switches to hours', () => {
    expect(minutesLabel(59.5 * 60_000)).toBe('1h');
  });

  it('hours round to nearest (89m → 1h, 90m → 2h)', () => {
    expect(minutesLabel(89 * 60_000)).toBe('1h');
    expect(minutesLabel(90 * 60_000)).toBe('2h');
  });

  it('huge delta stays finite', () => {
    expect(minutesLabel(1000 * 3_600_000)).toBe('1000h');
  });
});

describe('isUsableRepo', () => {
  const info = (isRepo: boolean, bare: boolean): RepoInfo => ({
    path: 'p',
    isRepo,
    bare,
    head: null,
  });

  it('true only for a non-bare repo', () => {
    expect(isUsableRepo(info(true, false))).toBe(true);
    expect(isUsableRepo(info(true, true))).toBe(false);
    expect(isUsableRepo(info(false, false))).toBe(false);
  });
});

describe('MAX_HISTORY_UI', () => {
  it('is the documented 200-entry cap', () => {
    expect(MAX_HISTORY_UI).toBe(200);
  });
});

describe('worktreeContainerPreview', () => {
  const wt = (absPath: string, isMain: boolean): WorktreeInfo => ({
    name: 'n',
    absPath,
    relPath: null,
    branch: null,
    headOid: null,
    locked: false,
    lockReason: null,
    isMain,
    isCurrent: false,
    prunable: false,
    valid: true,
  });

  it('derives <parent>/.worktrees/<repo-name> from the main worktree path', () => {
    expect(worktreeContainerPreview([wt('/home/me/repos/bonsai', true)], 'ignored')).toBe(
      '/home/me/repos/.worktrees/bonsai',
    );
  });

  it('normalizes Windows backslashes to forward slashes', () => {
    expect(worktreeContainerPreview([wt('D:\\Repos\\bonsai', true)], 'x')).toBe(
      'D:/Repos/.worktrees/bonsai',
    );
  });

  it('falls back to repoId when no main worktree is present', () => {
    expect(worktreeContainerPreview([wt('/linked/one', false)], 'C:/work/repo')).toBe(
      'C:/work/.worktrees/repo',
    );
  });

  it('empty worktree list uses repoId too', () => {
    expect(worktreeContainerPreview([], '/a/b')).toBe('/a/.worktrees/b');
  });

  it('path directly under root: parent stays the full base (cut === 0)', () => {
    // '/repo' → cut = 0, parent = '/repo', name = 'repo' (documented behavior).
    expect(worktreeContainerPreview([], '/repo')).toBe('/repo/.worktrees/repo');
  });

  it('path with no slash at all duplicates the base as parent and name', () => {
    expect(worktreeContainerPreview([], 'repo')).toBe('repo/.worktrees/repo');
  });
});
