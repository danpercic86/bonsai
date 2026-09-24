// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { resetPhaseForTests } from './phase';
import { addStabilitySink, resetSinksForTests, stabilitySinkCountForTests } from './sinks';
import { FakePerformanceObserver, shiftEntry } from './testFakes';
import type { StabilityEvent } from './types';

let moCount = 0;
class CountingMutationObserver extends MutationObserver {
  constructor(cb: MutationCallback) {
    super(cb);
    moCount += 1;
  }
}

beforeEach(() => {
  resetPhaseForTests();
  resetSinksForTests();
  FakePerformanceObserver.instances = [];
  moCount = 0;
  vi.stubGlobal('PerformanceObserver', FakePerformanceObserver);
  vi.stubGlobal('MutationObserver', CountingMutationObserver);
});
afterEach(() => {
  resetSinksForTests();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe('sink fan-out + 0<->1 attach (T-F8, AC 12)', () => {
  it('with no sinks registered nothing is attached', () => {
    const winAdd = vi.spyOn(window, 'addEventListener');
    const docAdd = vi.spyOn(document, 'addEventListener');
    expect(stabilitySinkCountForTests()).toBe(0);
    expect(winAdd).not.toHaveBeenCalled();
    expect(docAdd).not.toHaveBeenCalled();
    expect(FakePerformanceObserver.instances).toHaveLength(0);
    expect(moCount).toBe(0);
  });

  it('0->1 attaches every observer + tracker once; 1->0 detaches them all', () => {
    const winAdd = vi.spyOn(window, 'addEventListener');
    const winRemove = vi.spyOn(window, 'removeEventListener');
    const docAdd = vi.spyOn(document, 'addEventListener');
    const docRemove = vi.spyOn(document, 'removeEventListener');
    const offA = addStabilitySink(() => {});
    const winListeners = winAdd.mock.calls.length;
    const docListeners = docAdd.mock.calls.length;
    expect(winListeners).toBeGreaterThan(0);
    expect(docListeners).toBe(2); // focusin + focusout
    expect(FakePerformanceObserver.instances).toHaveLength(1);
    expect(moCount).toBe(1);
    const offB = addStabilitySink(() => {});
    expect(winAdd.mock.calls.length).toBe(winListeners); // a second sink attaches nothing
    offA();
    expect(FakePerformanceObserver.instances[0]?.disconnected).toBe(false);
    offB();
    offB(); // idempotent
    expect(FakePerformanceObserver.instances[0]?.disconnected).toBe(true);
    expect(winRemove.mock.calls.length).toBe(winListeners);
    expect(docRemove.mock.calls.length).toBe(docListeners);
    expect(stabilitySinkCountForTests()).toBe(0);
  });

  it('fans out to every sink; a throwing sink does not starve the others', () => {
    const a: StabilityEvent[] = [];
    const b: StabilityEvent[] = [];
    vi.spyOn(console, 'error').mockImplementation(() => {});
    addStabilitySink((e) => a.push(e));
    addStabilitySink(() => {
      throw new Error('boom');
    });
    addStabilitySink((e) => b.push(e));
    FakePerformanceObserver.instances[0]?.deliver([shiftEntry({ startTime: performance.now() + 1 })]);
    expect(a).toHaveLength(1);
    expect(b).toHaveLength(1);
  });

  it('only the first attach in the page life replays buffered entries (A7)', () => {
    addStabilitySink(() => {})();
    addStabilitySink(() => {});
    expect(FakePerformanceObserver.instances.map((o) => o.options?.buffered)).toEqual([true, false]);
  });
});
