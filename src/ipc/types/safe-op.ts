import type { ResetMode } from './common';

// ---------------------------------------------------------------- P55 NL→safe-op

/** The resolved-op kinds a plan can propose (P55). Mirrors the Rust `SafeOp` tag
 *  union; each maps 1:1 to an EXISTING typed command on confirm (safeOpDispatch). */
export type SafeOpKind =
  | 'reset'
  | 'revert'
  | 'switchBranch'
  | 'createBranch'
  | 'deleteBranch'
  | 'stash'
  | 'discard'
  | 'merge';

/** A fully-RESOLVED typed operation (P55). Rust resolved every ref/oid; the model
 *  never yields an oid. Discriminated on `kind`. Mirrors the Rust `SafeOp`. */
export type SafeOp =
  | { kind: 'reset'; targetOid: string; targetShort: string; mode: ResetMode }
  | { kind: 'revert'; oid: string; short: string }
  | { kind: 'switchBranch'; name: string; remote: boolean }
  | { kind: 'createBranch'; name: string; atOid: string | null }
  | { kind: 'deleteBranch'; name: string }
  | { kind: 'stash'; message: string | null; includeUntracked: boolean }
  | { kind: 'discard'; paths: string[] }
  | { kind: 'merge'; name: string };

/** Danger tier for the preview badge / confirm variant (P55). */
export type DangerLevel = 'safe' | 'caution' | 'destructive';

/** A ref that moves as part of an op, displayed `fromShort → toShort` (P55). */
export interface RefChange {
  name: string;
  fromShort: string;
  toShort: string;
}

/** One commit line in a preview's dropped list (P55). */
export interface CommitRef {
  short: string;
  summary: string;
}

/** Read-only description of what confirming a `SafeOp` will do (P55). All fields
 *  are display-ready; React only renders. Mirrors the Rust `OperationPreview`. */
export interface OperationPreview {
  title: string;
  summary: string;
  danger: DangerLevel;
  refChanges: RefChange[];
  droppedCommits: CommitRef[];
  addedCommits: number;
  worktreeWarning: string | null;
  confirmLabel: string;
}

/** A resolved, previewable proposal (P55). `rationale` is a one-line "why this
 *  maps to your ask" (Rust-generated). Mirrors the Rust `ProposedOperation`. */
export interface ProposedOperation {
  op: SafeOp;
  preview: OperationPreview;
  rationale: string;
  costUsd: number | null;
}

/** Result of aiPlanOperation (P55). `unsupported` is a NORMAL (non-error) outcome
 *  rendered as a calm "I can't do that safely" message. Mirrors the Rust
 *  `PlanOutcome`. */
export type OperationPlan =
  | { kind: 'proposed'; operation: ProposedOperation }
  | { kind: 'unsupported'; reason: string; costUsd: number | null };
