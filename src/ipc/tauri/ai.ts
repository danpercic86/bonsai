import { invoke, Channel } from '@tauri-apps/api/core';
import type { AiAnalysis, AiAnalysisMode, AiAvailability, AiChangelog, AiDiffTarget, AiDigestRange, AiResolveBatch, AiResolveProposal, AiRunEvent, AiSummary, BranchNameProposal, BranchNameSource, ChangelogRange, CommitMessageProposal, ComposeApplyResult, ComposePlan, ComposeProposal, OperationPlan, PrDescription } from '../types';

export const aiCommands = {

  // P13: Claude Code CLI health probe + AI conflict resolution (proposal only).
  checkAiAvailability(): Promise<AiAvailability> {
    return invoke<AiAvailability>('check_ai_availability');
  },

  aiResolveConflict(repoId: string, path: string): Promise<AiResolveProposal> {
    return invoke<AiResolveProposal>('ai_resolve_conflict', { repoId, path });
  },

  // P15a: generate a commit message from the staged diff (proposal only).
  generateCommitMessage(repoId: string): Promise<CommitMessageProposal> {
    return invoke<CommitMessageProposal>('generate_commit_message', { repoId });
  },

  // P15b: explain/review a diff target (read-only prose).
  aiAnalyzeDiff(
    repoId: string,
    target: AiDiffTarget,
    mode: AiAnalysisMode,
  ): Promise<AiAnalysis> {
    return invoke<AiAnalysis>('ai_analyze_diff', { repoId, target, mode });
  },

  // P28: AI "what changed" digest over a selectable range (read-only prose).
  aiDigest(repoId: string, range: AiDigestRange): Promise<AiAnalysis> {
    return invoke<AiAnalysis>('ai_digest', { repoId, range });
  },

  // P56a: grouped Markdown release notes for a tag/ref range (or since the last
  // tag). Read-only; WRITES NOTHING; does NOT emit repo-changed. Fully local.
  aiChangelog(repoId: string, range: ChangelogRange): Promise<AiChangelog> {
    return invoke<AiChangelog>('ai_changelog', { repoId, range });
  },

  // P64: AI PR title + Markdown body grounded in base..head. Read-only; WRITES
  // NOTHING; never posts. Fills the create-PR form for the user to review/edit.
  aiGeneratePrDescription(repoId: string, base: string, head: string): Promise<PrDescription> {
    return invoke<PrDescription>('ai_generate_pr_description', { repoId, base, head });
  },

  // P53a: AI "why does this line exist" — blame-why (read-only prose).
  aiExplainLine(
    repoId: string,
    path: string,
    lineNo: number,
    atOid: string | null,
  ): Promise<AiAnalysis> {
    return invoke<AiAnalysis>('ai_explain_line', { repoId, path, lineNo, atOid });
  },

  // P15c: summarize the commits/diff unique to `target` vs `base` (read-only prose).
  aiSummarizeRange(repoId: string, base: string, target: string): Promise<AiSummary> {
    return invoke<AiSummary>('ai_summarize_range', { repoId, base, target });
  },

  // P53c: AI branch-name suggestions from a grounding source (read-only; writes nothing).
  aiSuggestBranchName(repoId: string, source: BranchNameSource): Promise<BranchNameProposal> {
    return invoke<BranchNameProposal>('ai_suggest_branch_name', { repoId, source });
  },

  // P54a: propose grouping the working-tree changes into logical commits (read-only).
  aiComposeCommits(repoId: string, guidance: string | null): Promise<ComposeProposal> {
    return invoke<ComposeProposal>('ai_compose_commits', { repoId, guidance });
  },

  // P55a: map a natural-language request to ONE allowlisted, previewable git op
  // (read-only; WRITES NOTHING). The mutation runs later via the resolved op's
  // existing typed command on explicit confirm (safeOpDispatch, P55c).
  aiPlanOperation(repoId: string, request: string): Promise<OperationPlan> {
    return invoke<OperationPlan>('ai_plan_operation', { repoId, request });
  },

  // P54b: apply a reviewed composer plan as an ordered stage+commit sequence (atomic).
  applyComposedCommits(repoId: string, plan: ComposePlan): Promise<ComposeApplyResult> {
    return invoke<ComposeApplyResult>('apply_composed_commits', { repoId, plan });
  },

  // P68 §8.4: the streaming conflict resolver. `runId` is NOT the return value —
  // it arrives on the first channel event (`started`), because this promise settles
  // only when the whole run has ended (D8). Callers must therefore learn the id from
  // `onEvent` if they want to cancel or reply.
  aiResolveConflictStream(
    repoId: string,
    paths: string[],
    onEvent: (e: AiRunEvent) => void,
  ): Promise<AiResolveBatch> {
    const channel = new Channel<AiRunEvent>();
    channel.onmessage = onEvent;
    // Tauri auto-serializes the Channel as the `on_event` command argument
    // (mirrors historyIndexBuild / cloneRepo).
    return invoke<AiResolveBatch>('ai_resolve_conflict_stream', {
      repoId,
      paths,
      onEvent: channel,
    });
  },

  aiCancelRun(runId: string): Promise<void> {
    return invoke<void>('ai_cancel_run', { runId });
  },

  aiReplyRun(runId: string, text: string): Promise<void> {
    return invoke<void>('ai_reply_run', { runId, text });
  },
};
