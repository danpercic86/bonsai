import { useEffect, useRef } from 'react';
import { ipc } from '../../ipc';
import type { Unsubscribe } from '../../ipc';
import type { PushToast } from '../../ToastContext';
import type { RefreshOrigin } from './useCoalescedRefresh';
import type { RefreshScope } from './refreshScope';

type Refresh = (origin: RefreshOrigin, scope: RefreshScope) => Promise<void>;

/** P86a — subscribe THIS repo to the backend `repo-changed` + `tag-auto-sync`
 *  events and route them through the reason-aware coalesced refresh. Refetches
 *  regardless of active so a background tab stays fresh when its watcher fires (§7).
 *
 *  CI-1: the payload `reason` distinguishes the mutation's OWN filesystem echo
 *  (`"fs"`/unknown — the raw notify watcher, still echo-SUPPRESSIBLE via the
 *  `watcher` origin) from a backend-CONFIRMED genuine change (`"fetch"`/`"tags"`),
 *  which is NOT our own echo and so refreshes through the echo-BYPASSING
 *  `external` origin — even inside a fetch's own armed window.
 *
 *  CI-3: the `tag-auto-sync` completion event surfaces the adopted/moved count as a
 *  toast and refreshes the tag list (refsOnly). Extracted from RepoWorkspace so the
 *  container stays a thin composer. */
export function useRepoChangeSubscription(
  repoId: string,
  refresh: Refresh,
  pushToast: PushToast,
): void {
  // Read pushToast via a ref so its (settings-derived) identity churn never
  // re-subscribes the watcher — the effect stays keyed to [repoId, refresh].
  const pushToastRef = useRef(pushToast);
  pushToastRef.current = pushToast;

  useEffect(() => {
    let cancelled = false;
    const unsubs: Unsubscribe[] = [];
    const subscribe = async () => {
      const off = await ipc.onRepoChanged((p) => {
        if (p.repoId !== repoId) return;
        switch (p.reason) {
          case 'fetch':
            // Auto-fetch tick / fetch fold-in: genuine remote update.
            void refresh('external', 'remoteMeta');
            break;
          case 'tags':
            // Async tag-sync adopted/moved local tags: refsOnly refetches branches
            // (→ the tag list) + the graph's tag pills.
            void refresh('external', 'refsOnly');
            break;
          default:
            // Raw notify watcher (reason "fs"/unknown) = the mutation's own fs echo.
            void refresh('watcher', 'full');
        }
      });
      if (cancelled) {
        off();
        return;
      }
      unsubs.push(off);
      const offTag = await ipc.onTagAutoSync((e) => {
        if (e.repoId !== repoId) return;
        const { adopted, moved, remote } = e.report;
        const n = adopted.length + moved.length;
        if (n > 0) {
          const detail: string[] = [];
          if (adopted.length > 0) detail.push(`${adopted.length} adopted`);
          if (moved.length > 0) detail.push(`${moved.length} updated`);
          const from = remote !== '' ? ` from ${remote}` : '';
          pushToastRef.current(
            'success',
            `Synced ${n} tag${n === 1 ? '' : 's'}${from} (${detail.join(', ')})`,
          );
        }
        void refresh('external', 'refsOnly');
      });
      if (cancelled) {
        offTag();
        return;
      }
      unsubs.push(offTag);
    };
    // Subscription loss = degraded live refresh only (manual refresh + focus rescan
    // still work) — log, don't crash.
    void subscribe().catch((e: unknown) => {
      console.error('repo-changed subscription failed', e);
    });
    return () => {
      cancelled = true;
      for (const unsub of unsubs) unsub();
    };
  }, [repoId, refresh]);
}
