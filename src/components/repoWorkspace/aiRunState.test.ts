/**
 * P68d §C — the pure transforms behind the AI run store: `settleBatch` (which owns
 * THE MARKERFUL SAFETY GATE), `deriveRowStates`, `pruneRuns`, and the event decision
 * table in `aiRunEvent.ts`.
 *
 * These are unit-tested directly, not only through the hook, because the gate is a
 * blocking safety requirement: nothing markerful may ever be presented as clean, and
 * a safety check deserves a test that names it and cannot be obscured by React timing.
 */
import { describe, expect, it } from 'vitest';

import { decideEvent } from './aiRunEvent';
import {
  AI_TERMINAL_RUNS_MAX,
  deriveRowStates,
  newRun,
  pruneRuns,
  settleBatch,
  type AiRunState,
} from './aiRunState';
import type { AiResolveBatch, AiRunEvent } from '../../ipc';

const CLEAN = 'merged body\n';
const MARKERFUL = ['<<<<<<< HEAD', 'ours', '=======', 'theirs', '>>>>>>> feat', ''].join('\n');

function batch(over: Partial<AiResolveBatch> = {}): AiResolveBatch {
  return { runId: 'r', proposals: [], failed: [], costUsd: null, turns: 1, ...over };
}

function ev(over: Partial<AiRunEvent> & Pick<AiRunEvent, 'seq' | 'kind'>): AiRunEvent {
  return {
    runId: 'r',
    text: null,
    costUsd: null,
    elapsedMs: 0,
    path: null,
    turn: 0,
    partialText: null,
    thinkingTokens: null,
    ...over,
  };
}

describe('settleBatch — THE MARKERFUL SAFETY GATE', () => {
  /**
   * The reason this gate exists: a `proposedText` is a REVIEWABLE proposal, not a
   * verified-clean merge. Bulk fails a markerful body in Rust, but the SINGLE-path
   * stream returns the model's output VERBATIM (P13 parity). If `autoResolve` staged
   * it, `resolve_conflict_text` would trust it (the git-add model) and the markers
   * would land in the repository silently.
   */
  it('autoResolve: a markerful body is demoted to failed and is NEVER stageable', () => {
    const out = settleBatch(
      ['a.ts'],
      batch({ proposals: [{ path: 'a.ts', proposedText: MARKERFUL, costUsd: null }] }),
      'autoResolve',
    );
    expect(out.stageable).toEqual([]);
    expect(out.markerful.map((f) => f.path)).toEqual(['a.ts']);
    expect(out.files[0]?.status).toBe('failed');
    expect(out.files[0]?.error).toBe('AI left unresolved markers in a.ts — opened for review');
    // The proposal itself is KEPT so the review editor can show it.
    expect(out.files[0]?.proposal).toBe(MARKERFUL);
    expect(out.status).toBe('failed');
  });

  it('autoResolve: a clean body IS stageable', () => {
    const out = settleBatch(
      ['a.ts'],
      batch({ proposals: [{ path: 'a.ts', proposedText: CLEAN, costUsd: null }] }),
      'autoResolve',
    );
    expect(out.stageable.map((f) => f.path)).toEqual(['a.ts']);
    expect(out.markerful).toEqual([]);
    expect(out.status).toBe('ready');
    expect(out.proposal).toBe(CLEAN);
  });

  it('autoResolve over several files stages ONLY the clean ones', () => {
    const out = settleBatch(
      ['a.ts', 'b.ts', 'c.ts'],
      batch({
        proposals: [
          { path: 'a.ts', proposedText: CLEAN, costUsd: null },
          { path: 'b.ts', proposedText: MARKERFUL, costUsd: null },
          { path: 'c.ts', proposedText: CLEAN, costUsd: null },
        ],
      }),
      'autoResolve',
    );
    expect(out.stageable.map((f) => f.path)).toEqual(['a.ts', 'c.ts']);
    expect(out.markerful.map((f) => f.path)).toEqual(['b.ts']);
    // One bad file never costs the others their result (D11).
    expect(out.status).toBe('ready');
  });

  it('proposeReview never stages anything, so it reports no markerful demotions', () => {
    const out = settleBatch(
      ['a.ts'],
      batch({ proposals: [{ path: 'a.ts', proposedText: MARKERFUL, costUsd: null }] }),
      'proposeReview',
    );
    expect(out.markerful).toEqual([]);
    expect(out.status).toBe('ready');
    // It is offered for REVIEW verbatim — the editor is the gate in this mode.
    expect(out.stageable[0]?.proposal).toBe(MARKERFUL);
  });

  it('a path with no proposal fails individually with the backend reason', () => {
    const out = settleBatch(
      ['a.ts', 'b.ts'],
      batch({
        proposals: [{ path: 'a.ts', proposedText: CLEAN, costUsd: null }],
        failed: [{ path: 'b.ts', reason: 'file is binary' }],
      }),
      'proposeReview',
    );
    expect(out.files[1]).toMatchObject({ path: 'b.ts', status: 'failed', error: 'file is binary' });
    expect(out.status).toBe('ready');
  });

  it('a path missing from BOTH lists still fails, never silently succeeds', () => {
    const out = settleBatch(['a.ts'], batch(), 'proposeReview');
    expect(out.files[0]?.status).toBe('failed');
    expect(out.files[0]?.error).toBe('no result returned for this file');
    expect(out.status).toBe('failed');
  });

  it('an unknown extra path in the response is ignored', () => {
    const out = settleBatch(
      ['a.ts'],
      batch({
        proposals: [
          { path: 'a.ts', proposedText: CLEAN, costUsd: null },
          { path: 'ghost.ts', proposedText: CLEAN, costUsd: null },
        ],
      }),
      'proposeReview',
    );
    expect(out.files.map((f) => f.path)).toEqual(['a.ts']);
  });

  it('a bulk run exposes no single `proposal` (that is a 1-path concept)', () => {
    const out = settleBatch(
      ['a.ts', 'b.ts'],
      batch({
        proposals: [
          { path: 'a.ts', proposedText: CLEAN, costUsd: null },
          { path: 'b.ts', proposedText: CLEAN, costUsd: null },
        ],
      }),
      'proposeReview',
    );
    expect(out.proposal).toBeNull();
  });
});

