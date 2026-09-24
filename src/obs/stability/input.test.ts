// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from 'vitest';
import { attachInputTracker, attachResizeTracker, inputWithin, resizeWithin } from './input';

function fire(type: string, timeStamp: number, target: EventTarget = window): void {
  const e = new Event(type);
  Object.defineProperty(e, 'timeStamp', { value: timeStamp });
  target.dispatchEvent(e);
}

describe('input tracker (A8)', () => {
  const detach: Array<() => void> = [];
  afterEach(() => {
    for (const d of detach.splice(0)) d();
  });

  it('counts pointerdown, keydown, wheel, touchstart inside [t − ms, t]', () => {
    detach.push(attachInputTracker());
    fire('wheel', 1000);
    expect(inputWithin(1400, 500)).toBe(true);
    expect(inputWithin(1501, 500)).toBe(false);
    expect(inputWithin(999, 500)).toBe(false); // input after t does not count
    for (const t of ['pointerdown', 'keydown', 'touchstart']) {
      fire(t, 5000);
      expect(inputWithin(5000, 500)).toBe(true);
    }
  });

  it('ignores the DOM scroll event (app-initiated scrollIntoView must not mask a shift)', () => {
    detach.push(attachInputTracker());
    fire('scroll', 1000);
    expect(inputWithin(1000, 500)).toBe(false);
  });

  it('detach removes the listeners and clears the ring', () => {
    const off = attachInputTracker();
    fire('keydown', 100);
    off();
    fire('keydown', 200);
    expect(inputWithin(200, 500)).toBe(false);
  });

  it('resize tracker notes resize events', () => {
    detach.push(attachResizeTracker());
    fire('resize', 2000);
    expect(resizeWithin(2300, 500)).toBe(true);
    expect(resizeWithin(2600, 500)).toBe(false);
  });
});
