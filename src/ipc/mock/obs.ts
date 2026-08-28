/**
 * P91 §4/§5.1 "Mock IPC" — synthesised `span` records for the browser harness.
 *
 * Its own module (contract §1 names `src/ipc/mock/obs.ts`), kept as pure
 * fixture data with no side effects: `obsRing.ts` wires these into the ring and
 * exposes them on `window.__bonsaiSeedSpans()` so the harness can exercise the
 * §5.1 performance rules (`slow-command` / `slow-phase` / `cache-collapse`)
 * with no Tauri. Increment 3 only provides the fixtures; the detector that
 * consumes them ships in increment 5.
 */
import type { LogRecord } from '../types';
import type { SpanPayload } from '../types/obs';

let mockSpanSeq = 0;

/** Builds one mock `span` LogRecord. `seq` is (re)assigned by the ring on
 *  append, as the Rust writer thread does; `ts`/`mono` are plausible fillers. */
function spanRecord(payload: SpanPayload, trace: string): LogRecord {
  mockSpanSeq += 1;
  return {
    seq: 0,
    ts: Date.now(),
    mono: mockSpanSeq * 10,
    src: 'rust',
    lvl: 'debug',
    trace,
    kind: 'span',
    ...payload,
  } as unknown as LogRecord;
}

/**
 * A plausible span set: several normal `graph.get` results, one deliberately
 * SLOW `graph.get` dominated by the `lane` phase (so increment 5's
 * `slow-command` + `slow-phase` pair has something to fire on and point at),
 * plus a `status.scan` and a `diff.compute` so every instrumented op appears.
 */
export function spanFixtures(): LogRecord[] {
  const out: LogRecord[] = [];

  // A healthy baseline: five ~90 ms cache-served graph fetches.
  for (let i = 0; i < 5; i += 1) {
    out.push(
      spanRecord(
        {
          op: 'graph.get',
          ms: 88 + i,
          cache: 'hit',
          items: 1200,
          phases: [
            { name: 'decorate', ms: 6 },
            { name: 'serialize', ms: 80 },
          ],
          queuedMs: 0,
          poolInflight: 1,
          poolMax: 512,
          outcome: 'ok',
        },
        `mock-graph-${i}`,
      ),
    );
  }

  // The deliberately slow one: a cold walk whose time is almost all `lane`.
  out.push(
    spanRecord(
      {
        op: 'graph.get',
        ms: 4200,
        cache: 'miss',
        items: 21000,
        phases: [
          { name: 'decorate', ms: 40, n: 812 },
          { name: 'revwalk', ms: 380, n: 21000 },
          { name: 'lane', ms: 3600, n: 21000 },
          { name: 'serialize', ms: 150 },
        ],
        queuedMs: 12,
        poolInflight: 3,
        poolMax: 512,
        deadlineFrac: 0.42,
        outcome: 'ok',
      },
      'mock-graph-slow',
    ),
  );

  out.push(
    spanRecord(
      {
        op: 'status.scan',
        ms: 34,
        items: 18,
        phases: [{ name: 'statuses', ms: 33 }],
        queuedMs: 0,
        poolInflight: 1,
        poolMax: 512,
        outcome: 'ok',
      },
      'mock-status',
    ),
  );

  out.push(
    spanRecord(
      {
        op: 'diff.compute',
        ms: 12,
        items: 1,
        phases: [{ name: 'hunks', ms: 11 }],
        queuedMs: 0,
        poolInflight: 1,
        poolMax: 512,
        outcome: 'ok',
      },
      'mock-diff',
    ),
  );

  return out;
}
