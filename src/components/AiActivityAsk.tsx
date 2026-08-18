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
 *
 * SECURITY (audit 2026-08-18, M3): `question` is UNTRUSTED model output, not a Bonsai
 * prompt. Rust already requires the sentinel line to stand alone and strips control
 * characters (`ai::stream::sentinel_question`), which stops a merged file body from
 * arriving here at all; this component owns the other half — the text is attributed to
 * Claude, and a fixed line the model cannot influence states that Bonsai never asks for
 * secrets. Do not fold that line into the question string, and do not render `question`
 * as anything but plain text.
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
          <>
            {/* M3: attribution, NOT decoration. The question text is model output
             *  and reachable by an attacker without a jailbreak (a conflicted file
             *  whose both sides start with the sentinel line merges faithfully into
             *  one), so it must never read as Bonsai asking. */}
            <p className="ai-dock-ask-attrib">Claude wrote this — Bonsai did not:</p>
            <p className="ai-dock-ask-question">{question}</p>
          </>
        )}
        {/* Fixed chrome, deliberately OUTSIDE the interpolated text and rendered even
         *  when there is no question: the whole point is that this sentence is one
         *  the model cannot influence, so a request for a token is visibly refused
         *  by Bonsai itself. */}
        <p className="ai-dock-ask-guard" id="ai-dock-ask-guard">
          {'Bonsai never asks for passwords or tokens. Don’t paste secrets here.'}
        </p>
        <div className="ai-dock-ask-row">
          <textarea
            ref={inputRef}
            className="ai-dock-ask-input"
            aria-label="Your answer to Claude"
            // The guard line comes FIRST so a screen reader hears "never asks for
            // tokens" before the keyboard hint (M3).
            aria-describedby="ai-dock-ask-guard ai-dock-ask-hint"
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
