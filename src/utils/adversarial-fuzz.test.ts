/** T5b §5 pure-fn fuzz — pairSplitRows / conflictRegions / buildPathTree fed
 *  seeded random garbage (control chars, lone surrogates, marker fragments,
 *  huge inputs). Invariants: never throw; output shape well-formed. Seeded
 *  deterministic loops per the T5 contract (NO fast-check). */
import { describe, expect, it } from 'vitest';
import type { DiffLine, Hunk } from '../ipc';
import { pairSplitRows } from './splitRows';
import {
  applyResolution,
  hasUnresolvedMarkers,
  parseConflictRegions,
} from './conflictRegions';
import { buildPathTree, flattenTreeLeaves } from './pathTree';
import type { TreeNode } from './pathTree';

function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const GARBAGE = ['', '', '\uD800', '😀', '‮x', 'plain text', '\t', '   ', '中文', 'a'.repeat(300)];

function pick<T>(rand: () => number, arr: readonly T[]): T {
  return arr[Math.floor(rand() * arr.length)];
}

// ---------------------------------------------------------------------------
// pairSplitRows
// ---------------------------------------------------------------------------

describe('pairSplitRows fuzz (seeded)', () => {
  const KINDS = ['context', 'add', 'del', 'garbage-kind'] as const;

  function randomHunk(rand: () => number): Hunk {
    const count = Math.floor(rand() * 40);
    const lines: DiffLine[] = [];
    for (let i = 0; i < count; i++) {
      lines.push({
        kind: pick(rand, KINDS) as DiffLine['kind'], // hostile: unknown kind string
        oldNo: rand() < 0.5 ? i : null,
        newNo: rand() < 0.5 ? i : null,
        content: pick(rand, GARBAGE),
      });
    }
    return { oldStart: -1, oldLines: NaN, newStart: 1e9, newLines: 0, lines };
  }

  it('150 seeded iterations: no throw, no both-null row, every line placed exactly once', () => {
    const rand = mulberry32(0x51ee7);
    for (let iter = 0; iter < 150; iter++) {
      const hunk = randomHunk(rand);
      let rows: ReturnType<typeof pairSplitRows>;
      expect(() => {
        rows = pairSplitRows(hunk);
      }, `iter ${iter} threw`).not.toThrow();

      // No both-null filler row.
      for (const row of rows!) {
        expect(row.left !== null || row.right !== null, `iter ${iter}: both-null row`).toBe(true);
      }

      // Placement: del -> exactly once as a LEFT cell; add -> exactly once as a
      // RIGHT cell; anything else (context + unknown kinds) -> exactly one row
      // holding the SAME object in both cells.
      for (const line of hunk.lines) {
        const leftHits = rows!.filter((r) => r.left === line).length;
        const rightHits = rows!.filter((r) => r.right === line).length;
        if (line.kind === 'del') {
          expect([leftHits, rightHits], `iter ${iter}: del placement`).toEqual([1, 0]);
        } else if (line.kind === 'add') {
          expect([leftHits, rightHits], `iter ${iter}: add placement`).toEqual([0, 1]);
        } else {
          expect([leftHits, rightHits], `iter ${iter}: context placement`).toEqual([1, 1]);
          expect(rows!.some((r) => r.left === line && r.right === line)).toBe(true);
        }
      }
    }
  });

  it('empty hunk returns []', () => {
    expect(pairSplitRows({ oldStart: 0, oldLines: 0, newStart: 0, newLines: 0, lines: [] })).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// conflictRegions
// ---------------------------------------------------------------------------

describe('conflictRegions fuzz (seeded)', () => {
  const LINE_POOL = [
    '<<<<<<< HEAD',
    '<<<<<<<',
    '<<<<<<<<<<', // 10 '<' still matches /^<{7}/
    '=======',
    '======= trailing',
    '>>>>>>> feature/x',
    '>>>>>>>',
    '<<<<<< only-six',
    '=====',
    'normal line',
    '',
    '  indented <<<<<<< (not a marker)',
    '\uD800 lone surrogate',
    '‮rtl override',
    'x'.repeat(500),
  ];

  function randomDoc(rand: () => number, maxLines: number): string {
    const count = Math.floor(rand() * maxLines);
    const lines: string[] = [];
    for (let i = 0; i < count; i++) lines.push(pick(rand, LINE_POOL));
    return lines.join('\n');
  }

  it('150 seeded iterations: parse never throws; regions well-ordered; resolution terminates marker-free', () => {
    const rand = mulberry32(0xc0ff1c7);
    for (let iter = 0; iter < 150; iter++) {
      const doc = randomDoc(rand, 60);

      let regions: ReturnType<typeof parseConflictRegions>;
      expect(() => {
        regions = parseConflictRegions(doc);
      }, `iter ${iter} parse threw`).not.toThrow();
      expect(() => hasUnresolvedMarkers(doc)).not.toThrow();

      // Shape: sequential index, start < sep < end, bodies match slices.
      const lines = doc.split('\n');
      regions!.forEach((r, i) => {
        expect(r.index).toBe(i);
        expect(r.startLine).toBeLessThan(r.sepLine);
        expect(r.sepLine).toBeLessThan(r.endLine);
        expect(r.oursLines).toEqual(lines.slice(r.startLine + 1, r.sepLine));
        expect(r.theirsLines).toEqual(lines.slice(r.sepLine + 1, r.endLine));
      });

      // Repeated resolution: each apply removes exactly one '<<<<<<<' line, so
      // the loop is bounded by the initial start-marker count. Afterwards no
      // parseable region remains.
      const startMarkers = lines.filter((l) => /^<{7}/.test(l)).length;
      let current = doc;
      let steps = 0;
      const choices = ['ours', 'theirs', 'both'] as const;
      for (;;) {
        const rs = parseConflictRegions(current);
        if (rs.length === 0) break;
        expect(steps, `iter ${iter}: resolution loop did not terminate`).toBeLessThan(startMarkers + 1);
        expect(() => {
          current = applyResolution(current, rs[0], pick(rand, choices));
        }, `iter ${iter} apply threw`).not.toThrow();
        steps++;
      }
      expect(parseConflictRegions(current)).toEqual([]);
    }
  });

  it('large doc (20k lines, ~1 MB): parse + full resolution without throwing', () => {
    const block = ['<<<<<<< HEAD', 'ours '.repeat(10), '=======', 'theirs '.repeat(10), '>>>>>>> other'];
    const lines: string[] = [];
    for (let i = 0; i < 20_000 / 6; i++) lines.push(`ctx ${i} ${'pad'.repeat(4)}`, ...block);
    const doc = lines.join('\n');
    const regions = parseConflictRegions(doc);
    expect(regions.length).toBeGreaterThan(3000);
    const resolved = applyResolution(doc, regions[0], 'both');
    expect(parseConflictRegions(resolved).length).toBe(regions.length - 1);
  });
});

// ---------------------------------------------------------------------------
// buildPathTree
// ---------------------------------------------------------------------------

describe('buildPathTree fuzz (seeded)', () => {
  const PATH_POOL = [
    '',
    '/',
    '///',
    'a',
    'a/b',
    'a//b',
    '/leading',
    'trailing/',
    'a/b/c/d/e/f/g',
    '.',
    '..',
    '../escape',
    'dir/../other',
    'ünï/çode/中文.txt',
    '\uD800/lone.txt',
    'nul/x.txt',
    `${'deep/'.repeat(50)}leaf.txt`,
    `${'s'.repeat(10_000)}.txt`,
    'dup/same.txt',
  ];

  interface Item { path: string; tag: number }

  function walk<T>(nodes: readonly TreeNode<T>[], visit: (n: TreeNode<T>) => void): void {
    for (const n of nodes) {
      visit(n);
      if (n.kind === 'dir') walk(n.children, visit);
    }
  }

  it('150 seeded iterations: no throw; leaves == items with >=1 segment; dirs well-formed', () => {
    const rand = mulberry32(0xbadf00d);
    for (let iter = 0; iter < 150; iter++) {
      const count = Math.floor(rand() * 25);
      const items: Item[] = [];
      for (let i = 0; i < count; i++) items.push({ path: pick(rand, PATH_POOL), tag: i });
      const priorityPath = rand() < 0.3 ? pick(rand, PATH_POOL) : undefined;

      let nodes: TreeNode<Item>[];
      expect(() => {
        nodes = buildPathTree(items, (x) => x.path, { priorityPath });
      }, `iter ${iter} threw`).not.toThrow();

      // Every input item with at least one non-empty segment appears exactly
      // once as a leaf (duplicates included); empty-segment paths are skipped.
      const expected = items.filter((x) => x.path.split('/').some((s) => s !== ''));
      const leaves = flattenTreeLeaves(nodes!);
      expect(leaves.length, `iter ${iter}: leaf count`).toBe(expected.length);
      const leafTags = leaves.map((x) => x.tag).sort((a, b) => a - b);
      const expectedTags = expected.map((x) => x.tag).sort((a, b) => a - b);
      expect(leafTags, `iter ${iter}: leaf identity`).toEqual(expectedTags);

      // Structural invariants: leaf.name = last non-empty segment; leaf.path =
      // original path; dirs are non-empty; sibling dir fullPrefixes unique.
      walk(nodes!, (n) => {
        if (n.kind === 'leaf') {
          const segs = n.path.split('/').filter((s) => s !== '');
          expect(n.name).toBe(segs[segs.length - 1]);
          expect(items.some((x) => x.path === n.path)).toBe(true);
        } else {
          expect(n.children.length, `iter ${iter}: empty dir ${n.fullPrefix}`).toBeGreaterThan(0);
        }
      });
      const seen = new Set<string>();
      walk(nodes!, (n) => {
        if (n.kind === 'dir') {
          expect(seen.has(n.fullPrefix), `iter ${iter}: duplicate dir prefix ${n.fullPrefix}`).toBe(false);
          seen.add(n.fullPrefix);
        }
      });
    }
  });

  it('duplicate paths produce duplicate leaves (documented behavior, no throw)', () => {
    const items = [{ path: 'x/a.txt' }, { path: 'x/a.txt' }];
    const leaves = flattenTreeLeaves(buildPathTree(items, (i) => i.path));
    expect(leaves).toHaveLength(2);
  });
});
