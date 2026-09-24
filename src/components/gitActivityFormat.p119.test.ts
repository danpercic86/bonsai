/**
 * P119-ui §4 — the formatter additions: the `! Conflicts` pill + sentence, the
 * §4.7 outcome detail, the §4.4 count noun, the `<0.1s` duration floor, and the
 * §4.2-4 phase-label rule for the new categories (P87 rows stay locked; those
 * are pinned in `gitActivityFormat.test.ts`).
 */
import { describe, expect, it } from 'vitest';

import {
  dockBarFraction,
  durationLabel,
  gitAnnounceFor,
  outcomeLabel,
  phaseLabel,
  runNoun,
  runOutcomeDetail,
  runRowName,
  statusPill,
} from './gitActivityFormat';
import type { GitActivityRun } from './repoWorkspace/useGitActivity';

function run(over: Partial<GitActivityRun> = {}): GitActivityRun {
  return {
    id: over.id ?? 'r1',
    category: over.category ?? 'merge',
    phase: over.phase ?? { kind: 'preparing' },
    status: over.status ?? 'success',
    code: over.code ?? null,
    startedAt: over.startedAt ?? 1_000,
    endedAt: over.endedAt === undefined ? 1_300 : over.endedAt,
    progress: over.progress ?? null,
    hooks: over.hooks ?? [],
    lines: over.lines ?? [],
    linesDropped: over.linesDropped ?? 0,
    seq: over.seq ?? 0,
    target: over.target === undefined ? 'feature/x' : over.target,
    targetCount: over.targetCount ?? null,
    outcome: over.outcome ?? null,
  };
}

/** `HH:MM` for a timestamp, mirroring `timeLabel` (local time). */
const hhmm = (ms: number): string => {
  const d = new Date(ms);
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
};

describe('conflicts (§4.5)', () => {
  it('pill is `! Conflicts` with its own data-status', () => {
    expect(statusPill('conflicts')).toEqual({
      glyph: '!',
      label: 'Conflicts',
      dataStatus: 'conflicts',
    });
  });

  it('accessible name and announcer; no outcome detail', () => {
    const r = run({ status: 'conflicts', outcome: 'conflicts' });
    expect(runOutcomeDetail(r)).toBeNull();
    expect(runRowName(r, 0)).toBe(`Merge feature/x — conflicts, 0.3 seconds, ${hhmm(1_300)}`);
    expect(gitAnnounceFor([r], new Map())).toBe('Merge feature/x stopped — conflicts to resolve');
  });

  it('the rebase announcer carries its preposition', () => {
    const r = run({ category: 'rebase', target: 'main', status: 'conflicts', outcome: 'conflicts' });
    expect(gitAnnounceFor([r], new Map())).toBe('Rebase onto main stopped — conflicts to resolve');
  });
});

describe('outcome detail (§4.7)', () => {
  it('maps each outcome; null for conflicts / absent', () => {
    expect(outcomeLabel('fastForwarded')).toBe('Fast-forwarded');
    expect(outcomeLabel('merged')).toBe('Merge commit created');
    expect(outcomeLabel('upToDate')).toBe('Already up to date');
    expect(outcomeLabel('conflicts')).toBeNull();
    expect(outcomeLabel(null)).toBeNull();
  });

  it('the clause follows the status word in the name, and replaces `success` in the announcer', () => {
    const r = run({ outcome: 'fastForwarded', endedAt: 1_200 });
    expect(runRowName(r, 0)).toBe(
      `Merge feature/x — success, fast-forwarded, 0.2 seconds, ${hhmm(1_200)}`,
    );
    expect(gitAnnounceFor([r], new Map())).toBe('Merge feature/x finished — fast-forwarded');
    const up = run({ category: 'rebase', target: 'main', outcome: 'upToDate' });
    expect(gitAnnounceFor([up], new Map())).toBe('Rebase onto main finished — already up to date');
    const merged = run({ outcome: 'merged' });
    expect(gitAnnounceFor([merged], new Map())).toBe('Merge feature/x finished — merge commit created');
  });

  it('absent outcome keeps `finished — success`', () => {
    expect(gitAnnounceFor([run()], new Map())).toBe('Merge feature/x finished — success');
  });
});

