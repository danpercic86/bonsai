// T3.1 — workspaceMenus part 1: branchMenuItems, resetMenuItems,
// commitMenuItems (incl. the shared commit-action set + bisect), and
// buildContextItems routing. Part 2 (stash/submodule/worktree/tag/remote/
// external + clipboard wiring) lives in workspaceMenusMore.test.ts.
import { describe, expect, it, vi } from 'vitest';

import { createWorkspaceMenus } from './workspaceMenus';
import {
  OID_FEATURE,
  OID_HEAD,
  OID_OTHER,
  OID_REMOTE,
  featureBranch,
  itemByLabel,
  labelsOf,
  mainBranch,
  makeDeps,
  makeHead,
  makeSnapshot,
} from '../test/workspaceMenusFixtures';
import type { GraphContextTarget } from '../graph/GraphCanvas';
import type { RefLabel } from '../ipc';

const ref = (name: string, kind: RefLabel['kind'], isHead = false): RefLabel => ({
  name,
  kind,
  isHead,
});

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

describe('resetMenuItems', () => {
  it('single grouped item with Soft/Mixed/Hard… children; parent = mixed', () => {
    const deps = makeDeps();
    const items = createWorkspaceMenus(deps).resetMenuItems(OID_OTHER);
    expect(items).toHaveLength(1);
    expect(items[0].label).toBe('Reset main to here');
    expect(items[0].children?.map((c) => c.label)).toEqual(['Soft', 'Mixed', 'Hard…']);
    expect(items[0].children?.[2].tone).toBe('danger');
    items[0].onSelect?.();
    expect(deps.setPendingReset).toHaveBeenLastCalledWith({ oid: OID_OTHER, mode: 'mixed' });
    items[0].children?.[0].onSelect?.();
    expect(deps.setPendingReset).toHaveBeenLastCalledWith({ oid: OID_OTHER, mode: 'soft' });
    items[0].children?.[2].onSelect?.();
    expect(deps.setPendingReset).toHaveBeenLastCalledWith({ oid: OID_OTHER, mode: 'hard' });
  });

  it('[] when detached, unborn, head null, or target == current tip', () => {
    const m = (over: Parameters<typeof makeDeps>[0]) => createWorkspaceMenus(makeDeps(over));
    expect(m({ head: makeHead({ detached: true }) }).resetMenuItems(OID_OTHER)).toEqual([]);
    expect(m({ head: makeHead({ unborn: true }) }).resetMenuItems(OID_OTHER)).toEqual([]);
    expect(m({ head: null }).resetMenuItems(OID_OTHER)).toEqual([]);
    expect(m({}).resetMenuItems(OID_HEAD)).toEqual([]);
  });

  it('falls back to "HEAD" in the label when headBranch is null', () => {
    const items = createWorkspaceMenus(makeDeps({ headBranch: null })).resetMenuItems(OID_OTHER);
    expect(items[0].label).toBe('Reset HEAD to here');
  });
});

describe('commitMenuItems', () => {
  it('attached idle repo: full order incl. interactive rebase, bisect pair, reset', () => {
    expect(labelsOf(createWorkspaceMenus(makeDeps()).commitMenuItems(OID_OTHER))).toEqual([
      'Create branch here',
      'Create tag here',
      'Compare with HEAD',
      'Explain this commit',
      'Cherry-pick onto current…',
      'Revert commit',
      'Interactive rebase from here…',
      'Start bisect: mark this BAD',
      'Mark GOOD & start bisect',
      'Reset main to here',
    ]);
  });

  it('unborn / null head → []', () => {
    expect(
      createWorkspaceMenus(makeDeps({ head: makeHead({ unborn: true }) })).commitMenuItems(OID_OTHER),
    ).toEqual([]);
    expect(createWorkspaceMenus(makeDeps({ head: null })).commitMenuItems(OID_OTHER)).toEqual([]);
  });

  it('detached HEAD: read-only actions only (Explain still offered)', () => {
    const items = createWorkspaceMenus(
      makeDeps({ head: makeHead({ detached: true }), headBranch: null }),
    ).commitMenuItems(OID_OTHER);
    expect(labelsOf(items)).toEqual([
      'Create branch here',
      'Create tag here',
      'Compare with HEAD',
      'Explain this commit',
    ]);
  });

  it('oid == HEAD tip → no reset item', () => {
    expect(labelsOf(createWorkspaceMenus(makeDeps()).commitMenuItems(OID_HEAD))).not.toContain(
      'Reset main to here',
    );
  });

  it('Explain is gated by aiEligible only, and passes the short-oid title', () => {
    const deps = makeDeps({ aiEligible: true, mutating: true });
    const items = createWorkspaceMenus(deps).commitMenuItems(OID_OTHER);
    const explain = itemByLabel(items, 'Explain this commit');
    expect(explain.disabled).toBe(false); // NOT gated by mutating
    explain.onSelect?.();
    expect(deps.runAnalyze).toHaveBeenCalledWith(
      { kind: 'commit', oid: OID_OTHER },
      'explain',
      `Explain commit ${OID_OTHER.slice(0, 7)}`,
    );
    expect(
      itemByLabel(
        createWorkspaceMenus(makeDeps({ aiEligible: false })).commitMenuItems(OID_OTHER),
        'Explain this commit',
      ).disabled,
    ).toBe(true);
  });

  it('bisect: entries hidden while bisectActive', () => {
    const l = labelsOf(
      createWorkspaceMenus(makeDeps({ bisectActive: true })).commitMenuItems(OID_OTHER),
    );
    expect(l).not.toContain('Start bisect: mark this BAD');
    expect(l).not.toContain('Mark GOOD & start bisect');
    expect(l).toContain('Interactive rebase from here…');
  });

  it('bisect: Mark GOOD disabled with no pending bad or when good == bad', () => {
    const none = createWorkspaceMenus(makeDeps()).commitMenuItems(OID_OTHER);
    expect(itemByLabel(none, 'Mark GOOD & start bisect').disabled).toBe(true);
    const same = createWorkspaceMenus(makeDeps({ pendingBisectBad: OID_OTHER })).commitMenuItems(
      OID_OTHER,
    );
    expect(itemByLabel(same, 'Mark GOOD & start bisect').disabled).toBe(true);
  });

  it('bisect: mark BAD then GOOD fires the handlers with (bad, good)', () => {
    const deps = makeDeps({ pendingBisectBad: OID_FEATURE });
    const items = createWorkspaceMenus(deps).commitMenuItems(OID_OTHER);
    itemByLabel(items, 'Start bisect: mark this BAD').onSelect?.();
    expect(deps.handleMarkBisectBad).toHaveBeenCalledWith(OID_OTHER);
    const good = itemByLabel(items, 'Mark GOOD & start bisect');
    expect(good.disabled).toBe(false);
    good.onSelect?.();
    expect(deps.handleStartBisect).toHaveBeenCalledWith(OID_FEATURE, OID_OTHER);
  });
});

