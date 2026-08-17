/** P68c — the streaming AI mock (`handlers/aiStream.ts`) and the ten new
 *  AI-run settings.
 *
 *  Every seam here (`?aiSlow`, `?aiAsk`, `?aiFail`, `?ai=off`) is read at MODULE
 *  INIT, so each case re-imports the module graph under a rewritten location — the
 *  `urlSeams.test.tsx` pattern. The point of the suite is event ORDERING and state
 *  transitions (gap-free `seq`, the awaiting-input round trip, cancel, per-file
 *  attribution); smoothness is native-only (the harness pane pauses rAF). */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { freshRepoPath, run, runErr } from '../../../test/mockIpcKit';
import type { AiRunEvent, AiRunEventKind, AppError, ConflictEntry } from '../../types';

beforeEach(() => {
  vi.useFakeTimers();
  window.localStorage.clear();
});
afterEach(() => {
  vi.useRealTimers();
  vi.resetModules();
  window.history.replaceState({}, '', '/');
});

const AUTH = 'src/auth.ts';
const EXTRA = 'src/extra.ts';

/** Reload the mock module graph under `search`, open a repo and pause it in the
 *  merge-conflict fixture (`src/auth.ts` bothModified + `README.md`
 *  deletedByThem). `extra` adds further eligible paths for the bulk cases. */
async function loadWith(search: string, extra: string[] = []) {
  vi.resetModules();
  window.history.replaceState({}, '', search === '' ? '/' : `/?${search}`);
  const repo = (await import('./repo')).repoHandlers;
  const merge = (await import('./merge')).mergeHandlers;
  const { requireRepo } = await import('../repoState');
  const stream = (await import('./aiStream')).aiStreamHandlers;
  const { repoId } = await run(repo.openRepo(freshRepoPath('aistream')));
  await run(merge.mergeBranch(repoId, 'demo-conflict'));
  const state = requireRepo(repoId);
  for (const path of extra) {
    const entry: ConflictEntry = {
      path,
      kind: 'bothModified',
      hasBase: true,
      hasOurs: true,
      hasTheirs: true,
    };
    state.conflicts.push(entry);
    state.conflictTexts.set(path, {
      path,
      kind: 'bothModified',
      binary: false,
      tooLarge: false,
      missing: false,
      text: '<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\n',
      ours: 'ours\n',
      theirs: 'theirs\n',
    });
  }
  return { repoId, stream };
}

function collector() {
  const events: AiRunEvent[] = [];
  return {
    events,
    onEvent: (e: AiRunEvent) => events.push(e),
    kinds: (): AiRunEventKind[] => events.map((e) => e.kind),
    texts: (): string[] => events.flatMap((e) => (e.text === null ? [] : [e.text])),
  };
}

/** Attach the rejection handler BEFORE advancing timers (no unhandled rejection). */
function guard(p: Promise<unknown>): Promise<AppError> {
  return p.then(
    () => {
      throw new Error('expected the streaming call to reject, but it resolved');
    },
    (e: unknown) => e as AppError,
  );
}

