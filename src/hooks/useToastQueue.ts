// P3e §5.5 / P70: the single global toast stack. Extracted verbatim from App so
// the container only wires the `Toasts` render and the ToastContext provider —
// nothing else in the app owns toast identity, timers, or the dedupe key.
import { useCallback, useRef, useState } from 'react';
import { applyToastPush } from '../components/toastQueue';
import type { Toast, ToastTone } from '../components/Toasts';
import type { PushToast } from '../ToastContext';

export interface UseToastQueue {
  toasts: Toast[];
  pushToast: PushToast;
  dismissToast: (id: number) => void;
}

export function useToastQueue(): UseToastQueue {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const toastId = useRef(0);

  const dismissToast = useCallback((id: number) => {
    setToasts((cur) => cur.filter((t) => t.id !== id));
  }, []);

  // `key` (P70, UI §10.1) coalesces a repeatable failure into ONE toast:
  //   same key + same text -> no-op (no remount, no flicker, no re-announce)
  //   same key + new  text -> replace IN PLACE, same slot, new id + timer
  //   no key               -> the pre-P70 behaviour, byte for byte.
  // Error toasts are sticky, so without this three failed presses would leave
  // three permanent identical toasts — the exact symptom P70 exists to kill.
  const pushToast = useCallback(
    (tone: ToastTone, text: string, key?: string) => {
      const id = ++toastId.current;
      const sticky = tone === 'error';
      // The updater stays PURE — nothing is read back out of it (React may run
      // it at render time, and StrictMode runs it twice). The timer decision is
      // therefore made from the arguments alone: a same-key/same-text push is a
      // no-op inside `applyToastPush`, and arming a timer for its unrendered id
      // is harmless — `dismissToast` finds nothing to remove, exactly as it
      // already does for a keyed toast that was replaced in place.
      setToasts((cur) => applyToastPush(cur, { id, tone, text, sticky, key }));
      if (!sticky) window.setTimeout(() => dismissToast(id), 5000);
    },
    [dismissToast],
  );

  return { toasts, pushToast, dismissToast };
}
