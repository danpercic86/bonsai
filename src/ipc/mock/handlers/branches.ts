// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { randomOid } from '../../fixtures/oids';
import { buildStaleReport, delay, isInvalidBranchName, query, requireRepo } from '../repoState';
import { upsert } from '../statusHelpers';
import type { AppError, BranchDeleteResult, BranchDeleteStatus, BranchesSnapshot, CheckoutResult, CreateBranchHereResult, StaleReport } from '../../types';

export const branchHandlers = {
  async listBranches(repoId: string): Promise<BranchesSnapshot> {
    await delay(150);
    const state = requireRepo(repoId);
    const snapshot = structuredClone(state.branches);
    if (state.kind === 'detached') {
      snapshot.head = { branchName: null, oid: state.headOid, detached: true, unborn: false };
      for (const branch of snapshot.local) branch.isHead = false;
    } else {
      snapshot.head = {
        branchName: state.headBranch,
        oid: state.headOid,
        detached: false,
        unborn: false,
      };
    }
    return snapshot;
  },

  async createBranch(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (isInvalidBranchName(name)) {
      const err: AppError = { kind: 'invalidName', message: `invalid branch name: '${name}'` };
      throw err;
    }
    const trimmed = name.trim();
    if (state.branches.local.some((b) => b.name === trimmed)) {
      const err: AppError = {
        kind: 'branchExists',
        message: `branch '${trimmed}' already exists`,
      };
      throw err;
    }
    state.branches.local.push({
      name: trimmed,
      isHead: false,
      upstream: null,
      ahead: null,
      behind: null,
      tip: randomOid(),
    });
    state.branches.local.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
  },

  // P11 §1.3: create a local branch at `oid` and check it out, carrying any
  // dirty worktree across. Stateful so the graph HEAD/new-branch pills move on
  // the next refreshAll. `?branch=cbhconflict` exercises the Conflicts toast.
  async createBranchHere(
    repoId: string,
    name: string,
    oid: string,
  ): Promise<CreateBranchHereResult> {
    await delay(250);
    const state = requireRepo(repoId);
    if (isInvalidBranchName(name)) {
      const err: AppError = { kind: 'invalidName', message: `invalid branch name: '${name}'` };
      throw err;
    }
    const trimmed = name.trim();
    if (state.branches.local.some((b) => b.name === trimmed)) {
      const err: AppError = {
        kind: 'branchExists',
        message: `branch '${trimmed}' already exists`,
      };
      throw err;
    }
    const s = state.status;
    const dirty =
      s.staged.length > 0 ||
      s.unstaged.length > 0 ||
      s.untracked.length > 0 ||
      s.conflicted.length > 0;
    // Add the new branch at `oid` as the checked-out HEAD (unset previous head)
    // + move headBranch/headOid so the graph HEAD pill follows on refreshAll.
    for (const b of state.branches.local) b.isHead = false;
    state.branches.local.push({
      name: trimmed,
      isHead: true,
      upstream: null,
      ahead: null,
      behind: null,
      tip: oid,
    });
    state.branches.local.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
    state.headBranch = trimmed;
    state.headOid = oid;
    state.branches.head = { branchName: trimmed, oid, detached: false, unborn: false };
    if (!dirty) return { stashed: false, apply: null };
    // Simulate carrying work across the switch.
    if (query('branch') === 'cbhconflict') {
      // Carried with markers: worktree stays dirty (synthetic conflict entry)
      // and the stash would be RETAINED — do NOT clear the status.
      upsert(s.conflicted, { path: 'src/app.ts', origPath: null, status: 'conflicted' });
      return { stashed: true, apply: { kind: 'conflicts', paths: ['src/app.ts'] } };
    }
    // Clean carry-over: the changes moved with us — status preserved as-is.
    return { stashed: true, apply: { kind: 'applied' } };
  },

  // P33: dirty-safe switch — auto-stash → switch → auto fast-forward (no fetch)
  // → re-apply stash. Never hard-fails on a dirty tree; a conflicted re-apply is
  // a SUCCESS carrying `apply: {kind:'conflicts'}` (stash RETAINED).
  async checkoutBranch(repoId: string, name: string): Promise<CheckoutResult> {
    await delay(150);
    const state = requireRepo(repoId);
    // P36: deterministic worktree-collision refusal — a reserved fixture branch
    // name simulates the branch being checked out in another worktree.
    if (name === '__wt_locked__') {
      const err: AppError = {
        kind: 'branchCheckedOutElsewhere',
        message: `branch '${name}' is already checked out at '/repo/.worktrees/${name}'`,
      };
      throw err;
    }
    const branch = state.branches.local.find((b) => b.name === name);
    if (branch === undefined) {
      const err: AppError = { kind: 'branchNotFound', message: `branch '${name}' not found` };
      throw err;
    }
    const s = state.status;
    const dirty =
      s.staged.length > 0 ||
      s.unstaged.length > 0 ||
      s.untracked.length > 0 ||
      s.conflicted.length > 0;

    // Move HEAD to the target branch (unset previous head).
    for (const b of state.branches.local) b.isHead = false;
    branch.isHead = true;
    state.headBranch = name;
    state.branches.head = { branchName: name, oid: state.headOid, detached: false, unborn: false };
    // TODO(polish): move the HEAD/branch pills in the mock graph fixture too
    // (contract §5 decision: fixtures stay decoupled from branch state —
    // harness proof is the sidebar dot + header branch name).

    // Auto fast-forward: only when the target tracks an upstream and is strictly
    // behind (behind>0 && ahead==0). `feature/merged-a` is the deterministic FF
    // fixture (ahead 0, behind 3). Diverged (feature/sidebar) or up-to-date
    // (main) → no FF.
    const fastForwarded =
      branch.upstream != null && (branch.behind ?? 0) > 0 && (branch.ahead ?? 0) === 0;
    if (fastForwarded) {
      branch.behind = 0;
    }

    if (!dirty) return { stashed: false, fastForwarded, apply: null };

    // Carried work across the switch. `fix/watcher-debounce` is the designated
    // conflicted re-apply fixture (contract §4.3): worktree stays dirty (stash
    // RETAINED) with a synthetic conflict entry.
    if (name === 'fix/watcher-debounce') {
      upsert(s.conflicted, { path: 'src/app.ts', origPath: null, status: 'conflicted' });
      return { stashed: true, fastForwarded, apply: { kind: 'conflicts', paths: ['src/app.ts'] } };
    }
    // Clean carry-over: the changes moved with us — status preserved as-is.
    return { stashed: true, fastForwarded, apply: { kind: 'applied' } };
  },

  async deleteBranch(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const branch = state.branches.local.find((b) => b.name === name);
    if (branch === undefined) {
      const err: AppError = { kind: 'branchNotFound', message: `branch '${name}' not found` };
      throw err;
    }
    if (branch.isHead) {
      const err: AppError = {
        kind: 'git',
        message: `cannot delete '${name}': it is the currently checked-out branch`,
      };
      throw err;
    }
    // Designated unmerged branch (contract §5).
    if (name === 'experiment-unmerged') {
      const err: AppError = {
        kind: 'unmergedBranch',
        message:
          "branch 'experiment-unmerged' is not fully merged into HEAD (tip 1a2b3c4). " +
          'Bonsai v1 does not force-delete; use `git branch -D experiment-unmerged` ' +
          'if you are sure.',
      };
      throw err;
    }
    state.branches.local = state.branches.local.filter((b) => b.name !== name);
  },

  // P6 §3.5: GitKraken-style remote checkout — create/reuse a local tracking
  // branch for the remote-tracking ref and switch to it.
  async checkoutRemoteBranch(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const remote = state.branches.remote.find((r) => r.name === name);
    if (remote === undefined) {
      const err: AppError = {
        kind: 'branchNotFound',
        message: `remote-tracking branch '${name}' not found`,
      };
      throw err;
    }
    // Split on the FIRST '/' (remote names contain no '/').
    const slash = name.indexOf('/');
    const localName = slash === -1 ? name : name.slice(slash + 1);
    let local = state.branches.local.find((b) => b.name === localName);
    if (local === undefined) {
      // Create-and-track path: new local tracking branch at the remote tip.
      local = { name: localName, isHead: false, upstream: name, ahead: 0, behind: 0, tip: remote.tip };
      state.branches.local.push(local);
      state.branches.local.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
    }
    // Switch HEAD (same state transition as checkoutBranch).
    for (const b of state.branches.local) b.isHead = false;
    local.isHead = true;
    state.headBranch = local.name;
    state.headOid = local.tip;
    state.branches.head = {
      branchName: local.name,
      oid: state.headOid,
      detached: false,
      unborn: false,
    };
  },

  // P6 §3.5: delete the LOCAL remote-tracking ref only (never touches the server).
  async deleteRemoteBranch(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const remote = state.branches.remote.find((r) => r.name === name);
    if (remote === undefined) {
      const err: AppError = {
        kind: 'branchNotFound',
        message: `remote-tracking branch '${name}' not found`,
      };
      throw err;
    }
    state.branches.remote = state.branches.remote.filter((r) => r.name !== name);
  },

  // P25 §6.3: stale-branch cleanup. `listStaleBranches` recomputes the report
  // from the live branch list; `deleteBranches` mirrors the server's re-verified
  // safety rules and MUTATES `state.branches.local` for every deleted name so the
  // harness shows rows disappear + a summary toast.
  async listStaleBranches(repoId: string, _base?: string): Promise<StaleReport> {
    await delay(150);
    const state = requireRepo(repoId);
    return buildStaleReport(state);
  },

  async deleteBranches(
    repoId: string,
    names: string[],
    _base?: string,
  ): Promise<BranchDeleteResult[]> {
    await delay(200);
    const state = requireRepo(repoId);
    const report = buildStaleReport(state);
    const safe = new Set(report.branches.map((b) => b.name));
    const currentName = state.kind === 'detached' ? null : state.headBranch;

    const results: BranchDeleteResult[] = names.map((name) => {
      if (name === currentName) {
        return { name, status: 'skippedCurrent' as BranchDeleteStatus, message: 'checked-out branch' };
      }
      if (name === report.base) {
        return { name, status: 'skippedBase' as BranchDeleteStatus, message: 'base branch' };
      }
      if (!safe.has(name)) {
        return {
          name,
          status: 'skippedNotStale' as BranchDeleteStatus,
          message: 'not detected as stale',
        };
      }
      // Safe → remove it from the live branch list (mutating shrink).
      state.branches.local = state.branches.local.filter((b) => b.name !== name);
      return { name, status: 'deleted' as BranchDeleteStatus, message: null };
    });
    return results;
  },

  // Stateful remote mock (M6 contract §5). Failure triggers via `?remote=`
  // (authfail | network | rejected | conflict), composable with `?fixture=`.
} satisfies Partial<IpcApi>;
