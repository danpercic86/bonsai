import { describe, expect, it } from 'vitest';
import {
  buildFoldModel,
  displayToModel,
  modelToDisplay,
  pillRowOfStart,
  spanAt,
  spanContaining,
} from './foldModel';
import type { FoldSpan } from '../ipc';

const span = (start: number, count: number, lane = 0): FoldSpan => ({ start, count, lane });

describe('buildFoldModel', () => {
  it('identity when no spans / all expanded', () => {
    const none = buildFoldModel(10, [], new Set());
    expect(none.identity).toBe(true);
    expect(none.displayRowCount).toBe(10);
    const allExpanded = buildFoldModel(10, [span(2, 5)], new Set([2]));
    expect(allExpanded.identity).toBe(true);
    expect(modelToDisplay(allExpanded, 7)).toBe(7);
  });

  it('drops spans that fall outside totalRows (stale metadata)', () => {
    const m = buildFoldModel(6, [span(2, 5)], new Set());
    expect(m.identity).toBe(true);
  });

  it('shrinks by count-1 per collapsed span and sorts spans', () => {
    const m = buildFoldModel(30, [span(20, 6), span(2, 5)], new Set());
    expect(m.displayRowCount).toBe(30 - 4 - 5);
    expect(m.collapsed.map((s) => s.start)).toEqual([2, 20]);
  });
});

describe('display↔model mapping', () => {
  // rows 0,1 visible; 2..6 hidden (pill at display 2); 7.. visible.
  const m = buildFoldModel(12, [span(2, 5)], new Set());

  it('displayToModel', () => {
    expect(displayToModel(m, 0)).toEqual({ kind: 'commit', row: 0 });
    expect(displayToModel(m, 2)).toEqual({ kind: 'fold', span: span(2, 5) });
    expect(displayToModel(m, 3)).toEqual({ kind: 'commit', row: 7 });
    expect(displayToModel(m, 7)).toEqual({ kind: 'commit', row: 11 });
  });

  it('modelToDisplay round-trips visible rows and anchors hidden rows at the pill', () => {
    for (const r of [0, 1, 7, 8, 11]) {
      const d = modelToDisplay(m, r);
      expect(displayToModel(m, d)).toEqual({ kind: 'commit', row: r });
    }
    expect(modelToDisplay(m, 2)).toBe(2);
    expect(modelToDisplay(m, 6)).toBe(2); // hidden → pill row (§6)
  });

  it('two spans: prefix shrink accumulates', () => {
    const m2 = buildFoldModel(30, [span(2, 5), span(10, 6)], new Set());
    // pill 1 at display 2; rows 7,8,9 → 3,4,5; pill 2 at display 6; row 16 → 7.
    expect(modelToDisplay(m2, 9)).toBe(5);
    expect(displayToModel(m2, 6)).toEqual({ kind: 'fold', span: span(10, 6) });
    expect(modelToDisplay(m2, 16)).toBe(7);
    expect(displayToModel(m2, 7)).toEqual({ kind: 'commit', row: 16 });
    expect(m2.displayRowCount).toBe(30 - 4 - 5);
  });

  it('spanContaining / pillRowOfStart / spanAt', () => {
    expect(spanContaining(m, 1)).toBeNull();
    expect(spanContaining(m, 2)).toEqual(span(2, 5));
    expect(spanContaining(m, 6)).toEqual(span(2, 5));
    expect(spanContaining(m, 7)).toBeNull();
    expect(pillRowOfStart(m, 2)).toBe(2);
    expect(pillRowOfStart(m, 3)).toBeNull();
    expect(spanAt([span(2, 5)], 4)).toEqual(span(2, 5));
    expect(spanAt([span(2, 5)], 7)).toBeNull();
  });

  it('expansion set removes a span from the mapping (expand-in-place)', () => {
    const collapsed = buildFoldModel(12, [span(2, 5)], new Set());
    const expanded = buildFoldModel(12, [span(2, 5)], new Set([2]));
    expect(collapsed.displayRowCount).toBe(8);
    expect(expanded.displayRowCount).toBe(12);
    expect(modelToDisplay(expanded, 6)).toBe(6);
  });
});
