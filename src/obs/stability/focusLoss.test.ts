// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { MockInstance } from 'vitest';
import { attachFocusLoss } from './focusLoss';
import { attachInputTracker } from './input';
import { markInteractive, resetPhaseForTests } from './phase';
import { FOCUS_SETTLE_MS } from './types';
import type { StabilityEvent } from './types';

let now = 0;
let active: Element | null = null;
let hasFocus = true;
let got: StabilityEvent[] = [];
let detach: Array<() => void> = [];
let layoutReads: MockInstance[] = [];

function region(name: string): HTMLElement {
  const root = document.createElement('div');
  root.setAttribute('data-obs-region', name);
  document.body.appendChild(root);
  return root;
}
function focusable(parent: HTMLElement): HTMLButtonElement {
  const b = document.createElement('button');
  parent.appendChild(b);
  return b;
}
function focusIn(el: Element): void {
  active = el;
  el.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
}
function focusOut(el: Element): void {
  el.dispatchEvent(new FocusEvent('focusout', { bubbles: true, relatedTarget: null }));
}
function input(type: string, at: number): void {
  const e = new Event(type);
  Object.defineProperty(e, 'timeStamp', { value: at });
  window.dispatchEvent(e);
}
async function settle(): Promise<void> {
  await Promise.resolve(); // MutationObserver callbacks are microtasks
  await Promise.resolve();
  now += FOCUS_SETTLE_MS;
  vi.advanceTimersByTime(FOCUS_SETTLE_MS);
}

beforeEach(() => {
  resetPhaseForTests();
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
  now = 10_000;
  active = null;
  hasFocus = true;
  got = [];
  vi.spyOn(performance, 'now').mockImplementation(() => now);
  Object.defineProperty(document, 'activeElement', { configurable: true, get: () => active ?? document.body });
  Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'visible' });
  vi.spyOn(document, 'hasFocus').mockImplementation(() => hasFocus);
  layoutReads = [vi.spyOn(Element.prototype, 'getBoundingClientRect'), vi.spyOn(window, 'getComputedStyle')];
  markInteractive();
  now = 20_000;
  detach = [attachInputTracker(), attachFocusLoss((e) => got.push(e))];
});

afterEach(() => {
  for (const d of detach) d();
  // T-F13: the classifier never reads layout.
  for (const spy of layoutReads) expect(spy).not.toHaveBeenCalled();
  vi.useRealTimers();
  vi.restoreAllMocks();
  document.body.innerHTML = '';
});

describe('focus-loss truth table (T-F6, §3.6)', () => {
  it('row 1: body, window focused, no input → loss (refresh remount)', async () => {
    const el = focusable(region('details'));
    focusIn(el);
    active = null;
    focusOut(el);
    await settle();
    expect(got).toEqual([
      { type: 'focus-lost', fromRegion: 'details', phase: 'interactive', via: 'focusout', at: Math.round(performance.timeOrigin + 20_000) },
    ]);
  });

  it('row 2: body but input ≤500 ms → no loss (click empty space / Esc)', async () => {
    const el = focusable(region('details'));
    focusIn(el);
    input('pointerdown', now - 100);
    active = null;
    focusOut(el);
    await settle();
    expect(got).toEqual([]);
  });

  it('row 3: window not focused, recently blurred, or hidden → no loss (app switch)', async () => {
    const el = focusable(region('sidebar'));
    focusIn(el);
    active = null;
    hasFocus = false;
    focusOut(el);
    await settle();

    hasFocus = true;
    focusIn(el);
    window.dispatchEvent(new Event('blur'));
    active = null;
    focusOut(el);
    await settle();

    focusIn(el);
    Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'hidden' });
    now += 1000;
    active = null;
    focusOut(el);
    await settle();
    expect(got).toEqual([]);
  });

  it('row 4: focus moved to another element, no input → no loss', async () => {
    const r = region('graph');
    const a = focusable(r);
    const b = focusable(r);
    focusIn(a);
    focusOut(a);
    focusIn(b);
    await settle();
    expect(got).toEqual([]);
  });

  it('row 5: Tab navigation (other element + input) → no loss', async () => {
    const r = region('graph');
    const a = focusable(r);
    const b = focusable(r);
    focusIn(a);
    input('keydown', now);
    focusOut(a);
    focusIn(b);
    await settle();
    expect(got).toEqual([]);
  });

  it('row 6: dialog autofocus lands after focusout but before the settle → no loss', async () => {
    const a = focusable(region('details'));
    focusIn(a);
    active = null;
    focusOut(a); // briefly body here
    now += 20;
    vi.advanceTimersByTime(20);
    const dialogInput = focusable(region('other'));
    focusIn(dialogInput); // useEffect autofocus, after paint
    await settle();
    expect(got).toEqual([]);
  });

  it('row 7: element detached with no focusout → loss via detached', async () => {
    const el = focusable(region('diff'));
    focusIn(el);
    active = null;
    el.remove();
    await settle();
    expect(got).toHaveLength(1);
    expect(got[0]).toMatchObject({ type: 'focus-lost', fromRegion: 'diff', via: 'detached' });
  });

  it('row 8: nothing tracked (last focus was body) → no loss', async () => {
    document.body.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    focusOut(document.body);
    const stray = focusable(region('dock'));
    stray.remove();
    await settle();
    expect(got).toEqual([]);
  });

  it('focusout then removal of the same element reports once', async () => {
    const el = focusable(region('details'));
    focusIn(el);
    active = null;
    focusOut(el);
    el.remove();
    await settle();
    focusable(region('graph')).remove();
    await settle();
    expect(got).toHaveLength(1);
    expect(got[0]).toMatchObject({ via: 'detached' }); // isConnected wins over via0
  });

  it('boot-time losses are labelled boot by loss time', async () => {
    resetPhaseForTests();
    const el = focusable(region('sidebar'));
    focusIn(el);
    active = null;
    focusOut(el);
    await settle();
    expect(got[0]).toMatchObject({ phase: 'boot' });
  });
});
