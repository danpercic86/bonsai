/**
 * P123 §4.4 — owns ONE git-activity subscription for the lifetime of a mount.
 *
 * The backend mints the subscription id, so the id is only known once the
 * subscribe resolves. Two races are handled:
 * - StrictMode (mount → cleanup → mount): the first subscribe resolves AFTER
 *   its cleanup ran, so the `.then` unsubscribes it at once; exactly ONE live
 *   subscription remains.
 * - Events after cleanup (an emit that lands before the unsubscribe does): the
 *   `disposed` guard drops them, so nothing reaches a dead hook.
 *
 * Observability only: a failed subscribe/unsubscribe never throws.
 */
import { useEffect } from 'react';

import { ipc } from '../../ipc';
import type { GitActivityEvent, GitActivityListener } from '../../ipc';

const noop = (): void => undefined;

/** Subscribes `onEvent` to `repoId`'s runs for the lifetime of the mount. */
export function useGitActivitySubscription(repoId: string, onEvent: GitActivityListener): void {
  useEffect(() => {
    let disposed = false;
    let subId: string | null = null;
    const deliver = (ev: GitActivityEvent): void => {
      if (!disposed) onEvent(ev);
    };
    ipc
      .gitActivitySubscribe(repoId, deliver)
      .then((id) => {
        if (disposed) void ipc.gitActivityUnsubscribe(id).catch(noop);
        else subId = id;
      })
      .catch(noop);
    return () => {
      disposed = true;
      if (subId !== null) void ipc.gitActivityUnsubscribe(subId).catch(noop);
    };
  }, [repoId, onEvent]);
}
