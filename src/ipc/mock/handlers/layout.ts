// Shared mock graph-layout resolution (P50a). Extracted so getGraph and
// searchCommits resolve the SAME layout — search results then map onto exactly
// the rows the graph shows (highlight/jump in P50b).
import { buildMockGraph, buildMockGraphDetached, prependCommits, withStashNodes } from '../../fixtures/graph';
import { generateLayout20k } from '../../fixtures/graph20k';
import type { GraphLayout } from '../../types';
import type { MockRepoState } from '../repoState';

/** The graph layout a repo currently shows — identical to what getGraph serves:
 *  the 20k / detached fixtures verbatim, else the mock-commit rows prepended
 *  onto the default fixture with the live stash stack injected as offshoots. */
export function resolveLayout(state: MockRepoState): GraphLayout {
  if (state.graphFixture === '20k') return generateLayout20k();
  if (state.graphFixture === 'detached') return buildMockGraphDetached();
  const base = prependCommits(buildMockGraph(), state.commits);
  return withStashNodes(base, state.stashes);
}
