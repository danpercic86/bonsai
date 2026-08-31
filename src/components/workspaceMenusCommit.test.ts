// T3.1 — workspaceMenus part 1b: resetMenuItems, commitMenuItems (incl. the
// shared commit-action set + bisect), checkoutMenuItems, buildContextItems
// routing, and purity. Branch menus live in workspaceMenus.test.ts; part 2
// (stash/submodule/worktree/tag/remote/external + clipboard wiring) lives in
// workspaceMenusMore.test.ts.
import { describe, expect, it, vi } from 'vitest';

import { createWorkspaceMenus } from './workspaceMenus';
import {
  OID_FEATURE,
  OID_HEAD,
  OID_OTHER,
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
      'Checkout commit (detached)',
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
      'Checkout commit (detached)',
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

// checkoutMenuItems is a private closure; its output is prepended FIRST into
// commitMenuItems, so we assert its structure through the head of that list.
// UI contract §1/§2/§2b: 0-tip → single top-level detached item; 1-tip →
// grouped parent (default = branch checkout) + flyout [branch, detached];
// ≥2-tip → INERT parent (no onSelect) + flyout [branch…, detached last].
describe('checkoutMenuItems (via commitMenuItems head)', () => {
  const LABEL = 'Checkout commit (detached)';

  it('0 local tips → a single top-level detached item, ordered first, exact label', () => {
    const deps = makeDeps();
    const items = createWorkspaceMenus(deps).commitMenuItems(OID_OTHER);
    // items[0] is the only checkout entry (no grouping) with the exact label.
    expect(items[0].label).toBe(LABEL);
    expect(items[0].children).toBeUndefined();
    // Exactly one checkout entry exists overall.
    expect(labelsOf(items).filter((l) => l === LABEL || l === 'Checkout')).toEqual([LABEL]);
    items[0].onSelect?.();
    expect(deps.handleCheckoutCommit).toHaveBeenCalledWith(OID_OTHER);
  });

  it('1 local tip → grouped parent (default = branch checkout) + flyout [branch, detached]', () => {
    const deps = makeDeps();
    // feature (non-head) tips OID_FEATURE.
    const items = createWorkspaceMenus(deps).commitMenuItems(OID_FEATURE);
    const parent = items[0];
    expect(parent.label).toBe('Checkout feature');
    // Parent click = default branch checkout.
    parent.onSelect?.();
    expect(deps.handleCheckoutBranch).toHaveBeenCalledWith('feature');
    // Flyout: branch first, detached LAST.
    expect(parent.children?.map((c) => c.label)).toEqual(['Checkout feature', LABEL]);
    parent.children?.[1].onSelect?.();
    expect(deps.handleCheckoutCommit).toHaveBeenCalledWith(OID_FEATURE);
  });

  it('≥2 local tips → INERT parent (no onSelect) + flyout [branch…, detached last]', () => {
    const deps = makeDeps({
      branches: makeSnapshot({
        local: [
          featureBranch({ name: 'alpha', tip: OID_OTHER }),
          featureBranch({ name: 'beta', tip: OID_OTHER }),
          mainBranch(),
        ],
      }),
    });
    const items = createWorkspaceMenus(deps).commitMenuItems(OID_OTHER);
    const parent = items[0];
    expect(parent.label).toBe('Checkout');
    // INERT: parent has NO default action (only opens the flyout).
    expect(parent.onSelect).toBeUndefined();
    // One `Checkout <name>` per branch in snapshot order, detached LAST.
    expect(parent.children?.map((c) => c.label)).toEqual([
      'Checkout alpha',
      'Checkout beta',
      LABEL,
    ]);
    // Children carry the real actions.
    parent.children?.[0].onSelect?.();
    expect(deps.handleCheckoutBranch).toHaveBeenCalledWith('alpha');
    parent.children?.[2].onSelect?.();
    expect(deps.handleCheckoutCommit).toHaveBeenCalledWith(OID_OTHER);
  });

  it('the current-HEAD branch tip is excluded from the tip list (§1)', () => {
    // main (head) tips OID_HEAD → filtered out → single detached item, no group.
    const items = createWorkspaceMenus(makeDeps()).commitMenuItems(OID_HEAD);
    expect(items[0].label).toBe(LABEL);
    expect(items[0].children).toBeUndefined();
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
    // Checkout is prepended FIRST (most-primary action).
    expect(labelsOf(items)[0]).toBe('Checkout commit (detached)');
    // P77 §3: with no sync report the first tag item is the publish action.
    expect(labelsOf(items)).toContain('Push tag to origin');
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
      // Detaching from the current branch is a real state change (UI §2b case e):
      // the branch child is filtered as current HEAD → single detached item.
      'Checkout commit (detached)',
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
