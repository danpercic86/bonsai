/**
 * P68e §3 — the log body: the child's stdout/stderr, line for line.
 *
 * U11: this IS the terminal view. No PTY, no ANSI parsing, no interactive shell —
 * `⚙ tool(arg)` and `stderr:` lines are already the CLI's own output as Rust
 * forwards it, and an interactive surface would contradict D1/D10.
 *
 * WRAPPING, not two-axis scrolling: `.hook-output-body`'s `white-space: pre` +
 * `overflow: auto` is right for a lint dump and wrong for a 2000-char assistant
 * paragraph. Tool and stderr lines wrap identically.
 *
 * U7 — partial output is QUARANTINED at the bottom: closed disclosure, dashed
 * border, muted mono, no Copy, no Apply, no editable textarea (the deliberate
 * difference from `AiOutputPanel`'s editable card), and a fixed sentence saying
 * Bonsai will not apply it. It is a truncated fragment and must never read as a
 * usable result.
 */
import { useEffect, useRef, useState } from 'react';

import { AI_EVENT_TEXT_MAX } from './aiDockFormat';
import type { AiRunLogLine, AiRunStatus } from './repoWorkspace/useAiRuns';

/** §3: the exact sentence, asserted verbatim by the acceptance test. */
export const PARTIAL_NOTE =
  'Stopped before Claude finished. This text is incomplete — Bonsai will not apply it.';

/** §3 stick-to-bottom hysteresis: a 1-line growth spurt must not re-stick you. */
const UNSTICK_PX = 24;
const RESTICK_PX = 4;

export interface AiActivityLogProps {
  log: AiRunLogLine[];
  logDropped: number;
  status: AiRunStatus;
  partialText: string | null;
  /** `UiSettings.aiStreamLog`. Without it a user who turned streaming off sees an
   *  empty log and concludes the feature is broken again — the exact bug class
   *  P68 exists to remove. */
  streamLogEnabled: boolean;
  /** §5.1-3: the one-line "Proposal is open in the center pane." hint. */
  hint?: string | null;
}

function emptyCopy(props: AiActivityLogProps): string | null {
  if (props.log.length > 0) return null;
  if (!props.streamLogEnabled) {
    return 'Live output is off — turn on "Stream AI output" in Settings to see it here.';
  }
  if (props.status === 'running' || props.status === 'awaitingInput') return 'Starting Claude…';
  return 'No output was captured.';
}

export function AiActivityLog(props: AiActivityLogProps) {
  const { log, logDropped, partialText, hint } = props;
  const scrollRef = useRef<HTMLOListElement | null>(null);
  // Stickiness is a REF, not state: it is read inside the scroll handler and the
  // post-append effect, and a re-render per scroll event would be absurd.
  const stickRef = useRef(true);
  const [unstuck, setUnstuck] = useState(false);
  const [partialOpen, setPartialOpen] = useState(false);

  useEffect(() => {
    const el = scrollRef.current;
    if (el === null || !stickRef.current) return;
    el.scrollTop = el.scrollHeight;
  }, [log]);

  function onScroll() {
    const el = scrollRef.current;
    if (el === null) return;
    const distance = el.scrollHeight - el.scrollTop - el.clientHeight;
    if (distance > UNSTICK_PX) {
      stickRef.current = false;
      setUnstuck(true);
    } else if (distance <= RESTICK_PX) {
      stickRef.current = true;
      setUnstuck(false);
    }
  }

  function jumpToLatest() {
    const el = scrollRef.current;
    stickRef.current = true;
    setUnstuck(false);
    if (el !== null) el.scrollTop = el.scrollHeight;
  }

  const empty = emptyCopy(props);

  return (
    <>
      {hint !== null && hint !== undefined && hint !== '' && (
        <p className="ai-dock-hint">{hint}</p>
      )}
      {/* U4: NO aria-live and NO role="log" — a streaming NDJSON log announced to a
          screen reader is hostile. It is focusable, so it can be read on demand. */}
      <ol
        className="ai-log"
        id="ai-dock-log"
        ref={scrollRef}
        tabIndex={0}
        aria-label="AI output"
        onScroll={onScroll}
      >
        {logDropped > 0 && (
          <li
            className="ai-log-dropped"
            title="Bonsai keeps the last 500 lines of AI output"
          >
            {`↑ ${logDropped.toLocaleString()} earlier lines trimmed`}
          </li>
        )}
        {empty !== null && (
          <li className="ai-log-empty" data-kind="meta">
            {empty}
          </li>
        )}
        {log.map((line) => (
          <li key={line.seq} className="ai-log-line" data-kind={line.kind}>
            {line.text}
            {line.text.length === AI_EVENT_TEXT_MAX && (
              <span
                className="ai-log-trunc"
                title="This line was cut off at 2,000 characters"
              >
                truncated
              </span>
            )}
          </li>
        ))}
      </ol>

      {unstuck && (
        <button
          type="button"
          className="ai-log-jump"
          aria-label="Jump to latest AI output"
          onClick={jumpToLatest}
        >
          {'↓ Jump to latest'}
        </button>
      )}

      {partialText !== null && (
        <div className="ai-dock-partial">
          <button
            type="button"
            className="ai-dock-partial-toggle"
            aria-expanded={partialOpen}
            onClick={() => setPartialOpen((open) => !open)}
          >
            <span aria-hidden="true">{partialOpen ? '▾' : '▸'}</span>
            {' Unfinished output (not usable)'}
          </button>
          {/* The warning is ALWAYS visible; only the fragment itself is behind the
              disclosure. Hiding the "will not apply it" sentence until the user
              opens the block would leave the dangerous half unlabelled. */}
          <p className="ai-dock-partial-note">{PARTIAL_NOTE}</p>
          {partialOpen && <pre className="ai-dock-partial-body mono">{partialText}</pre>}
        </div>
      )}
    </>
  );
}
