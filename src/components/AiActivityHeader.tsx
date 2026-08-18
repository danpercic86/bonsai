/**
 * P68e §2 — THE COLLAPSED BAR. This one row is the actual deliverable: the reported
 * failure was "I clicked AI and had no feedback", and the fix is a bar the user
 * cannot miss — status word + subject + live elapsed + cost + the latest output line
 * + Cancel. Expanding is for detail, never for basic reassurance (U2).
 *
 * PURE: every decision is made by `AiActivityPanel` and handed down as a primitive,
 * so this file has no state, no effects, and reads top-to-bottom in element order.
 */
import {
  COST_UNKNOWN_TITLE,
  formatCost,
  formatElapsed,
  formatThinkingTokens,
  pillFor,
  THINKING_TOKENS_TITLE,
} from './aiDockFormat';
import { splitPath } from './StatusFileRow';
import type { AiRunStatus } from './repoWorkspace/useAiRuns';

export interface AiActivityHeaderProps {
  status: AiRunStatus;
  cancelRequested: boolean;
  /** A path (rendered dir + name) or a synthetic label like `3 conflicts`. */
  subject: string;
  subjectIsPath: boolean;
  collapsed: boolean;
  /** Latest log line; shown COLLAPSED ONLY, and only while running (§2). */
  latest: string | null;
  turn: number;
  elapsedMs: number;
  /** Still counting ⇒ `title="Elapsed"`; terminal ⇒ `title="Took m:ss"`. */
  ticking: boolean;
  costUsd: number | null;
  /** §12-B1: the live thinking-token estimate, shown beside the cost while `$—` is
   *  all the cost column can honestly say. Null ⇒ nothing is rendered. */
  thinkingTokens: number | null;
  canReview: boolean;
  canCancel: boolean;
  canDismiss: boolean;
  onToggleCollapsed(next: boolean): void;
  onCancel(): void;
  onDismiss(): void;
  onReview(): void;
  onAnswer(): void;
}

export function AiActivityHeader(props: AiActivityHeaderProps) {
  const {
    status,
    cancelRequested,
    subject,
    subjectIsPath,
    collapsed,
    latest,
    turn,
    elapsedMs,
    ticking,
    costUsd,
    thinkingTokens,
    canReview,
    canCancel,
    canDismiss,
    onToggleCollapsed,
    onCancel,
    onDismiss,
    onReview,
    onAnswer,
  } = props;

  const pill = pillFor(status, cancelRequested);
  const elapsed = formatElapsed(elapsedMs);
  const thinking = formatThinkingTokens(thinkingTokens);
  const split = subjectIsPath ? splitPath(subject) : null;
  // The activity line answers "what is it doing" at a glance, so it is only useful
  // while something IS happening; a stale tool call under `Ready` would mislead.
  const showActivity = collapsed && status === 'running' && latest !== null && latest !== '';

  return (
    <div className="ai-dock-header">
      <button
        type="button"
        className="ai-dock-toggle"
        aria-expanded={!collapsed}
        aria-controls="ai-dock-body"
        aria-label="AI activity"
        title={collapsed ? 'Show AI output' : 'Hide AI output'}
        onClick={() => onToggleCollapsed(!collapsed)}
      >
        <span aria-hidden="true">{collapsed ? '⌃' : '⌄'}</span>
      </button>

      <span className="ai-dock-status" data-status={pill.dataStatus}>
        <span className="ai-dock-status-glyph" aria-hidden="true">
          {pill.glyph}
        </span>
        {pill.label}
      </span>

      <span className="ai-dock-subject mono" title={subject}>
        {split === null ? (
          subject
        ) : (
          <>
            {split.dir !== null && <span className="ai-dock-dir">{split.dir}</span>}
            <span className="ai-dock-name">{split.name}</span>
          </>
        )}
      </span>

      {showActivity && (
        <span className="ai-dock-activity mono" title={latest ?? undefined}>
          {latest}
        </span>
      )}

      {turn >= 2 && (
        <span className="ai-dock-turn" title="Each reply from Claude is one turn">
          {`turn ${turn}`}
        </span>
      )}

      <span
        className="ai-dock-elapsed mono"
        title={ticking ? 'Elapsed' : `Took ${elapsed}`}
      >
        {elapsed}
      </span>

      {/* §12-B1: the ONLY live spend signal before the first turn boundary. Estimated,
          thinking-tokens only, and never converted into money. */}
      {thinking !== null && (
        <span className="ai-dock-thinking mono" title={THINKING_TOKENS_TITLE}>
          {thinking}
        </span>
      )}

      <span
        className="ai-dock-cost mono"
        title={costUsd === null ? COST_UNKNOWN_TITLE : 'Cost of this run so far'}
      >
        {formatCost(costUsd)}
      </span>

      {canReview && (
        <button
          type="button"
          className="btn-primary ai-dock-review"
          title="Open the proposal in the center pane"
          onClick={onReview}
        >
          Review proposal
        </button>
      )}

      {collapsed && status === 'awaitingInput' && (
        <button type="button" className="btn-primary ai-dock-answer" onClick={onAnswer}>
          Answer
        </button>
      )}

      {canCancel && (
        <button
          type="button"
          className="btn-danger ai-dock-cancel"
          disabled={cancelRequested}
          aria-label={cancelRequested ? 'Stopping the AI run' : 'Cancel the AI run'}
          onClick={onCancel}
        >
          {cancelRequested ? 'Stopping…' : 'Cancel'}
        </button>
      )}

      {canDismiss && (
        <button
          type="button"
          className="btn-icon ai-dock-dismiss"
          aria-label="Dismiss this run"
          title="Remove from the AI activity dock"
          onClick={onDismiss}
        >
          <span aria-hidden="true">✕</span>
        </button>
      )}
    </div>
  );
}
