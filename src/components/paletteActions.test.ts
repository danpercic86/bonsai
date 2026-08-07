import { describe, expect, it } from 'vitest';

import { filterActions, fuzzyScore, type PaletteAction } from './paletteActions';

const act = (title: string, keywords?: string): PaletteAction => ({
  id: title,
  title,
  group: 'action',
  keywords,
  run: () => {},
});

describe('fuzzyScore', () => {
  it('empty query is neutral (matches everything)', () => {
    expect(fuzzyScore('', 'anything')).toBe(0);
    expect(fuzzyScore('   ', 'anything')).toBe(0);
  });

  it('non-subsequence returns -1', () => {
    expect(fuzzyScore('zzz', 'Fetch')).toBe(-1);
    expect(fuzzyScore('fetchx', 'Fetch')).toBe(-1);
  });

  it('is case-insensitive', () => {
    expect(fuzzyScore('FETCH', 'fetch')).toBeGreaterThanOrEqual(0);
    expect(fuzzyScore('fetch', 'FETCH')).toBeGreaterThanOrEqual(0);
  });

  it('matches a non-contiguous subsequence', () => {
    // f..t..h in "Fetch" (f-e-t-c-h): all present in order.
    expect(fuzzyScore('fth', 'Fetch')).toBeGreaterThanOrEqual(0);
  });

  it('prefers a contiguous run over a scattered match', () => {
    // "push" contiguous in "Push" scores higher than scattered p..u..s..h.
    expect(fuzzyScore('push', 'Push')).toBeGreaterThan(fuzzyScore('push', 'Publish shell'));
  });

  it('rewards a start-of-string match', () => {
    expect(fuzzyScore('re', 'Refresh')).toBeGreaterThan(fuzzyScore('re', 'Compare'));
  });

  it('rewards a word-boundary match (after / - _ . space)', () => {
    // "m" after the "/" boundary in "origin/main" beats a mid-word "m".
    expect(fuzzyScore('m', 'origin/main')).toBeGreaterThan(fuzzyScore('m', 'commit'));
  });
});

describe('filterActions', () => {
  it('empty query preserves order (identity)', () => {
    const list = [act('Fetch'), act('Pull'), act('Push')];
    expect(filterActions(list, '')).toEqual(list);
  });

  it('drops non-matches and ranks matches by score', () => {
    const list = [act('Publish shell'), act('Compare'), act('Push')];
    const res = filterActions(list, 'push');
    // "Compare" has no p-u-s-h subsequence → dropped; the contiguous "Push"
    // outranks the scattered match in "Publish shell".
    expect(res.map((a) => a.title)).toEqual(['Push', 'Publish shell']);
  });

  it('matches against keywords, not just the title', () => {
    const list = [act('Fetch', 'remote sync download'), act('Refresh', 'reload rescan')];
    const res = filterActions(list, 'download');
    expect(res.map((a) => a.title)).toEqual(['Fetch']);
  });

  it('ties keep source order (stable)', () => {
    const list = [act('Push'), act('Pull')];
    // both start with "pu"; equal score → original order retained.
    expect(filterActions(list, 'pu').map((a) => a.title)).toEqual(['Push', 'Pull']);
  });
});
