/**
 * P68e — the AI activity dock shell.
 *
 * U1: a DISCLOSURE, not a pane. Collapsed it is one 30px status bar; expanded it is
 * that bar plus a resizable body. It renders `null` until the first run exists, so a
 * user who never touches AI never pays a pixel.
 *
 * U3: NO height animation, anywhere. Changing the dock height re-lays out `.panes`,
 * which re-lays out the graph canvas — a 120ms height transition would force ~8
 * canvas relayouts of a 20k-row graph per toggle. Collapse/expand/resize snap
 * instantly; only opacity/colour fades exist, and they are ≤150ms.
 *
 * U4: nothing in the log is announced. ONE visually-hidden `role="status"` region
 * announces status TRANSITIONS only (§11) — at CLI output speed a live log in an
 * aria-live region makes a screen reader unusable.
 */
import { forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState } from 'react';

import { AiActivityAsk, type AiActivityAskHandle } from './AiActivityAsk';
import { AiActivityHeader } from './AiActivityHeader';
import { AiActivityLog } from './AiActivityLog';
import { AiRunQueue } from './AiRunQueue';
import { AiRunStrip } from './AiRunStrip';
import { PaneDivider } from './PaneDivider';
import {
  AI_DOCK_HEIGHT_DEFAULT,
  AI_DOCK_HEIGHT_MAX,
  AI_DOCK_HEIGHT_MIN,
  AI_DOCK_NUDGE_PX,
  aggregateRuns,
  announceFor,
  clampDockHeight,
  isLiveStatus,
  type AiActivityPanelProps,
  type AiActivityRun,
} from './aiDockFormat';

export type { AiActivityFile, AiActivityRun, AiActivityPanelProps } from './aiDockFormat';

export interface AiActivityPanelHandle {
  /** `Ctrl/Cmd+Shift+A` and the conflict row's `?` affordance: an EXPLICIT user
   *  action, so focusing the reply box is correct here (§4.4-5). */
  focusReply(): void;
  focusLog(): void;
}

/** §4.4-4: focus moves only when the user is demonstrably idle. Claude's question
 *  can arrive while the user is mid-sentence in the commit box, and stealing that
 *  caret is unacceptable. */
function userIsIdle(root: HTMLElement | null): boolean {
  const active = document.activeElement;
  if (active === null || active === document.body) return true;
  return root !== null && root.contains(active);
}

/** §5.1-3, both branches. The dock is one of the four redundant paths to a finished
 *  proposal, so it MUST NOT claim the proposal is in the center pane when FOLD-IN 1
 *  suppressed that open (the user had navigated away) — being told a result is
 *  somewhere it is not is the bug class P68 exists to eliminate. The store records the
 *  outcome as `openedInPane`; this maps it to the one true sentence.
 *
 *  Accepted limit: the flag records what Bonsai DID, not what the centre pane shows
 *  right now, so closing that diff afterwards leaves the first sentence stale. `Review
 *  proposal` re-opens it either way, and a stale "is open" is far milder than the
 *  wrong-by-construction claim it replaces. */
export const HINT_OPENED = 'Proposal is open in the center pane.';
export const HINT_NOT_OPENED =
  'Proposal is ready — choose “Review proposal” to open it in the center pane.';

function reviewHint(canReview: boolean, active: AiActivityRun): string | null {
  if (!canReview || active.status !== 'ready') return null;
  return active.openedInPane ? HINT_OPENED : HINT_NOT_OPENED;
}

