import { createContext, useContext } from 'react';
import type { ToastTone } from './components/Toasts';

/** P3e §5.5: the single global toast stack lives in App. This context exposes
 *  its `pushToast` so `RepoWorkspace` (one per open tab) and its children can
 *  raise toasts without prop-drilling the callback through every handler. */
export type PushToast = (tone: ToastTone, text: string) => void;

/** Default no-op — App always provides a real implementation via the Provider. */
export const ToastContext = createContext<PushToast>(() => {});

export function usePushToast(): PushToast {
  return useContext(ToastContext);
}
