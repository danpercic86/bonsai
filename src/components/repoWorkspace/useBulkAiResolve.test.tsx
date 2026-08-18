/**
 * P68f §9 — `useBulkAiResolve`: eligibility, the confirm gate, the ONE-run guarantee
 * and cancel-all.
 *
 * The load-bearing assertion of the whole increment is `spy.toHaveBeenCalledTimes(1)`
 * with ALL the paths: bulk is deliberately ONE Claude run over every conflict (D11)
 * so the model can reason across them — the reported case being one logical change
 * split over several i18n JSON files. N single runs would be a different (and worse)
 * feature wearing the same button.
 *
 * The hook is driven together with the real `useAiRuns` rather than a fake, because
 * "one run" and "one cancel" are properties of the pair.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { mockIpc } from '../../ipc/mock';
import { REPO } from '../../test/actionHookKit';
import { batch, gatedStream, makeDeps, neverSettles, stubStream } from '../../test/aiRunsKit';
import { useAiRuns, type AiRunsDeps } from './useAiRuns';
import { useBulkAiResolve, type BulkAiResolveApi } from './useBulkAiResolve';
import type { AiAutonomy, ConflictEntry, ConflictKind } from '../../ipc';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

function conflict(path: string, kind: ConflictKind = 'bothModified'): ConflictEntry {
  return { path, kind, hasBase: true, hasOurs: true, hasTheirs: true };
}

/** The two eligible kinds plus one that is NOT (deletion — no text to merge). */
const THREE: ConflictEntry[] = [
  conflict('src/auth.ts'),
  conflict('src/locales/de.json'),
  conflict('README.md', 'deletedByThem'),
];

interface Harness {
  bulk: BulkAiResolveApi;
  runs: ReturnType<typeof useAiRuns>;
}

function renderBulk(
  conflicts: ConflictEntry[] = THREE,
  over: { aiEligible?: boolean; autonomy?: AiAutonomy; deps?: Partial<AiRunsDeps> } = {},
) {
  const deps = makeDeps({ conflictPaths: conflicts.map((c) => c.path), ...over.deps });
  const hook = renderHook((): Harness => {
    const runs = useAiRuns(deps);
    return {
      runs,
      bulk: useBulkAiResolve({
        conflicts,
        aiEligible: over.aiEligible ?? true,
        aiConflictAutonomy: over.autonomy ?? 'proposeReview',
        aiRuns: runs,
      }),
    };
  });
  return { ...hook, deps };
}

describe('useBulkAiResolve — eligibility', () => {
  it('offers itself only for the two AI-mergeable kinds, and only from two of them', () => {
    const { result } = renderBulk();
    expect(result.current.bulk.control.shown).toBe(true);
    // `deletedByThem` has no text to merge — the same `aiShown` gate the row uses.
    expect(result.current.bulk.control.paths).toEqual(['src/auth.ts', 'src/locales/de.json']);
    expect(result.current.bulk.control.count).toBe(2);
  });

  it('is hidden with a single eligible conflict (its row button already covers it)', () => {
    const { result } = renderBulk([conflict('src/auth.ts'), conflict('README.md', 'bothDeleted')]);
    expect(result.current.bulk.control.shown).toBe(false);
  });

  it('is disabled but still shown when AI is not eligible, with the row copy', () => {
    const { result } = renderBulk(THREE, { aiEligible: false });
    expect(result.current.bulk.control.shown).toBe(true);
    expect(result.current.bulk.control.disabled).toBe(true);
    expect(result.current.bulk.control.title).toBe(
      'Enable AI features in Settings to use this',
    );
  });
});