describe('aiResolveConflictStream — the default happy path', () => {
  it('emits started → log+ → turnEnd → done with a gap-free seq and the runId first', async () => {
    const { repoId, stream } = await loadWith('');
    const sink = collector();

    const batch = await run(stream.aiResolveConflictStream(repoId, [AUTH], sink.onEvent));

    expect(sink.kinds()[0]).toBe('started');
    expect(sink.kinds().at(-1)).toBe('done');
    expect(sink.kinds().filter((k) => k === 'started')).toHaveLength(1);
    expect(sink.kinds()).toContain('turnEnd');
    expect(sink.kinds().filter((k) => k === 'log').length).toBeGreaterThan(2);
    // D8: the id arrives on the FIRST event, not as the return value.
    expect(sink.events[0]?.runId).toBe(batch.runId);
    sink.events.forEach((ev, i) => {
      expect(ev.seq).toBe(i);
      expect(ev.runId).toBe(batch.runId);
      expect(typeof ev.elapsedMs).toBe('number');
    });
    // A single-path run attributes every event to its file.
    expect(sink.events.every((e) => e.path === AUTH)).toBe(true);
    // Cost: the run's value on `done`, never a sum.
    expect(sink.events.at(-1)?.costUsd).toBe(0.0263);

    expect(batch.proposals).toHaveLength(1);
    expect(batch.proposals[0]?.path).toBe(AUTH);
    expect(batch.proposals[0]?.proposedText).not.toContain('<<<<<<<');
    expect(batch.failed).toEqual([]);
    expect(batch.turns).toBe(1);
    expect(batch.costUsd).toBe(0.0263);
    // The read-only allowlist is visible in the log (D10).
    expect(sink.texts().some((t) => t.startsWith('⚙ Grep('))).toBe(true);
  });

  it('writes nothing: the conflict is still unresolved afterwards', async () => {
    const { repoId, stream } = await loadWith('');
    const { requireRepo } = await import('../repoState');
    const before = structuredClone(requireRepo(repoId).status.conflicted);
    await run(stream.aiResolveConflictStream(repoId, [AUTH], () => undefined));
    expect(requireRepo(repoId).status.conflicted).toEqual(before);
  });

  it('attributes per file in bulk and fails an ineligible kind individually', async () => {
    const { repoId, stream } = await loadWith('', [EXTRA]);
    const sink = collector();

    const batch = await run(
      stream.aiResolveConflictStream(repoId, [AUTH, 'README.md', EXTRA], sink.onEvent),
    );

    expect(batch.proposals.map((p) => p.path)).toEqual([AUTH, EXTRA]);
    // deletedByThem is not text-mergeable: its own failure, never fatal (D11).
    expect(batch.failed).toEqual([
      { path: 'README.md', reason: 'AI resolution is not available for this file' },
    ]);
    // Per-file attribution: one ⚙ Read line per eligible path, carrying that path.
    for (const path of [AUTH, EXTRA]) {
      expect(sink.events.some((e) => e.path === path && e.text === `⚙ Read(${path})`)).toBe(true);
    }
    // Run-level events of a MULTI-path run are not attributed to any one file.
    expect(sink.events[0]?.path).toBeNull();
    expect(batch.proposals.every((p) => p.costUsd === null)).toBe(true);
  });

  it('rejects an empty path list and a request with no eligible path', async () => {
    const { repoId, stream } = await loadWith('');
    expect((await runErr(stream.aiResolveConflictStream(repoId, [], () => undefined))).kind).toBe(
      'aiFailed',
    );
    const sink = collector();
    const err = await runErr(
      stream.aiResolveConflictStream(repoId, ['README.md'], sink.onEvent),
    );
    expect(err.kind).toBe('aiFailed');
    expect(sink.events).toEqual([]);
  });
});

describe('?aiAsk — the mid-run question round trip', () => {
  it('emits awaitingInput and completes only after aiReplyRun', async () => {
    const { repoId, stream } = await loadWith('aiAsk');
    const sink = collector();

    const pending = stream.aiResolveConflictStream(repoId, [AUTH], sink.onEvent);
    await vi.advanceTimersByTimeAsync(100);

    expect(sink.kinds()).toEqual(['started', 'log', 'log', 'log', 'turnEnd', 'awaitingInput']);
    const asking = sink.events.at(-1);
    expect(asking?.text).toContain('Einträge');
    expect(asking?.turn).toBe(1);
    const runId = sink.events[0]?.runId ?? '';

    // A reply for a run that is NOT awaiting (or is unknown) is refused, never
    // swallowed into a channel nobody reads.
    expect((await runErr(stream.aiReplyRun('mock-run-nope', 'x'))).kind).toBe('aiFailed');

    await run(stream.aiReplyRun(runId, 'Einträge'));
    const batch = await run(pending);

    expect(sink.texts().some((t) => t.startsWith('» answered ('))).toBe(true);
    expect(sink.kinds().at(-1)).toBe('done');
    expect(batch.turns).toBe(2);
    expect(batch.proposals).toHaveLength(1);
    // Still one gap-free sequence across both turns.
    sink.events.forEach((ev, i) => expect(ev.seq).toBe(i));
    // The reply was consumed: a second one has nothing to answer.
    expect((await runErr(stream.aiReplyRun(runId, 'again'))).kind).toBe('aiFailed');
  });
});

