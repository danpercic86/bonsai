// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { delay, query } from '../repoState';
import type { AppError, UpdateCheckResult, UpdateProgress } from '../../types';

// P42 (INV-2): the update flow's harness seam. `?update=available|none|error`
// read once at module init (mirrors AI_OFF). Default (absent/other) ⇒ 'none'.
const UPDATE_MODE = ((): 'available' | 'none' | 'error' => {
  const q = query('update');
  return q === 'available' || q === 'error' ? q : 'none';
})();
const MOCK_CURRENT_VERSION = '0.1.0';
const MOCK_NEXT_VERSION = '0.2.0';
/** Set true by a successful `checkForUpdate` in `available` mode; gates
 *  `downloadAndInstallUpdate` (mirrors the real `pendingUpdate` handle). */
let mockUpdateReady = false;

export const updateHandlers = {
  async checkForUpdate(): Promise<UpdateCheckResult> {
    await delay(400);
    if (UPDATE_MODE === 'error') {
      mockUpdateReady = false;
      const err: AppError = {
        kind: 'networkError',
        message: 'mock: could not reach the update endpoint (?update=error)',
      };
      throw err;
    }
    if (UPDATE_MODE === 'available') {
      mockUpdateReady = true;
      return {
        available: true,
        currentVersion: MOCK_CURRENT_VERSION,
        version: MOCK_NEXT_VERSION,
        notes: '- Mock release notes\n- Harness fixture',
        date: '2026-08-04',
      };
    }
    mockUpdateReady = false;
    return {
      available: false,
      currentVersion: MOCK_CURRENT_VERSION,
      version: null,
      notes: null,
      date: null,
    };
  },

  async downloadAndInstallUpdate(onProgress: (p: UpdateProgress) => void): Promise<void> {
    if (!mockUpdateReady) {
      const err: AppError = {
        kind: 'noOperationInProgress',
        message: 'No update found — call checkForUpdate first',
      };
      throw err;
    }
    const contentLength = 5_000_000;
    onProgress({ phase: 'started', downloadedBytes: 0, contentLength });
    const ticks = 15;
    const chunk = Math.ceil(contentLength / ticks);
    let downloadedBytes = 0;
    for (let i = 0; i < ticks; i += 1) {
      await delay(120);
      downloadedBytes = Math.min(contentLength, downloadedBytes + chunk);
      onProgress({ phase: 'downloading', downloadedBytes, contentLength });
    }
    onProgress({ phase: 'finished', downloadedBytes: contentLength, contentLength });
  },

  async relaunchApp(): Promise<void> {
    // No reload — keeps harness state so the flow stays inspectable (D1/INV-2).
    console.info('[mock] relaunch');
  },
} satisfies Partial<IpcApi>;
