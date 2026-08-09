import { describe, expect, it } from 'vitest';

import type { RefLabel } from '../ipc';
import type { CiBadge, PrBadge } from './forgeBadges';
import {
  chipHitAt,
  fallbackBranchRef,
  hitTestRow,
  pillHitAt,
  prBadgeHitAt,
  sameTarget,
  signalHitAt,
  targetRefOf,
} from './hitTest';
import type { TooltipState } from './hitTest';
import type { LaidRefLabel, RefEntity } from './refLabels';
import type { Rect } from './viewport';

const RH = 32;

// ---------- hitTestRow ----------

describe('hitTestRow', () => {
  it('maps y within the first row to row 0', () => {
    expect(hitTestRow(0, 0, 0, 10, RH)).toBe(0);
    expect(hitTestRow(31.999, 0, 0, 10, RH)).toBe(0);
  });

  it('exact row boundary belongs to the NEXT row (floor semantics)', () => {
    expect(hitTestRow(RH, 0, 0, 10, RH)).toBe(1);
    expect(hitTestRow(RH - 0.001, 0, 0, 10, RH)).toBe(0);
  });

  it('adds scrollTop before dividing', () => {
    expect(hitTestRow(0, 5 * RH, 0, 10, RH)).toBe(5);
    expect(hitTestRow(RH / 2, 5 * RH + RH / 2, 0, 10, RH)).toBe(6);
  });

  it('negative y above the list → null', () => {
    expect(hitTestRow(-1, 0, 0, 10, RH)).toBeNull();
    expect(hitTestRow(-1, 0, 1, 10, RH)).toBeNull(); // even with a WIP row
  });

  it('below the last row → null', () => {
    expect(hitTestRow(10 * RH, 0, 0, 10, RH)).toBeNull();
    expect(hitTestRow(10 * RH - 0.5, 0, 0, 10, RH)).toBe(9);
  });

  it('empty graph → null everywhere (except a WIP row)', () => {
    expect(hitTestRow(0, 0, 0, 0, RH)).toBeNull();
    expect(hitTestRow(0, 0, 1, 0, RH)).toBe('wip');
  });

  it('WIP offset: raw row 0 is the WIP row; layout rows shift by one', () => {
    expect(hitTestRow(0, 0, 1, 10, RH)).toBe('wip');
    expect(hitTestRow(RH, 0, 1, 10, RH)).toBe(0);
    expect(hitTestRow(11 * RH - 1, 0, 1, 10, RH)).toBe(9);
    expect(hitTestRow(11 * RH, 0, 1, 10, RH)).toBeNull();
  });

  it('WIP row is only hittable near the top (scrolled away → real rows)', () => {
    expect(hitTestRow(0, 2 * RH, 1, 10, RH)).toBe(1);
  });

  it('fractional scrollTop + compact rowHeight', () => {
    expect(hitTestRow(10, 12.5, 0, 100, 22)).toBe(1); // (10+12.5)/22 = 1.02…
  });

  it('20k rows: last-row edge is exact', () => {
    const n = 20_000;
    expect(hitTestRow(0, (n - 1) * RH, 0, n, RH)).toBe(n - 1);
    expect(hitTestRow(RH, (n - 1) * RH, 0, n, RH)).toBeNull();
  });
});

// ---------- ref targeting ----------

const local = (name: string, isHead = false): RefLabel => ({ name, kind: 'localBranch', isHead });
const remote = (name: string): RefLabel => ({ name, kind: 'remoteBranch', isHead: false });
const tag = (name: string): RefLabel => ({ name, kind: 'tag', isHead: false });

const branchEntity = (over: Partial<Extract<RefEntity, { kind: 'branch' }>> = {}): RefEntity => ({
  kind: 'branch',
  name: 'main',
  hasLocal: true,
  remotes: ['origin/main'],
  isHead: false,
  refs: [local('main'), remote('origin/main')],
  ...over,
});

