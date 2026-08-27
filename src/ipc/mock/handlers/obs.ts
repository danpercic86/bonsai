/**
 * P91 §6 — mock observability handlers (browser harness).
 *
 * Path note: the contract writes `src/ipc/mock/obs.ts`; every other mock domain
 * lives under `mock/handlers/`, so this follows the codebase convention and the
 * ring buffer itself sits in `mock/obsRing.ts`.
 *
 * Fidelity rules followed here: `logAppend` is a no-op when Dev mode is off (as
 * in Rust), the session summary is derived from the ring rather than invented,
 * and `logSessionInfo` reports a FIXED salt so the frontend redactor mirror is
 * deterministic across harness reloads.
 */
import type { IpcApi, LogRecord, LogSessionInfo } from '../../types';
import { delay } from '../repoState';
import { readUiSettings } from '../persistence';
import { installLogDump, ringAppend, ringStats } from '../obsRing';

installLogDump();

/** Fixed so a harness assertion on `ref#…` ordinals is reproducible. The real
 *  backend mints 16 random bytes per session and never persists them (§7.2). */
const MOCK_SALT = '00112233445566778899aabbccddeeff';
const MOCK_DIR = '/mock/config/com.bonsai.app/logs';
/** §6.2: exports are a SIBLING of logs/, never inside it. */
const MOCK_EXPORTS_DIR = '/mock/config/com.bonsai.app/exports';
const MOCK_SESSION_FILE = 'bonsai-2026-08-27T14-03-11-smock0001.jsonl';

export const obsHandlers = {
  async logAppend(records: LogRecord[]): Promise<void> {
    // Mirrors Rust: with no session there is nothing to append to, and a racing
    // toggle must not surface an error to the caller.
    if (!readUiSettings().dev.enabled) return;
    ringAppend(records);
  },

  async logSessionInfo(): Promise<LogSessionInfo> {
    await delay(30);
    const dev = readUiSettings().dev;
    const { records, dropped } = ringStats();
    const redaction = dev.includeRawNames ? 'raw' : 'strict';
    if (!dev.enabled) {
      return {
        sessionId: '',
        dir: MOCK_DIR,
        files: [],
        bytes: 0,
        records: 0,
        anomalies: 0,
        dropped: 0,
        redaction,
        salt: '',
        totalFiles: 0,
        totalBytes: 0,
        exportFiles: 0,
        exportBytes: 0,
      };
    }
    // ~180 bytes per JSONL line is what a real strict-mode record measures.
    const bytes = records * 180;
    return {
      sessionId: 'smock0001',
      dir: MOCK_DIR,
      files: [MOCK_SESSION_FILE],
      bytes,
      records,
      anomalies: 0,
      dropped,
      redaction,
      salt: MOCK_SALT,
      totalFiles: 1,
      totalBytes: bytes,
      // §6.2: exports live in <config>/exports and are inside the delete scope.
      exportFiles: 0,
      exportBytes: 0,
    };
  },

  async logRevealDir(): Promise<void> {
    await delay(30);
    // No OS file manager in a browser; the harness only proves the wiring.
    console.info(`[mock] reveal logs folder: ${MOCK_DIR}`);
  },

  async logExportSession(dest?: string | null): Promise<string> {
    await delay(60);
    // Mirrors the backend refusal VERBATIM (`commands/obs.rs`): with no session
    // recorded and nothing on disk there is nothing to zip, and the harness must
    // exercise that copy path rather than resolving with a fictional archive.
    if (!readUiSettings().dev.enabled && ringStats().records === 0) {
      throw new Error('there are no log files to export');
    }
    // §6.2: the default destination is exports/, NEVER logs/.
    return dest ?? `${MOCK_EXPORTS_DIR}/bonsai-2026-08-27T14-03-11-smock0001.zip`;
  },
} satisfies Partial<IpcApi>;
