import { describe, expect, it } from 'vitest';

import {
  branchSignals,
  ciBadgeVisual,
  prBadgeVisual,
  prBadgeWidth,
} from './forgeBadges';
import type { CiBadge, PrBadge } from './forgeBadges';
import type { Theme } from './colors';
import { METRICS } from './metrics';
import type { GraphNode } from '../ipc';
import type { RefEntity } from './refLabels';
import type { GraphDisplayOptions } from './rightColumns';

// Only the theme fields the classifiers read. Distinct sentinels so assertions
// can prove which palette slot each visual pulled from.
const THEME = {
  badgeGood: '#2ea043',
  badgeWarn: '#f85149',
  badgeUnknown: '#8b949e',
  warning: '#d29922',
  text2: '#c9d1d9',
  text3: '#8b949e',
  bg2: '#161b22',
} as unknown as Theme;

function pr(over: Partial<PrBadge> = {}): PrBadge {
  return { number: 12, title: 'Add badges', state: 'open', isDraft: false, url: 'u', ...over };
}
function ci(over: Partial<CiBadge> = {}): CiBadge {
  return { rollup: 'success', passed: 3, failed: 0, pending: 0, total: 3, ...over };
}

// A deterministic 2D context: every char is CHAR_W wide, font-independent — so
// prBadgeWidth's measurement is exact and headless (no canvas), mirroring the
// refLabels.test.ts fake.
const CHAR_W = 10;
function makeCtx(): CanvasRenderingContext2D {
  return {
    font: '',
    measureText: (t: string) => ({ width: t.length * CHAR_W }) as TextMetrics,
  } as unknown as CanvasRenderingContext2D;
}

