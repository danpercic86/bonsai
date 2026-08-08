// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { asFullContext, lineDiff, mockCommitDiff, mockCommitFileDiff, mockCompareDiff, mockWorkdirDiff } from '../../fixtures/diffs';
import { buildMockGraph, buildMockGraphDetached, prependCommits } from '../../fixtures/graph';
import { generateLayout20k } from '../../fixtures/graph20k';
import { resolveLayout } from './layout';
import { annotateIntraline } from '../intralineMock';
import { delay, isRefTip, requireRepo } from '../repoState';
import { MAIN_RS_PATH } from '../statusHelpers';
import type { AppError, CommitDiff, CompareDiff, FileDiff, GraphLayout } from '../../types';

export const diffHandlers = {
  async getWorkdirFileDiff(
    repoId: string,
    path: string,
    origPath: string | null,
    staged: boolean,
    fullContext: boolean,
    intraline: boolean,
  ): Promise<FileDiff> {
    await delay(150);
    const state = requireRepo(repoId);
    // P17: src/main.rs is the live three-way model — its diff is computed from
    // the current head/index/workdir arrays (staged => head vs index; else
    // index vs workdir), honoring fullContext (true => one whole-file hunk).
    if (path === MAIN_RS_PATH) {
      const { head, index, workdir } = state.mainRs;
      const fd = staged
        ? lineDiff(head, index, MAIN_RS_PATH, 'modified', fullContext)
        : lineDiff(index, workdir, MAIN_RS_PATH, 'modified', fullContext);
      const cloned = structuredClone(fd);
      return intraline ? annotateIntraline(cloned) : cloned;
    }
    const base = mockWorkdirDiff(path, origPath, staged);
    const cloned = structuredClone(fullContext ? asFullContext(base) : base);
    return intraline ? annotateIntraline(cloned) : cloned;
  },

  async getCommitDiff(repoId: string, oid: string): Promise<CommitDiff> {
    await delay(150);
    const state = requireRepo(repoId);
    // Route by row index of the ACTIVE fixture layout (contract §5: robust
    // against oid spelling; 20k rows fall through to the generic diff).
    const layout =
      state.graphFixture === '20k'
        ? generateLayout20k()
        : state.graphFixture === 'detached'
          ? buildMockGraphDetached()
          : prependCommits(buildMockGraph(), state.commits);
    const index = layout.nodes.findIndex((n) => n.id === oid);
    if (index === -1) {
      // Fallback: a branch/tracking-branch tip is a resolvable commit in the
      // real backend but is NOT a graph node id in the mock (tips are decoupled
      // from walkable rows). Serve a representative multi-line-message commit
      // (fixture index 1 carries a subject + body) so ref-pill actions like
      // "Cherry-pick onto current…" prefill visibly. Genuinely-unknown oids
      // still throw below.
      if (isRefTip(state, oid)) {
        return structuredClone(mockCommitDiff(1, oid));
      }
      const err: AppError = { kind: 'git', message: 'mock: unknown commit' };
      throw err;
    }
    return structuredClone(mockCommitDiff(index, oid));
  },

  async getCommitFileDiff(
    repoId: string,
    oid: string,
    path: string,
    origPath: string | null,
    fullContext: boolean,
    intraline: boolean,
  ): Promise<FileDiff> {
    await delay(150);
    requireRepo(repoId);
    // Commit diffs are read-only: honor fullContext with the best-effort collapse.
    const base = mockCommitFileDiff(oid, path, origPath);
    const cloned = structuredClone(fullContext ? asFullContext(base) : base);
    return intraline ? annotateIntraline(cloned) : cloned;
  },

  async compareWithHead(repoId: string, oid: string): Promise<CompareDiff> {
    await delay(150);
    const state = requireRepo(repoId);
    // Route by row index of the ACTIVE fixture layout, exactly like getCommitDiff.
    const layout =
      state.graphFixture === '20k'
        ? generateLayout20k()
        : state.graphFixture === 'detached'
          ? buildMockGraphDetached()
          : prependCommits(buildMockGraph(), state.commits);
    // P6: a ref whose tip IS HEAD (e.g. origin/main == main, tip === headOid)
    // compares HEAD-to-itself → "No differences". Handled up front because
    // branch tips are intentionally decoupled from graph-row ids in the mock,
    // so headOid need not appear as a walkable node.
    if (oid === state.headOid) {
      return structuredClone(mockCompareDiff(state.headOid, oid, 0, layout));
    }
    const index = layout.nodes.findIndex((n) => n.id === oid);
    if (index === -1) {
      // Fallback: a branch/tracking-branch tip resolves to a real commit in the
      // backend but is not a graph node id in the mock. Compare HEAD against a
      // representative row (index 1) so "Compare with HEAD" on a ref pill yields
      // a plausible diff. Genuinely-unknown oids still throw below.
      if (isRefTip(state, oid)) {
        return structuredClone(mockCompareDiff(state.headOid, oid, 1, layout));
      }
      const err: AppError = { kind: 'git', message: 'mock: unknown commit' };
      throw err;
    }
    // OLD = HEAD (state.headOid), NEW = the right-clicked commit oid.
    return structuredClone(mockCompareDiff(state.headOid, oid, index, layout));
  },

  async compareWithHeadFileDiff(
    repoId: string,
    oid: string,
    path: string,
    origPath: string | null,
    fullContext: boolean,
    intraline: boolean,
  ): Promise<FileDiff> {
    await delay(150);
    requireRepo(repoId);
    // A compare file diff has the same FileDiff shape — reuse the commit builder.
    // Read-only: honor fullContext with the best-effort collapse.
    const base = mockCommitFileDiff(oid, path, origPath);
    const cloned = structuredClone(fullContext ? asFullContext(base) : base);
    return intraline ? annotateIntraline(cloned) : cloned;
  },

  async getGraph(repoId: string): Promise<GraphLayout> {
    await delay(150);
    // Built fresh per call (timestamps relative to now; callers own the copy).
    // The default fixture prepends synthetic mock-commit rows (P1 §3.5) and
    // injects the live stash stack as offshoot nodes (P10 §3.3) so create/apply/
    // pop/drop reflect on the next repo-changed refetch. Shared with
    // searchCommits via resolveLayout so both agree on the visible rows.
    return resolveLayout(requireRepo(repoId));
  },

} satisfies Partial<IpcApi>;
