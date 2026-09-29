import type { Unsubscribe } from './common';
import type { DepsChangedPayload, DepTrackerView } from './depTracker';

/**
 * P125 §8 — the dependency-tracker IPC surface, split out of `ipc-api.ts` for the
 * same reason as `ipc-api-tools.ts`: that file is over the file-size limit and
 * may only shrink. `IpcApi` extends this.
 *
 * Settings (`depTracker.enabled` / `intervalHours`) are NOT here — they ride
 * `getUiSettings` / `setUiSettings` like every other preference.
 */
export interface IpcApiDeps {
  /** `deps_get_view`. Never touches the repo when disabled. Rejects noRepo. */
  getDepTracker(repoId: string): Promise<DepTrackerView>;
  /** `deps_check_now`. Manual refresh; resolves the running-or-new checkId.
   *  Rejects noRepo | other("dependency tracker is disabled"). */
  depsCheckNow(repoId: string): Promise<number>;
  /** The `deps-changed` event. Progress fires at most 4/s. */
  onDepsChanged(cb: (p: DepsChangedPayload) => void): Promise<Unsubscribe>;
}
