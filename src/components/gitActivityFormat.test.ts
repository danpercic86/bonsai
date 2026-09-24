/** P87b — pure formatter tests: the phase→copy table, pills, readouts, duration,
 *  and the announcer transition table. */
import { describe, expect, it } from 'vitest';

import {
  categoryMeta,
  durationLabel,
  formatBytes,
  gitAnnounceFor,
  hookPill,
  objectsReadout,
  phaseLabel,
  progressFraction,
  runRowName,
  runTarget,
  statusPill,
} from './gitActivityFormat';
import type { GitActivityCategory, GitPhase, GitTransferProgress } from '../ipc';
import type { GitActivityRun } from './repoWorkspace/useGitActivity';

function run(over: Partial<GitActivityRun> = {}): GitActivityRun {
  return {
    id: over.id ?? 'r1',
    category: over.category ?? 'push',
    phase: over.phase ?? { kind: 'network' },
    status: over.status ?? 'running',
    code: over.code ?? null,
    startedAt: over.startedAt ?? 1000,
    endedAt: over.endedAt ?? null,
    progress: over.progress ?? null,
    hooks: over.hooks ?? [],
    lines: over.lines ?? [],
    linesDropped: over.linesDropped ?? 0,
    seq: over.seq ?? 0,
    target: over.target ?? null,
    targetCount: over.targetCount ?? null,
    outcome: over.outcome ?? null,
  };
}

describe('phaseLabel — the §1 LOCKED table', () => {
  const cases: Array<[GitActivityCategory, GitPhase, string]> = [
    ['push', { kind: 'preparing' }, 'Preparing…'],
    ['push', { kind: 'runningHook', hook: 'pre-push' }, 'Running pre-push hook…'],
    ['push', { kind: 'network' }, 'Sending objects…'],
    ['forcePush', { kind: 'runningHook', hook: 'pre-push' }, 'Running pre-push hook…'],
    ['forcePush', { kind: 'network' }, 'Force-pushing…'],
    ['fetch', { kind: 'network' }, 'Fetching…'],
    ['pull', { kind: 'network' }, 'Fetching…'],
    ['pull', { kind: 'finalizing' }, 'Pulling…'],
    ['commit', { kind: 'runningHook', hook: 'pre-commit' }, 'Running pre-commit hook…'],
    ['commit', { kind: 'runningHook', hook: 'commit-msg' }, 'Running commit-msg hook…'],
    ['commit', { kind: 'runningHook', hook: 'post-commit' }, 'Running post-commit hook…'],
    ['commit', { kind: 'finalizing' }, 'Writing commit…'],
    ['amend', { kind: 'finalizing' }, 'Amending…'],
    ['mergeCommit', { kind: 'finalizing' }, 'Writing merge commit…'],
    // generic fallbacks
    ['commit', { kind: 'runningHook', hook: 'my-hook' }, 'Running my-hook hook…'],
    ['commit', { kind: 'network' }, 'Working…'],
    ['fetch', { kind: 'runningHook' }, 'Working…'],
  ];
  it.each(cases)('%s / %o → %s', (category, phase, expected) => {
    expect(phaseLabel(category, phase)).toBe(expected);
  });
});

describe('categoryMeta', () => {
  it('gives layout-stable participles + nouns', () => {
    expect(categoryMeta('push').participle).toBe('Pushing…');
    expect(categoryMeta('forcePush').participle).toBe('Force-pushing…');
    expect(categoryMeta('fetch').participle).toBe('Fetching…');
    expect(categoryMeta('pull').participle).toBe('Pulling…');
    expect(categoryMeta('mergeCommit').noun).toBe('Merge commit');
    expect(categoryMeta('push').verb).toBe('Push');
  });
});

