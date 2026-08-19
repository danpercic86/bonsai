// Toast stack (P1 contract §5). Presentational — App owns the state, the
// monotonic id counter, the 5-toast cap, and the 5 s auto-dismiss timers.

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

/** Fixed top-right overlay, newest on top; rendered unconditionally in App
 *  (also over the empty state). */
export function Toasts({ toasts, onDismiss }: ToastsProps) {
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