describe('deriveRowStates', () => {
  function run(over: Partial<AiRunState>): AiRunState {
    return { ...newRun(over.key ?? 'k', 'l', over.paths ?? ['a.ts'], 1000), ...over };
  }

  it('elapsed counts to `now` while live and to `endedAt` once terminal', () => {
    const rows = deriveRowStates(
      {
        live: run({ key: 'live', paths: ['a.ts'] }),
        done: run({ key: 'done', paths: ['b.ts'], status: 'ready', endedAt: 4000 }),
      },
      9000,
    );
    expect(rows['a.ts']?.elapsedSecs).toBe(8);
    expect(rows['b.ts']?.elapsedSecs).toBe(3);
  });

  it('a per-file failure inside a ready batch shows on ITS row only', () => {
    const rows = deriveRowStates(
      {
        bulk: run({
          key: 'bulk',
          paths: ['a.ts', 'b.ts'],
          status: 'ready',
          endedAt: 2000,
          files: [
            { path: 'a.ts', status: 'ready', proposal: CLEAN, error: null },
            { path: 'b.ts', status: 'failed', proposal: null, error: 'no result block' },
          ],
        }),
      },
      2000,
    );
    expect(rows['a.ts']?.status).toBe('ready');
    expect(rows['b.ts']?.status).toBe('failed');
    expect(rows['b.ts']?.error).toBe('no result block');
  });

  it('a LIVE run wins over a terminal one covering the same path', () => {
    const rows = deriveRowStates(
      {
        old: run({ key: 'old', paths: ['a.ts'], status: 'failed', endedAt: 2000 }),
        retry: run({ key: 'retry', paths: ['a.ts'] }),
      },
      3000,
    );
    expect(rows['a.ts']?.status).toBe('running');
    expect(rows['a.ts']?.key).toBe('retry');
  });
});

