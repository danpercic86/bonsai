// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { MockCommit } from '../fixtures/graph';
import { randomOid } from '../fixtures/oids';
import type { MockBisect, MockRepoState } from './repoState';
import type { BisectOutcome, RebaseOutcome, RebaseTodoOp } from '../types';

/**
 * Completes a paused rebase (shared by rebaseContinue/rebaseSkip): clears the
 * op + conflicted status, moves HEAD, and prepends `steps` plain replayed
 * MockCommits atop the graph so they visibly appear.
 */
export function finishRebase(state: MockRepoState, steps: number): RebaseOutcome {
  state.opState = { kind: 'none' };
  state.status.conflicted = [];
  state.headOid = randomOid();
  // commits[0] is the topmost row = the new HEAD tip (prependCommits maps
  // index 0 to headIndex 0), so the tip carries headOid.
  const replayed: MockCommit[] = Array.from({ length: steps }, (_, i) => ({
    oid: i === 0 ? state.headOid : randomOid(),
    summary: `pick: replayed ${steps - i}`,
  }));
  state.commits.unshift(...replayed);
  return { kind: 'rebased', branch: state.headBranch, head: state.headOid, steps };
}

/** P23b: first line of a (possibly multi-line) message. */
export function firstLine(msg: string): string {
  return msg.split('\n', 1)[0] ?? '';
}

/**
 * P23b §7.2: apply an interactive-rebase plan (execution order = oldest-first)
 * to the mock commit list DETERMINISTICALLY, producing the rewritten commits.
 * Drops `drop` rows; keeps the array order (reorder); combines `squash`/`fixup`
 * into the preceding kept commit (squash uses `newMessage`'s first line when
 * provided, else concatenates summaries; fixup keeps the predecessor's summary);
 * `reword` applies its `newMessage`'s first line as the summary. Every rewritten
 * commit gets a fresh oid (a rebase always rewrites). Returns NEWEST-first so the
 * result is ready to unshift onto the graph.
 */
export function applyInteractivePlan(
  todos: RebaseTodoOp[],
  summaryOf: (oid: string) => string,
): MockCommit[] {
  const result: MockCommit[] = []; // execution order (oldest-first)
  for (const t of todos) {
    if (t.action === 'drop') continue;
    if (t.action === 'squash' || t.action === 'fixup') {
      const prev = result[result.length - 1];
      if (prev === undefined) {
        // squash/fixup as the first applied row is invalid; the editor blocks it
        // and the backend rejects it, but treat it as a pick here defensively.
        result.push({ oid: randomOid(), summary: summaryOf(t.oid) });
        continue;
      }
      if (t.action === 'squash') {
        prev.summary =
          t.newMessage !== null && t.newMessage.trim() !== ''
            ? firstLine(t.newMessage)
            : `${prev.summary}; ${summaryOf(t.oid)}`;
      }
      // fixup keeps the predecessor's summary. Either way the combined commit is
      // rewritten → fresh oid.
      prev.oid = randomOid();
      continue;
    }
    // pick | reword
    const summary =
      t.action === 'reword' && t.newMessage !== null
        ? firstLine(t.newMessage)
        : summaryOf(t.oid);
    result.push({ oid: randomOid(), summary });
  }
  return result.reverse(); // newest-first
}

/**
 * P23b §7.2: finish (or skip-then-finish) a paused interactive rebase — clears
 * the op + conflicted status and prepends the rewritten commits onto the graph
 * so the harness shows the rewritten history. `dropCurrent` (Skip) drops the
 * oldest remaining rewritten commit (the conflicting op).
 */
export function finishInteractiveRebase(state: MockRepoState, dropCurrent: boolean): RebaseOutcome {
  const plan = state.interactive;
  state.opState = { kind: 'none' };
  state.status.conflicted = [];
  state.conflicts = [];
  state.conflictTexts = new Map();
  state.interactive = null;
  if (plan === null) {
    // Shouldn't happen (callers check first); fall back to a no-op finish.
    return { kind: 'rebased', branch: state.headBranch, head: state.headOid, steps: 0 };
  }
  // Skip drops the current (conflicting) op — the oldest remaining rewritten row.
  let rewritten = dropCurrent && plan.rewritten.length > 0 ? plan.rewritten.slice(0, -1) : plan.rewritten;
  // Remove the original range commits (base..old-HEAD) so the rewritten commits
  // REPLACE them rather than stacking a duplicate set on top (true rewrite).
  const removed = new Set(plan.originalOids);
  state.commits = state.commits.filter((c) => !removed.has(c.oid));
  state.headOid = randomOid();
  if (rewritten.length > 0) {
    // The topmost row (newest-first index 0) is the new HEAD tip.
    rewritten = rewritten.map((c, i) => (i === 0 ? { ...c, oid: state.headOid } : c));
    state.commits.unshift(...rewritten);
  }
  return { kind: 'rebased', branch: plan.headName, head: state.headOid, steps: rewritten.length };
}

// -------------------------------------------------------------------- P39 bisect
//
// Stateful, deterministic binary search over a synthetic candidate chain so the
// harness walks start → mark → found with the banner counts halving each step.

/** Number of untested candidates strictly between the good/bad bounds, minus
 *  skipped ones (== the `revisionsRemaining` the banner shows). */
export function bisectTestable(mb: MockBisect): number[] {
  const out: number[] = [];
  for (let i = mb.lo + 1; i < mb.hi; i++) {
    if (!mb.skipped.includes(mb.chain[i])) out.push(i);
  }
  return out;
}

/** ceil(log2(remaining)); 0 when remaining ≤ 1 — mirrors the Rust estimate. */
export function bisectSteps(remaining: number): number {
  if (remaining <= 1) return 0;
  return Math.ceil(Math.log2(remaining));
}

/** Projects the mock bisect state onto the RepoOpState.bisect wire shape and
 *  stores it as the repo's opState (what getOpState returns). */
export function bisectProject(state: MockRepoState, mb: MockBisect): void {
  const remaining = mb.firstBad === null ? bisectTestable(mb).length : 0;
  state.opState = {
    kind: 'bisect',
    current: mb.current === null ? null : mb.chain[mb.current],
    bad: mb.chain[mb.hi],
    good: [mb.chain[mb.lo]],
    skipped: [...mb.skipped],
    firstBad: mb.firstBad,
    revisionsRemaining: remaining,
    estimatedSteps: bisectSteps(remaining),
  };
}

/** Picks the next midpoint (or converges) and returns the outcome, mutating the
 *  mock bisect + opState. Shared by start / mark / skip. */
export function driveMockBisect(state: MockRepoState, mb: MockBisect): BisectOutcome {
  const testable = bisectTestable(mb);
  if (testable.length === 0) {
    mb.current = null;
    // Any skipped candidate still between the bounds → cannot determine.
    const unresolved = mb.chain
      .slice(mb.lo + 1, mb.hi)
      .some((oid) => mb.skipped.includes(oid));
    if (unresolved) {
      bisectProject(state, mb);
      return { kind: 'cannotDetermine', skipped: [...mb.skipped] };
    }
    // Converge: the bad bound is the first-bad commit.
    mb.firstBad = mb.chain[mb.hi];
    bisectProject(state, mb);
    return { kind: 'found', firstBad: mb.firstBad };
  }
  const mid = testable[Math.floor(testable.length / 2)];
  mb.current = mid;
  mb.firstBad = null;
  bisectProject(state, mb);
  const remaining = testable.length;
  return {
    kind: 'testing',
    current: mb.chain[mid],
    revisionsRemaining: remaining,
    estimatedSteps: bisectSteps(remaining),
  };
}
