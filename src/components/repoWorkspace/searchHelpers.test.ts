import { describe, expect, it } from 'vitest';
import { queryKey } from './searchHelpers';
import type { SearchQuery } from '../../ipc';

const base: SearchQuery = {
  text: '',
  field: 'message',
  regex: false,
  caseSensitive: false,
  maxResults: 0,
  scopeRef: null,
};

describe('queryKey (audit fix: NUL-joined, collision-free identity)', () => {
  it('is stable for an identical query', () => {
    const q: SearchQuery = { ...base, text: 'fix bug', scopeRef: 'main' };
    expect(queryKey(q)).toBe(queryKey({ ...q }));
  });

  it('differs when any field differs', () => {
    const q = { ...base, text: 'a' };
    expect(queryKey(q)).not.toBe(queryKey({ ...q, regex: true }));
    expect(queryKey(q)).not.toBe(queryKey({ ...q, caseSensitive: true }));
    expect(queryKey(q)).not.toBe(queryKey({ ...q, field: 'author' as const }));
    expect(queryKey(q)).not.toBe(queryKey({ ...q, scopeRef: 'main' }));
    expect(queryKey(q)).not.toBe(queryKey({ ...q, maxResults: 100 }));
  });

  it('does not collide when text contains the visual separator characters', () => {
    // A space-joined key would collide here; the '\0' join must not.
    const a = queryKey({ ...base, text: 'foo bar', scopeRef: '' });
    const b = queryKey({ ...base, text: 'foo', scopeRef: 'bar' });
    expect(a).not.toBe(b);
    // text ending in the boolean flag digits must not bleed into neighbors
    const c = queryKey({ ...base, text: 'x 1', regex: false });
    const d = queryKey({ ...base, text: 'x', regex: true });
    expect(c).not.toBe(d);
  });
});
