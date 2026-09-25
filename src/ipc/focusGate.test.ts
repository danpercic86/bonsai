/** P124d — the leading-edge focus gate: WebView2 delivers `onFocusChanged(true)`
 *  twice within ~22–47 ms; each ran a full refresh round. PURE (injected clock). */
import { describe, expect, it } from 'vitest';

import { createFocusGate, FOCUS_COALESCE_MS } from './focusGate';

describe('createFocusGate (P124d)', () => {
  it('accepts the first true, drops duplicates inside the window, ignores false', () => {
    let t = 0;
    const accept = createFocusGate(FOCUS_COALESCE_MS, () => t);
    t = 0;
    expect(accept(true)).toBe(true);
    t = 47;
    expect(accept(true)).toBe(false);
    t = 60;
    expect(accept(false)).toBe(false); // false never accepts …
    t = 70;
    expect(accept(true)).toBe(false); // … and does NOT reset the window
    t = 300;
    expect(accept(true)).toBe(true);
  });

  it('measures the window from the last ACCEPTED focus, not the last delivery', () => {
    let t = 0;
    const accept = createFocusGate(250, () => t);
    expect(accept(true)).toBe(true);
    t = 200;
    expect(accept(true)).toBe(false);
    t = 260; // 260 ms after the accepted one (only 60 after the dropped one)
    expect(accept(true)).toBe(true);
  });

  it('defaults to FOCUS_COALESCE_MS = 250', () => {
    expect(FOCUS_COALESCE_MS).toBe(250);
  });
});