describe('targetRefOf', () => {
  it('local branch entity → the LOCAL ref, even with a remote present', () => {
    expect(targetRefOf(branchEntity())).toEqual(local('main'));
  });

  it('remote-only branch entity → the first remote ref', () => {
    const e = branchEntity({ hasLocal: false, refs: [remote('origin/main')] });
    expect(targetRefOf(e)).toEqual(remote('origin/main'));
  });

  it('hasLocal but refs missing a localBranch → null (defensive)', () => {
    const e = branchEntity({ refs: [remote('origin/main')] });
    expect(targetRefOf(e)).toBeNull();
  });

  it('tag / head / stash entities return their own ref', () => {
    const t: RefEntity = { kind: 'tag', name: 'v1.0', ref: tag('v1.0') };
    expect(targetRefOf(t)).toEqual(tag('v1.0'));
  });
});

describe('fallbackBranchRef', () => {
  it('first branch entity wins', () => {
    const ents: RefEntity[] = [
      { kind: 'tag', name: 'v1', ref: tag('v1') },
      branchEntity({ name: 'a', refs: [local('a')] }),
      branchEntity({ name: 'b', refs: [local('b')] }),
    ];
    expect(fallbackBranchRef(ents)).toEqual(local('a'));
  });

  it('no branch entity → null', () => {
    expect(fallbackBranchRef([{ kind: 'tag', name: 'v1', ref: tag('v1') }])).toBeNull();
    expect(fallbackBranchRef([])).toBeNull();
  });

  it('skips a branch whose target resolves null, takes the next', () => {
    const broken = branchEntity({ refs: [] }); // hasLocal but no local ref
    const good = branchEntity({ name: 'ok', refs: [local('ok')] });
    expect(fallbackBranchRef([broken, good])).toEqual(local('ok'));
  });
});

// ---------- laid-label hits ----------

const style = { fill: '#000', text: '#fff', border: null, label: 'x' };
const icons = { laptop: false, cloud: false, stash: false };

const laidPill = (x: number, w: number, entity: RefEntity | null = branchEntity()): LaidRefLabel => ({
  entity,
  style,
  x,
  w,
  icons,
  chip: null,
  signals: null,
});

const pr: PrBadge = { number: 42, title: 'T', state: 'open', isDraft: false, url: 'u' };
const ci: CiBadge = { rollup: 'success', passed: 3, failed: 0, pending: 0, total: 3 };

describe('chipHitAt / pillHitAt', () => {
  const laid = [laidPill(10, 40), laidPill(56, 30), laidPill(92, 20, null)]; // last = "+n" chip

  it('pill edges are INCLUSIVE on both sides', () => {
    expect(pillHitAt(laid, 10)).toBe(laid[0]);
    expect(pillHitAt(laid, 50)).toBe(laid[0]); // x + w
    expect(pillHitAt(laid, 50.001)).toBeUndefined();
    expect(pillHitAt(laid, 9.999)).toBeUndefined();
  });

  it('pillHitAt skips the chip; chipHitAt skips pills', () => {
    expect(pillHitAt(laid, 100)).toBeUndefined();
    expect(chipHitAt(laid, 100)).toBe(laid[2]);
    expect(chipHitAt(laid, 30)).toBeUndefined();
    expect(chipHitAt(laid, 92)).toBe(laid[2]);
    expect(chipHitAt(laid, 112)).toBe(laid[2]); // inclusive right edge
    expect(chipHitAt(laid, 112.5)).toBeUndefined();
  });

  it('gap between pills hits nothing', () => {
    expect(pillHitAt(laid, 53)).toBeUndefined();
    expect(chipHitAt(laid, 53)).toBeUndefined();
  });

  it('empty laid list → undefined (empty graph rows)', () => {
    expect(pillHitAt([], 10)).toBeUndefined();
    expect(chipHitAt([], 10)).toBeUndefined();
  });

  it('negative x hits nothing', () => {
    expect(pillHitAt(laid, -5)).toBeUndefined();
  });
});

