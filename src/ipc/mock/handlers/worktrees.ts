// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { randomOid } from '../../fixtures/oids';
import { delay, requireRepo } from '../repoState';
import { worktreesFor } from '../worktreeState';
import type { AppError, CopyCandidate, CopyPlanEntry, CopySelection, WorktreeInfo } from '../../types';

export const worktreeHandlers = {
  // Stateful worktree mock (P27 §5): list + add/remove/lock/unlock over the
  // shared module-level list (all default-kind tabs view one repository, so
  // mutations show up everywhere). Refusal errors mirror the backend's
  // messages so the harness exercises the toast path.
  async listWorktrees(repoId: string): Promise<WorktreeInfo[]> {
    await delay(150);
    const state = requireRepo(repoId);
    const rows = worktreesFor(state);
    // `isCurrent` is per viewing repo: the row whose path this tab has open,
    // falling back to the main row when no row matches.
    const hasMatch = rows.some((w) => w.absPath === state.path);
    return rows.map((w) => ({
      ...structuredClone(w),
      isCurrent: hasMatch ? w.absPath === state.path : w.isMain,
    }));
  },

  async addWorktree(repoId: string, branch: string, name: string): Promise<WorktreeInfo> {
    await delay(150);
    const state = requireRepo(repoId);
    const worktrees = worktreesFor(state);
    // Non-default fixtures have no worktree list — refuse rather than push into
    // a throwaway [] and report a success that listWorktrees never shows.
    if (state.kind !== 'default' || state.graphFixture !== 'default') {
      const err: AppError = {
        kind: 'git',
        message: 'mock: this fixture repo does not support worktrees',
      };
      throw err;
    }
    if (branch.trim() === '') {
      const err: AppError = { kind: 'invalidName', message: 'branch name is empty' };
      throw err;
    }
    // P32 Part A: the slug source is the user-editable `name` (defaults to the
    // branch when blank), NOT the branch. Sanitize, then collision-suffix
    // against existing worktree names. (Branch existence is not enforced — the
    // mock list is authoritative; the real backend rejects unknown branches.)
    const nameSrc = name.trim() === '' ? branch : name;
    const slug = nameSrc
      .replace(/[^A-Za-z0-9._-]+/g, '-')
      .replace(/-{2,}/g, '-')
      .replace(/^[-.]+|[-.]+$/g, '');
    if (slug === '' || slug.includes('..')) {
      const err: AppError = {
        kind: 'invalidName',
        message: `cannot derive a worktree name from '${nameSrc}'`,
      };
      throw err;
    }
    // The branch-uniqueness guard keys off `branch`, independent of `name`.
    if (worktrees.some((w) => w.branch === branch)) {
      const err: AppError = {
        kind: 'git',
        message: `branch '${branch}' is already checked out in another worktree`,
      };
      throw err;
    }
    // Nested per-repo container: `.worktrees/<repo-name>/<leaf>`, where the
    // repo name is the main row's on-disk basename.
    const repoName = worktrees.find((w) => w.isMain)?.name ?? 'repo';
    const taken = new Set(worktrees.map((w) => w.name));
    let leaf = slug;
    for (let i = 2; taken.has(leaf); i += 1) leaf = `${slug}-${i}`;
    const row: WorktreeInfo = {
      name: leaf,
      absPath: `/mock/.worktrees/${repoName}/${leaf}`,
      relPath: null,
      branch,
      headOid: randomOid(),
      locked: false,
      lockReason: null,
      isMain: false,
      isCurrent: false,
      prunable: false,
      valid: true,
    };
    worktrees.push(row);
    return structuredClone(row);
  },

  async removeWorktree(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const worktrees = worktreesFor(state);
    const idx = worktrees.findIndex((w) => w.name === name);
    if (idx === -1) {
      const err: AppError = { kind: 'git', message: `worktree '${name}' not found` };
      throw err;
    }
    const wt = worktrees[idx];
    if (wt.isMain) {
      const err: AppError = { kind: 'git', message: 'cannot remove the main worktree' };
      throw err;
    }
    if (wt.absPath === state.path) {
      const err: AppError = {
        kind: 'git',
        message: 'cannot remove the worktree you currently have open',
      };
      throw err;
    }
    if (wt.locked) {
      const err: AppError = { kind: 'git', message: 'worktree is locked; unlock it first' };
      throw err;
    }
    // Dirty is not modeled in the mock — the seeded rows are clean.
    worktrees.splice(idx, 1);
  },

  async lockWorktree(repoId: string, name: string, reason?: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const wt = worktreesFor(state).find((w) => w.name === name);
    if (wt === undefined || wt.isMain) {
      const err: AppError = { kind: 'git', message: `worktree '${name}' not found` };
      throw err;
    }
    if (wt.locked) {
      const err: AppError = { kind: 'git', message: 'worktree is already locked' };
      throw err;
    }
    wt.locked = true;
    const trimmed = reason?.trim() ?? '';
    wt.lockReason = trimmed === '' ? null : trimmed;
  },

  async unlockWorktree(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const wt = worktreesFor(state).find((w) => w.name === name);
    if (wt === undefined || wt.isMain) {
      const err: AppError = { kind: 'git', message: `worktree '${name}' not found` };
      throw err;
    }
    if (!wt.locked) {
      const err: AppError = { kind: 'git', message: 'worktree is not locked' };
      throw err;
    }
    wt.locked = false;
    wt.lockReason = null;
  },

  // P32 Part B: copy uncommitted changes into a new worktree. Only the default
  // fixture surfaces candidates; every other fixture returns []. The
  // deterministic seeded conflict is `src/staged-change.ts` (see below), so the
  // harness always exercises the badge + Overwrite/Skip toggle.
  async listCopyCandidates(repoId: string): Promise<CopyCandidate[]> {
    await delay(120);
    const state = requireRepo(repoId);
    if (state.kind !== 'default' || state.graphFixture !== 'default') return [];
    const fixture: CopyCandidate[] = [
      { path: '.claude/skills/new-skill.md', group: 'untracked' },
      { path: '.claude/skills/edited.md', group: 'unstaged' },
      { path: 'src/staged-change.ts', group: 'staged' },
      { path: '.env.local', group: 'ignored' },
    ];
    return structuredClone(fixture);
  },

  async previewWorktreeCopy(
    repoId: string,
    branch: string,
    paths: string[],
  ): Promise<CopyPlanEntry[]> {
    await delay(120);
    requireRepo(repoId);
    if (branch.trim() === '') {
      const err: AppError = { kind: 'branchNotFound', message: 'branch name is empty' };
      throw err;
    }
    // Deterministic conflict: `src/staged-change.ts` (a tracked file the target
    // branch also modified) always conflicts; everything else is clean.
    return paths.map((path) => ({
      path,
      verdict: path === 'src/staged-change.ts' ? 'conflict' : 'clean',
    }));
  },

  async addWorktreeWithChanges(
    repoId: string,
    branch: string,
    name: string,
    selections: CopySelection[],
  ): Promise<WorktreeInfo> {
    // Same guards + row-push as addWorktree; the byte copy is a no-op in the
    // browser mock. `selections` length is observable for the success toast.
    void selections;
    return worktreeHandlers.addWorktree(repoId, branch, name);
  },

  // P29: repo health. Static warn-heavy fixture (§7) with a fresh generatedAt
  // per call; repo ids (paths) ending in '-err' flip the stats section to the
  // error envelope so the harness renders one errored section alongside three
  // healthy ones.
} satisfies Partial<IpcApi>;
