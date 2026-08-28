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
import type { Histogram, IpcApi, LogRecord, LogSessionInfo, MetricsSnapshot } from '../../types';
import { delay } from '../repoState';
import { readUiSettings } from '../persistence';
import { installLogDump, ringAnomalies, ringAppend, ringStats } from '../obsRing';

installLogDump();

/** A plausible bounded histogram whose DERIVED p50/p95 the snapshot fills — the
 *  mock mirrors the backend by computing them here rather than storing them. */
function hist(buckets: number[], maxMs: number, sumMs: number): Histogram {
  const count = buckets.reduce((a, b) => a + b, 0);
  const bounds = [1, 5, 10, 50, 100, 500, 2000];
  const pct = (p: number): number | undefined => {
    if (count === 0) return undefined;
    const target = p * count;
    let cum = 0;
    for (let i = 0; i < 8; i += 1) {
      const bc = buckets[i] ?? 0;
      if ((cum + bc >= target && bc > 0) || i === 7) {
        if (bc === 0) return maxMs;
        const lower = i === 0 ? 0 : bounds[i - 1];
        const upper = i === 7 ? maxMs : bounds[i];
        const frac = Math.min(Math.max((target - cum) / bc, 0), 1);
        return Math.round(Math.min(lower + frac * (upper - lower), maxMs));
      }
      cum += bc;
    }
    return maxMs;
  };
  return { count, sumMs, maxMs, buckets, p50Ms: pct(0.5), p95Ms: pct(0.95) };
}

/** Freshly-built fixture aggregate (never mutated in place). */
function freshMetrics(): MetricsSnapshot {
  return {
    schema: 1,
    firstSeen: '2026-08-01',
    sessions: 12,
    days: [
      {
        date: '2026-08-27',
        totals: {
          counters: {
            'perf.repo_opens': 3,
            'perf.graph_walks': 5,
            'perf.graph_cache_hits': 41,
            'perf.graph_redecorates': 7,
            'perf.status_scans': 9,
          },
          durations: {
            'op.graph.get': hist([0, 0, 0, 2, 6, 10, 1, 0], 720, 6100),
            'op.graph.get.lane': hist([0, 0, 3, 9, 6, 1, 0, 0], 210, 1400),
            'cmd.get_status': hist([0, 4, 8, 6, 1, 0, 0, 0], 90, 640),
            'queue.blocking': hist([12, 6, 2, 0, 0, 0, 0, 0], 8, 40),
          },
          errors: { Git: 1 },
          sessionMs: 1_845_000,
        },
      },
    ],
    lifetime: {
      counters: { 'perf.repo_opens': 58, 'perf.graph_walks': 120 },
      durations: {
        'op.graph.get': hist([0, 0, 1, 40, 180, 260, 20, 2], 2400, 190_000),
      },
      errors: { Git: 4, Io: 1 },
      sessionMs: 92_400_000,
    },
  };
}

let mockMetrics: MetricsSnapshot = freshMetrics();

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
      anomalies: ringAnomalies().length,
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

  async metricsSnapshot(): Promise<MetricsSnapshot> {
    await delay(20);
    // Deep-ish clone so a caller can't mutate the fixture backing the ring.
    return structuredClone(mockMetrics);
  },

  async metricsReset(): Promise<void> {
    await delay(20);
    // Mirrors the backend: clears every aggregate to an empty file. `firstSeen`
    // is reset and `sessions` returns to 0.
    mockMetrics = {
      schema: 1,
      firstSeen: '2026-08-27',
      sessions: 0,
      days: [],
      lifetime: { counters: {}, durations: {}, errors: {}, sessionMs: 0 },
    };
  },
} satisfies Partial<IpcApi>;
