// P81 §3 — Refresh coalescer: leading + at-most-one-trailing, in-flight-based.
// Pure (no React, no timers). The "window" is the duration of the round itself:
// a request arriving while a round is in flight schedules exactly ONE trailing
// round, regardless of how many arrive. Idle requests run immediately (leading
// edge → instant user feedback).

export interface RefreshCoalescer {
  /** Enqueue a round. Resolves when the round serving this call (the leading
   *  round, or the single trailing round it collapsed into) settles. */
  request(): Promise<void>;
  /** True while a round is in flight (test/inspection helper). */
  readonly isRunning: boolean;
}

interface Deferred {
  promise: Promise<void>;
  resolve: () => void;
}

function newDeferred(): Deferred {
  let resolve: () => void = () => {};
  const promise = new Promise<void>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

/**
 * `run` MUST NOT throw (the canonical refresh round is already try/caught). If
 * it ever rejects, the rejection is swallowed here so the state machine cannot
 * wedge — a superseded round's late slice responses are already no-ops via the
 * round's internal per-slice request-id guards.
 */
export function createRefreshCoalescer(run: () => Promise<void>): RefreshCoalescer {
  let phase: 'idle' | 'running' = 'idle';
  let trailing = false;
  let runningTail: Promise<void> = Promise.resolve();
  // Requests that collapse into the next trailing round await this deferred.
  let trailingTail = newDeferred();

  const start = (): void => {
    // Call `run` SYNCHRONOUSLY so the leading edge fires immediately (AC9). It is
    // contractually non-throwing; guard anyway so a stray sync throw or rejection
    // still drives onSettle and cannot leave phase stuck at 'running'.
    let p: Promise<void>;
    try {
      p = run();
    } catch {
      p = Promise.resolve();
    }
    runningTail = p.then(undefined, () => {});
    void runningTail.then(onSettle);
  };

  const onSettle = (): void => {
    if (trailing) {
      trailing = false;
      const prev = trailingTail;
      trailingTail = newDeferred();
      start(); // chain the single trailing round
      // Resolve the batch that requested this trailing round once it settles.
      void runningTail.then(() => prev.resolve());
    } else {
      phase = 'idle';
    }
  };

  const request = (): Promise<void> => {
    if (phase === 'idle') {
      phase = 'running';
      start();
      return runningTail;
    }
    // phase === 'running': collapse ALL mid-flight requests into one trailing.
    trailing = true;
    return trailingTail.promise;
  };

  return {
    request,
    get isRunning(): boolean {
      return phase === 'running';
    },
  };
}
