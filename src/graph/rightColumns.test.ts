import { describe, expect, it } from 'vitest';

import { effectiveMetrics } from './metrics';
import { computeRightColumns } from './rightColumns';
import type { GraphDisplayOptions } from './rightColumns';

// Real (comfortable-mode) metrics so the column widths under test match what
// the draw pass uses: colGap 12, dateColWidth 72, shaColWidth 54,
// badgeSlotWidth 14, badgeGap 4, authorColWidth 120.
const M = effectiveMetrics({ avatarRadius: 10, rowHeight: 32, laneWidth: 16, compact: false });
const SHA_W = M.badgeSlotWidth + M.badgeGap + M.shaColWidth;
const EFF_RIGHT = 1000;

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

describe('computeRightColumns', () => {
  it('all columns off: everything null, summary flexes to effRight - colGap', () => {
    const c = computeRightColumns(EFF_RIGHT, disp(), M);
    expect(c.author).toBeNull();
    expect(c.sha).toBeNull();
    expect(c.date).toBeNull();
    expect(c.summaryEndX).toBe(EFF_RIGHT - M.colGap);
  });

  it('packs author, SHA, date right→left in fixed order separated by colGaps', () => {
    const c = computeRightColumns(EFF_RIGHT, disp({ showAuthor: true, showSha: true, showDate: true }), M);
    // date is rightmost.
    expect(c.date).toEqual({
      leftX: EFF_RIGHT - M.colGap - M.dateColWidth,
      rightX: EFF_RIGHT - M.colGap,
      width: M.dateColWidth,
    });
    // sha sits one colGap left of the date column.
    const shaRight = c.date!.leftX - M.colGap;
    expect(c.sha).toEqual({ leftX: shaRight - SHA_W, rightX: shaRight, width: SHA_W });
    // author is leftmost, one colGap left of the sha column.
    const authorRight = c.sha!.leftX - M.colGap;
    expect(c.author).toEqual({ leftX: authorRight - M.authorColWidth, rightX: authorRight, width: M.authorColWidth });
    // summary ends one colGap left of the author column.
    expect(c.summaryEndX).toBe(c.author!.leftX - M.colGap);
  });

  it('every ColRect satisfies rightX - leftX === width', () => {
    const c = computeRightColumns(EFF_RIGHT, disp({ showAuthor: true, showSha: true, showDate: true }), M);
    for (const col of [c.author, c.sha, c.date]) {
      expect(col).not.toBeNull();
      expect(col!.rightX - col!.leftX).toBe(col!.width);
    }
  });

  it('SHA column width includes the badge slot + gap', () => {
    const c = computeRightColumns(EFF_RIGHT, disp({ showSha: true }), M);
    expect(c.sha?.width).toBe(M.badgeSlotWidth + M.badgeGap + M.shaColWidth);
  });

  it('default toggles (SHA + date, no author): author null, others packed', () => {
    const c = computeRightColumns(EFF_RIGHT, disp({ showSha: true, showDate: true }), M);
    expect(c.author).toBeNull();
    expect(c.date?.rightX).toBe(EFF_RIGHT - M.colGap);
    expect(c.sha?.rightX).toBe(c.date!.leftX - M.colGap);
    expect(c.summaryEndX).toBe(c.sha!.leftX - M.colGap);
  });

  it('a disabled column reserves NO space (summary reclaims width + one gap)', () => {
    const withSha = computeRightColumns(EFF_RIGHT, disp({ showDate: true, showSha: true }), M);
    const noSha = computeRightColumns(EFF_RIGHT, disp({ showDate: true, showSha: false }), M);
    // The rightmost (date) column is identical either way.
    expect(noSha.date).toEqual(withSha.date);
    // Dropping SHA hands the summary back exactly SHA width + one colGap.
    expect(noSha.summaryEndX - withSha.summaryEndX).toBe(SHA_W + M.colGap);
  });

  it('date-only case reproduces the pre-P51 summary/date geometry', () => {
    const c = computeRightColumns(EFF_RIGHT, disp({ showDate: true }), M);
    const dateRight = EFF_RIGHT - M.colGap;
    const dateLeft = dateRight - M.dateColWidth;
    expect(c.date).toEqual({ leftX: dateLeft, rightX: dateRight, width: M.dateColWidth });
    // Old summaryMax === dateLeft - colGap - summaryStartX === summaryEndX - summaryStartX.
    expect(c.summaryEndX).toBe(dateLeft - M.colGap);
  });

  it('compact metrics still produce a valid packing (denser SHA width)', () => {
    const compact = effectiveMetrics({ avatarRadius: 10, rowHeight: 32, laneWidth: 16, compact: true });
    const c = computeRightColumns(EFF_RIGHT, disp({ showSha: true, showDate: true }), compact);
    expect(c.sha?.width).toBe(compact.badgeSlotWidth + compact.badgeGap + compact.shaColWidth);
    expect(c.date?.rightX).toBe(EFF_RIGHT - compact.colGap);
  });

  // PR-badge-placement §2.2: the forge column.
  const PR = { number: 7, title: 't', state: 'open' as const, isDraft: false, url: 'u' };
  const CI = { rollup: 'success' as const, passed: 1, failed: 0, pending: 0, total: 1 };

  it('forge is null when the toggles are off, even with data present', () => {
    const c = computeRightColumns(
      EFF_RIGHT,
      disp({ showPrBadge: false, showCiStatus: false, prByBranch: new Map([['f', PR]]), ciBySha: new Map([['s', CI]]) }),
      M,
    );
    expect(c.forge).toBeNull();
  });

  it('forge is null when enabled but the maps are empty (width handed back)', () => {
    const c = computeRightColumns(EFF_RIGHT, disp({ showPrBadge: true, showCiStatus: true }), M);
    expect(c.forge).toBeNull();
  });

  it('forge reserved when PR data is present + enabled; leftmost of the pack', () => {
    const c = computeRightColumns(
      EFF_RIGHT,
      disp({ showDate: true, showPrBadge: true, prByBranch: new Map([['f', PR]]) }),
      M,
    );
    expect(c.forge?.width).toBe(M.forgeColWidth);
    // forge sits one colGap left of the date column (leftmost placed = leftmost).
    expect(c.forge?.rightX).toBe(c.date!.leftX - M.colGap);
    // summary ends one colGap left of the forge column.
    expect(c.summaryEndX).toBe(c.forge!.leftX - M.colGap);
  });

  it('CI-only (no PR) still reserves the forge column', () => {
    const c = computeRightColumns(EFF_RIGHT, disp({ showCiStatus: true, ciBySha: new Map([['s', CI]]) }), M);
    expect(c.forge?.width).toBe(M.forgeColWidth);
  });

  it('reserving forge hands the summary back exactly forgeColWidth + one colGap', () => {
    const withForge = computeRightColumns(EFF_RIGHT, disp({ showPrBadge: true, prByBranch: new Map([['f', PR]]) }), M);
    const noForge = computeRightColumns(EFF_RIGHT, disp(), M);
    expect(noForge.summaryEndX - withForge.summaryEndX).toBe(M.forgeColWidth + M.colGap);
  });
});
