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
