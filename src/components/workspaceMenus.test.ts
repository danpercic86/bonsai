// T3.1 — workspaceMenus part 1a: branchMenuItems (local/remote/AI/degraded
// HEAD states). resetMenuItems, commitMenuItems, checkoutMenuItems,
// buildContextItems routing, and purity live in workspaceMenusCommit.test.ts.
// Part 2 (stash/submodule/worktree/tag/remote/external + clipboard wiring)
// lives in workspaceMenusMore.test.ts.
import { describe, expect, it } from 'vitest';

import { createWorkspaceMenus } from './workspaceMenus';
import {
  OID_FEATURE,
  OID_HEAD,
  OID_REMOTE,
  featureBranch,
  itemByLabel,
  labelsOf,
  mainBranch,
  makeDeps,
  makeHead,
  makeSnapshot,
} from '../test/workspaceMenusFixtures';

describe('branchMenuItems — local branch, idle attached repo', () => {
  it('exact item order (baseline combo)', () => {
    const menus = createWorkspaceMenus(makeDeps());
    expect(labelsOf(menus.branchMenuItems('feature', 'localBranch'))).toEqual([
      'Checkout',
      'Copy branch name',
      'Rename…',
      'View reflog',
      'Merge feature into main',
      'Rebase main onto feature',
      'Create branch here',
      'Create tag here',
      'Compare with HEAD',
      'Explain this commit',
      'Cherry-pick onto current…',
      'Revert commit',
      'Delete',
      'Reset main to here',
    ]);
  });

  it('wires the tip oid: checkout/delete/reset/create-branch handlers', () => {
    const deps = makeDeps();
    const items = createWorkspaceMenus(deps).branchMenuItems('feature', 'localBranch');
    itemByLabel(items, 'Checkout').onSelect?.();
    expect(deps.handleCheckoutBranch).toHaveBeenCalledWith('feature');
    itemByLabel(items, 'Delete').onSelect?.();
    expect(deps.setPendingDeleteBranch).toHaveBeenCalledWith('feature');
    itemByLabel(items, 'Create branch here').onSelect?.();
    expect(deps.setPendingCreateBranch).toHaveBeenCalledWith({ oid: OID_FEATURE });
    itemByLabel(items, 'Rename…').onSelect?.();
    expect(deps.setPendingRenameBranch).toHaveBeenCalledWith({ name: 'feature' });
    itemByLabel(items, 'View reflog').onSelect?.();
    expect(deps.onViewReflog).toHaveBeenCalledWith('feature');
  });

  it('Delete is danger-toned; rebase parent has Standard + Interactive… children', () => {
    const deps = makeDeps();
    const items = createWorkspaceMenus(deps).branchMenuItems('feature', 'localBranch');
    expect(itemByLabel(items, 'Delete').tone).toBe('danger');
    const rebase = itemByLabel(items, 'Rebase main onto feature');
    expect(rebase.children?.map((c) => c.label)).toEqual(['Standard', 'Interactive…']);
    rebase.onSelect?.();
    expect(deps.setPendingRebase).toHaveBeenCalledWith({ name: 'feature', cur: 'main' });
    rebase.children?.[1].onSelect?.();
    expect(deps.openRebasePlan).toHaveBeenCalledWith({ ontoOid: OID_FEATURE, ontoLabel: 'feature' });
  });

  it('null snapshot / unknown name / current HEAD branch → []', () => {
    expect(
      createWorkspaceMenus(makeDeps({ branches: null })).branchMenuItems('feature', 'localBranch'),
    ).toEqual([]);
    expect(createWorkspaceMenus(makeDeps()).branchMenuItems('nope', 'localBranch')).toEqual([]);
    expect(createWorkspaceMenus(makeDeps()).branchMenuItems('main', 'localBranch')).toEqual([]);
  });

  it('gate (mutating): mutations disabled, read-only items stay enabled', () => {
    const items = createWorkspaceMenus(
      makeDeps({ mutating: true, aiEligible: true }),
    ).branchMenuItems('feature', 'localBranch');
    const disabled = (l: string) => itemByLabel(items, l).disabled;
    expect(disabled('Checkout')).toBe(true);
    expect(disabled('Rename…')).toBe(true);
    expect(disabled('Merge feature into main')).toBe(true);
    expect(disabled('Rebase main onto feature')).toBe(true);
    expect(disabled('Delete')).toBe(true);
    expect(disabled('Reset main to here')).toBe(true);
    // Read-only / AI items are never gated by mutating.
    expect(disabled('Copy branch name')).toBe(false);
    expect(disabled('View reflog')).toBe(false);
    expect(disabled('Compare with HEAD')).toBe(false);
    expect(disabled('Explain this commit')).toBe(false);
    expect(disabled('Summarize branch…')).toBe(false);
  });

  it('opActive gates the same subset as mutating', () => {
    const items = createWorkspaceMenus(makeDeps({ opActive: true })).branchMenuItems(
      'feature',
      'localBranch',
    );
    expect(itemByLabel(items, 'Checkout').disabled).toBe(true);
    expect(itemByLabel(items, 'Copy branch name').disabled).toBe(false);
  });
});

