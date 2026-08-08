import { describe, expect, it } from 'vitest';

import { segmentLine } from './intralineSegments';

describe('segmentLine', () => {
  it('returns the whole line as one unchanged segment when spans are absent', () => {
    expect(segmentLine('const x = 1;')).toEqual([{ text: 'const x = 1;', changed: false }]);
  });

  it('returns the whole line unchanged for an empty spans array', () => {
    expect(segmentLine('abc', [])).toEqual([{ text: 'abc', changed: false }]);
    expect(segmentLine('', [])).toEqual([{ text: '', changed: false }]);
  });

  it('splits a single interior span into plain / changed / plain', () => {
    // `const x = 1;` : emphasis only on `1` at code-point index 10.
    expect(segmentLine('const x = 1;', [[10, 1]])).toEqual([
      { text: 'const x = ', changed: false },
      { text: '1', changed: true },
      { text: ';', changed: false },
    ]);
  });

  it('handles a span at the very start (no leading plain segment)', () => {
    expect(segmentLine('foobar', [[0, 3]])).toEqual([
      { text: 'foo', changed: true },
      { text: 'bar', changed: false },
    ]);
  });

  it('handles a span reaching the end (no trailing plain segment)', () => {
    expect(segmentLine('foobar', [[3, 3]])).toEqual([
      { text: 'foo', changed: false },
      { text: 'bar', changed: true },
    ]);
  });

  it('emits multiple non-adjacent spans as separate emphasis runs', () => {
    // "a b c" -> emphasize "a" (0) and "c" (4).
    expect(segmentLine('a b c', [[0, 1], [4, 1]])).toEqual([
      { text: 'a', changed: true },
      { text: ' b ', changed: false },
      { text: 'c', changed: true },
    ]);
  });

  it('slices by CODE POINT for multibyte content (accents + emoji)', () => {
    // "café 1" -> the digit is at code-point index 5, though `é` is 2 UTF-8
    // bytes; Array.from keeps it one unit. Plain-string slicing would mis-place.
    expect(segmentLine('café 1', [[5, 1]])).toEqual([
      { text: 'café ', changed: false },
      { text: '1', changed: true },
    ]);
    // Emoji occupies one code point at index 0.
    expect(segmentLine('👍 ok', [[2, 2]])).toEqual([
      { text: '👍 ', changed: false },
      { text: 'ok', changed: true },
    ]);
  });

  it('clamps out-of-range / overlapping spans without throwing', () => {
    // len past the end is clamped to the content length.
    expect(segmentLine('abc', [[1, 99]])).toEqual([
      { text: 'a', changed: false },
      { text: 'bc', changed: true },
    ]);
    // A descending/overlapping second span is skipped (cursor already past it).
    expect(segmentLine('abcd', [[1, 2], [0, 1]])).toEqual([
      { text: 'a', changed: false },
      { text: 'bc', changed: true },
      { text: 'd', changed: false },
    ]);
  });
});
