import { describe, expect, it, vi } from 'vitest';

import { measure, truncateToWidth } from './textMeasure';

// The module keeps ONE global cache keyed by `${ctx.font} ${text}` — each test
// uses a unique font string so cached entries never leak between tests.
// textMeasure is "pure over the passed 2D context", so a 10px-per-char stub
// stands in for a real canvas (no DOM needed in the node project).
let fontSeq = 0;
function stubCtx(pxPerChar = 10) {
  const measureText = vi.fn((text: string) => ({ width: text.length * pxPerChar }));
  const ctx = { font: `stub-font-${fontSeq++}`, measureText } as unknown as CanvasRenderingContext2D & {
    measureText: ReturnType<typeof vi.fn>;
  };
  return { ctx, measureText };
}

describe('measure', () => {
  it('returns measureText width and caches per (font, text)', () => {
    const { ctx, measureText } = stubCtx();
    expect(measure(ctx, 'abc')).toBe(30);
    expect(measure(ctx, 'abc')).toBe(30);
    expect(measureText).toHaveBeenCalledTimes(1);
  });

  it('different texts are separate cache entries', () => {
    const { ctx, measureText } = stubCtx();
    measure(ctx, 'a');
    measure(ctx, 'bb');
    expect(measureText).toHaveBeenCalledTimes(2);
  });

  it('changing ctx.font invalidates the key (font is part of the cache key)', () => {
    const { ctx, measureText } = stubCtx();
    measure(ctx, 'same');
    (ctx as { font: string }).font = `stub-font-${fontSeq++}`;
    measure(ctx, 'same');
    expect(measureText).toHaveBeenCalledTimes(2);
  });

  it('empty string measures 0 and is cacheable', () => {
    const { ctx, measureText } = stubCtx();
    expect(measure(ctx, '')).toBe(0);
    expect(measure(ctx, '')).toBe(0);
    expect(measureText).toHaveBeenCalledTimes(1);
  });

  it('cache overflow (>4096 entries) drops all and re-measures', () => {
    const { ctx, measureText } = stubCtx();
    measure(ctx, 'first');
    for (let i = 0; i < 4100; i++) measure(ctx, `filler-${i}`);
    measureText.mockClear();
    measure(ctx, 'first'); // evicted by the drop-all → re-measured
    expect(measureText).toHaveBeenCalledTimes(1);
  });
});

describe('truncateToWidth', () => {
  it('maxPx <= 0 → empty string (even for empty text)', () => {
    const { ctx } = stubCtx();
    expect(truncateToWidth(ctx, 'hello', 0)).toBe('');
    expect(truncateToWidth(ctx, 'hello', -5)).toBe('');
    expect(truncateToWidth(ctx, '', 0)).toBe('');
  });

  it('text that fits is returned unchanged (boundary: exactly equal width)', () => {
    const { ctx } = stubCtx();
    expect(truncateToWidth(ctx, 'abcd', 40)).toBe('abcd'); // 4×10 == 40
    expect(truncateToWidth(ctx, 'ab', 100)).toBe('ab');
  });

  it('truncates to the longest prefix + ellipsis that fits', () => {
    const { ctx } = stubCtx();
    // 'abcdef' = 60px; budget 35px fits 'ab…' (30px) but not 'abc…' (40px).
    expect(truncateToWidth(ctx, 'abcdef', 35)).toBe('ab…');
  });

  it('budget too small for even one char + ellipsis → empty string', () => {
    const { ctx } = stubCtx();
    expect(truncateToWidth(ctx, 'abcdef', 10)).toBe(''); // 'a…' is 20px
  });

  it('budget exactly fitting prefix+ellipsis is accepted (<=)', () => {
    const { ctx } = stubCtx();
    expect(truncateToWidth(ctx, 'abcdef', 40)).toBe('abc…'); // 4 chars × 10
  });

  it('single-char text wider than budget → empty', () => {
    const { ctx } = stubCtx(100);
    expect(truncateToWidth(ctx, 'x', 50)).toBe('');
  });

  it('long text (10k chars) truncates without excessive measuring (binary search)', () => {
    const { ctx, measureText } = stubCtx();
    const text = 'y'.repeat(10_000);
    const out = truncateToWidth(ctx, text, 500);
    expect(out).toBe('y'.repeat(49) + '…'); // 50 chars × 10px == 500
    // ~log2(10k) probes + the initial full measure — far fewer than 10k.
    expect(measureText.mock.calls.length).toBeLessThan(40);
  });
});
