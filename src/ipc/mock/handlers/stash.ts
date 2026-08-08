// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { hasIdentity } from '../../fixtures/config';
import { buildMockGraph, prependCommits } from '../../fixtures/graph';
import { randomOid } from '../../fixtures/oids';
import { RESERVED_STASH_PATHS, stashHasReserved } from '../../fixtures/stashes';
import { delay, requireRepo } from '../repoState';
import { hookRejectionFor } from '../hooksGate';
import type { AppError, ApplyStashOutcome, CommitResult, CreateStashResult, StashEntry, StashScope } from '../../types';

export const stashHandlers = {
  async listStashes(repoId: string): Promise<StashEntry[]> {
    await delay(150);
    const state = requireRepo(repoId);
    return structuredClone(state.stashes);
  },

  async createStash(
    repoId: string,
    _message: string | null,
    scope: StashScope,
  ): Promise<CreateStashResult> {
    await delay(150);
    const state = requireRepo(repoId);
    const s = state.status;
    // "Nothing to stash" is scope-specific (mirrors the Rust created:false rule).
    // The mock is file-level coarse: it cannot split a path that is both staged and
    // unstaged; for `staged` it simply clears `staged` and leaves `unstaged` intact.
    const nothing =
      scope === 'staged'
        ? s.staged.length === 0
        : scope === 'all'
          ? s.staged.length === 0 && s.unstaged.length === 0
          : s.staged.length === 0 && s.unstaged.length === 0 && s.untracked.length === 0;
    if (nothing) {
      return { created: false };
    }
    // Push a new stash@{0} and re-index the rest (+1).
    for (const entry of state.stashes) entry.index += 1;
    // P10 §8 risk #3: the new stash's baseOid must match the CURRENT HEAD node's
    // id in the graph so withStashNodes renders a node for it. `state.headOid`
    // (MOCK_OID) does NOT match the default fixture's row-0 id, so derive the
    // head node id from the layout getGraph builds (headIndex is always 0).
    const base = prependCommits(buildMockGraph(), state.commits);
    const headNodeId = base.nodes[base.headIndex ?? 0]?.id ?? state.headOid;
    state.stashes.unshift({
      index: 0,
      message: `WIP on ${state.headBranch}: mock stashed changes`,
      oid: randomOid(),
      baseOid: headNodeId,
      ts: Math.floor(Date.now() / 1000),
    });
    // Post-state per scope: `staged` clears only staged; `all` clears tracked
    // (staged+unstaged) but keeps untracked; `allWithUntracked` clears everything.
    s.staged = [];
    if (scope !== 'staged') s.unstaged = [];
    if (scope === 'allWithUntracked') s.untracked = [];
    return { created: true };
  },

  async applyStash(
    repoId: string,
    index: number,
    skipReserved: boolean,
  ): Promise<ApplyStashOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    const entry = state.stashes.find((e) => e.index === index);
    // Demo conflict trigger — mirrors the P8 mergeBranch "conflict" convention.
    if (entry !== undefined && entry.message.includes('conflict')) {
      return { kind: 'conflicts', paths: ['src/app.ts'] };
    }
    // Windows reserved-path recovery flow (mirrors the core preflight): first
    // attempt is blocked and applies nothing; retry with skipReserved applies the
    // rest, leaving the stack unchanged either way (apply never drops).
    if (stashHasReserved(entry)) {
      return skipReserved
        ? { kind: 'appliedSkippingReserved', skipped: [...RESERVED_STASH_PATHS] }
        : { kind: 'reservedPaths', paths: [...RESERVED_STASH_PATHS] };
    }
    // Apply leaves the stack unchanged.
    return { kind: 'applied' };
  },

  async popStash(
    repoId: string,
    index: number,
    skipReserved: boolean,
  ): Promise<ApplyStashOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    const entry = state.stashes.find((e) => e.index === index);
    // Conflict trigger: the entry is RETAINED (libgit2 only drops on clean pop).
    if (entry !== undefined && entry.message.includes('conflict')) {
      return { kind: 'conflicts', paths: ['src/app.ts'] };
    }
    // Reserved-path flow: first attempt blocked; a skipping retry applies the
    // rest but KEEPS the stash (lossless — the reserved blobs live only here).
    if (stashHasReserved(entry)) {
      return skipReserved
        ? { kind: 'appliedSkippingReserved', skipped: [...RESERVED_STASH_PATHS] }
        : { kind: 'reservedPaths', paths: [...RESERVED_STASH_PATHS] };
    }
    // Clean pop: remove the entry, then re-index the survivors.
    state.stashes = state.stashes.filter((e) => e.index !== index);
    state.stashes.forEach((e, i) => (e.index = i));
    return { kind: 'applied' };
  },

  async dropStash(repoId: string, index: number): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    state.stashes = state.stashes.filter((e) => e.index !== index);
    state.stashes.forEach((e, i) => (e.index = i));
  },

  // P58: `sign` accepted but ignored (mock cannot sign; native-only).
  // P59a: `skipHooks` ≡ --no-verify; git runs the commit hooks on amend too.
  async commitAmend(
    repoId: string,
    message: string,
    _sign?: boolean | null,
    skipHooks?: boolean,
  ): Promise<CommitResult> {
    await delay(150);
    const state = requireRepo(repoId);
    const rejection = hookRejectionFor(state, message, skipHooks);
    if (rejection) throw rejection;
    if (message.trim() === '') {
      const err: AppError = { kind: 'emptyMessage', message: 'commit message is empty' };
      throw err;
    }
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
    // Amend rewrites the tip: new oid, staged content folded in, message-only
    // amend allowed (no nothing-to-commit guard). Replace the top commit's
    // summary in the synthetic lane-0 fixture rows.
    state.status.staged = [];
    state.headOid = randomOid();
    const summary = message.trim().split('\n', 1)[0] ?? '';
    if (state.commits.length > 0) {
      state.commits[0] = { ...state.commits[0], oid: state.headOid, summary };
    } else {
      state.commits.unshift({ oid: state.headOid, summary });
    }
    return { oid: state.headOid, summary, branch: state.headBranch };
  },

} satisfies Partial<IpcApi>;