describe('status + hook pills (word + glyph, never colour alone)', () => {
  it('run pills', () => {
    expect(statusPill('running')).toMatchObject({ glyph: '●', label: 'Running' });
    expect(statusPill('success')).toMatchObject({ glyph: '✓', label: 'Success' });
    expect(statusPill('failed')).toMatchObject({ glyph: '⚠', label: 'Failed' });
  });
  it('hook verdict pills carry the exit code in the label', () => {
    expect(hookPill(0, true)).toMatchObject({ glyph: '✓', label: 'exit 0' });
    expect(hookPill(1, false)).toMatchObject({ glyph: '⚠', label: 'exit 1' });
    expect(hookPill(null, false)).toMatchObject({ glyph: '⊘', label: 'killed' });
  });
});

describe('progress readouts (§2.3/§14.10)', () => {
  const counts: GitTransferProgress = {
    receivedObjects: 12_340,
    totalObjects: 50_000,
    indexedObjects: 12_340,
    receivedBytes: 1_000_000,
  };
  it('objectsReadout — objects when totals known', () => {
    // Locale-independent: the formatter uses toLocaleString (correct for i18n),
    // so mirror it here rather than hard-coding US thousands separators.
    const expected = `${(12_340).toLocaleString()} / ${(50_000).toLocaleString()} objects`;
    expect(objectsReadout(run({ progress: counts }))).toBe(expected);
  });
  it('objectsReadout — byte fallback', () => {
    const p: GitTransferProgress = {
      receivedObjects: 0,
      totalObjects: 0,
      indexedObjects: 0,
      receivedBytes: 4_404_019,
    };
    expect(objectsReadout(run({ progress: p }))).toBe('4.2 MB received');
  });
  it('objectsReadout — null when nothing known', () => {
    expect(objectsReadout(run({ progress: null }))).toBeNull();
    const empty: GitTransferProgress = {
      receivedObjects: 0,
      totalObjects: 0,
      indexedObjects: 0,
      receivedBytes: 0,
    };
    expect(objectsReadout(run({ progress: empty }))).toBeNull();
  });
  it('progressFraction guards totalObjects === 0', () => {
    expect(progressFraction(run({ progress: counts }))).toBeCloseTo(0.2468, 4);
    expect(progressFraction(run({ progress: { ...counts, totalObjects: 0 } }))).toBeNull();
    expect(progressFraction(run({ progress: null }))).toBeNull();
  });
});

describe('formatBytes', () => {
  it('scales base-1024 with one decimal past KB', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(500)).toBe('500 B');
    expect(formatBytes(4_404_019)).toBe('4.2 MB');
  });
});

describe('durationLabel', () => {
  it('shows tenths under a minute and m:ss past it', () => {
    expect(durationLabel(run({ startedAt: 1000, endedAt: 3400 }), 0)).toBe('2.4s');
    expect(durationLabel(run({ startedAt: 1000, endedAt: null }), 66_000)).toBe('1:05');
  });
});

describe('gitAnnounceFor — phase transitions + terminal only (§6)', () => {
  it('announces meaningful phase changes, skips preparing, dedupes, and reports terminals', () => {
    const seen = new Map<string, string>();
    // preparing is NOT announced.
    expect(gitAnnounceFor([run({ phase: { kind: 'preparing' } })], seen)).toBeNull();
    // a network phase announces the stripped phase label.
    expect(gitAnnounceFor([run({ phase: { kind: 'network' } })], seen)).toBe('Sending objects');
    // the same phase again → nothing new.
    expect(gitAnnounceFor([run({ phase: { kind: 'network' } })], seen)).toBeNull();
    // terminal success → verb sentence.
    expect(gitAnnounceFor([run({ status: 'success', endedAt: 2000 })], seen)).toBe(
      'Push finished — success',
    );
    // failed run of a fresh id.
    const seen2 = new Map<string, string>();
    expect(gitAnnounceFor([run({ id: 'r2', status: 'failed', endedAt: 2000 })], seen2)).toBe(
      'Push failed',
    );
  });
});