describe('branchMenuItems — AI entries', () => {
  it('aiEligible adds Summarize + Review after View reflog; summarize base = main', () => {
    const deps = makeDeps({ aiEligible: true });
    const items = createWorkspaceMenus(deps).branchMenuItems('feature', 'localBranch');
    const l = labelsOf(items);
    expect(l.indexOf('Summarize branch…')).toBe(l.indexOf('View reflog') + 1);
    expect(l.indexOf('Review branch…')).toBe(l.indexOf('Summarize branch…') + 1);
    itemByLabel(items, 'Summarize branch…').onSelect?.();
    expect(deps.runSummarize).toHaveBeenCalledWith('main', 'feature');
    itemByLabel(items, 'Review branch…').onSelect?.();
    expect(deps.runAnalyze).toHaveBeenCalledWith(
      { kind: 'branch', name: 'feature' },
      'review',
      'Review branch feature',
    );
  });

  it('primary falls back to master when main is absent', () => {
    const deps = makeDeps({
      aiEligible: true,
      branches: makeSnapshot({
        local: [featureBranch(), mainBranch({ name: 'master' })],
      }),
      headBranch: mainBranch({ name: 'master' }),
    });
    const items = createWorkspaceMenus(deps).branchMenuItems('feature', 'localBranch');
    itemByLabel(items, 'Summarize branch…').onSelect?.();
    expect(deps.runSummarize).toHaveBeenCalledWith('master', 'feature');
  });

  it('target IS the primary → base is its upstream; no upstream → Summarize omitted', () => {
    // main is NOT head here (dev is), so main gets a full branch menu.
    const dev = featureBranch({ name: 'dev', isHead: true, tip: OID_HEAD });
    const withUpstream = makeDeps({
      aiEligible: true,
      branches: makeSnapshot({ local: [dev, mainBranch({ isHead: false, tip: OID_FEATURE })] }),
      headBranch: dev,
    });
    const items = createWorkspaceMenus(withUpstream).branchMenuItems('main', 'localBranch');
    itemByLabel(items, 'Summarize branch…').onSelect?.();
    expect(withUpstream.runSummarize).toHaveBeenCalledWith('origin/main', 'main');

    const noUpstream = makeDeps({
      aiEligible: true,
      branches: makeSnapshot({
        local: [dev, mainBranch({ isHead: false, tip: OID_FEATURE, upstream: null })],
      }),
      headBranch: dev,
    });
    const items2 = createWorkspaceMenus(noUpstream).branchMenuItems('main', 'localBranch');
    expect(labelsOf(items2)).not.toContain('Summarize branch…');
    expect(labelsOf(items2)).toContain('Review branch…'); // review needs no base
  });

  it('AI entries never appear for remote branches', () => {
    const items = createWorkspaceMenus(makeDeps({ aiEligible: true })).branchMenuItems(
      'origin/feature',
      'remoteBranch',
    );
    expect(labelsOf(items)).not.toContain('Summarize branch…');
    expect(labelsOf(items)).not.toContain('Review branch…');
  });
});

describe('branchMenuItems — remote branch + degraded HEAD states', () => {
  it('remote branch: no Rename/reflog; checkout + delete route to remote handlers', () => {
    const deps = makeDeps();
    const items = createWorkspaceMenus(deps).branchMenuItems('origin/feature', 'remoteBranch');
    expect(labelsOf(items)).toEqual([
      'Checkout',
      'Copy branch name',
      'Merge origin/feature into main',
      'Rebase main onto origin/feature',
      'Create branch here',
      'Create tag here',
      'Compare with HEAD',
      'Explain this commit',
      'Cherry-pick onto current…',
      'Revert commit',
      'Delete',
      'Reset main to here',
    ]);
    itemByLabel(items, 'Checkout').onSelect?.();
    expect(deps.handleCheckoutRemote).toHaveBeenCalledWith('origin/feature');
    itemByLabel(items, 'Delete').onSelect?.();
    expect(deps.setPendingDeleteRemote).toHaveBeenCalledWith('origin/feature');
    itemByLabel(items, 'Create branch here').onSelect?.();
    expect(deps.setPendingCreateBranch).toHaveBeenCalledWith({ oid: OID_REMOTE });
  });

  it('detached HEAD: no merge/rebase (cur null), no cherry-pick/revert, no reset', () => {
    const items = createWorkspaceMenus(
      makeDeps({ head: makeHead({ detached: true, branchName: null }), headBranch: null }),
    ).branchMenuItems('feature', 'localBranch');
    expect(labelsOf(items)).toEqual([
      'Checkout',
      'Copy branch name',
      'Rename…',
      'View reflog',
      'Create branch here',
      'Create tag here',
      'Compare with HEAD',
      'Explain this commit',
      'Delete',
    ]);
  });

  it('unborn HEAD: only checkout/copy/rename/reflog/delete survive', () => {
    const items = createWorkspaceMenus(
      makeDeps({ head: makeHead({ unborn: true, branchName: null }), headBranch: null }),
    ).branchMenuItems('feature', 'localBranch');
    expect(labelsOf(items)).toEqual([
      'Checkout',
      'Copy branch name',
      'Rename…',
      'View reflog',
      'Delete',
    ]);
  });
});