describe('useBulkAiResolve — the confirm gate and the ONE run', () => {
  it('clicking arms the confirm and starts NOTHING', () => {
    const spy = vi.spyOn(mockIpc, 'aiResolveConflictStream');
    const { result } = renderBulk();
    act(() => result.current.bulk.control.onClick());
    expect(spy).not.toHaveBeenCalled();
    expect(result.current.bulk.confirm.open).toBe(true);
    expect(result.current.bulk.confirm.paths).toEqual([
      'src/auth.ts',
      'src/locales/de.json',
    ]);
  });

  it('cancelling the confirm starts nothing and closes it', () => {
    const spy = vi.spyOn(mockIpc, 'aiResolveConflictStream');
    const { result } = renderBulk();
    act(() => result.current.bulk.control.onClick());
    act(() => result.current.bulk.confirm.onCancel());
    expect(result.current.bulk.confirm.open).toBe(false);
    expect(spy).not.toHaveBeenCalled();
  });

  it('confirming issues EXACTLY ONE stream call with ALL eligible paths', () => {
    const spy = neverSettles();
    const { result } = renderBulk();
    act(() => result.current.bulk.control.onClick());
    act(() => result.current.bulk.confirm.onConfirm());
    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy).toHaveBeenCalledWith(
      REPO,
      ['src/auth.ts', 'src/locales/de.json'],
      expect.any(Function),
    );
    // One store entry for the whole batch, with a per-file row each.
    const keys = Object.keys(result.current.runs.runs);
    expect(keys).toHaveLength(1);
    expect(result.current.runs.runs[keys[0] ?? '']?.files.map((f) => f.path)).toEqual([
      'src/auth.ts',
      'src/locales/de.json',
    ]);
  });

  it('the ineligible kind is never part of the run', () => {
    const spy = neverSettles();
    const { result } = renderBulk();
    act(() => result.current.bulk.control.onClick());
    act(() => result.current.bulk.confirm.onConfirm());
    const paths = spy.mock.calls[0]?.[1] ?? [];
    expect(paths).not.toContain('README.md');
  });

  it('refuses to start at the concurrency cap', () => {
    const spy = neverSettles();
    const { result } = renderBulk();
    // Fill the cap with single runs.
    act(() => {
      result.current.runs.startConflictRun('a.ts');
      result.current.runs.startConflictRun('b.ts');
      result.current.runs.startConflictRun('c.ts');
    });
    expect(result.current.runs.atCapacity).toBe(true);
    expect(result.current.bulk.control.disabled).toBe(true);
    expect(result.current.bulk.control.title).toContain('Too many AI runs');
    spy.mockClear();
    act(() => result.current.bulk.control.onClick());
    expect(result.current.bulk.confirm.open).toBe(true);
    act(() => result.current.bulk.confirm.onConfirm());
    // The store's own cap check is the backstop if the button is bypassed.
    expect(spy).not.toHaveBeenCalled();
  });
});

describe('useBulkAiResolve — cancel-all is ONE cancel', () => {
  it('turns into Cancel all while live and issues a single ai_cancel_run', () => {
    const stream = stubStream();
    const cancel = vi.spyOn(mockIpc, 'aiCancelRun').mockResolvedValue(undefined);
    const { result } = renderBulk();
    act(() => result.current.bulk.control.onClick());
    act(() => result.current.bulk.confirm.onConfirm());
    // The runId only arrives on the first event (D8).
    act(() =>
      stream.send({
        runId: 'run-bulk',
        seq: 0,
        kind: 'started',
        text: null,
        costUsd: null,
        elapsedMs: 0,
        path: null,
        turn: 0,
        partialText: null,
        thinkingTokens: null,
      }),
    );

    expect(result.current.bulk.control.active).toBe(true);
    expect(result.current.bulk.control.label).toBe('Cancel all');
    expect(result.current.bulk.control.ariaLabel).toBe('Cancel the AI run for all 2 files');

    act(() => result.current.bulk.control.onClick());
    // ONE run covers N files ⇒ ONE cancel, not one per file.
    expect(cancel).toHaveBeenCalledTimes(1);
    expect(cancel).toHaveBeenCalledWith('run-bulk');
    // And the button says so immediately, before any IPC settles.
    expect(result.current.bulk.control.label).toBe('Stopping…');
    expect(result.current.bulk.control.disabled).toBe(true);
  });

  it('stays visible while the run drains the conflicts list under it', async () => {
    const gate = gatedStream();
    const conflicts = [conflict('src/auth.ts'), conflict('src/locales/de.json')];
    const { result, rerender } = renderHook(
      (props: { conflicts: ConflictEntry[] }): Harness => {
        const runs = useAiRuns(makeDeps({ conflictPaths: props.conflicts.map((c) => c.path) }));
        return {
          runs,
          bulk: useBulkAiResolve({
            conflicts: props.conflicts,
            aiEligible: true,
            aiConflictAutonomy: 'proposeReview',
            aiRuns: runs,
          }),
        };
      },
      { initialProps: { conflicts } },
    );
    act(() => result.current.bulk.control.onClick());
    act(() => result.current.bulk.confirm.onConfirm());
    // One file got resolved mid-run: a `paths.length >= 2` test alone would now hide
    // the button and trap the user with a run they cannot stop.
    act(() => rerender({ conflicts: [conflict('src/auth.ts')] }));
    expect(result.current.bulk.control.shown).toBe(true);
    expect(result.current.bulk.control.active).toBe(true);
    expect(result.current.bulk.control.count).toBe(2);

    await act(async () => {
      gate.resolve(batch({ proposals: [], failed: [] }));
      await Promise.resolve();
    });
    // Terminal ⇒ no longer "active"; with one eligible conflict left it hides again.
    expect(result.current.bulk.control.active).toBe(false);
    expect(result.current.bulk.control.shown).toBe(false);
  });
});
