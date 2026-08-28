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

/** Builds one mock `ipc.result` LogRecord. `seq` is (re)assigned by the ring. */
function resultRecord(cmd: string, ms: number, trace: string): LogRecord {
  mockSpanSeq += 1;
  return {
    seq: 0,
    ts: Date.now(),
    mono: mockSpanSeq * 10,
    src: 'ui',
    lvl: 'debug',
    trace,
    kind: 'ipc.result',
    cmd,
    argsHash: 'ui:h#1',
    ms,
    outcome: 'ok',
  } as unknown as LogRecord;
}

/**
 * P91 §5.1 — the `slow-command` fixture, paired with the slow `graph.get` span
 * above. The Rust `slow-command` rule (and the mock analyzer that mirrors it)
 * consumes `ipc.result` records keyed by `cmd` (`get_graph`), not spans, and is
 * gated by `MIN_SAMPLES = 20`. So a lone slow span cannot trip it: we seed 20
 * healthy `get_graph` results (~90 ms) to establish the baseline, then one slow
 * result at 4200 ms carrying the same `trace` as the slow span. That fires
 * `slow-command` via the calibrated branch (4200 > max(floor 1200, 3×p95≈270))
 * and, via the correlated span whose `lane` phase is ~86 % of `ms`, `slow-phase`.
 *
 * The 20 identical fast results also double as a `dup-ipc` true-NEGATIVE: they are
 * `ipc.result`, never `ipc.call`, so the explicit-kind guard must ignore them.
 */
export function slowCommandFixtures(): LogRecord[] {
  const out: LogRecord[] = [];
  for (let i = 0; i < 20; i += 1) {
    out.push(resultRecord('get_graph', 88 + (i % 5), `mock-graph-${i}`));
  }
  out.push(resultRecord('get_graph', 4200, 'mock-graph-slow'));
  return out;
}