describe('?aiSlow + aiCancelRun', () => {
  it('ends in a cancelled event + aiCancelled rejection, keeping the log (D2)', async () => {
    const { repoId, stream } = await loadWith('aiSlow');
    const sink = collector();

    const guarded = guard(stream.aiResolveConflictStream(repoId, [AUTH], sink.onEvent));
    await vi.advanceTimersByTimeAsync(2000);
    const beforeCancel = sink.texts().filter((t) => t.startsWith('analysing…'));
    expect(beforeCancel.length).toBeGreaterThan(0);

    void stream.aiCancelRun(sink.events[0]?.runId ?? '');
    await vi.advanceTimersByTimeAsync(3000);
    const err = await guarded;

    expect(err.kind).toBe('aiCancelled');
    expect(sink.kinds().at(-1)).toBe('cancelled');
    expect(sink.kinds().filter((k) => k === 'cancelled')).toHaveLength(1);
    // D2: everything read before the cancel is still in the caller's hands, and the
    // terminal event echoes the accumulated assistant text (display-only).
    expect(sink.texts().filter((t) => t.startsWith('analysing…'))).toEqual(beforeCancel);
    expect(sink.events.at(-1)?.partialText).toContain('analysing…');
  });

  it('cancels a run that is parked on a question', async () => {
    const { repoId, stream } = await loadWith('aiSlow&aiAsk');
    const sink = collector();

    const guarded = guard(stream.aiResolveConflictStream(repoId, [AUTH], sink.onEvent));
    await vi.advanceTimersByTimeAsync(100);
    expect(sink.kinds()).toContain('awaitingInput');

    void stream.aiCancelRun(sink.events[0]?.runId ?? '');
    await vi.advanceTimersByTimeAsync(1000);

    expect((await guarded).kind).toBe('aiCancelled');
    expect(sink.kinds().at(-1)).toBe('cancelled');
  });

  it('aiCancelRun resolves for an unknown id (idempotent)', async () => {
    const { stream } = await loadWith('');
    await expect(run(stream.aiCancelRun('mock-run-nope'))).resolves.toBeUndefined();
  });
});

describe('?aiFail', () => {
  it('single path ⇒ a failed event and an aiFailed rejection', async () => {
    const { repoId, stream } = await loadWith('aiFail');
    const sink = collector();
    const err = await runErr(stream.aiResolveConflictStream(repoId, [AUTH], sink.onEvent));
    expect(err.kind).toBe('aiFailed');
    expect(sink.kinds().at(-1)).toBe('failed');
    expect(sink.events.at(-1)?.text).toBe(err.message);
    expect(sink.events.at(-1)?.partialText).not.toBeNull();
  });

  it('bulk ⇒ one entry in failed[] and the others in proposals[]', async () => {
    const { repoId, stream } = await loadWith('aiFail', [EXTRA]);
    const sink = collector();
    const batch = await run(
      stream.aiResolveConflictStream(repoId, [AUTH, EXTRA], sink.onEvent),
    );
    expect(batch.proposals.map((p) => p.path)).toEqual([AUTH]);
    expect(batch.failed).toEqual([{ path: EXTRA, reason: 'no result block returned' }]);
    // A per-file failure is NOT fatal to the run.
    expect(sink.kinds().at(-1)).toBe('done');
  });
});

describe('?ai=off and the concurrency cap', () => {
  it('?ai=off ⇒ aiUnavailable with no event and no run', async () => {
    const { repoId, stream } = await loadWith('ai=off');
    const sink = collector();
    const err = await runErr(stream.aiResolveConflictStream(repoId, [AUTH], sink.onEvent));
    expect(err).toEqual({
      kind: 'aiUnavailable',
      message: 'Claude Code CLI not found on PATH',
    });
    expect(sink.events).toEqual([]);
  });

  it('rejects the run past AI_MAX_CONCURRENT_RUNS, then accepts again once one ends', async () => {
    const { repoId, stream } = await loadWith('aiSlow', [EXTRA]);
    const { AI_MAX_CONCURRENT_RUNS } = await import('../../../settings/ranges');
    const live = Array.from({ length: AI_MAX_CONCURRENT_RUNS }, () =>
      collector(),
    ).map((sink) => ({
      sink,
      guarded: guard(stream.aiResolveConflictStream(repoId, [AUTH], sink.onEvent)),
    }));
    await vi.advanceTimersByTimeAsync(100);

    const err = await runErr(stream.aiResolveConflictStream(repoId, [EXTRA], () => undefined));
    expect(err.kind).toBe('aiFailed');
    expect(err.message).toMatch(/^too many AI runs in progress/);

    // Freeing a slot lets the next request through: the guard rejects, never queues.
    void stream.aiCancelRun(live[0]?.sink.events[0]?.runId ?? '');
    await vi.advanceTimersByTimeAsync(3000);
    expect((await live[0]?.guarded)?.kind).toBe('aiCancelled');
    const ok = collector();
    const accepted = guard(stream.aiResolveConflictStream(repoId, [EXTRA], ok.onEvent));
    await vi.advanceTimersByTimeAsync(100);
    expect(ok.kinds()[0]).toBe('started');

    // Drain the still-running runs so nothing leaks into another test.
    for (const entry of [...live.slice(1), { guarded: accepted, sink: ok }]) {
      void stream.aiCancelRun(entry.sink.events[0]?.runId ?? '');
    }
    await vi.advanceTimersByTimeAsync(3000);
    for (const entry of [...live.slice(1), { guarded: accepted }]) {
      expect((await entry.guarded).kind).toBe('aiCancelled');
    }
  });
});