describe('prBadgeHitAt / signalHitAt', () => {
  const withSignals: LaidRefLabel = {
    ...laidPill(10, 40),
    signals: { pr: { badge: pr, x: 56, w: 30 }, ci: { badge: ci, cx: 95 } },
  };
  const laid = [withSignals, laidPill(120, 30)];
  const CI_SIZE = 11;

  it('PR rect edges are inclusive', () => {
    expect(prBadgeHitAt(laid, 56)?.badge.number).toBe(42);
    expect(prBadgeHitAt(laid, 86)?.badge.number).toBe(42);
    expect(prBadgeHitAt(laid, 86.5)).toBeNull();
    expect(prBadgeHitAt(laid, 55.5)).toBeNull();
  });

  it('no signals anywhere → null', () => {
    expect(prBadgeHitAt([laidPill(10, 40)], 20)).toBeNull();
    expect(signalHitAt([laidPill(10, 40)], 20, CI_SIZE)).toBeNull();
    expect(signalHitAt([], 20, CI_SIZE)).toBeNull();
  });

  it('signalHitAt: PR hit', () => {
    const hit = signalHitAt(laid, 60, CI_SIZE);
    expect(hit).not.toBeNull();
    expect(hit?.kind).toBe('pr');
  });

  it('signalHitAt: CI dot is a half-size box around cx, edges inclusive', () => {
    const half = CI_SIZE / 2;
    expect(signalHitAt(laid, 95, CI_SIZE)?.kind).toBe('ci');
    expect(signalHitAt(laid, 95 - half, CI_SIZE)?.kind).toBe('ci');
    expect(signalHitAt(laid, 95 + half, CI_SIZE)?.kind).toBe('ci');
    expect(signalHitAt(laid, 95 + half + 0.01, CI_SIZE)).toBeNull();
    expect(signalHitAt(laid, 95 - half - 0.01, CI_SIZE)).toBeNull();
  });

  it('per-label precedence: PR is checked before CI when boxes touch', () => {
    const overlapping: LaidRefLabel = {
      ...laidPill(10, 40),
      signals: { pr: { badge: pr, x: 56, w: 30 }, ci: { badge: ci, cx: 86 } },
    };
    // x=84 is inside BOTH the PR rect (56..86) and the CI box (80.5..91.5).
    expect(signalHitAt([overlapping], 84, CI_SIZE)?.kind).toBe('pr');
  });

  it('ci-only signals (pr null) resolve', () => {
    const ciOnly: LaidRefLabel = { ...laidPill(10, 40), signals: { pr: null, ci: { badge: ci, cx: 60 } } };
    expect(signalHitAt([ciOnly], 60, CI_SIZE)?.kind).toBe('ci');
    expect(prBadgeHitAt([ciOnly], 60)).toBeNull();
  });
});

// ---------- tooltip identity ----------

describe('sameTarget', () => {
  const anchor: Rect = { left: 0, top: 0, width: 10, height: 10 };
  const anchor2: Rect = { left: 99, top: 99, width: 1, height: 1 };
  const av = (text: string, a: Rect = anchor): TooltipState => ({ kind: 'avatar', text, anchor: a });

  it('null identity', () => {
    expect(sameTarget(null, null)).toBe(true);
    expect(sameTarget(null, av('x'))).toBe(false);
    expect(sameTarget(av('x'), null)).toBe(false);
  });

  it('same kind + content → equal even with a different anchor (content identity)', () => {
    expect(sameTarget(av('Dan'), av('Dan', anchor2))).toBe(true);
    expect(sameTarget(av('Dan'), av('Dana'))).toBe(false);
  });

  it('kind mismatch → false', () => {
    expect(sameTarget(av('x'), { kind: 'ref', text: 'x', anchor })).toBe(false);
  });

  it('line-list kinds compare joined lines', () => {
    const ov = (lines: string[]): TooltipState => ({ kind: 'overflow', lines, anchor });
    expect(sameTarget(ov(['a', 'b']), ov(['a', 'b']))).toBe(true);
    expect(sameTarget(ov(['a', 'b']), ov(['a', 'c']))).toBe(false);
    expect(sameTarget(ov(['a', 'b']), ov(['a']))).toBe(false);
    const prT = (lines: string[]): TooltipState => ({ kind: 'pr', lines, anchor });
    const ciT = (lines: string[]): TooltipState => ({ kind: 'ci', lines, anchor });
    expect(sameTarget(prT(['x']), prT(['x']))).toBe(true);
    expect(sameTarget(prT(['x']), ciT(['x']))).toBe(false);
    const dt = (lines: string[]): TooltipState => ({ kind: 'date', lines, anchor });
    expect(sameTarget(dt(['a', 'b']), dt(['a', 'b']))).toBe(true);
  });
});