describe('buildContextItems', () => {
  const menus = (over: Parameters<typeof makeDeps>[0] = {}) =>
    createWorkspaceMenus(makeDeps(over));
  const target = (r: RefLabel, oid = OID_OTHER): GraphContextTarget => ({ kind: 'ref', ref: r, oid });

  it('stash pill parses the index; malformed stash name → []', () => {
    const deps = makeDeps();
    const items = createWorkspaceMenus(deps).buildContextItems(
      target(ref('stash@{2}', 'stash')),
    );
    expect(labelsOf(items)).toEqual(['Apply', 'Pop', 'Drop']);
    itemByLabel(items, 'Apply').onSelect?.();
    // F-A6-B: the graph stash pill only knows the base commit oid, not the stash
    // entry oid, so it omits the wrong-target guard (oid === undefined).
    expect(deps.handleApplyStash).toHaveBeenCalledWith(2, undefined);
    expect(menus().buildContextItems(target(ref('stash@{x}', 'stash')))).toEqual([]);
    expect(menus().buildContextItems(target(ref('stash', 'stash')))).toEqual([]);
  });

  it('head pill → [] (no menu)', () => {
    expect(menus().buildContextItems(target(ref('HEAD', 'head', true)))).toEqual([]);
  });

  it('graph tag pill passes the node oid → commit actions appended', () => {
    const items = menus().buildContextItems(target(ref('v1.0', 'tag'), OID_OTHER));
    expect(labelsOf(items)).toContain('Create branch here'); // only with a non-null oid
    // P77 §3: with no sync report the first tag item is the publish action.
    expect(labelsOf(items)[0]).toBe('Push tag to origin');
  });

  it('local branch pill delegates to branchMenuItems', () => {
    const items = menus().buildContextItems(target(ref('feature', 'localBranch')));
    expect(labelsOf(items)[0]).toBe('Checkout');
    expect(labelsOf(items)).toContain('Merge feature into main');
  });

  it('current-HEAD branch pill falls back to Rename… + commit menu at the tip', () => {
    const deps = makeDeps();
    const items = createWorkspaceMenus(deps).buildContextItems(
      target(ref('main', 'localBranch', true), OID_HEAD),
    );
    expect(labelsOf(items)).toEqual([
      'Rename…',
      'Create branch here',
      'Create tag here',
      'Compare with HEAD',
      'Explain this commit',
      'Cherry-pick onto current…',
      'Revert commit',
      'Interactive rebase from here…',
      'Start bisect: mark this BAD',
      'Mark GOOD & start bisect',
      // No reset: the tip IS the current HEAD.
    ]);
    itemByLabel(items, 'Rename…').onSelect?.();
    expect(deps.setPendingRenameBranch).toHaveBeenCalledWith({ name: 'main' });
  });

  it('branch pill with no snapshot / unknown branch → []', () => {
    expect(
      menus({ branches: null }).buildContextItems(target(ref('feature', 'localBranch'))),
    ).toEqual([]);
    expect(menus().buildContextItems(target(ref('ghost', 'localBranch')))).toEqual([]);
  });

  it('commit row target routes to commitMenuItems', () => {
    const items = menus().buildContextItems({ kind: 'commit', index: 3, oid: OID_OTHER });
    expect(labelsOf(items)).toContain('Compare with HEAD');
    expect(labelsOf(items)).toContain('Reset main to here');
  });
});

describe('purity', () => {
  it('building menus fires NO handlers until an onSelect is invoked', () => {
    const deps = makeDeps({ aiEligible: true, pendingBisectBad: OID_FEATURE });
    const menus = createWorkspaceMenus(deps);
    menus.branchMenuItems('feature', 'localBranch');
    menus.commitMenuItems(OID_OTHER);
    menus.buildContextItems({ kind: 'commit', index: 0, oid: OID_OTHER });
    for (const [k, v] of Object.entries(deps)) {
      if (typeof v === 'function' && vi.isMockFunction(v)) {
        expect(v, k).not.toHaveBeenCalled();
      }
    }
  });
});
