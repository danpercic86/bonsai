// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { AI_OFF, delay, requireRepo, stripConflictMarkers } from '../repoState';
import type { AiAnalysis, AiAnalysisMode, AiAvailability, AiDiffTarget, AiDigestRange, AiResolveProposal, AiSummary, AppError, CommitMessageProposal } from '../../types';

export const aiHandlers = {
  async checkAiAvailability(): Promise<AiAvailability> {
    await delay(150);
    if (AI_OFF) {
      return {
        installed: false,
        loggedIn: false,
        version: null,
        detail: 'Claude Code CLI not found on PATH',
      };
    }
    return {
      installed: true,
      loggedIn: true,
      version: '2.1.220',
      detail: 'Claude Code 2.1.220 ready',
    };
  },

  // P13: propose an AI resolution for one conflicted path. Writes NOTHING — the
  // apply step is the existing resolveConflictText (P12). Only text-mergeable
  // kinds (bothModified/bothAdded) are eligible; anything else → aiFailed.
  async aiResolveConflict(repoId: string, path: string): Promise<AiResolveProposal> {
    await delay(600);
    const state = requireRepo(repoId);
    const entry = state.conflicts.find((c) => c.path === path);
    if (entry === undefined || (entry.kind !== 'bothModified' && entry.kind !== 'bothAdded')) {
      const err: AppError = { kind: 'aiFailed', message: 'AI resolution unavailable for this file' };
      throw err;
    }
    const file = state.conflictTexts.get(path);
    // Derive a plausible markerless body from the seeded marker fixture. Do NOT
    // mutate state: the proposal is only applied when the caller feeds it to
    // resolveConflictText (ProposeReview accept / AutoResolve).
    const proposedText = file !== undefined ? stripConflictMarkers(file.text) : '';
    return { path, proposedText, costUsd: 0.012 };
  },

  // P15a: propose a commit message from the staged diff. Writes NOTHING — the
  // caller drops the text into the commit box to edit before committing. `?ai=off`
  // simulates a missing CLI; an empty staged set → nothingToCommit (no CLI call).
  async generateCommitMessage(repoId: string): Promise<CommitMessageProposal> {
    await delay(500);
    const state = requireRepo(repoId);
    if (AI_OFF) {
      const err: AppError = {
        kind: 'aiFailed',
        message: 'Claude Code CLI not found on PATH',
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
    return {
      message:
        'feat(sidebar): add branch summary action\n\n' +
        '- wire ai_summarize_range command\n' +
        '- add context-menu entry',
      costUsd: 0.004,
    };
  },

  // P15b: explain/review a diff target (read-only prose). Writes NOTHING. Does
  // NOT enforce the consent gate (matches aiResolveConflict; the frontend gates
  // the affordances). `?ai=off` simulates a missing CLI; else canned prose keyed
  // on `mode`, with a tiny per-target prefix so the panel shows what was analyzed.
  async aiAnalyzeDiff(
    repoId: string,
    target: AiDiffTarget,
    mode: AiAnalysisMode,
  ): Promise<AiAnalysis> {
    await delay(500);
    requireRepo(repoId);
    if (AI_OFF) {
      const err: AppError = {
        kind: 'aiFailed',
        message: 'Claude Code CLI not found on PATH',
      };
      throw err;
    }
    let prefix = '';
    if (target.kind === 'commit') {
      prefix = `Commit ${target.oid.slice(0, 7)}: `;
    } else if (target.kind === 'workdirFile') {
      prefix = `${target.path}: `;
    } else if (target.kind === 'worktree') {
      prefix = 'Working tree: ';
    } else if (target.kind === 'branch') {
      prefix = `Branch ${target.name} vs main: `;
    }
    // P25 B1: worktree/branch review scopes get canned Review prose so the
    // browser harness exercises the same AiOutputPanel plumbing.
    if (mode === 'review' && target.kind === 'worktree') {
      return {
        text:
          prefix +
          'Review: 3 files changed; the new error path in commands.rs lacks a ' +
          'test; otherwise LGTM.',
        costUsd: 0.006,
      };
    }
    if (mode === 'review' && target.kind === 'branch') {
      return {
        text:
          prefix +
          'Review: the branch adds a focused feature; consider squashing the ' +
          'two fixup commits and adding a test for the new base-resolution path. ' +
          'No correctness concerns spotted.',
        costUsd: 0.006,
      };
    }
    const text =
      mode === 'review'
        ? prefix +
          'Review: no blocking issues. Consider a null-check on the new branch ' +
          'lookup in Sidebar.tsx; the added revwalk is unbounded — confirm the ' +
          'AI_SUMMARY_MAX_COMMITS cap is applied. Style LGTM.'
        : prefix +
          'This change adds a "Summarize branch" context-menu action in the sidebar ' +
          'and a matching ai_summarize_range command that gathers base..target ' +
          'commits plus a diffstat and calls the local Claude CLI.';
    return { text, costUsd: 0.006 };
  },

  // P28: AI "what changed" digest over a selectable range (read-only prose).
  // Writes NOTHING. Does NOT enforce the consent gate (matches aiAnalyzeDiff;
  // the frontend gates the affordance). `?ai=off` simulates a missing CLI;
  // else canned prose keyed on `range.kind`, echoing the range so the harness
  // shows what was digested.
  async aiDigest(repoId: string, range: AiDigestRange): Promise<AiAnalysis> {
    await delay(700);
    requireRepo(repoId);
    if (AI_OFF) {
      const err: AppError = {
        kind: 'aiFailed',
        message: 'Claude Code CLI not found on PATH',
      };
      throw err;
    }
    let text: string;
    if (range.kind === 'betweenRefs') {
      text =
        `Digest ${range.from}..${range.to}: Over this range the team landed the ` +
        'worktrees feature (sidebar section, create dialog, lifecycle commands) and ' +
        'hardened the AI review path; most churn is in src-tauri/src and src/components.';
    } else if (range.kind === 'lastDays') {
      text =
        `Digest, last ${range.days} day(s): Mostly polish — mock-harness fixes and ` +
        'docs updates; one behavioral change in the watcher debounce.';
    } else {
      text =
        `Digest since ${range.oid.slice(0, 7)}: Two workstreams — worktree UX and ` +
        'stale-branch cleanup — plus test scaffolding.';
    }
    return { text, costUsd: 0.01 };
  },

  // P15c: summarize the commits/diff unique to `target` vs `base` (read-only
  // prose). Writes NOTHING. Does NOT enforce the consent gate (matches
  // aiAnalyzeDiff; the frontend gates the affordance). `?ai=off` simulates a
  // missing CLI; else a canned summary echoing base/target.
  async aiSummarizeRange(repoId: string, base: string, target: string): Promise<AiSummary> {
    await delay(500);
    requireRepo(repoId);
    if (AI_OFF) {
      const err: AppError = {
        kind: 'aiFailed',
        message: 'Claude Code CLI not found on PATH',
      };
      throw err;
    }
    return {
      text:
        'This branch introduces the P15 in-app AI features: commit-message ' +
        'generation, explain/review of diffs, and branch/range summaries — three ' +
        'thin consumers of the existing run_claude primitive. No new settings or ' +
        'process code; all read-only.',
      base,
      target,
      commitCount: 3,
      costUsd: 0.008,
    };
  },

  // Stateful rebase mock (P3d contract §7.2). A repo seeded with a rebase starts
  // paused (step 2/3); rebaseBranch is the clean-rebase demo path. Shares
  // opState/conflicts/conflictTexts with merge, now per-repo.
} satisfies Partial<IpcApi>;