// FU-1 §3.3/§3.7 — the run target: the visible string table, the accessible
// name, and the announcer's terminal sentences.
//
// Timestamps are built with `new Date(y, m, d, h, m)` (LOCAL), never epoch
// literals, because `timeLabel` renders local time and an epoch literal would
// make these assertions timezone-dependent.
function atLocal(hour: number, minute: number): number {
  return new Date(2026, 0, 1, hour, minute, 0).getTime();
}

describe('runTarget — the §3.3 string table', () => {
  const cases: Array<[GitActivityCategory, string | null, string | null]> = [
    ['push', 'origin/main', 'origin/main'],
    ['push', 'origin/feature/x', 'origin/feature/x'],
    // push where the remote is known but the branch is not.
    ['push', 'origin', 'origin'],
    ['forcePush', 'origin/main', 'origin/main'],
    ['pull', 'origin/main', 'origin/main'],
    // fetch of ONE remote (mock-only today — there is no per-remote backend op).
    ['fetch', 'origin', 'origin'],
    // fetch-all: the ONLY derived string in the table.
    ['fetch', null, 'all remotes'],
    ['commit', 'main', 'main'],
    // commit on a detached / unborn HEAD: no target, and NO placeholder.
    ['commit', null, null],
    ['amend', 'main', 'main'],
    ['mergeCommit', 'main', 'main'],
    // an old event / a mock without the knob, for every non-fetch category.
    ['push', null, null],
    ['forcePush', null, null],
    ['pull', null, null],
    ['amend', null, null],
    ['mergeCommit', null, null],
  ];
  it.each(cases)('%s + %o → %o', (category, target, expected) => {
    expect(runTarget(run({ category, target }))).toBe(expected);
  });

  it('never invents a placeholder and never adds an arrow, quote or preposition', () => {
    for (const [category, target] of cases) {
      const out = runTarget(run({ category, target }));
      if (out === null) continue;
      if (category === 'fetch' && target === null) continue; // the one derived phrase
      expect(out).not.toMatch(/['"→]/);
      expect(out).not.toContain(' ');
    }
  });

  it('does NOT re-sanitize — a backend regression must surface, not be masked', () => {
    // A bidi override that reached the store means the BACKEND funnel broke; the
    // formatter is pure and passes it straight through so the harness sees it.
    // Written as the ESCAPE, never the literal char: a literal U+202E reorders
    // this source line in every editor and review diff (run-target §8).
    const dirty = 'origin/ma\u{202e}in';
    expect(runTarget(run({ category: 'push', target: dirty }))).toBe(dirty);
  });
});

describe('runRowName — the §3.7 accessible-name table', () => {
  it.each([
    [
      'push, success',
      run({
        category: 'push',
        target: 'origin/main',
        status: 'success',
        startedAt: atLocal(14, 32) - 1_200,
        endedAt: atLocal(14, 32),
      }),
      'Push to origin/main — success, 1.2 seconds, 14:32',
    ],
    [
      'fetch-all, success',
      run({
        category: 'fetch',
        target: null,
        status: 'success',
        startedAt: atLocal(14, 31) - 800,
        endedAt: atLocal(14, 31),
      }),
      'Fetch from all remotes — success, 0.8 seconds, 14:31',
    ],
    [
      'commit on a branch',
      run({
        category: 'commit',
        target: 'feature/api/retry-budget',
        status: 'success',
        startedAt: atLocal(14, 30) - 200,
        endedAt: atLocal(14, 30),
      }),
      'Commit on feature/api/retry-budget — success, 0.2 seconds, 14:30',
    ],
    [
      'push, running',
      run({
        category: 'push',
        target: 'origin/main',
        status: 'running',
        phase: { kind: 'network' },
        startedAt: 0,
        endedAt: null,
      }),
      'Push to origin/main — running, sending objects, 2.4 seconds',
    ],
    [
      'push, running a hook',
      run({
        category: 'push',
        target: 'origin/main',
        status: 'running',
        phase: { kind: 'runningHook', hook: 'pre-push' },
        startedAt: 2_000,
        endedAt: null,
      }),
      // §3.7-1: the pill already said `running`, so the phase clause drops its
      // own leading `running ` — never `running, running pre-push hook`.
      'Push to origin/main — running, pre-push hook, 0.4 seconds',
    ],
    [
      'push, running, preparing',
      run({
        category: 'push',
        target: 'origin/main',
        status: 'running',
        phase: { kind: 'preparing' },
        startedAt: 2_300,
        endedAt: null,
      }),
      'Push to origin/main — running, preparing, 0.1 seconds',
    ],
    [
      'push, running, unknown phase',
      run({
        category: 'push',
        target: 'origin/main',
        status: 'running',
        // No hook name ⇒ the generic `Working…` fallback; it proves the
        // `running ` strip is anchored and does not touch other labels.
        phase: { kind: 'runningHook' },
        startedAt: 2_300,
        endedAt: null,
      }),
      'Push to origin/main — running, working, 0.1 seconds',
    ],
    [
      'commit, detached HEAD',
      run({
        category: 'commit',
        target: null,
        status: 'success',
        startedAt: atLocal(14, 30) - 200,
        endedAt: atLocal(14, 30),
      }),
      'Commit — success, 0.2 seconds, 14:30',
    ],
    [
      'failed push with a blocking hook',
      run({
        category: 'push',
        target: 'origin/main',
        status: 'failed',
        startedAt: atLocal(14, 29) - 900,
        endedAt: atLocal(14, 29),
      }),
      'Push to origin/main — failed, 0.9 seconds, 14:29',
    ],
  ])('%s', (_label, r, expected) => {
    expect(runRowName(r, 2_400)).toBe(expected);
  });

  it('omits the `⋯ trimmed` chip (§3.1 shows the chip; §3.7 names that row without it)', () => {
    const r = run({
      category: 'push',
      target: 'origin/main',
      status: 'success',
      linesDropped: 120,
      startedAt: atLocal(14, 32) - 1_200,
      endedAt: atLocal(14, 32),
    });
    expect(runRowName(r, 0)).toBe('Push to origin/main — success, 1.2 seconds, 14:32');
  });

  it('spells a minute-plus elapsed out in words (`2:05` would read as a clock time)', () => {
    const r = run({
      category: 'push',
      target: 'origin/main',
      status: 'success',
      startedAt: atLocal(14, 32) - 125_000,
      endedAt: atLocal(14, 32),
    });
    expect(runRowName(r, 0)).toBe(
      'Push to origin/main — success, 2 minutes 5 seconds, 14:32',
    );
  });
});

describe('the announcer carries the target on TERMINAL results only (§3.7)', () => {
  it('names the target when finishing and when failing', () => {
    const seen = new Map<string, string>();
    expect(
      gitAnnounceFor(
        [run({ id: 'a', target: 'origin/main', status: 'success', endedAt: 2000 })],
        seen,
      ),
    ).toBe('Push to origin/main finished — success');
    expect(
      gitAnnounceFor(
        [run({ id: 'b', target: 'origin/main', status: 'failed', endedAt: 2000 })],
        seen,
      ),
    ).toBe('Push to origin/main failed');
    expect(
      gitAnnounceFor(
        [run({ id: 'c', category: 'fetch', target: null, status: 'success', endedAt: 2000 })],
        seen,
      ),
    ).toBe('Fetch from all remotes finished — success');
  });

  it('leaves phase transitions BARE — repeating the target on every phase is hostile', () => {
    const seen = new Map<string, string>();
    expect(
      gitAnnounceFor(
        [run({ id: 'd', target: 'origin/main', phase: { kind: 'network' } })],
        seen,
      ),
    ).toBe('Sending objects');
  });
});
