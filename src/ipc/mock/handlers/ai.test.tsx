/** T3.4 — ai.ts (default AI-ON paths): availability, conflict-resolution
 *  eligibility, commit-message / compose gating, plan-operation keyword
 *  routing, branch-name validity. The ?ai=off seam lives in urlSeams.test.tsx
 *  (module-init flag → resetModules). */
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { freshRepoPath, run, runErr } from '../../../test/mockIpcKit';
import { repoHandlers } from './repo';
import { aiHandlers } from './ai';
import { mergeHandlers } from './merge';
import { isInvalidBranchName, requireRepo } from '../repoState';

beforeAll(() => vi.useFakeTimers());
afterAll(() => vi.useRealTimers());

async function openDefault(): Promise<string> {
  const { repoId } = await run(repoHandlers.openRepo(freshRepoPath('ai')));
  return repoId;
}

/** Empties every status list + the three-way model (a truly clean tree). */
function makeClean(repoId: string): void {
  const s = requireRepo(repoId);
  s.status.staged = [];
  s.status.unstaged = [];
  s.status.untracked = [];
  s.status.conflicted = [];
  s.mainRs.index = [...s.mainRs.head];
  s.mainRs.workdir = [...s.mainRs.head];
}

describe('checkAiAvailability (default: CLI present)', () => {
  it('reports installed + loggedIn with a version', async () => {
    expect(await run(aiHandlers.checkAiAvailability())).toMatchObject({
      installed: true,
      loggedIn: true,
      version: expect.stringMatching(/^\d/),
    });
  });
});

describe('aiResolveConflict eligibility', () => {
  it('proposes a markerless body for a bothModified conflict, mutating nothing', async () => {
    const repoId = await openDefault();
    await run(mergeHandlers.mergeBranch(repoId, 'demo-conflict'));
    const before = structuredClone(requireRepo(repoId).status.conflicted);
    const proposal = await run(aiHandlers.aiResolveConflict(repoId, 'src/auth.ts'));
    expect(proposal.path).toBe('src/auth.ts');
    expect(proposal.proposedText).not.toContain('<<<<<<<');
    expect(proposal.proposedText).not.toContain('=======');
    expect(proposal.proposedText.length).toBeGreaterThan(0);
    // Read-only: the conflict is still unresolved.
    expect(requireRepo(repoId).status.conflicted).toEqual(before);
    expect(await run(mergeHandlers.listConflicts(repoId))).toHaveLength(2);
  });

  it('rejects aiFailed for deletedByThem and for non-conflicted paths', async () => {
    const repoId = await openDefault();
    await run(mergeHandlers.mergeBranch(repoId, 'demo-conflict'));
    expect((await runErr(aiHandlers.aiResolveConflict(repoId, 'README.md'))).kind).toBe(
      'aiFailed',
    );
    expect((await runErr(aiHandlers.aiResolveConflict(repoId, 'nope.ts'))).kind).toBe('aiFailed');
  });
});

describe('generateCommitMessage / aiComposeCommits gating', () => {
  it('empty staged set → nothingToCommit (no CLI call)', async () => {
    const repoId = await openDefault();
    requireRepo(repoId).status.staged = [];
    expect((await runErr(aiHandlers.generateCommitMessage(repoId))).kind).toBe(
      'nothingToCommit',
    );
  });

  it('with staged changes returns a conventional-commit-shaped proposal', async () => {
    const repoId = await openDefault();
    const proposal = await run(aiHandlers.generateCommitMessage(repoId));
    expect(proposal.message.split('\n')[0]).toMatch(/^\w+(\(.+\))?: /);
    expect(proposal.costUsd).toBeGreaterThan(0);
  });

  it('aiComposeCommits partitions ALL changed paths into disjoint groups', async () => {
    const repoId = await openDefault();
    const proposal = await run(aiHandlers.aiComposeCommits(repoId, null));
    expect(proposal.unassigned).toEqual([]);
    const s = requireRepo(repoId);
    const expected = new Set([
      ...s.status.staged.map((e) => e.path),
      ...s.status.unstaged.map((e) => e.path),
      ...s.status.untracked.map((e) => e.path),
      'src/main.rs', // model workdir differs from head
    ]);
    const assigned = proposal.groups.flatMap((g) => g.files);
    expect(new Set(assigned)).toEqual(expected);
    expect(assigned.length).toBe(expected.size); // disjoint (no duplicates)
    expect(proposal.groups.every((g) => g.message.trim().length > 0)).toBe(true);
  });

  it('a clean working tree → nothingToCommit', async () => {
    const repoId = await openDefault();
    makeClean(repoId);
    expect((await runErr(aiHandlers.aiComposeCommits(repoId, null))).kind).toBe(
      'nothingToCommit',
    );
  });
});

