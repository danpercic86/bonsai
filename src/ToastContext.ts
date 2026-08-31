import { createContext, useContext } from 'react';
import type { ToastTone } from './components/Toasts';

/** P3e §5.5: the single global toast stack lives in App. This context exposes
 *  its `pushToast` so `RepoWorkspace` (one per open tab) and its children can
 *  raise toasts without prop-drilling the callback through every handler. */
/** `key` (P70, UI §10.1): an optional dedupe key. A push whose key matches a
 *  visible toast replaces it IN PLACE (or is a no-op when the text is identical)
 *  instead of stacking — see App's `pushToast`. Omitting it is the pre-P70
 *  behaviour and remains correct for every existing call site. */
export type PushToast = (tone: ToastTone, text: string, key?: string) => void;

/** Default no-op — App always provides a real implementation via the Provider. */
export const ToastContext = createContext<PushToast>(() => {});

export function usePushToast(): PushToast {
  return useContext(ToastContext);
}
