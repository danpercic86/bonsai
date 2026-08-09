import { describe, expect, it } from 'vitest';

import { applyResolution, hasUnresolvedMarkers, parseConflictRegions } from './conflictRegions';

const S = '<'.repeat(7);
const M = '='.repeat(7);
const E = '>'.repeat(7);

const doc = (...lines: string[]): string => lines.join('\n');

const ONE = doc('a', `${S} HEAD`, 'ours1', 'ours2', M, 'theirs1', `${E} feature/login`, 'z');

describe('parseConflictRegions', () => {
  it('no markers → []', () => {
    expect(parseConflictRegions(doc('a', 'b', 'c'))).toEqual([]);
  });

  it('empty string → []', () => {
    expect(parseConflictRegions('')).toEqual([]);
  });

  it('one well-formed region — indices, labels, bodies', () => {
    const [r, ...rest] = parseConflictRegions(ONE);
    expect(rest).toEqual([]);
    expect(r).toEqual({
      index: 0,
      startLine: 1,
      sepLine: 4,
      endLine: 6,
      oursLabel: 'HEAD',
      theirsLabel: 'feature/login',
      oursLines: ['ours1', 'ours2'],
      theirsLines: ['theirs1'],
    });
  });

  it('markers without labels → empty labels', () => {
    const [r] = parseConflictRegions(doc(S, 'o', M, 't', E));
    expect(r.oursLabel).toBe('');
    expect(r.theirsLabel).toBe('');
  });

  it('label leading whitespace is trimmed', () => {
    const [r] = parseConflictRegions(doc(`${S}   HEAD`, M, `${E}\t x`));
    expect(r.oursLabel).toBe('HEAD');
    expect(r.theirsLabel).toBe('x');
  });

  it('empty ours/theirs bodies are allowed', () => {
    const [r] = parseConflictRegions(doc(S, M, E));
    expect(r.oursLines).toEqual([]);
    expect(r.theirsLines).toEqual([]);
    expect([r.startLine, r.sepLine, r.endLine]).toEqual([0, 1, 2]);
  });

  it('multiple regions get sequential indices in document order', () => {
    const text = doc(S, M, E, 'x', `${S} b`, 'o', M, 't', `${E} c`);
    const rs = parseConflictRegions(text);
    expect(rs.map((r) => r.index)).toEqual([0, 1]);
    expect(rs[1].startLine).toBe(4);
    expect(rs[1].endLine).toBe(8);
  });

  it('stray ======= / >>>>>>> with no start are ignored', () => {
    expect(parseConflictRegions(doc(M, E, 'body'))).toEqual([]);
  });

  it('start without sep/end (truncated conflict) → no region, no throw', () => {
    expect(parseConflictRegions(doc(`${S} HEAD`, 'ours'))).toEqual([]);
    expect(parseConflictRegions(doc(S, 'ours', M, 'theirs'))).toEqual([]);
  });

  it('a second <<<<<<< before ======= abandons the partial and restarts', () => {
    const text = doc(`${S} stale`, 'x', `${S} fresh`, 'o', M, 't', `${E} th`);
    const rs = parseConflictRegions(text);
    expect(rs).toHaveLength(1);
    expect(rs[0].startLine).toBe(2);
    expect(rs[0].oursLabel).toBe('fresh');
  });

  it('a <<<<<<< between ======= and >>>>>>> abandons and restarts (no nesting)', () => {
    const text = doc(S, 'o', M, 't', `${S} inner`, 'o2', M, 't2', E);
    const rs = parseConflictRegions(text);
    expect(rs).toHaveLength(1);
    expect(rs[0].oursLabel).toBe('inner');
    expect(rs[0].theirsLines).toEqual(['t2']);
  });

  it('fewer than 7 marker chars is body content, not a marker', () => {
    expect(parseConflictRegions(doc('<'.repeat(6), '='.repeat(6), '>'.repeat(6)))).toEqual([]);
  });

  it('marker not at line start is body content', () => {
    expect(parseConflictRegions(doc(` ${S}`, ` ${M}`, ` ${E}`))).toEqual([]);
  });

  it('8+ marker chars still open/close (prefix match)', () => {
    const rs = parseConflictRegions(doc('<'.repeat(9), M, '>'.repeat(9)));
    expect(rs).toHaveLength(1);
  });

  it('huge doc (10k lines) with a region at the end parses fine', () => {
    const filler = Array.from({ length: 10_000 }, (_, i) => `line ${i}`);
    const rs = parseConflictRegions(doc(...filler, S, 'o', M, 't', E));
    expect(rs).toHaveLength(1);
    expect(rs[0].startLine).toBe(10_000);
  });
});

describe('applyResolution', () => {
  const region = parseConflictRegions(ONE)[0];

  it("'ours' keeps only the ours body", () => {
    expect(applyResolution(ONE, region, 'ours')).toBe(doc('a', 'ours1', 'ours2', 'z'));
  });

  it("'theirs' keeps only the theirs body", () => {
    expect(applyResolution(ONE, region, 'theirs')).toBe(doc('a', 'theirs1', 'z'));
  });

  it("'both' keeps ours THEN theirs", () => {
    expect(applyResolution(ONE, region, 'both')).toBe(doc('a', 'ours1', 'ours2', 'theirs1', 'z'));
  });

  it('preserves the trailing newline of the document', () => {
    const text = ONE + '\n';
    const r = parseConflictRegions(text)[0];
    expect(applyResolution(text, r, 'ours')).toBe(doc('a', 'ours1', 'ours2', 'z') + '\n');
  });

  it('region at document start and end (whole doc is the conflict)', () => {
    const text = doc(S, 'o', M, 't', E);
    const r = parseConflictRegions(text)[0];
    expect(applyResolution(text, r, 'theirs')).toBe('t');
  });

  it('resolving with empty chosen body deletes the block cleanly', () => {
    const text = doc('a', S, M, 't', E, 'z');
    const r = parseConflictRegions(text)[0];
    expect(applyResolution(text, r, 'ours')).toBe(doc('a', 'z'));
  });

  it('second region indices stay valid when parsed fresh after resolving the first', () => {
    const text = doc(S, 'o1', M, 't1', E, 'mid', S, 'o2', M, 't2', E);
    const first = parseConflictRegions(text)[0];
    const next = applyResolution(text, first, 'ours');
    const rs = parseConflictRegions(next);
    expect(rs).toHaveLength(1);
    expect(applyResolution(next, rs[0], 'theirs')).toBe(doc('o1', 'mid', 't2'));
  });
});

describe('hasUnresolvedMarkers', () => {
  it('true for each marker kind alone', () => {
    expect(hasUnresolvedMarkers(doc('x', S))).toBe(true);
    expect(hasUnresolvedMarkers(doc('x', M))).toBe(true);
    expect(hasUnresolvedMarkers(doc('x', E))).toBe(true);
  });

  it('false for clean text / empty text / short runs / indented markers', () => {
    expect(hasUnresolvedMarkers('plain text')).toBe(false);
    expect(hasUnresolvedMarkers('')).toBe(false);
    expect(hasUnresolvedMarkers('<'.repeat(6))).toBe(false);
    expect(hasUnresolvedMarkers(`  ${S}`)).toBe(false);
  });

  it('false after resolving the only region', () => {
    const r = parseConflictRegions(ONE)[0];
    expect(hasUnresolvedMarkers(applyResolution(ONE, r, 'both'))).toBe(false);
  });
});
