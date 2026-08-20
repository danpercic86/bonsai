// Toast stack (P1 contract §5). Presentational — App owns the state, the
// monotonic id counter, the 5-toast cap, and the 5 s auto-dismiss timers.

import { useMockToastSeam } from '../ipc/mock/handlers/toasts';
import { usePushToast } from '../ToastContext';

export type ToastTone = 'error' | 'success' | 'warning' | 'info';

export interface Toast {
  /** Monotonic, App-owned counter. */
  id: number;
  tone: ToastTone;
  text: string;
  /** true => stays until dismissed (all 'error' toasts); false => auto-dismiss 5 s. */
  sticky: boolean;
  /** P70 (UI §10.1): optional dedupe key. Two toasts never share a key — a push
   *  with an existing key REPLACES that toast in place (or is a no-op when the
   *  text is identical), which is what keeps repeated failed presses from
   *  stacking N identical sticky error toasts. Ignored by this presentational
   *  component; the rule lives in App's `pushToast`. */
  key?: string;
}

export interface ToastsProps {
  toasts: Toast[];
  onDismiss(id: number): void;
}

/** P74 §1.2: tone glyphs — shape, not colour, carries severity (WCAG 1.4.1).
 *  Four distinct silhouettes from the existing house vocabulary. Module-local
 *  on purpose: exporting a non-component would trip
 *  `react-refresh/only-export-components`. Deliberately not `⚠` for error
 *  (that means "failed" in the AI dock), not `ℹ` for info (emoji
 *  presentation ignores `--h`), and not `✕` (the dismiss glyph). */
const TONE_GLYPH: Record<ToastTone, string> = {
  error: '⊘',
  warning: '⚠',
  success: '✓',
  info: '●',
};

/** Fixed top-right overlay, newest on top; rendered unconditionally in App
 *  (also over the empty state). */
export function Toasts({ toasts, onDismiss }: ToastsProps) {
  // P74: browser-harness seam (?toasts=demo|long|cap) — a no-op outside
  // VITE_MOCK_IPC. Hosted here because this is the only always-mounted node
  // inside App's ToastContext.Provider that isn't over the file-size ratchet;
  // pushes go through App's real pushToast, so the cap/sticky/dedupe rules of
  // §10.1 are exercised rather than bypassed.
  useMockToastSeam(usePushToast());
  if (toasts.length === 0) return null;
  return (
    <div className="toast-stack" aria-live="polite">
      {toasts
        .slice()
        .reverse()
        .map((toast) => (
          <div
            key={toast.id}
            className={`toast toast-${toast.tone}`}
            role={toast.tone === 'error' ? 'alert' : undefined}
          >
            <span className="toast-glyph" aria-hidden="true">
              {TONE_GLYPH[toast.tone]}
            </span>
            <span className="toast-text">{toast.text}</span>
            <button
              type="button"
              className="toast-dismiss"
              aria-label="Dismiss"
              onClick={() => onDismiss(toast.id)}
            >
              {'✕'}
            </button>
          </div>
        ))}
    </div>
  );
}
