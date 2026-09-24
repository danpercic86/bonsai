/** P120 T-F10 — the mock `analyzeStability` mirrors Rust T-R1–T-R7 on the shared fixture. */
import { describe, expect, it } from 'vitest';
import type { LogRecord } from '../types';
import cases from './fixtures/stabilityAnomalyCases.json';
import { analyzeAnomalies } from './obsAnomaly';
import { analyzeStability } from './obsAnomalyStability';

interface Expected {
  rule: string;
  severity: string;
  refs: number[];
  traces: string[];
  ts: number;
  detail: string;
}
interface Case {
  name: string;
  records: Array<{ seq: number; ts: number; kind: string } & Record<string, unknown>>;
  expect: Expected[];
}

function toRecords(c: Case): LogRecord[] {
  return c.records.map((r) => ({
    ...r,
    mono: r.ts,
    src: r.kind === 'watcher' ? 'rust' : 'ui',
    lvl: 'debug',
  })) as unknown as LogRecord[];
}

function project(r: LogRecord): Expected {
  return {
    rule: String(r.rule),
    severity: String(r.severity),
    refs: r.refs as number[],
    traces: r.traces as string[],
    ts: r.ts,
    detail: String(r.detail),
  };
}

function focusLost(seq: number, lostAt: number): LogRecord {
  return {
    seq, ts: lostAt + 50, mono: lostAt, src: 'ui', lvl: 'info', kind: 'focus.lost',
    fromRegion: 'details', phase: 'interactive', via: 'detached', lostAt,
  };
}

describe('analyzeStability (T-F10, shared fixture with T-R1–T-R6)', () => {
  it.each((cases as { cases: Case[] }).cases.map((c) => [c.name, c] as const))('%s', (_name, c) => {
    const out = analyzeStability(toRecords(c));
    expect(out.map(project)).toEqual(c.expect);
    for (const a of out) {
      expect(a.kind).toBe('anomaly');
      expect(a.lvl).toBe(a.severity);
      expect(String(a.detail)).not.toMatch(/[/\\]|:\/\//);
    }
  });

  it('T-R7 mirror: 70 held losses overflow the OLDEST 6 as info, 70 anomalies overall', () => {
    const records = Array.from({ length: 70 }, (_, i) => focusLost(i + 1, 10_000 + i));
    const out = analyzeStability(records);
    expect(out).toHaveLength(70);
    expect(out.every((a) => a.severity === 'info')).toBe(true);
    // The first six emitted are the cap overflow, in FIFO order.
    expect(out.slice(0, 6).map((a) => (a.refs as number[])[0])).toEqual([1, 2, 3, 4, 5, 6]);
  });

  it('analyzeAnomalies appends the stability anomalies with fresh, non-colliding seqs', () => {
    const records = [focusLost(1, 10_000), focusLost(2, 11_000)];
    const out = analyzeAnomalies(records).filter((r) => r.rule === 'focus-steal');
    expect(out.map((r) => r.seq)).toEqual([3, 4]);
    expect(out.map((r) => r.refs)).toEqual([[1], [2]]);
    expect(out.every((r) => r.src === 'rust' && r.mono !== 0)).toBe(true);
  });
});
