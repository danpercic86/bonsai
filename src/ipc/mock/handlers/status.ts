// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { hasIdentity } from '../../fixtures/config';
import { lineDiff, reconstructLines } from '../../fixtures/diffs';
import { randomOid } from '../../fixtures/oids';
import { delay, requireRepo } from '../repoState';
import { MAIN_RS_PATH, collectSelection, linesEqual, sortByPath, takeMatching, upsert } from '../statusHelpers';
import type { AppError, CommitResult, LineSelection, StatusSnapshot } from '../../types';

export const statusHandlers = {
  async getStatus(repoId: string): Promise<StatusSnapshot> {
    await delay(150);
    const state = requireRepo(repoId);
    // Fresh copy so callers can't mutate the fixture between fetches.
    const snapshot = structuredClone(state.status);
    // P17: append the model-derived src/main.rs rows. It shows in `staged` when
    // the index differs from HEAD, and in `unstaged` when the workdir differs
    // from the index — so a partial stage/unstage can put it in BOTH sections.
    const { head, index, workdir } = state.mainRs;
    if (!linesEqual(index, head)) {
      snapshot.staged.push({ path: MAIN_RS_PATH, origPath: null, status: 'modified' });
      sortByPath(snapshot.staged);
    }
    if (!linesEqual(workdir, index)) {
      snapshot.unstaged.push({ path: MAIN_RS_PATH, origPath: null, status: 'modified' });
      sortByPath(snapshot.unstaged);
    }
    return snapshot;
  },

  async stage(repoId: string, paths: string[]): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    for (const entry of takeMatching(state.status.unstaged, paths)) {
      upsert(state.status.staged, entry);
    }
    for (const entry of takeMatching(state.status.untracked, paths)) {
      upsert(state.status.staged, { ...entry, status: 'added' });
    }
    sortByPath(state.status.staged);
  },

  async unstage(repoId: string, paths: string[]): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    for (const entry of takeMatching(state.status.staged, paths)) {
      if (entry.status === 'added') {
        upsert(state.status.untracked, { ...entry, status: 'untracked' });
      } else {
        upsert(state.status.unstaged, entry); // status + origPath preserved
      }
    }
    sortByPath(state.status.unstaged);
    sortByPath(state.status.untracked);
  },

  // P17: partial (line-level) staging is modeled ONLY for the live src/main.rs
  // three-way file. Any other path rejects (mirrors the backend rejecting
  // non-model files). Both mutate `state.mainRs.index` via reconstructLines
  // (the SAME rule as the Rust §2.4 reconstruction), return void, and DO NOT
  // emit repo-changed (the frontend refetches imperatively).
  async stagePartial(
    repoId: string,
    path: string,
    _origPath: string | null,
    selection: LineSelection[],
  ): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (path !== MAIN_RS_PATH) {
      const err: AppError = {
        kind: 'other',
        message: 'mock: partial staging is only modeled for src/main.rs',
      };
      throw err;
    }
    const { selAdd, selDel } = collectSelection(selection);
    const { index, workdir } = state.mainRs;
    // Stage: recompute index-vs-workdir and move the selected lines index->workdir.
    const { hunks } = lineDiff(index, workdir, MAIN_RS_PATH, 'modified', false);
    state.mainRs.index = reconstructLines('stage', hunks, index, workdir, selAdd, selDel);
  },

  async unstagePartial(
    repoId: string,
    path: string,
    _origPath: string | null,
    selection: LineSelection[],
  ): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (path !== MAIN_RS_PATH) {
      const err: AppError = {
        kind: 'other',
        message: 'mock: partial staging is only modeled for src/main.rs',
      };
      throw err;
    }
    const { selAdd, selDel } = collectSelection(selection);
    const { head, index } = state.mainRs;
    // Unstage: recompute head-vs-index and move the selected lines index->head.
    const { hunks } = lineDiff(head, index, MAIN_RS_PATH, 'modified', false);
    state.mainRs.index = reconstructLines('unstage', hunks, head, index, selAdd, selDel);
  },

  // P28: partial discard — same three-way model, but the WORKDIR moves toward
  // the INDEX (side-substituted 'unstage': old=index, new=workdir). The index
  // is never touched; getStatus derives the unstaged row from workdir !== index,
  // so a full discard clears the row naturally. No repo-changed emit.
  async discardPartial(
    repoId: string,
    path: string,
    _origPath: string | null,
    selection: LineSelection[],
  ): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (path !== MAIN_RS_PATH) {
      const err: AppError = {
        kind: 'other',
        message: 'mock: partial discard is only modeled for src/main.rs',
      };
      throw err;
    }
    const { selAdd, selDel } = collectSelection(selection);
    const { index, workdir } = state.mainRs;
    // Discard: recompute index-vs-workdir and revert the selected lines in the
    // workdir toward the index ('unstage' = base NEW side, undo toward OLD).
    const { hunks } = lineDiff(index, workdir, MAIN_RS_PATH, 'modified', false);
    state.mainRs.workdir = reconstructLines('unstage', hunks, index, workdir, selAdd, selDel);
  },

  async commit(repoId: string, message: string): Promise<CommitResult> {
    await delay(150);
    const state = requireRepo(repoId);
    if (message.trim() === '') {
      const err: AppError = { kind: 'emptyMessage', message: 'commit message is empty' };
      throw err;
    }
    // Signature resolution happens before the nothing-to-commit check in the
    // backend (contract §2.4 steps 4→6) — mirror that precedence here. P40:
    // read identity from the config store so setting it in Settings clears
    // this error end-to-end (the `?fixture=noconfig` store starts empty).
    if (!hasIdentity(state.config)) {
      const err: AppError = {
        kind: 'configMissing',
        message:
          'git identity not configured: user.name and user.email are not set. ' +
          'Run: git config --global user.name "Your Name" and ' +
          'git config --global user.email "you@example.com"',
      };
      throw err;
    }
    if (state.status.staged.length === 0) {
      const err: AppError = {
        kind: 'nothingToCommit',
        message: 'nothing to commit (index matches HEAD)',
      };
      throw err;
    }
    state.status.staged = [];
    state.headOid = randomOid();
    // M6 contract §5: bump the current branch's ahead count so the harness
    // gets the natural commit → push story (main: 0/0 → ↑1 → push clears).
    const headBranch = state.branches.local.find((b) => b.name === state.headBranch);
    if (headBranch !== undefined && headBranch.upstream !== null) {
      headBranch.ahead = (headBranch.ahead ?? 0) + 1;
    }
    const summary = message.trim().split('\n', 1)[0] ?? '';
    // P1 contract §3.5: the DEFAULT graph fixture gains a synthetic lane-0 row
    // per mock commit (newest first) so the harness shows the commit on top.
    state.commits.unshift({ oid: state.headOid, summary });
    return { oid: state.headOid, summary, branch: state.headBranch };
  },

} satisfies Partial<IpcApi>;