describe('count noun (§4.4)', () => {
  it('fills the noun slot and every sentence', () => {
    const r = run({ category: 'deleteBranches', target: null, targetCount: 3, endedAt: 1_100 });
    expect(runNoun(r)).toBe('Delete 3 branches');
    expect(runRowName(r, 0)).toBe(`Delete 3 branches — success, 0.1 seconds, ${hhmm(1_100)}`);
    expect(gitAnnounceFor([r], new Map())).toBe('Delete 3 branches finished — success');
  });

  it('discard reads `Discard changes in N files`, localised', () => {
    expect(runNoun(run({ category: 'discard', target: null, targetCount: 1200 }))).toBe(
      `Discard changes in ${(1200).toLocaleString()} files`,
    );
  });

  it('a single item keeps the noun + raw target', () => {
    const r = run({ category: 'deleteBranches', target: 'feature/x' });
    expect(runNoun(r)).toBe('Delete branch');
    expect(gitAnnounceFor([r], new Map())).toBe('Delete branch feature/x finished — success');
  });
});

describe('checkout noun vs verb', () => {
  it('row noun `Checkout`, announcer verb `Check out`', () => {
    const r = run({ category: 'checkoutBranch', target: 'main' });
    expect(runNoun(r)).toBe('Checkout');
    expect(gitAnnounceFor([r], new Map())).toBe('Check out main finished — success');
  });
});

describe('duration floor (§4.2-5)', () => {
  it('under 100 ms reads `<0.1s` / `under 0.1 seconds`', () => {
    const r = run({ category: 'createTag', target: 'v1.2.0', endedAt: 1_040 });
    expect(durationLabel(r, 0)).toBe('<0.1s');
    expect(runRowName(r, 0)).toBe(`Create tag v1.2.0 — success, under 0.1 seconds, ${hhmm(1_040)}`);
    expect(durationLabel(run({ endedAt: 1_100 }), 0)).toBe('0.1s');
  });
});

describe('phaseLabel for P119 categories (§4.2-4)', () => {
  it('preparing / finalizing read the participle', () => {
    expect(phaseLabel('checkoutBranch', { kind: 'preparing' })).toBe('Checking out…');
    expect(phaseLabel('rebase', { kind: 'finalizing' })).toBe('Rebasing…');
  });

  it('network reads networkLabel, else the participle', () => {
    expect(phaseLabel('pushTag', { kind: 'network' })).toBe('Sending objects…');
    expect(phaseLabel('cloneRepo', { kind: 'network' })).toBe('Receiving objects…');
    expect(phaseLabel('createTag', { kind: 'network' })).toBe('Creating tag…');
  });

  it('runningHook is shared', () => {
    expect(phaseLabel('merge', { kind: 'runningHook', hook: 'pre-merge-commit' })).toBe(
      'Running pre-merge-commit hook…',
    );
  });

  it('P87 rows are untouched', () => {
    expect(phaseLabel('push', { kind: 'preparing' })).toBe('Preparing…');
    expect(phaseLabel('fetch', { kind: 'finalizing' })).toBe('Finalizing…');
  });
});

describe('dockBarFraction (§2.2)', () => {
  const progress = (receivedObjects: number, totalObjects: number) => ({
    receivedObjects,
    totalObjects,
    indexedObjects: 0,
    receivedBytes: 0,
  });

  it('null without a run or a total; clamped to [0, 1]', () => {
    expect(dockBarFraction(null)).toBeNull();
    expect(dockBarFraction(run({ progress: progress(5, 0) }))).toBeNull();
    expect(dockBarFraction(run({ progress: progress(50, 100) }))).toBe(0.5);
    expect(dockBarFraction(run({ progress: progress(120, 100) }))).toBe(1);
  });
});
