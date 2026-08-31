import { describe, expect, it } from 'vitest';
import { createRefreshCoalescer } from './refreshCoalescer';

// A controllable round: each start() takes a fresh deferred the test resolves
// manually, so we drive the state machine deterministically (no timers).
function makeControllableRun() {
  const resolvers: Array<() => void> = [];
  let calls = 0;
  const run = (): Promise<void> => {
    calls += 1;
    return new Promise<void>((resolve) => {
      resolvers.push(resolve);
    });
  };
  return {
    run,
    get calls(): number {
      return calls;
    },
    /** Settle the Nth (0-based) in-flight round. */
    settle(index: number): void {
      const r = resolvers[index];
      if (r) r();
    },
  };
}

describe('createRefreshCoalescer', () => {
  it('runs immediately when idle (leading edge)', async () => {
    const ctl = makeControllableRun();
    const c = createRefreshCoalescer(ctl.run);
    const p = c.request();
    expect(ctl.calls).toBe(1);
    expect(c.isRunning).toBe(true);
    ctl.settle(0);
    await p;
    expect(c.isRunning).toBe(false);
  });

  it('collapses K mid-flight requests into exactly one trailing round (AC5)', async () => {
    const ctl = makeControllableRun();
    const c = createRefreshCoalescer(ctl.run);

    const leading = c.request(); // starts round 0
    expect(ctl.calls).toBe(1);

    // K = 5 requests arrive while round 0 is in flight.
    const collapsed = [c.request(), c.request(), c.request(), c.request(), c.request()];
    expect(ctl.calls).toBe(1); // no new round yet — collapsed into trailing

    ctl.settle(0); // leading settles → trailing round 1 starts
    await leading;
    expect(ctl.calls).toBe(2);
    expect(c.isRunning).toBe(true);

    ctl.settle(1); // trailing settles
    await Promise.all(collapsed);
    expect(ctl.calls).toBe(2); // exactly 2 rounds total, independent of K
    expect(c.isRunning).toBe(false);
  });

  it("resolves each caller's promise when its serving round settles", async () => {
    const ctl = makeControllableRun();
    const c = createRefreshCoalescer(ctl.run);

    let leadingDone = false;
    let trailingDone = false;
    const leading = c.request().then(() => {
      leadingDone = true;
    });
    const trailing = c.request().then(() => {
      trailingDone = true;
    });

    ctl.settle(0);
    await leading;
    expect(leadingDone).toBe(true);
    expect(trailingDone).toBe(false); // trailing round hasn't settled yet

    ctl.settle(1);
    await trailing;
    expect(trailingDone).toBe(true);
  });

  it('does not start extra rounds after all settle (no late-slice restart)', async () => {
    const ctl = makeControllableRun();
    const c = createRefreshCoalescer(ctl.run);

    const leading = c.request();
    const trailing = c.request();
    ctl.settle(0);
    await leading;
    ctl.settle(1);
    await trailing;

    expect(ctl.calls).toBe(2);
    expect(c.isRunning).toBe(false);

    // A brand-new request after quiescence runs immediately (leading edge again).
    const again = c.request();
    expect(ctl.calls).toBe(3);
    ctl.settle(2);
    await again;
    expect(c.isRunning).toBe(false);
  });

  it('recovers if a round rejects (contract says it never does)', async () => {
    let calls = 0;
    const run = (): Promise<void> => {
      calls += 1;
      return calls === 1 ? Promise.reject(new Error('boom')) : Promise.resolve();
    };
    const c = createRefreshCoalescer(run);
    await c.request(); // rejection swallowed, phase returns to idle
    expect(c.isRunning).toBe(false);
    await c.request(); // still functional
    expect(calls).toBe(2);
  });
});
