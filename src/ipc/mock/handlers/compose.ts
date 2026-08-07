// P54b: apply a reviewed commit-composer plan (mutation mock). Mirrors the backend
// ATOMICALLY — validate the WHOLE plan first, then (only if valid AND no '#fail'
// marker) mutate: remove each group's files from the status lists and push one new
// commit per group, oldest→newest (newest on top, like the `commit` mock). Files in
// no group stay in `status` ("left uncommitted"). A '#fail' in ANY message drives
// the atomic-rollback error path: it throws and mutates NOTHING. Spread into
// `mockIpc` via `composeHandlers`.
import { randomOid } from '../../fixtures/oids';
import { delay, requireRepo } from '../repoState';
import { MAIN_RS_PATH, takeMatching } from '../statusHelpers';
import type { AppError, ComposeApplyResult, ComposeCommit, ComposePlan, IpcApi } from '../../types';

export const composeHandlers = {
  async applyComposedCommits(repoId: string, plan: ComposePlan): Promise<ComposeApplyResult> {
    await delay(200 * Math.max(1, plan.groups.length));
    const state = requireRepo(repoId);

    // --- validation parity with the backend (nothing mutates yet) ---
    if (plan.groups.length === 0) {
      const err: AppError = { kind: 'nothingToCommit', message: 'nothing to commit (empty plan)' };
      throw err;
    }
    for (const g of plan.groups) {
      if (g.message.trim() === '') {
        const err: AppError = { kind: 'emptyMessage', message: 'commit message is empty' };
        throw err;
      }
      if (g.files.length === 0) {
        const err: AppError = { kind: 'other', message: 'a group has no files' };
        throw err;
      }
    }
    // '#fail' in ANY message => atomic rollback: throw, mutate NOTHING.
    if (plan.groups.some((g) => g.message.includes('#fail'))) {
      const err: AppError = { kind: 'git', message: 'Mock: composer apply failed (rolled back)' };
      throw err;
    }

    // --- apply, oldest -> newest ---
    const commits: ComposeCommit[] = [];
    for (const g of plan.groups) {
      // This group's files are now committed → drop them from every status list.
      takeMatching(state.status.staged, g.files);
      takeMatching(state.status.unstaged, g.files);
      takeMatching(state.status.untracked, g.files);
      // The live three-way model file: committing it clears its dirty sections.
      if (g.files.includes(MAIN_RS_PATH)) {
        state.mainRs.index = [...state.mainRs.head];
        state.mainRs.workdir = [...state.mainRs.head];
      }
      const oid = randomOid();
      const summary = g.message.trim().split('\n')[0] ?? '';
      state.headOid = oid;
      state.commits.unshift({ oid, summary });
      commits.push({ oid, summary });
    }
    // Files NOT in any group remain in `status` (left uncommitted).
    return { commits };
  },
} satisfies Partial<IpcApi>;
