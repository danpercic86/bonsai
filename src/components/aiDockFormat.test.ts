/** P68e §13.1 — the dock's pure layer: elapsed/cost formatting, the locked pill
 *  copy, log-line classification, multi-run aggregation and the height clamp. */
import { describe, expect, it } from 'vitest';

import {
  AI_DOCK_HEIGHT_MAX,
  AI_DOCK_HEIGHT_MIN,
  aggregateRuns,
  announceFor,
  classifyLogLine,
  clampDockHeight,
  formatCost,
  formatElapsed,
  formatThinkingTokens,
  pillFor,
  THINKING_TOKENS_TITLE,
  type AiActivityRun,
} from './aiDockFormat';
import type { AiRunStatus } from './repoWorkspace/useAiRuns';

function run(over: Partial<AiActivityRun> = {}): AiActivityRun {
  return {
    key: 'conflict:a.ts',
    label: 'a.ts',
    status: 'running',
    elapsedMs: 0,
    costUsd: null,
    question: null,
    error: null,
    partialText: null,
    log: [],
    logDropped: 0,
    files: [],
    paths: ['a.ts'],
    cancelRequested: false,
    turn: 0,
    thinkingTokens: null,
    openedInPane: false,
    ...over,
  };
}

describe('formatElapsed', () => {
  it('matches the §13.1-5 known-answer table', () => {
    expect(formatElapsed(0)).toBe('0:00');
    expect(formatElapsed(7_400)).toBe('0:07');
    expect(formatElapsed(725_000)).toBe('12:05');
    expect(formatElapsed(3_723_000)).toBe('1:02:03');
  });

  it('never renders a negative or non-finite clock', () => {
    expect(formatElapsed(-5_000)).toBe('0:00');
    expect(formatElapsed(Number.NaN)).toBe('0:00');
  });
});

describe('formatCost', () => {
  // U13: a guess is worse than nothing — `costUsd` only lands on turnEnd/done.
  it('shows $— while unknown and never extrapolates', () => {
    expect(formatCost(null)).toBe('$—');
    expect(formatCost(Number.NaN)).toBe('$—');
  });

  it('uses 4 decimals under a dollar and 2 above it', () => {
    expect(formatCost(0.0238)).toBe('$0.0238');
    expect(formatCost(1.2)).toBe('$1.20');
  });
});

describe('formatThinkingTokens', () => {
  // §12-B1: the only live spend signal before the first `costUsd`. Estimated,
  // thinking-tokens only, and NEVER converted into money.
  it('renders an estimate marker and a grouped count, and never a price', () => {
    expect(formatThinkingTokens(450)).toBe('~450 tok');
    expect(formatThinkingTokens(12_500)).toBe(`~${(12_500).toLocaleString()} tok`);
    expect(formatThinkingTokens(1)).not.toMatch(/\$/);
    expect(THINKING_TOKENS_TITLE).toMatch(/not a price/);
  });

  it('is ABSENT rather than zero when the run reports nothing', () => {
    expect(formatThinkingTokens(null)).toBeNull();
    expect(formatThinkingTokens(0)).toBeNull();
    expect(formatThinkingTokens(Number.NaN)).toBeNull();
    expect(formatThinkingTokens(-10)).toBeNull();
  });
});

describe('pillFor', () => {
  it('is the §2 locked copy, word first', () => {
    const table: [AiRunStatus, boolean, string, string][] = [
      ['running', false, 'Running', 'running'],
      ['running', true, 'Stopping…', 'stopping'],
      ['awaitingInput', false, 'Needs you', 'awaiting'],
      ['ready', false, 'Ready', 'ready'],
      ['failed', false, 'Failed', 'failed'],
      ['cancelled', false, 'Cancelled', 'cancelled'],
    ];
    for (const [status, cancelling, label, dataStatus] of table) {
      const pill = pillFor(status, cancelling);
      expect(pill.label).toBe(label);
      expect(pill.dataStatus).toBe(dataStatus);
      // U8: every status carries a glyph too, so colour is never the only cue.
      expect(pill.glyph).not.toBe('');
    }
  });

  it('cancelRequested is ignored once the run is terminal', () => {
    expect(pillFor('cancelled', true).label).toBe('Cancelled');
    expect(pillFor('ready', true).label).toBe('Ready');
  });
});