describe('prBadgeVisual', () => {
  it('open → filled green, white text, no border', () => {
    expect(prBadgeVisual(pr({ number: 12, state: 'open' }), THEME)).toEqual({
      label: '#12',
      fill: THEME.badgeGood,
      text: '#ffffff',
      border: null,
    });
  });

  it('closed → filled red (badgeWarn), white text, no border', () => {
    expect(prBadgeVisual(pr({ number: 5, state: 'closed' }), THEME)).toEqual({
      label: '#5',
      fill: THEME.badgeWarn,
      text: '#ffffff',
      border: null,
    });
  });

  it('merged → filled purple (distinct from good/warn), white text, no border', () => {
    const v = prBadgeVisual(pr({ number: 99, state: 'merged' }), THEME);
    expect(v.label).toBe('#99');
    expect(v.text).toBe('#ffffff');
    expect(v.border).toBeNull();
    expect(v.fill).not.toBe(THEME.badgeGood);
    expect(v.fill).not.toBe(THEME.badgeWarn);
    expect(v.fill).toMatch(/^#[0-9a-f]{6}$/i);
  });

  it('draft → grey OUTLINE (border set), regardless of the open state underneath', () => {
    const v = prBadgeVisual(pr({ number: 7, state: 'open', isDraft: true }), THEME);
    expect(v).toEqual({ label: '#7', fill: THEME.bg2, text: THEME.text2, border: THEME.text3 });
  });

  it('label is always `#<number>`', () => {
    expect(prBadgeVisual(pr({ number: 12345 }), THEME).label).toBe('#12345');
  });
});

describe('ciBadgeVisual', () => {
  it('success → green check', () => {
    expect(ciBadgeVisual('success', THEME)).toEqual({ glyph: 'check', color: THEME.badgeGood });
  });
  it('failure AND error → red x', () => {
    expect(ciBadgeVisual('failure', THEME)).toEqual({ glyph: 'x', color: THEME.badgeWarn });
    expect(ciBadgeVisual('error', THEME)).toEqual({ glyph: 'x', color: THEME.badgeWarn });
  });
  it('pending → amber dot', () => {
    expect(ciBadgeVisual('pending', THEME)).toEqual({ glyph: 'dot', color: THEME.warning });
  });
  it('neutral → grey dash', () => {
    expect(ciBadgeVisual('neutral', THEME)).toEqual({ glyph: 'dash', color: THEME.text3 });
  });
  it('none → null (nothing draws — copies verifyBadgeKind’s null pattern)', () => {
    expect(ciBadgeVisual('none', THEME)).toBeNull();
  });
});

function disp(over: Partial<GraphDisplayOptions> = {}): GraphDisplayOptions {
  return {
    showSha: false,
    showAuthor: false,
    showDate: false,
    dateBasis: 'author',
    showAheadBehind: false,
    branchStats: new Map(),
    showSignatureBadge: false,
    showPrBadge: false,
    showCiStatus: false,
    prByBranch: new Map(),
    ciBySha: new Map(),
    ...over,
  };
}
function branch(name: string): RefEntity {
  return { kind: 'branch', name, hasLocal: true, remotes: [], isHead: false, refs: [] };
}
const NODE = { lane: 0, id: 'tip-sha' } as unknown as GraphNode;

describe('branchSignals (pure display-time gate)', () => {
  const prByBranch = new Map([['feat', pr({ number: 42 })]]);
  const ciBySha = new Map([['tip-sha', ci({ rollup: 'failure' })]]);

  it('both null when the toggles are OFF (compact-suppressed arrives as OFF)', () => {
    const d = disp({ showPrBadge: false, showCiStatus: false, prByBranch, ciBySha });
    expect(branchSignals(branch('feat'), NODE, d)).toEqual({ pr: null, ci: null });
  });

  it('both null for a NON-branch entity even when the maps have entries', () => {
    const tag: RefEntity = { kind: 'tag', name: 'feat', ref: { name: 'feat', kind: 'tag', isHead: false } };
    const d = disp({ showPrBadge: true, showCiStatus: true, prByBranch, ciBySha });
    expect(branchSignals(tag, NODE, d)).toEqual({ pr: null, ci: null });
  });

  it('present when ON and cached — PR keyed by branch name, CI keyed by node id', () => {
    const d = disp({ showPrBadge: true, showCiStatus: true, prByBranch, ciBySha });
    const { pr: p, ci: c } = branchSignals(branch('feat'), NODE, d);
    expect(p?.number).toBe(42);
    expect(c?.rollup).toBe('failure');
  });

  it('null per-signal when the specific toggle is off (independently toggleable)', () => {
    const prOnly = disp({ showPrBadge: true, showCiStatus: false, prByBranch, ciBySha });
    expect(branchSignals(branch('feat'), NODE, prOnly).ci).toBeNull();
    expect(branchSignals(branch('feat'), NODE, prOnly).pr?.number).toBe(42);
    const ciOnly = disp({ showPrBadge: false, showCiStatus: true, prByBranch, ciBySha });
    expect(branchSignals(branch('feat'), NODE, ciOnly).pr).toBeNull();
    expect(branchSignals(branch('feat'), NODE, ciOnly).ci?.rollup).toBe('failure');
  });

  it('null when the branch/sha is absent from the maps', () => {
    const d = disp({ showPrBadge: true, showCiStatus: true, prByBranch, ciBySha });
    expect(branchSignals(branch('other'), NODE, d).pr).toBeNull();
    const otherNode = { lane: 0, id: 'no-such-sha' } as unknown as GraphNode;
    expect(branchSignals(branch('feat'), otherNode, d).ci).toBeNull();
  });
});

describe('prBadgeWidth', () => {
  it('= 2*padX + measure("#num") when under the cap', () => {
    // "#7" == 2 chars * 10 = 20; + 2*5 = 30 (< 46).
    expect(prBadgeWidth(makeCtx(), pr({ number: 7 }))).toBe(2 * METRICS.prBadgePadX + 2 * CHAR_W);
  });
  it('is clamped to prBadgeMaxWidth for a long number', () => {
    // "#1234567" == 8 chars * 10 = 80; + 10 = 90, clamped to 46.
    expect(prBadgeWidth(makeCtx(), pr({ number: 1234567 }))).toBe(METRICS.prBadgeMaxWidth);
  });
});
