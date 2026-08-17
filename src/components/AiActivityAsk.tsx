/**
 * P68e §4 — the awaiting-input affordance: the most consequential state in the dock.
 *
 * The user's question was "what if Claude needs to ask me something about a conflict
 * and it's crucial for resolving it?" — so this block is warning-tinted, sits below
 * the log, and owns all of its keyboard handling.
 *
 * U5 — `Enter` sends, `Shift+Enter` is a newline, `Ctrl/Cmd+Enter` also sends. The
 * reply is a chat answer and the user is blocked, so Enter is the idiom they expect;
 * Ctrl/Cmd+Enter is kept as a superset so commit-box muscle memory works.
 *
 * `Esc` blurs back to the log and NEVER collapses the dock or cancels the run — Esc
 * must not be able to destroy a blocked run's context.
 *
 * U6 — focus is never STOLEN here: this component only focuses when its imperative
 * `focus()` is called, and the caller (`AiActivityPanel`) is the one that decides the
 * user is demonstrably idle.
 */
import { forwardRef, useImperativeHandle, useRef, useState } from 'react';

export interface AiActivityAskHandle {
  focus(): void;
}

export interface AiActivityAskProps {
  question: string | null;
  /** The reply is in flight (status left `awaitingInput` yet?). */
  sending: boolean;
  onReply(text: string): void;
}

export const AiActivityAsk = forwardRef<AiActivityAskHandle, AiActivityAskProps>(
  function AiActivityAsk({ question, sending, onReply }, ref) {
    const [draft, setDraft] = useState('');
    const inputRef = useRef<HTMLTextAreaElement | null>(null);
    const logId = 'ai-dock-log';

    useImperativeHandle(ref, () => ({
      focus() {
        inputRef.current?.focus();
      },
    }));

    function send() {
      const text = draft.trim();
      // An all-whitespace draft cannot submit, and does so silently — no error flash
      // in front of someone who is already blocked.
      if (text === '' || sending) return;
      setDraft('');
      onReply(text);
    }

    function onKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
      if (e.key === 'Escape') {
        e.stopPropagation();
        document.getElementById(logId)?.focus();
        return;
      }
      if (e.key !== 'Enter') return;
      if (e.shiftKey) return; // newline
      e.preventDefault();
      send();
    }

    return (
      <div className="ai-dock-ask" role="group" aria-label="Claude needs your answer">
        <div className="ai-dock-ask-head">
          <span className="ai-dock-ask-glyph" aria-hidden="true">
            ?
          </span>
          <span className="ai-dock-ask-label">Claude needs your answer</span>
        </div>
        {question !== null && question !== '' && (
          <p className="ai-dock-ask-question">{question}</p>
        )}
        <div className="ai-dock-ask-row">
          <textarea
            ref={inputRef}
            className="ai-dock-ask-input"
            aria-label="Your answer to Claude"
            aria-describedby="ai-dock-ask-hint"
            placeholder="Type your answer for Claude…"
            value={draft}
            disabled={sending}
            rows={2}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={onKeyDown}
          />
          <button
            type="button"
            className="btn-primary ai-dock-send"
            disabled={sending || draft.trim() === ''}
            onClick={send}
          >
            {sending ? 'Sending…' : 'Send'}
          </button>
        </div>
        <p className="ai-dock-ask-hint" id="ai-dock-ask-hint">
          {'Enter sends · Shift+Enter for a new line'}
        </p>
      </div>
    );
  },
);
