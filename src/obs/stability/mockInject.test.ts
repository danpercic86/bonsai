// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { attachFocusLoss } from './focusLoss';
import { installPerfInject } from './mockInject';
import { markInteractive, resetPhaseForTests } from './phase';
import type { StabilityEvent } from './types';

let now = 0;
beforeEach(() => {
  resetPhaseForTests();
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'requestAnimationFrame'] });
  now = 1000;
  vi.spyOn(performance, 'now').mockImplementation(() => now);
  vi.spyOn(document, 'hasFocus').mockImplementation(() => true);
  document.body.innerHTML =
    '<main data-obs-region="graph"><p>row</p></main><div data-obs-region="details"></div>';
});
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  document.body.innerHTML = '';
});

describe('?perfInject* mock flags', () => {
  it('no or invalid flags install nothing', () => {
    installPerfInject('?perfInjectShift=../../x')();
    installPerfInject('?perfInjectShift=other&perfInjectFocusSteal=0')();
    markInteractive();
    vi.advanceTimersByTime(5000);
    expect(document.querySelector('[data-perf-inject]')).toBeNull();
  });

  it('perfInjectShift=<region> inserts one in-flow spacer after interactive, once', () => {
    const off = installPerfInject('?perfInjectShift=graph');
    vi.advanceTimersByTime(5000);
    expect(document.querySelector('[data-perf-inject]')).toBeNull(); // still boot
    markInteractive();
    vi.advanceTimersByTime(5000);
    const graph = document.querySelector('[data-obs-region="graph"]');
    expect(graph?.firstElementChild?.getAttribute('data-perf-inject')).toBe('shift');
    expect(document.querySelectorAll('[data-perf-inject]')).toHaveLength(1);
    off();
  });

  it('perfInjectFocusSteal=1 REMOVES the focused element and the detector reports it', async () => {
    const got: StabilityEvent[] = [];
    const detach = attachFocusLoss((e) => got.push(e));
    const off = installPerfInject('?perfInjectFocusSteal=1');
    markInteractive();
    now = 5000;
    vi.advanceTimersByTime(750); // inject (INPUT_WINDOW_MS + 250): focus; rAF not yet
    expect(document.activeElement?.getAttribute('data-perf-inject')).toBe('focus');
    vi.advanceTimersByTime(30); // next frame: remove (never .blur())
    expect(document.querySelector('[data-perf-inject]')).toBeNull();
    await Promise.resolve();
    await Promise.resolve();
    vi.advanceTimersByTime(100);
    expect(got).toHaveLength(1);
    expect(got[0]).toMatchObject({ type: 'focus-lost', fromRegion: 'details', phase: 'interactive' });
    off();
    detach();
  });
});