describe('classifyLogLine (re-exported from the store, §12-A1)', () => {
  it('routes each wire prefix to its kind', () => {
    expect(classifyLogLine('⚙ Read(x)')).toBe('tool');
    expect(classifyLogLine('stderr: boom')).toBe('stderr');
    expect(classifyLogLine('» answered (12 bytes)')).toBe('meta');
    expect(classifyLogLine('Hello')).toBe('text');
  });
});

describe('aggregateRuns', () => {
  it('picks the most urgent status: awaitingInput > running > failed > ready > cancelled', () => {
    const order: AiRunStatus[] = ['cancelled', 'ready', 'failed', 'running', 'awaitingInput'];
    for (let i = 1; i < order.length; i++) {
      const worse = order[i] as AiRunStatus;
      const better = order[i - 1] as AiRunStatus;
      const agg = aggregateRuns([run({ key: 'a', status: better }), run({ key: 'b', status: worse })]);
      expect(agg.status).toBe(worse);
    }
  });

  it('sums cost across runs (separate processes) and keeps null when nothing is known', () => {
    expect(aggregateRuns([run({ costUsd: 0.01 }), run({ key: 'b', costUsd: 0.02 })]).costUsd).toBeCloseTo(
      0.03,
    );
    expect(aggregateRuns([run(), run({ key: 'b' })]).costUsd).toBeNull();
  });

  it('takes elapsed + the activity line from the longest-running ACTIVE run', () => {
    const agg = aggregateRuns([
      run({ key: 'a', elapsedMs: 5_000, log: [{ seq: 1, text: 'short', kind: 'text' }] }),
      run({ key: 'b', elapsedMs: 40_000, log: [{ seq: 2, text: '⚙ Read(x)', kind: 'tool' }] }),
    ]);
    expect(agg.elapsedMs).toBe(40_000);
    expect(agg.latest).toBe('⚙ Read(x)');
  });

  it('offers Cancel only when exactly one run is active', () => {
    expect(aggregateRuns([run({ key: 'a' }), run({ key: 'b', status: 'ready' })]).cancelKey).toBe('a');
    expect(aggregateRuns([run({ key: 'a' }), run({ key: 'b' })]).cancelKey).toBeNull();
  });
});

describe('announceFor (§11)', () => {
  it('announces the six transitions and NOTHING else', () => {
    const seen = new Map<string, string>();
    expect(announceFor([run()], seen)).toBe('AI run started for a.ts');
    // A second render with no change announces nothing.
    expect(announceFor([run()], seen)).toBeNull();
    expect(announceFor([run({ cancelRequested: true })], seen)).toBe(
      'Stopping the AI run for a.ts',
    );
    expect(announceFor([run({ status: 'awaitingInput' })], seen)).toBe(
      'Claude needs your answer about a.ts',
    );
    expect(announceFor([run({ status: 'cancelled' })], seen)).toBe(
      'AI run cancelled. Nothing was changed.',
    );
    expect(announceFor([run({ status: 'failed', error: 'boom' })], seen)).toBe(
      'AI run failed: boom',
    );
    expect(announceFor([run({ status: 'ready' })], seen)).toBe('AI proposal ready for a.ts');
  });

  it('counts ready files for a bulk run', () => {
    const seen = new Map<string, string>();
    const bulk = run({
      status: 'ready',
      files: [
        { path: 'a.ts', status: 'ready', error: null, hasProposal: true },
        { path: 'b.ts', status: 'ready', error: null, hasProposal: true },
        { path: 'c.ts', status: 'failed', error: 'no result', hasProposal: false },
      ],
    });
    expect(announceFor([bulk], seen)).toBe('AI proposals ready for 2 of 3 files');
  });

  it('forgets runs that disappeared, so a retry announces again', () => {
    const seen = new Map<string, string>();
    announceFor([run()], seen);
    announceFor([], seen);
    expect(seen.size).toBe(0);
    expect(announceFor([run()], seen)).toBe('AI run started for a.ts');
  });
});

describe('clampDockHeight', () => {
  it('respects the persisted range', () => {
    expect(clampDockHeight(10, 1_000)).toBe(AI_DOCK_HEIGHT_MIN);
    expect(clampDockHeight(9_000, 4_000)).toBe(AI_DOCK_HEIGHT_MAX);
  });

  it('never lets the dock swallow the graph on a short window (60% cap)', () => {
    // 400px viewport -> effective max 240px.
    expect(clampDockHeight(600, 400)).toBe(240);
    // Below the minimum the minimum still wins — the dock stays usable.
    expect(clampDockHeight(600, 100)).toBe(AI_DOCK_HEIGHT_MIN);
  });
});
