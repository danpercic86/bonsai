/** Spec-003 — mock persistence of the graph-declutter prefs: round-trip of
 *  `graphFirstParent` / `graphRefFilter` plus the malformed-shape matrix (every
 *  bad `graphRefFilter` degrades to null; a bad `graphFirstParent` to false).
 *  jsdom (.tsx) for localStorage, mirroring persistence.test.tsx. */
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { readUiSettings, sanitizeGraphRefFilter, writeUiSettings } from './persistence';

const UI_KEY = 'bonsai.mockUiSettings';

beforeEach(() => window.localStorage.clear());
afterEach(() => window.localStorage.clear());

describe('graph declutter prefs (spec-003)', () => {
  it('defaults: missing storage → firstParent false, refFilter null', () => {
    const s = readUiSettings();
    expect(s.graphFirstParent).toBe(false);
    expect(s.graphRefFilter).toBeNull();
  });

  it('round-trips both prefs', () => {
    const s = readUiSettings();
    writeUiSettings({
      ...s,
      graphFirstParent: true,
      graphRefFilter: { mode: 'solo', refs: ['refs/heads/main', 'refs/tags/v1.0'] },
    });
    const back = readUiSettings();
    expect(back.graphFirstParent).toBe(true);
    expect(back.graphRefFilter).toEqual({
      mode: 'solo',
      refs: ['refs/heads/main', 'refs/tags/v1.0'],
    });
  });

  it('malformed graphRefFilter shapes → null (never throw)', () => {
    for (const bad of [
      42,
      'solo',
      { mode: 'nope', refs: [] },
      { mode: 'solo' }, // refs missing
      { mode: 'hide', refs: 'refs/heads/x' }, // refs not an array
      null,
    ]) {
      window.localStorage.setItem(
        UI_KEY,
        JSON.stringify({ graphRefFilter: bad, graphFirstParent: 'yes' }),
      );
      const s = readUiSettings();
      expect(s.graphRefFilter).toBeNull();
      expect(s.graphFirstParent).toBe(false); // non-boolean coerces to default
    }
  });

  it('sanitizeGraphRefFilter drops non-string refs but keeps the rest', () => {
    expect(
      sanitizeGraphRefFilter({ mode: 'hide', refs: ['refs/heads/a', 7, null, 'refs/tags/b'] }),
    ).toEqual({ mode: 'hide', refs: ['refs/heads/a', 'refs/tags/b'] });
  });
});
