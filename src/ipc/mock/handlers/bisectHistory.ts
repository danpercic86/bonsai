// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { BLAME_FIXTURE_PATHS, MOCK_BLAME, MOCK_FILE_HISTORY } from '../../fixtures/history';
import { MOCK_BRANCH_REFLOGS, MOCK_HEAD_REFLOG } from '../../fixtures/reflog';
import { driveMockBisect } from '../rebaseBisectHelpers';
import { delay, requireRepo } from '../repoState';
import type { MockBisect } from '../repoState';
import type { AppError, BisectOutcome, BlameLine, FileHistoryEntry, ReflogEntry } from '../../types';

export const bisectHistoryHandlers = {
  async startBisect(repoId: string, bad: string, good: string[]): Promise<BisectOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'none') {
      const err: AppError = {
        kind: 'operationInProgress',
        message: 'an operation is already in progress — commit or abort it first',
      };
      throw err;
    }
    if (good.length === 0 || good[0] === bad) {
      const err: AppError = {
        kind: 'git',
        message: 'nothing to bisect: good and bad must differ',
      };
      throw err;
    }
    // Seed a 6-commit-wide candidate chain: good, s1..s6, bad (oldest→newest).
    const middle = Array.from({ length: 6 }, (_, i) =>
      (i + 1).toString(16).padStart(40, '0'),
    );
    const chain = [good[0], ...middle, bad];
    const mb: MockBisect = {
      chain,
      lo: 0,
      hi: chain.length - 1,
      current: null,
      skipped: [],
      firstBad: null,
    };
    state.bisect = mb;
    return driveMockBisect(state, mb);
  },

  async bisectMark(repoId: string, isGood: boolean): Promise<BisectOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    const mb = state.bisect;
    if (mb === null || mb.current === null) {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no bisect in progress' };
      throw err;
    }
    if (isGood) {
      mb.lo = mb.current;
    } else {
      mb.hi = mb.current;
    }
    return driveMockBisect(state, mb);
  },

  async bisectSkip(repoId: string): Promise<BisectOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    const mb = state.bisect;
    if (mb === null || mb.current === null) {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no bisect in progress' };
      throw err;
    }
    const oid = mb.chain[mb.current];
    if (!mb.skipped.includes(oid)) mb.skipped.push(oid);
    return driveMockBisect(state, mb);
  },

  async bisectReset(repoId: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.bisect === null) {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no bisect in progress' };
      throw err;
    }
    state.opState = { kind: 'none' };
    state.bisect = null;
  },

  // P23d §10.2: per-line blame + per-file commit history. Canned fixtures for
  // the designated path `src/app.ts` attributed to the deterministic mock graph
  // commit oids (see fixtures/graph.ts `oid(row)`), so clicking a gutter block /
  // history row reveals a REAL node in the graph. Any other path rejects
  // (blame) / returns [] (history), matching the backend contract.
  async blameFile(repoId: string, path: string, _atOid: string | null): Promise<BlameLine[]> {
    await delay(150);
    requireRepo(repoId);
    if (!BLAME_FIXTURE_PATHS.has(path)) {
      const err: AppError = { kind: 'git', message: `mock: no blame fixture for ${path}` };
      throw err;
    }
    return structuredClone(MOCK_BLAME);
  },

  async fileHistory(repoId: string, path: string, limit: number): Promise<FileHistoryEntry[]> {
    await delay(150);
    requireRepo(repoId);
    if (!BLAME_FIXTURE_PATHS.has(path)) return [];
    return structuredClone(MOCK_FILE_HISTORY).slice(0, Math.max(0, limit) || MOCK_FILE_HISTORY.length);
  },

  // P38: reflog read (stateful-read, mirrors fileHistory). HEAD returns the
  // seeded recovery story; a known local branch returns its reflog; any other
  // ref → [] (never-updated ref), matching the backend contract.
  async readReflog(repoId: string, refName: string): Promise<ReflogEntry[]> {
    await delay(120);
    requireRepo(repoId);
    if (refName === 'HEAD') return structuredClone(MOCK_HEAD_REFLOG);
    const branch = MOCK_BRANCH_REFLOGS[refName];
    return branch ? structuredClone(branch) : [];
  },

  // P40: stateful config store per repo (Local | Global). Validation mirrors
  // the Rust §4.5 shape checks so the harness exercises client + server-shaped
  // `invalidName` errors identically.
} satisfies Partial<IpcApi>;
