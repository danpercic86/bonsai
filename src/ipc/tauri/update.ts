import { getVersion } from '@tauri-apps/api/app';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import type { AppError, UpdateCheckResult, UpdateProgress } from '../types';

// P42 (INV-1 / D1): the Tauri updater flow is stateful JS-side — `check()`
// returns an `Update` handle that must be held to call `.downloadAndInstall()`.
// We keep it in this module-level var between `checkForUpdate()` and
// `downloadAndInstallUpdate()` rather than bridging it across a Rust command.
let pendingUpdate: Update | null = null;

/** Maps a thrown updater/plugin error into a Bonsai `AppError`. A signature or
 *  manifest-parse failure is `updateFailed`; a genuine connectivity failure is
 *  `networkError`. */
function toUpdateAppError(e: unknown): AppError {
  const message = e instanceof Error ? e.message : String(e);
  const lower = message.toLowerCase();
  const isNetwork =
    lower.includes('network') ||
    lower.includes('connect') ||
    lower.includes('timed out') ||
    lower.includes('timeout') ||
    lower.includes('dns') ||
    lower.includes('unreachable') ||
    lower.includes('sending request');
  return { kind: isNetwork ? 'networkError' : 'updateFailed', message };
}

export const updateCommands = {

  // P42: auto-update (INV-1 / D1). React talks ONLY to these wrappers; the JS
  // updater/process plugins are imported here, never in components.
  async checkForUpdate(): Promise<UpdateCheckResult> {
    const currentVersion = await getVersion();
    try {
      const u = await check();
      pendingUpdate = u;
      return {
        available: u !== null,
        currentVersion,
        version: u?.version ?? null,
        notes: u?.body ?? null,
        date: u?.date ?? null,
      };
    } catch (e) {
      throw toUpdateAppError(e);
    }
  },

  async downloadAndInstallUpdate(onProgress: (p: UpdateProgress) => void): Promise<void> {
    if (pendingUpdate === null) {
      const err: AppError = {
        kind: 'noOperationInProgress',
        message: 'No update found — call checkForUpdate first',
      };
      throw err;
    }
    let downloadedBytes = 0;
    let contentLength: number | null = null;
    try {
      await pendingUpdate.downloadAndInstall((evt) => {
        switch (evt.event) {
          case 'Started':
            contentLength = evt.data.contentLength ?? null;
            onProgress({ phase: 'started', downloadedBytes: 0, contentLength });
            break;
          case 'Progress':
            downloadedBytes += evt.data.chunkLength;
            onProgress({ phase: 'downloading', downloadedBytes, contentLength });
            break;
          case 'Finished':
            onProgress({ phase: 'finished', downloadedBytes, contentLength });
            break;
        }
      });
    } catch (e) {
      throw toUpdateAppError(e);
    }
  },

  relaunchApp(): Promise<void> {
    return relaunch();
  },
};