describe('pruneRuns', () => {
  function terminal(key: string, paths: string[], endedAt: number): AiRunState {
    return { ...newRun(key, key, paths, endedAt - 10), status: 'ready', endedAt };
  }

  it('drops a terminal run once none of its paths is conflicted', () => {
    const runs = { 'conflict:a.ts': terminal('conflict:a.ts', ['a.ts'], 1) };
    expect(pruneRuns(runs, ['a.ts'])).toBeNull();
    const pruned = pruneRuns(runs, ['b.ts']);
    expect(pruned?.dropped).toEqual(['conflict:a.ts']);
    expect(pruned?.kept).toEqual({});
  });

  it('never drops a LIVE run, even when its path left the conflicts list', () => {
    const runs = { 'conflict:a.ts': newRun('conflict:a.ts', 'a.ts', ['a.ts'], 0) };
    expect(pruneRuns(runs, [])).toBeNull();
  });

  it('caps retained terminal runs, oldest first', () => {
    const runs: Record<string, AiRunState> = {};
    const paths: string[] = [];
    for (let i = 0; i < AI_TERMINAL_RUNS_MAX + 3; i++) {
      const key = `conflict:f${i}.ts`;
      runs[key] = terminal(key, [`f${i}.ts`], 100 + i);
      paths.push(`f${i}.ts`); // all still conflicted, so only the cap can drop them
    }
    const pruned = pruneRuns(runs, paths);
    expect(pruned?.dropped).toEqual([
      'conflict:f0.ts',
      'conflict:f1.ts',
      'conflict:f2.ts',
    ]);
    expect(Object.keys(pruned?.kept ?? {})).toHaveLength(AI_TERMINAL_RUNS_MAX);
  });
});

describe('decideEvent — the §5.2 mapping table', () => {
  const live = newRun('conflict:a.ts', 'a.ts', ['a.ts'], 0);

  it('ignores a stale or duplicate seq', () => {
    const seen = { ...live, lastSeq: 5 };
    expect(decideEvent(seen, ev({ seq: 5, kind: 'log', text: 'x' }), 0).patch).toEqual({});
    expect(decideEvent(seen, ev({ seq: 3, kind: 'failed' }), 0).patch).toEqual({});
  });

  it('never resurrects a terminal run, but still records a late log line (D2)', () => {
    const done = { ...live, status: 'cancelled' as const };
    expect(decideEvent(done, ev({ seq: 1, kind: 'awaitingInput', text: 'q' }), 0).patch).toEqual({});
    expect(decideEvent(done, ev({ seq: 1, kind: 'log', text: 'tail' }), 0).logLine?.text).toBe(
      'tail',
    );
  });

  it('started carries the runId and fires a queued cancel (D8)', () => {
    const plain = decideEvent(live, ev({ seq: 0, kind: 'started', runId: 'ai-1' }), 0);
    expect(plain.patch).toMatchObject({ runId: 'ai-1' });
    expect(plain.fireQueuedCancel).toBeNull();
    const queued = decideEvent(
      { ...live, cancelRequested: true },
      ev({ seq: 0, kind: 'started', runId: 'ai-1' }),
      0,
    );
    expect(queued.fireQueuedCancel).toBe('ai-1');
  });

  it('a text log line is buffered and classified; it never flushes immediately (D5)', () => {
    const d = decideEvent(live, ev({ seq: 1, kind: 'log', text: '⚙ Read(a.ts)' }), 0);
    expect(d.logLine).toEqual({ seq: 1, text: '⚙ Read(a.ts)', kind: 'tool' });
    expect(d.flushNow).toBe(false);
  });

  it('a textless log event is metrics-only: tokens, no line (A4)', () => {
    const d = decideEvent(live, ev({ seq: 1, kind: 'log', text: null, thinkingTokens: 350 }), 0);
    expect(d.thinkingTokens).toBe(350);
    expect(d.logLine).toBeNull();
    expect(d.flushNow).toBe(false);
  });

  it('turnEnd keeps a null cost rather than clobbering the last known value (A10)', () => {
    const withCost = { ...live, costUsd: 0.02 };
    const d = decideEvent(withCost, ev({ seq: 1, kind: 'turnEnd', costUsd: null, turn: 2 }), 0);
    expect(d.patch).toMatchObject({ costUsd: 0.02, turn: 2 });
  });

  it('terminal events stamp endedAt from the injected clock and flush at once', () => {
    const failed = decideEvent(
      live,
      ev({ seq: 1, kind: 'failed', text: 'watchdog', partialText: 'half' }),
      7777,
    );
    expect(failed.patch).toMatchObject({
      status: 'failed',
      error: 'watchdog',
      partialText: 'half',
      endedAt: 7777,
    });
    expect(failed.flushNow).toBe(true);
  });
});
