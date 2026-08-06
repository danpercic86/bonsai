// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { StaleReason } from '../types';

/**
 * P25 §6.3: stale-branch classification seed. Maps a fixture local-branch name
 * to the classification the backend would compute. Branches absent from this map
 * (or the current HEAD / base) are never stale. `experiment-unmerged` is
 * intentionally omitted (NEITHER merged nor gone → excluded from the report).
 */
export const STALE_SEED: Record<
  string,
  { reason: StaleReason; merged: boolean; goneUpstream: boolean }
> = {
  'feature/merged-a': { reason: 'merged', merged: true, goneUpstream: false },
  'feature/merged-b': { reason: 'merged', merged: true, goneUpstream: false },
  'feature/gone': { reason: 'goneUpstream', merged: false, goneUpstream: true },
};
