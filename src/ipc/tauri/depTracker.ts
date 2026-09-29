// P125 §8: the dependency tracker's IPC surface. Thin wrappers — discovery,
// parsing, the privacy classifier, the network checks, the scheduler and the
// cache all live in Rust (`crates/bonsai-deps` + `src-tauri/src/deps`).
import { listen } from '@tauri-apps/api/event';
import { invoke } from './invoke';
import type { DepsChangedPayload, DepTrackerView } from '../types/depTracker';
import type { Unsubscribe } from '../types/common';

export const depTrackerCommands = {
  getDepTracker(repoId: string): Promise<DepTrackerView> {
    return invoke<DepTrackerView>('deps_get_view', { repoId });
  },

  depsCheckNow(repoId: string): Promise<number> {
    return invoke<number>('deps_check_now', { repoId });
  },

  onDepsChanged(cb: (p: DepsChangedPayload) => void): Promise<Unsubscribe> {
    return listen<DepsChangedPayload>('deps-changed', (e) => cb(e.payload));
  },
};
