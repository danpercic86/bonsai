import { describe, expect, it } from 'vitest';

import type { DiffLine, Hunk } from '../ipc';
import { pairSplitRows } from './splitRows';

// --- builders (identity matters: each call returns a fresh object we hold onto) ---
const ctx = (content: string, oldNo: number, newNo: number): DiffLine => ({
  kind: 'context',
  oldNo,
  newNo,
  content,
});
const del = (content: string, oldNo: number): DiffLine => ({
  kind: 'del',
  oldNo,
  newNo: null,
  content,
});
const add = (content: string, newNo: number): DiffLine => ({
  kind: 'add',
  oldNo: null,
  newNo,
  content,
});
const hunk = (lines: DiffLine[]): Hunk => ({
  oldStart: 1,
  oldLines: lines.filter((l) => l.kind !== 'add').length,
  newStart: 1,
  newLines: lines.filter((l) => l.kind !== 'del').length,
  lines,
});

describe('pairSplitRows', () => {
  it('1. empty hunk → []', () => {
    expect(pairSplitRows(hunk([]))).toEqual([]);
  });

  it('2. context passthrough — each row has left === right === source line', () => {
    const c0 = ctx('a', 1, 1);
    const c1 = ctx('b', 2, 2);
    const c2 = ctx('c', 3, 3);
    const h = hunk([c0, c1, c2]);
    const rows = pairSplitRows(h);
    expect(rows).toHaveLength(3);
    for (const [i, src] of [c0, c1, c2].entries()) {
      expect(rows[i].left).toBe(src);
      expect(rows[i].right).toBe(src);
      expect(rows[i].left).toBe(rows[i].right);
    }
  });

  it('3. even del/add run — 2 del + 2 add → 2 paired rows (identity per cell)', () => {
    const d0 = del('x0', 1);
    const d1 = del('x1', 2);
    const a0 = add('y0', 1);
    const a1 = add('y1', 2);
    const rows = pairSplitRows(hunk([d0, d1, a0, a1]));
    expect(rows).toHaveLength(2);
    expect(rows[0]).toMatchObject({ left: { kind: 'del' }, right: { kind: 'add' } });
    expect(rows[0].left).toBe(d0);
    expect(rows[0].right).toBe(a0);
    expect(rows[1].left).toBe(d1);
    expect(rows[1].right).toBe(a1);
  });

  it('4. surplus dels — 3 del + 1 add → {d0,a0},{d1,null},{d2,null}', () => {
    const d0 = del('x0', 1);
    const d1 = del('x1', 2);
    const d2 = del('x2', 3);
    const a0 = add('y0', 1);
    const rows = pairSplitRows(hunk([d0, d1, d2, a0]));
    expect(rows).toHaveLength(3);
    expect(rows[0].left).toBe(d0);
    expect(rows[0].right).toBe(a0);
    expect(rows[1].left).toBe(d1);
    expect(rows[1].right).toBeNull();
    expect(rows[2].left).toBe(d2);
    expect(rows[2].right).toBeNull();
  });

  it('5. surplus adds — 1 del + 3 add → {d0,a0},{null,a1},{null,a2}', () => {
    const d0 = del('x0', 1);
    const a0 = add('y0', 1);
    const a1 = add('y1', 2);
    const a2 = add('y2', 3);
    const rows = pairSplitRows(hunk([d0, a0, a1, a2]));
    expect(rows).toHaveLength(3);
    expect(rows[0].left).toBe(d0);
    expect(rows[0].right).toBe(a0);
    expect(rows[1].left).toBeNull();
    expect(rows[1].right).toBe(a1);
    expect(rows[2].left).toBeNull();
    expect(rows[2].right).toBe(a2);
  });

  it('6. pure deletions — N del, 0 add → N rows {del[i], null}', () => {
    const dels = [del('x0', 1), del('x1', 2), del('x2', 3), del('x3', 4)];
    const rows = pairSplitRows(hunk([...dels]));
    expect(rows).toHaveLength(dels.length);
    dels.forEach((d, i) => {
      expect(rows[i].left).toBe(d);
      expect(rows[i].right).toBeNull();
      expect(rows[i].left?.kind).toBe('del');
    });
  });

  it('7. pure additions — 0 del, N add → N rows {null, add[i]}', () => {
    const adds = [add('y0', 1), add('y1', 2), add('y2', 3)];
    const rows = pairSplitRows(hunk([...adds]));
    expect(rows).toHaveLength(adds.length);
    adds.forEach((a, i) => {
      expect(rows[i].left).toBeNull();
      expect(rows[i].right).toBe(a);
      expect(rows[i].right?.kind).toBe('add');
    });
  });

  it('8. flush at context boundary — [ctx,del,add,ctx] → [{ctx,ctx},{del,add},{ctx,ctx}]', () => {
    const c0 = ctx('a', 1, 1);
    const d0 = del('x', 2);
    const a0 = add('y', 2);
    const c1 = ctx('b', 3, 3);
    const rows = pairSplitRows(hunk([c0, d0, a0, c1]));
    expect(rows).toHaveLength(3);
    // context rows are NOT merged into the del/add run
    expect(rows[0].left).toBe(c0);
    expect(rows[0].right).toBe(c0);
    expect(rows[1].left).toBe(d0);
    expect(rows[1].right).toBe(a0);
    expect(rows[2].left).toBe(c1);
    expect(rows[2].right).toBe(c1);
    expect(rows.map((r) => [r.left?.kind, r.right?.kind])).toEqual([
      ['context', 'context'],
      ['del', 'add'],
      ['context', 'context'],
    ]);
  });

  it('9. two separated runs — [del,add,ctx,del,add] → runs flushed independently (3 rows)', () => {
    const d0 = del('x0', 1);
    const a0 = add('y0', 1);
    const c = ctx('m', 2, 2);
    const d1 = del('x1', 3);
    const a1 = add('y1', 3);
    const rows = pairSplitRows(hunk([d0, a0, c, d1, a1]));
    expect(rows).toHaveLength(3);
    expect(rows[0].left).toBe(d0);
    expect(rows[0].right).toBe(a0);
    expect(rows[1].left).toBe(c);
    expect(rows[1].right).toBe(c);
    expect(rows[2].left).toBe(d1);
    expect(rows[2].right).toBe(a1);
  });

  it('10. identity for global lookup — every non-null cell === the exact hunk.lines[*] object', () => {
    const c0 = ctx('a', 1, 1);
    const d0 = del('x0', 2);
    const d1 = del('x1', 3);
    const a0 = add('y0', 2);
    const c1 = ctx('b', 4, 4);
    const a1 = add('y1', 5);
    const h = hunk([c0, d0, d1, a0, c1, a1]);
    const rows = pairSplitRows(h);
    const bySource = new Set<DiffLine>(h.lines);
    for (const row of rows) {
      if (row.left) expect(bySource.has(row.left)).toBe(true);
      if (row.right) expect(bySource.has(row.right)).toBe(true);
      // both-null never occurs
      expect(row.left === null && row.right === null).toBe(false);
    }
    // and every source line is reachable by identity from some cell
    const reached = new Set<DiffLine>();
    for (const row of rows) {
      if (row.left) reached.add(row.left);
      if (row.right) reached.add(row.right);
    }
    for (const src of h.lines) expect(reached.has(src)).toBe(true);
  });
});
