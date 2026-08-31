import { describe, it, expect } from 'vitest';
import type { CommitStatus, StatusContext } from '../../ipc';
import { checkVisual, rollupPill, sortContexts } from './checkVisuals';

function c(name: string, state: StatusContext['state']): StatusContext {
  return { name, state, description: null, targetUrl: null };
}

function status(over: Partial<CommitStatus>): CommitStatus {
  return { sha: 's', state: 'success', total: 0, passed: 0, failed: 0, pending: 0, contexts: [], ...over };
}

describe('checkVisual', () => {
  it('gives a distinct glyph + state word per state', () => {
    expect(checkVisual('success')).toMatchObject({ glyph: '✓', word: 'Passed', tone: 'good' });
    expect(checkVisual('failure')).toMatchObject({ glyph: '⚠', word: 'Failed', tone: 'warn' });
    expect(checkVisual('error')).toMatchObject({ glyph: '⊘', word: 'Errored', tone: 'warn' });
    expect(checkVisual('pending')).toMatchObject({ glyph: '●', tone: 'pending' });
    expect(checkVisual('neutral')).toMatchObject({ glyph: '–', tone: 'neutral' });
  });
});

describe('sortContexts', () => {
  it('sorts failure, error, pending, success, neutral; stable within a group', () => {
    const input = [
      c('s1', 'success'),
      c('n1', 'neutral'),
      c('f1', 'failure'),
      c('p1', 'pending'),
      c('e1', 'error'),
      c('s2', 'success'),
    ];
    expect(sortContexts(input).map((x) => x.name)).toEqual(['f1', 'e1', 'p1', 's1', 's2', 'n1']);
  });

  it('does not mutate the input array', () => {
    const input = [c('s', 'success'), c('f', 'failure')];
    sortContexts(input);
    expect(input.map((x) => x.name)).toEqual(['s', 'f']);
  });
});

describe('rollupPill', () => {
  it('counts + pluralizes the passed summary', () => {
    expect(rollupPill(status({ state: 'success', total: 1, passed: 1 }))?.label).toBe('1 check passed');
    expect(rollupPill(status({ state: 'success', total: 3, passed: 3 }))?.label).toBe('3 checks passed');
  });

  it('shows the failed-of-total summary', () => {
    const p = rollupPill(status({ state: 'failure', total: 8, failed: 3 }));
    expect(p?.label).toBe('3 of 8 failed');
    expect(p?.aria).toBe('3 of 8 checks failed');
  });

  it('returns null for the none rollup (drives the empty state)', () => {
    expect(rollupPill(status({ state: 'none' }))).toBeNull();
  });
});
