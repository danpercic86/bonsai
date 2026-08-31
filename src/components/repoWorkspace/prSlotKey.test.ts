/** P93 AC3: the `pr:<baseOid>:<headOid>:<path>` slot key must round-trip a path
 *  that itself contains a colon — the parser drops exactly the two oid segments
 *  and takes the remainder verbatim, never a naive split. */
import { describe, expect, it } from 'vitest';
import { isPrSlotKey, parsePrSlotPath, prSlotKey } from './prSlotKey';

const BASE = 'a'.repeat(40);
const HEAD = 'b'.repeat(40);

describe('prSlotKey', () => {
  it('builds and parses a plain path', () => {
    const key = prSlotKey(BASE, HEAD, 'src/app/main.rs');
    expect(key).toBe(`pr:${BASE}:${HEAD}:src/app/main.rs`);
    expect(isPrSlotKey(key)).toBe(true);
    expect(parsePrSlotPath(key)).toBe('src/app/main.rs');
  });

  it('round-trips a path containing colons', () => {
    const path = 'docs/notes:draft/a:b.md';
    const key = prSlotKey(BASE, HEAD, path);
    // Both directions: the key is built verbatim (no escaping) and parsed back
    // whole — the parser drops exactly the two oid segments, nothing else.
    expect(key).toBe(`pr:${BASE}:${HEAD}:${path}`);
    expect(parsePrSlotPath(key)).toBe(path);
  });

  it('keeps a leading / trailing colon of the path verbatim', () => {
    for (const path of [':odd/name.ts', 'weird/name.ts:', '::', 'a:b:c:d:e']) {
      expect(parsePrSlotPath(prSlotKey(BASE, HEAD, path))).toBe(path);
    }
  });

  it('parses an empty path as the empty string, not null', () => {
    // Not reachable from the UI (a header always has a path); pinned so the
    // "drop two segments, take the remainder" rule stays unambiguous.
    expect(parsePrSlotPath(prSlotKey(BASE, HEAD, ''))).toBe('');
  });

  it('rejects non-pr and malformed keys', () => {
    expect(isPrSlotKey('unstaged:src/a.ts')).toBe(false);
    expect(parsePrSlotPath('unstaged:src/a.ts')).toBeNull();
    expect(parsePrSlotPath(`pr:${BASE}`)).toBeNull();
    expect(parsePrSlotPath(`pr:${BASE}:${HEAD}`)).toBeNull();
  });
});