describe('the ten P68 AI-run settings', () => {
  it('getUiSettings returns the §8.3 defaults', async () => {
    vi.resetModules();
    const session = (await import('./session')).sessionHandlers;
    const s = await run(session.getUiSettings());
    expect(s).toMatchObject({
      aiIdleTimeoutSecs: 300,
      // LOCKED: 0 = unbounded (Cancel is the stop mechanism), 0 = no budget flag.
      aiHardCapSecs: 0,
      aiMaxTurns: 6,
      aiStreamLog: true,
      aiIncludePartialMessages: false,
      aiConflictTools: 'readOnly',
      aiBulkMaxBytes: 400_000,
      aiMaxBudgetUsd: 0,
      aiDockHeight: 180,
      aiDockCollapsed: false,
    });
  });

  it('round-trips a patch independently of graph/listView/panelDensity', async () => {
    vi.resetModules();
    const session = (await import('./session')).sessionHandlers;
    const before = await run(session.getUiSettings());
    const next = await run(
      session.setUiSettings({
        aiIdleTimeoutSecs: 600,
        aiConflictTools: 'none',
        aiStreamLog: false,
        aiDockHeight: 320,
        aiDockCollapsed: true,
        aiMaxBudgetUsd: 2.5,
      }),
    );
    expect(next).toMatchObject({
      aiIdleTimeoutSecs: 600,
      aiConflictTools: 'none',
      aiStreamLog: false,
      aiDockHeight: 320,
      aiDockCollapsed: true,
      aiMaxBudgetUsd: 2.5,
    });
    expect(next.graph).toEqual(before.graph);
    expect(next.listView).toBe(before.listView);
    expect(next.panelDensity).toBe(before.panelDensity);
    // Persisted: a reload sees the patch.
    expect(await run(session.getUiSettings())).toMatchObject({ aiIdleTimeoutSecs: 600 });
  });

  it('clamps out-of-range values and keeps the 0 sentinels', async () => {
    vi.resetModules();
    const session = (await import('./session')).sessionHandlers;
    const clamped = await run(
      session.setUiSettings({
        aiIdleTimeoutSecs: 0,
        aiHardCapSecs: 0,
        aiMaxTurns: 999,
        aiBulkMaxBytes: 1,
        aiMaxBudgetUsd: 1e9,
        aiDockHeight: 9_000,
      }),
    );
    expect(clamped).toMatchObject({
      aiIdleTimeoutSecs: 0,
      aiHardCapSecs: 0,
      aiMaxTurns: 20,
      aiBulkMaxBytes: 20_000,
      aiMaxBudgetUsd: 100,
      aiDockHeight: 600,
    });
  });

  it('a pre-P68 stored blob loads every AI-run default', async () => {
    vi.resetModules();
    window.localStorage.setItem(
      'bonsai.mockUiSettings',
      JSON.stringify({ theme: 'light', listView: 'flat' }),
    );
    const session = (await import('./session')).sessionHandlers;
    const s = await run(session.getUiSettings());
    expect(s.theme).toBe('light');
    expect(s).toMatchObject({
      aiIdleTimeoutSecs: 300,
      aiHardCapSecs: 0,
      aiConflictTools: 'readOnly',
      aiDockHeight: 180,
    });
  });
});
