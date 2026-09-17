// The mock's `?forge=off` offline gate, shared by every forge handler that
// would hit the network in the real backend.
//
// Own module (CLAUDE.md file-size discipline) so the handler files split out of
// `forge.ts` — currently `./forgePrDiffHandlers` — can enforce the same gate
// without duplicating the sentinel or the error text.
import { query as urlParam } from '../repoState';
import type { AppError } from '../../types';

/** `?forge=off` — the whole provider is unreachable for this session. */
export const FORGE_OFF = urlParam('forge') === 'off';

/**
 * Throws the mock's offline rejection when `?forge=off` is set. Mirrors what a
 * real network failure surfaces (`networkError`), so the UI takes its
 * fetch-failed branch.
 */
export function offGuard(): void {
  if (FORGE_OFF) {
    const err: AppError = { kind: 'networkError', message: 'mock: forge is offline (?forge=off)' };
    throw err;
  }
}
