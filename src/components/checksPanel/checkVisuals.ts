// P90: PURE per-check + rollup visual model (mirrors forgeBadges.ts's colour
// language so the graph badge and this detailed list read identically). No React.
import type { CheckRollup, CommitStatus, StatusContext } from '../../ipc';

/** Per-check state → glyph + a11y state word + CSS modifier (colour carrier is the
 *  glyph class, never colour alone — the word is folded into the row's a11y name). */
export interface CheckVisual {
  glyph: string;
  /** Sentence-leading state word for the accessible name, e.g. "Failed". */
  word: string;
  /** CSS modifier suffix → `.checks-glyph--<tone>`. */
  tone: 'good' | 'warn' | 'pending' | 'neutral';
}

export function checkVisual(state: CheckRollup): CheckVisual {
  switch (state) {
    case 'success':
      return { glyph: '✓', word: 'Passed', tone: 'good' };
    case 'failure':
      return { glyph: '⚠', word: 'Failed', tone: 'warn' };
    case 'error':
      return { glyph: '⊘', word: 'Errored', tone: 'warn' };
    case 'pending':
      return { glyph: '●', word: 'Pending', tone: 'pending' };
    case 'neutral':
    case 'none':
      return { glyph: '–', word: 'Neutral', tone: 'neutral' };
  }
}

/** Sort order for the check list (§4.9): problems first, stable within a group. */
const RANK: Record<CheckRollup, number> = {
  failure: 0,
  error: 1,
  pending: 2,
  success: 3,
  neutral: 4,
  none: 5,
};

/** Stable sort: failure, error, pending, success, neutral — preserving forge
 *  order within each group. Returns a new array (does not mutate input). */
export function sortContexts(contexts: StatusContext[]): StatusContext[] {
  return contexts
    // §3.1: a `none`-state context is never rendered as a row.
    .filter((c) => c.state !== 'none')
    .map((c, i) => ({ c, i }))
    .sort((a, b) => RANK[a.c.state] - RANK[b.c.state] || a.i - b.i)
    .map((x) => x.c);
}

/** Overall rollup pill copy + tone from a CommitStatus. Singular/plural aware. */
export interface RollupPill {
  glyph: string;
  label: string;
  /** Full a11y sentence, e.g. "3 of 8 checks failed". */
  aria: string;
  tone: 'good' | 'warn' | 'pending' | 'neutral';
}

function plural(n: number, noun: string): string {
  return `${n} ${noun}${n === 1 ? '' : 's'}`;
}

export function rollupPill(status: CommitStatus): RollupPill | null {
  const { state, total, passed, failed, pending } = status;
  switch (state) {
    case 'success':
      return {
        glyph: '✓',
        label: `${plural(passed || total, 'check')} passed`,
        aria: 'All checks passed',
        tone: 'good',
      };
    case 'failure':
      return {
        glyph: '⚠',
        label: `${failed} of ${total} failed`,
        aria: `${failed} of ${total} checks failed`,
        tone: 'warn',
      };
    case 'error':
      return {
        glyph: '⊘',
        label: `${plural(failed, 'check')} errored`,
        aria: `${plural(failed, 'check')} errored`,
        tone: 'warn',
      };
    case 'pending':
      return {
        glyph: '●',
        label: `${pending} running`,
        aria: `${plural(pending, 'check')} running`,
        tone: 'pending',
      };
    case 'neutral':
      return {
        glyph: '–',
        label: `${plural(total, 'check')} neutral`,
        aria: `${plural(total, 'check')} neutral`,
        tone: 'neutral',
      };
    case 'none':
      return null;
  }
}
