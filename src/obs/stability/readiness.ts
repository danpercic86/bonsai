/**
 * P120 §3.3 — when is the CURRENT repo interactive? For the repo passed to the
 * last `beginRepoBoot`, the first graph `meta` AND the first status round have
 * both landed (a handled error or an empty result counts; an unborn repo streams
 * `Meta` + `Done`). `markInteractive()` then fires one frame + one task later,
 * i.e. after React committed and painted what landed — so that paint is still
 * labelled `boot`.
 *
 * The signals come from `tapReadiness` on the IPC object (src/ipc/index.ts), the
 * only place that sees both rounds keyed by repo without touching
 * RepoWorkspace.tsx. Other tabs' rounds are ignored.
 */
import { markBoot, markInteractive } from './phase';
import type { IpcApi } from '../../ipc/types';

type Part = 'graph' | 'status';

let current: string | null = null;
let generation = 0;
let landed = { graph: false, status: false };

function nextFrame(cb: () => void): void {
  if (typeof requestAnimationFrame === 'function') requestAnimationFrame(() => setTimeout(cb, 0));
  else setTimeout(cb, 16);
}

/** Repo open / tab switch begins (`null` = no repo; useRepoTabs marks the
 *  empty state interactive once the launch restore has settled). Re-running with the same id is a no-op. */
export function beginRepoBoot(repoId: string | null): void {
  if (repoId === current) return;
  current = repoId;
  generation += 1;
  landed = { graph: false, status: false };
  if (repoId !== null) markBoot();
}

export function noteLanded(repoId: string, part: Part): void {
  if (repoId !== current || landed[part]) return;
  landed[part] = true;
  if (!landed.graph || !landed.status) return;
  const gen = generation;
  nextFrame(() => {
    if (gen === generation) markInteractive();
  });
}

/** Wrap `getStatus` / `streamGraph` so their first settle per boot is noted.
 *  A Proxy, not a spread: every other method is read LIVE from `api` (identity
 *  preserved, and a test's `vi.spyOn(mockIpc, …)` still takes effect), and the
 *  two wrappers call through `api` at call time for the same reason. */
export function tapReadiness(api: IpcApi): IpcApi {
  const getStatus: IpcApi['getStatus'] = (repoId) =>
    api.getStatus(repoId).finally(() => noteLanded(repoId, 'status'));
  const streamGraph: IpcApi['streamGraph'] = (repoId, filter, onChunk) =>
    api
      .streamGraph(repoId, filter, (chunk) => {
        onChunk(chunk);
        if (chunk.kind === 'meta') noteLanded(repoId, 'graph');
      })
      .finally(() => noteLanded(repoId, 'graph'));
  return new Proxy(api, {
    get(target, prop, receiver) {
      if (prop === 'getStatus') return getStatus;
      if (prop === 'streamGraph') return streamGraph;
      return Reflect.get(target, prop, receiver);
    },
  });
}

export function resetReadinessForTests(): void {
  current = null;
  generation = 0;
  landed = { graph: false, status: false };
}