export const AiActivityPanel = forwardRef<AiActivityPanelHandle, AiActivityPanelProps>(
  function AiActivityPanel(props, ref) {
    const {
      runs,
      activeKey,
      onSelectRun,
      collapsed,
      onToggleCollapsed,
      height,
      onResizeHeight,
      onCancel,
      onReply,
      onDismiss,
      onReviewFile,
      onRetryFile,
      density,
      streamLogEnabled,
      atCapacity,
    } = props;

    const rootRef = useRef<HTMLElement | null>(null);
    const askRef = useRef<AiActivityAskHandle | null>(null);
    // Live drag height: the persisted value only moves on pointerup, so a drag is
    // ONE settings write, not one per frame (§8). The ref is authoritative (the
    // commit reads it outside of a state updater, so StrictMode's double-invoked
    // updaters can never turn one drag into two settings writes).
    const [dragHeight, setDragHeight] = useState<number | null>(null);
    const dragRef = useRef<number | null>(null);
    const [announce, setAnnounce] = useState('');
    const prevRef = useRef(new Map<string, string>());
    const focusedForRef = useRef<string | null>(null);
    // §4.3: which run+question a reply is in flight for. The store flips the status out
    // of `awaitingInput` optimistically, so this is normally one render long — but it is
    // what makes the disabled textarea + `Sending…` label real rather than hard-coded,
    // and it covers a reply that takes a beat to land. The question is part of the key
    // so a SECOND question on the same run always arrives unlocked.
    const [sending, setSending] = useState<{ key: string; question: string | null } | null>(null);

    const active: AiActivityRun | null =
      runs.find((r) => r.key === activeKey) ?? runs[0] ?? null;

    // ---- announcements (§11): six sentences, each replacing the previous.
    useEffect(() => {
      const next = announceFor(runs, prevRef.current);
      if (next !== null) setAnnounce(next);
    }, [runs]);

    // ---- §4.3: the lock lifts the moment the run leaves `awaitingInput`, asks a new
    // question, or disappears — it is never latched on anything else.
    useEffect(() => {
      if (sending === null) return;
      const run = runs.find((r) => r.key === sending.key);
      if (
        run === undefined ||
        run.status !== 'awaitingInput' ||
        run.question !== sending.question
      ) {
        setSending(null);
      }
    }, [runs, sending]);

    // ---- §4.4-4: focus the reply box at most once per run, and only when idle.
    useEffect(() => {
      if (active === null || active.status !== 'awaitingInput' || collapsed) return;
      if (focusedForRef.current === active.key) return;
      focusedForRef.current = active.key;
      if (!userIsIdle(rootRef.current)) return;
      askRef.current?.focus();
    }, [active, collapsed]);

    useImperativeHandle(ref, () => ({
      focusReply() {
        askRef.current?.focus();
      },
      focusLog() {
        document.getElementById('ai-dock-log')?.focus();
      },
    }));

    const commitHeight = useCallback(() => {
      const live = dragRef.current;
      dragRef.current = null;
      setDragHeight(null);
      if (live !== null) onResizeHeight(live);
    }, [onResizeHeight]);

    const onDrag = useCallback(
      (delta: number) => {
        const next = clampDockHeight((dragRef.current ?? height) + delta, window.innerHeight);
        dragRef.current = next;
        setDragHeight(next);
      },
      [height],
    );

    const onDragStart = useCallback(() => {
      dragRef.current = height;
    }, [height]);

    if (runs.length === 0) return null;

    const aggregate = aggregateRuns(runs);
    const single = runs.length === 1;
    const shown = single ? runs[0] : null;
    const status = single ? (shown?.status ?? 'running') : aggregate.status;
    const cancelRequested = single ? (shown?.cancelRequested ?? false) : aggregate.cancelRequested;
    const live = isLiveStatus(status);
    const bulk = active !== null && active.files.length > 1;
    // §5.1-3: a single ready run has no queue row, so the header carries the button.
    const canReview =
      single && shown !== null && shown.status === 'ready' && shown.files.length <= 1;
    const cancelKey = single ? (shown?.key ?? null) : aggregate.cancelKey;
    const effectiveHeight = dragHeight ?? height;

    return (
      <section
        className="ai-dock"
        role="region"
        aria-label="AI activity"
        data-density={density}
        data-attention={runs.some((r) => r.status === 'awaitingInput') ? 'true' : undefined}
        ref={rootRef}
      >
        {/* §2: the 2px indeterminate sweep, reusing @keyframes header-progress-sweep.
            Rendered only while something is actually happening. */}
        {live && <div className="ai-dock-progress" aria-hidden="true" />}

        {!collapsed && (
          <PaneDivider
            side="ai-dock"
            onResize={onDrag}
            onResizeStart={onDragStart}
            onResizeEnd={commitHeight}
            onReset={() => onResizeHeight(AI_DOCK_HEIGHT_DEFAULT)}
            onExtreme={(edge) =>
              onResizeHeight(
                edge === 'min'
                  ? AI_DOCK_HEIGHT_MIN
                  : clampDockHeight(AI_DOCK_HEIGHT_MAX, window.innerHeight),
              )
            }
            ariaLabel="Resize AI activity dock"
            ariaValues={{
              now: effectiveHeight,
              min: AI_DOCK_HEIGHT_MIN,
              max: AI_DOCK_HEIGHT_MAX,
            }}
            nudgePx={AI_DOCK_NUDGE_PX}
          />
        )}

        <AiActivityHeader
          status={status}
          cancelRequested={cancelRequested}
          subject={single ? (shown?.label ?? '') : `${runs.length} AI runs`}
          subjectIsPath={single && shown !== null && shown.paths.length === 1}
          collapsed={collapsed}
          latest={
            single
              ? (shown?.log[shown.log.length - 1]?.text ?? null)
              : aggregate.latest
          }
          turn={single ? (shown?.turn ?? 0) : 0}
          elapsedMs={single ? (shown?.elapsedMs ?? 0) : aggregate.elapsedMs}
          ticking={live}
          costUsd={single ? (shown?.costUsd ?? null) : aggregate.costUsd}
          thinkingTokens={single ? (shown?.thinkingTokens ?? null) : aggregate.thinkingTokens}
          canReview={canReview}
          canCancel={cancelKey !== null && live}
          canDismiss={single && shown !== null && !isLiveStatus(shown.status)}
          onToggleCollapsed={onToggleCollapsed}
          onCancel={() => cancelKey !== null && onCancel(cancelKey)}
          onDismiss={() => shown !== null && onDismiss(shown.key)}
          onReview={() =>
            shown !== null && onReviewFile(shown.key, shown.paths[0] ?? shown.label)
          }
          onAnswer={() => {
            onToggleCollapsed(false);
            // The dock has to exist before the textarea can take focus.
            window.setTimeout(() => askRef.current?.focus(), 0);
          }}
        />

        {!single && (
          <AiRunStrip
            runs={runs}
            activeKey={active?.key ?? null}
            onSelectRun={onSelectRun}
            atCapacity={atCapacity}
          />
        )}

        {!collapsed && active !== null && (
          <div
            className="ai-dock-body"
            id="ai-dock-body"
            role={single ? undefined : 'tabpanel'}
            aria-labelledby={single ? undefined : `ai-dock-tab-${active.key}`}
            style={{ height: `${effectiveHeight}px` }}
          >
            {active.status === 'failed' && active.error !== null && (
              <div className="error-banner" role="alert">
                <span className="error-banner-text">{active.error}</span>
                <span className="ai-dock-error-next">
                  Nothing was changed. You can retry, or resolve this file by hand.
                </span>
              </div>
            )}
            {bulk && (
              <AiRunQueue
                files={active.files}
                onReviewFile={(path) => onReviewFile(active.key, path)}
                onRetryFile={(path) => onRetryFile(active.key, path)}
              />
            )}
            <AiActivityLog
              log={active.log}
              logDropped={active.logDropped}
              status={active.status}
              partialText={active.partialText}
              streamLogEnabled={streamLogEnabled}
              hint={reviewHint(canReview, active)}
            />
            {active.status === 'awaitingInput' && (
              <AiActivityAsk
                ref={askRef}
                question={active.question}
                sending={sending?.key === active.key}
                onReply={(text) => {
                  setSending({ key: active.key, question: active.question });
                  onReply(active.key, text);
                }}
              />
            )}
          </div>
        )}

        <p className="ai-dock-announce sr-only" role="status" aria-live="polite" aria-atomic="true">
          {announce}
        </p>
      </section>
    );
  },
);
