/** P87b FU-1 §9.8 — the MOCK's half of the run-target guarantees.
 *
 *  The mock is a second backend: it never runs Rust, so §3.10's `?gitBidiTarget`
 *  seam can only prove the contract is *modeled* if the mock applies the same
 *  funnel (run-target §6.1, flag F-2). These tests pin both halves of that
 *  mirror:
 *
 *   1. `mockActivityTarget` == `ActivityTarget::new` — C0/C1 + bidi + zero-width
 *      stripped, trimmed, empty → null, capped at 255 CHARS with a trailing `…`.
 *   2. Guarantee 1 (raw identifier, never a phrase) for every target the mock can
 *      emit: the seven call-site fixtures AND the two seam fixtures carry no
 *      space, arrow or quote.
 *
 *  Every invisible char below is written as an ESCAPE, never the literal
 *  character — a literal bidi override in a source file reorders the code around
 *  it, which is the whole reason for stripping it.
 *
 *  Own file, like `handlers/amendActivity.test.tsx`: `subscribeGitActivity`
 *  registers a process-wide listener the mock never unregisters, which would
 *  switch `runMockActivity` from passthrough to sequenced for every other test
 *  sharing the module graph.
 */
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { freshRepoPath, run } from '../../test/mockIpcKit';
import {
  MOCK_BIDI_TARGET,
  MOCK_LONG_TARGET,
  mockActivityTarget,
  subscribeGitActivity,
} from './gitActivity';
import { mergeHandlers } from './handlers/merge';
import { remotesSyncHandlers } from './handlers/remotesSync';
import { repoHandlers } from './handlers/repo';
import { stashHandlers } from './handlers/stash';
import { statusHandlers } from './handlers/status';
import type { GitActivityEvent } from '../types';

beforeAll(() => vi.useFakeTimers());
afterAll(() => vi.useRealTimers());

const events: GitActivityEvent[] = [];
subscribeGitActivity((e) => events.push(e));

/** `[category, target]` for every `started` collected so far. */
function startedTargets(): Array<[string | undefined, string | undefined]> {
  return events.filter((e) => e.kind === 'started').map((e) => [e.category, e.target]);
}

/** Every non-absent `target` currently on the collected wire. */
function wireTargets(): string[] {
  const out: string[] = [];
  for (const e of events) {
    if (e.kind === 'started' && e.target !== undefined) out.push(e.target);
  }
  return out;
}

/** Drive `op` and return the `started` events it produced. Deliberately
 *  outcome-agnostic: `runMockActivity` emits `started` BEFORE the inner body
 *  runs, so the target is observable even for an op that rejects (merge-commit
 *  with no merge in progress). */
async function startedFor(
  op: () => Promise<unknown>,
): Promise<Array<[string | undefined, string | undefined]>> {
  events.length = 0;
  const settled = op().then(
    () => undefined,
    () => undefined,
  );
  await vi.advanceTimersByTimeAsync(10_000);
  await settled;
  return startedTargets();
}

describe('mockActivityTarget — MIRRORS ActivityTarget::new', () => {
  it('passes a clean ref through untouched', () => {
    expect(mockActivityTarget('origin/main')).toBe('origin/main');
    expect(mockActivityTarget('feature/api/retry-budget')).toBe('feature/api/retry-budget');
  });

  it('strips the bidi override — the §9.9 harness proof, as a unit test', () => {
    expect(mockActivityTarget(MOCK_BIDI_TARGET)).toBe('origin/main');
    expect(mockActivityTarget('origin/ma\u{202e}in')).toBe('origin/main');
    expect(mockActivityTarget('origin/\u{202a}ma\u{202c}in')).toBe('origin/main');
  });

  it('strips zero-width chars, LRM/RLM, bidi isolates, the BOM, and C0/C1', () => {
    expect(mockActivityTarget('ori\u{200b}gin/m\u{200c}ai\u{200d}n')).toBe('origin/main');
    expect(mockActivityTarget('origin\u{200e}/\u{200f}main')).toBe('origin/main');
    expect(mockActivityTarget('origin/\u{2066}ma\u{2069}in')).toBe('origin/main');
    expect(mockActivityTarget('\u{feff}origin/main')).toBe('origin/main');
    expect(mockActivityTarget('origin/ma\nin')).toBe('origin/main');
    expect(mockActivityTarget('origin/ma\tin')).toBe('origin/main');
    expect(mockActivityTarget('origin/ma\u{0085}in')).toBe('origin/main');
  });

  it('trims, and returns null when nothing survives', () => {
    expect(mockActivityTarget('  origin/main  ')).toBe('origin/main');
    expect(mockActivityTarget('   ')).toBeNull();
    expect(mockActivityTarget('\u{200b}\u{202e}')).toBeNull();
    expect(mockActivityTarget('')).toBeNull();
    expect(mockActivityTarget(null)).toBeNull();
  });

  it('caps at 255 CHARS, ending in the ellipsis', () => {
    const capped = mockActivityTarget(`origin/${'x'.repeat(400)}`);
    if (capped === null) throw new Error('expected a capped target');
    expect([...capped]).toHaveLength(255);
    expect(capped.endsWith('…')).toBe(true);
  });

  it('leaves a target of exactly the cap alone', () => {
    const exact = 'x'.repeat(255);
    expect(mockActivityTarget(exact)).toBe(exact);
  });
});