describe('aiPlanOperation keyword routing (deterministic canned plans)', () => {
  const cases: Array<[string, string, string]> = [
    ['undo the merge please', 'reset', 'destructive'],
    ['undo my last commit', 'reset', 'caution'],
    ['switch to main', 'switchBranch', 'safe'],
    ['stash everything', 'stash', 'safe'],
    ['delete that old branch', 'deleteBranch', 'caution'],
    ['discard my changes', 'discard', 'destructive'],
  ];

  it.each(cases)('%j → op %s (danger %s)', async (request, opKind, danger) => {
    const repoId = await openDefault();
    const plan = await run(aiHandlers.aiPlanOperation(repoId, request));
    expect(plan.kind).toBe('proposed');
    if (plan.kind === 'proposed') {
      expect(plan.operation.op.kind).toBe(opKind);
      expect(plan.operation.preview.danger).toBe(danger);
      expect(plan.operation.preview.confirmLabel.length).toBeGreaterThan(0);
    }
  });

  it('anything else → the calm unsupported outcome (not an error)', async () => {
    const repoId = await openDefault();
    const plan = await run(aiHandlers.aiPlanOperation(repoId, 'make me a sandwich'));
    expect(plan.kind).toBe('unsupported');
    if (plan.kind === 'unsupported') expect(plan.reason).toContain('safe git operations');
  });
});

describe('read-only prose handlers return shape-coherent, echoing text', () => {
  it('aiSuggestBranchName candidates are all VALID branch names', async () => {
    const repoId = await openDefault();
    const sources: import('../../types').BranchNameSource[] = [
      { kind: 'working' },
      { kind: 'commitRange', from: 'a'.repeat(40), to: 'b'.repeat(40) },
    ];
    for (const source of sources) {
      const proposal = await run(aiHandlers.aiSuggestBranchName(repoId, source));
      expect(proposal.names.length).toBeGreaterThan(0);
      for (const name of proposal.names) expect(isInvalidBranchName(name)).toBe(false);
    }
  });

  it('aiAnalyzeDiff prefixes the analyzed target; aiDigest echoes the range', async () => {
    const repoId = await openDefault();
    const analysis = await run(
      aiHandlers.aiAnalyzeDiff(repoId, { kind: 'commit', oid: 'abcdef1'.padEnd(40, '0') }, 'explain'),
    );
    expect(analysis.text).toContain('Commit abcdef1');
    const digest = await run(aiHandlers.aiDigest(repoId, { kind: 'lastDays', days: 7 }));
    expect(digest.text).toContain('last 7 day');
  });

  it('aiChangelog resolves sinceLastTag to the canned previous tag', async () => {
    const repoId = await openDefault();
    const log = await run(aiHandlers.aiChangelog(repoId, { kind: 'sinceLastTag', target: null }));
    expect(log).toMatchObject({ fromRef: 'v1.2.0', toRef: 'HEAD' });
    expect(log.text).toContain('### Features');
    const ranged = await run(
      aiHandlers.aiChangelog(repoId, { kind: 'betweenRefs', from: 'v1.0', to: 'v1.1.0' }),
    );
    expect(ranged).toMatchObject({ fromRef: 'v1.0', toRef: 'v1.1.0' });
  });

  it('aiExplainLine / aiSummarizeRange echo their inputs', async () => {
    const repoId = await openDefault();
    const why = await run(aiHandlers.aiExplainLine(repoId, 'src/app.ts', 42, null));
    expect(why.text).toContain('line 42 of src/app.ts');
    const sum = await run(aiHandlers.aiSummarizeRange(repoId, 'main', 'feat'));
    expect(sum).toMatchObject({ base: 'main', target: 'feat', commitCount: 3 });
  });
});
