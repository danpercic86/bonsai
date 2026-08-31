/** Result of IpcApi.checkForUpdate (P42). `available` false ⇒ up to date;
 *  version/notes/date populated only when available. currentVersion is always set. */
export interface UpdateCheckResult {
  available: boolean;
  currentVersion: string;
  /** Target version when available, else null. */
  version: string | null;
  /** Release notes (may be markdown/plain), else null. */
  notes: string | null;
  /** Publish date string from the manifest, else null. */
  date: string | null;
}

/** Streamed progress of downloadAndInstallUpdate (P42). Bytes are cumulative. */
export interface UpdateProgress {
  phase: 'started' | 'downloading' | 'finished';
  downloadedBytes: number;
  /** Total size when the manifest/server provides it, else null. */
  contentLength: number | null;
}