describe('fixture guard — guarantee 1 on the mock side (§9.8)', () => {
  it('every op emits a target that is a raw identifier, never a phrase', async () => {
    const { repoId } = await run(repoHandlers.openRepo(freshRepoPath('target')));
    const seen: string[] = [];

    // push / force-push / pull — the configured upstream.
    expect(await startedFor(() => remotesSyncHandlers.push(repoId))).toEqual([
      ['push', 'origin/main'],
    ]);
    seen.push(...wireTargets());
    expect(await startedFor(() => remotesSyncHandlers.forcePush(repoId))).toEqual([
      ['forcePush', 'origin/main'],
    ]);
    seen.push(...wireTargets());
    expect(await startedFor(() => remotesSyncHandlers.pull(repoId))).toEqual([
      ['pull', 'origin/main'],
    ]);
    seen.push(...wireTargets());

    // fetch-all sends NO target — the key is DROPPED, matching serde's
    // `skip_serializing_if`; the frontend derives `all remotes` from that.
    expect(await startedFor(() => remotesSyncHandlers.fetch(repoId))).toEqual([
      ['fetch', undefined],
    ]);

    // commit / amend / merge-commit — the branch SHORT name.
    expect(await startedFor(() => statusHandlers.commit(repoId, 'a commit'))).toEqual([
      ['commit', 'main'],
    ]);
    seen.push(...wireTargets());
    expect(await startedFor(() => stashHandlers.commitAmend(repoId, 'an amend'))).toEqual([
      ['amend', 'main'],
    ]);
    seen.push(...wireTargets());
    // No merge is in progress, so this op rejects — `started` still carried one.
    expect(await startedFor(() => mergeHandlers.commitMerge(repoId, 'a merge'))).toEqual([
      ['mergeCommit', 'main'],
    ]);
    seen.push(...wireTargets());

    // …and every one of them, plus both seam fixtures post-funnel, is bare.
    seen.push(
      mockActivityTarget(MOCK_LONG_TARGET) ?? '',
      mockActivityTarget(MOCK_BIDI_TARGET) ?? '',
    );
    expect(seen).toHaveLength(8);
    for (const target of seen) {
      expect(target).not.toBe('');
      expect(target).not.toContain(' ');
      expect(target).not.toMatch(/['"→]/);
    }
  });

  it('the ?gitLongTarget fixture is long enough to prove the 22ch ellipsis', () => {
    // §3.10/§9.10 want >=90 chars. The contract's own literal is 83, so this one
    // lengthens a MIDDLE segment — the leaf (`retry-budget-tuning`) stays the
    // recognisable part P111's leaf-preserving split exists to protect.
    expect([...MOCK_LONG_TARGET].length).toBeGreaterThanOrEqual(90);
    expect(MOCK_LONG_TARGET.split('/').pop()).toBe('retry-budget-tuning');
    // It survives the funnel unchanged — well under the 255 cap.
    expect(mockActivityTarget(MOCK_LONG_TARGET)).toBe(MOCK_LONG_TARGET);
  });
});
