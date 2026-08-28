import { describe, expect, it } from 'vitest';
import type { LogRecord } from '../types';
import { analyzeAnomalies } from './obsAnomaly';
import { slowCommandFixtures, spanFixtures } from './obs';

/** Assigns monotonic seqs exactly as the ring does, so refs resolve to them. */
function withSeqs(records: LogRecord[]): LogRecord[] {
  return records.map((r, i) => ({ ...r, seq: i + 1 }));
}

function call(cmd: string, argsHash: string, ts: number): LogRecord {
  return { seq: 0, ts, mono: ts, src: 'ui', lvl: 'debug', kind: 'ipc.call', cmd, argsHash } as unknown as LogRecord;
}

function result(cmd: string, ms: number, ts: number, trace?: string): LogRecord {
  return {
    seq: 0, ts, mono: ts, src: 'ui', lvl: 'debug', kind: 'ipc.result', cmd, ms, outcome: 'ok', trace, argsHash: 'ui:h#1',
  } as unknown as LogRecord;
}

describe('dup-ipc', () => {
  it('fires exactly one dup-ipc for a same cmd+args double-call within 300ms', () => {
    const records = withSeqs([call('get_status', 'h1', 1000), call('get_status', 'h1', 1150)]);
    const out = analyzeAnomalies(records);
    const dups = out.filter((r) => r.rule === 'dup-ipc');
    expect(dups).toHaveLength(1);
    expect(dups[0].refs).toEqual([records[0].seq, records[1].seq]);
    expect(dups[0].kind).toBe('anomaly');
    expect(dups[0].severity).toBe('warn');
  });

  it('does not fire when the two calls are outside the 300ms window', () => {
    const records = withSeqs([call('get_status', 'h1', 1000), call('get_status', 'h1', 1400)]);
    expect(analyzeAnomalies(records).filter((r) => r.rule === 'dup-ipc')).toHaveLength(0);
  });

  it('does not fire when a mutation command intervenes', () => {
    const records = withSeqs([
      call('get_status', 'h1', 1000),
      call('commit_create', 'm1', 1050),
      call('get_status', 'h1', 1150),
    ]);
    expect(analyzeAnomalies(records).filter((r) => r.rule === 'dup-ipc')).toHaveLength(0);
  });

  it('ignores a non-ipc.call record carrying an argsHash (explicit-kind guard)', () => {
    // Two ipc.RESULT records with identical cmd+argsHash in-window must NOT dup.
    const records = withSeqs([result('get_status', 5, 1000), result('get_status', 5, 1100)]);
    expect(analyzeAnomalies(records).filter((r) => r.rule === 'dup-ipc')).toHaveLength(0);
  });
});

describe('slow-command + slow-phase (mock fixture)', () => {
  it('the slow graph.get fixture trips slow-command + slow-phase referencing the span', () => {
    const records = withSeqs([...spanFixtures(), ...slowCommandFixtures()]);
    const out = analyzeAnomalies(records);
    const slowCmd = out.filter((r) => r.rule === 'slow-command');
    const slowPhase = out.filter((r) => r.rule === 'slow-phase');
    expect(slowCmd).toHaveLength(1);
    expect(slowPhase).toHaveLength(1);

    const slowResult = records.find((r) => r.kind === 'ipc.result' && r.trace === 'mock-graph-slow');
    const slowSpan = records.find((r) => r.kind === 'span' && r.trace === 'mock-graph-slow');
    expect(slowResult).toBeDefined();
    expect(slowSpan).toBeDefined();
    expect(slowCmd[0].refs).toEqual([slowResult!.seq]);
    // slow-phase refs = [span seq, result seq]; both resolve to real ring records.
    expect(slowPhase[0].refs).toEqual([slowSpan!.seq, slowResult!.seq]);
    expect(slowPhase[0].severity).toBe('info');
    expect(String(slowPhase[0].detail)).toContain('lane');
  });

  it('a uniform-fast get_graph baseline fires nothing', () => {
    const records = withSeqs(
      Array.from({ length: 25 }, (_, i) => result('get_graph', 90, 1000 + i * 10, `t${i}`)),
    );
    const out = analyzeAnomalies(records);
    expect(out.filter((r) => r.rule === 'slow-command')).toHaveLength(0);
    expect(out.filter((r) => r.rule === 'slow-phase')).toHaveLength(0);
  });

  it('fires nothing below MIN_SAMPLES when under the hard cap', () => {
    // 5 fast then one at 4200ms (> get_graph floor but < 10s catch-all): too few
    // samples for the calibrated branch, so no fire — mirrors gate test (c).
    const records = withSeqs([
      ...Array.from({ length: 5 }, (_, i) => result('get_graph', 90, 1000 + i * 10, `t${i}`)),
      result('get_graph', 4200, 1100, 'slow'),
    ]);
    expect(analyzeAnomalies(records).filter((r) => r.rule === 'slow-command')).toHaveLength(0);
  });
});
