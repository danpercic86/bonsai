import { describe, expect, it } from 'vitest';

import { CATEGORY_META, GIT_ACTIVITY_CATEGORIES, TARGET_PREPOSITION } from './gitActivityCategories';

/** The Rust `GitActivityCategory::ALL` wire strings, in declaration order
 *  (`crates/bonsai-core/src/git/activity_category_tests.rs` pins the same list). */
const WIRE = [
  'commit',
  'amend',
  'mergeCommit',
  'push',
  'forcePush',
  'fetch',
  'pull',
  'checkoutBranch',
  'checkoutCommit',
  'checkoutRemote',
  'createBranch',
  'deleteBranch',
  'deleteBranches',
  'renameBranch',
  'deleteRemoteTracking',
  'merge',
  'abortMerge',
  'rebase',
  'interactiveRebase',
  'rebaseContinue',
  'rebaseSkip',
  'rebaseAbort',
  'cherryPick',
  'cherryPickContinue',
  'cherryPickAbort',
  'revert',
  'revertContinue',
  'revertAbort',
  'resetSoft',
  'resetMixed',
  'resetHard',
  'stashCreate',
  'stashApply',
  'stashPop',
  'stashDrop',
  'createTag',
  'deleteTag',
  'pushTag',
  'deleteRemoteTag',
  'forceRefreshTag',
  'submoduleAdd',
  'submoduleInit',
  'submoduleUpdate',
  'submoduleSync',
  'submoduleDeinit',
  'submoduleRemove',
  'worktreeAdd',
  'worktreeRemove',
  'worktreeLock',
  'worktreeUnlock',
  'discard',
  'bisectStart',
  'bisectGood',
  'bisectBad',
  'bisectSkip',
  'bisectReset',
  'composeCommits',
  'cloneRepo',
  'initRepo',
  'addRemote',
  'removeRemote',
  'renameRemote',
  'setRemoteUrl',
];

describe('gitActivityCategories', () => {
  it('lists exactly the 63 backend categories, in declaration order', () => {
    expect(WIRE).toHaveLength(63);
    expect([...GIT_ACTIVITY_CATEGORIES]).toEqual(WIRE);
  });

  it('keys the preposition table by the same set', () => {
    expect(Object.keys(TARGET_PREPOSITION).sort()).toEqual([...WIRE].sort());
  });

  it('gives every category non-empty copy and a glyph', () => {
    for (const cat of GIT_ACTIVITY_CATEGORIES) {
      const m = CATEGORY_META[cat];
      expect(m.verb, cat).not.toBe('');
      expect(m.noun, cat).not.toBe('');
      expect(m.participle.endsWith('…'), cat).toBe(true);
      expect(typeof m.glyph, cat).toBe('function');
    }
  });
});
