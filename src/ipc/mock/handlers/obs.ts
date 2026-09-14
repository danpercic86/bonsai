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
import type {
  Histogram,
  IpcApi,
  LogRecord,
  LogSessionInfo,
  LogsDeleteResult,
  MetricsSnapshot,
} from '../../types';
import { delay, query } from '../repoState';
import { readUiSettings } from '../persistence';
import {
  DELETE_THROW_MESSAGE,
  deleteFailMode,
  deleteOutcome,
  logFixtureState,
} from '../obsLogFixture';
import { installLogDump, ringAnomalies, ringAppend, ringClear, ringStats } from '../obsRing';

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

/** A date `daysAgo` before today, `YYYY-MM-DD` in UTC — so the fixture always
 *  satisfies the §F6 window (no bucket older than 90 days) instead of drifting out
 *  of it as the fixed dates it used to carry aged. */
function daysAgo(n: number): string {
  const d = new Date(Date.now() - n * 86_400_000);
  return d.toISOString().slice(0, 10);
}

/** Freshly-built fixture aggregate (never mutated in place).
 *
 *  §F6: `days` holds one in-window bucket and `firstSeen` is a LIFETIME figure
 *  that legitimately predates the window — the 90-day rule applies to `days[]`
 *  only. */
function freshMetrics(): MetricsSnapshot {
  return {
    schema: 1,
    firstSeen: daysAgo(400),
    sessions: 12,
    days: [
      {
        date: daysAgo(2),
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
            // `cmd.*` keys are camelCase `IpcApi` METHOD names (§13 row 28) — a
            // snake_case fixture here is the input shape whose acceptance by a
            // validator is indistinguishable from a validator that rejects all.
            'cmd.getStatus': hist([0, 4, 8, 6, 1, 0, 0, 0], 90, 640),
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

/** What BOTH clears leave behind (§F6 §4.1): schema kept, `firstSeen` re-set to
 *  today — which is why the copy says "the date the count started", not "the date
 *  you first used Bonsai" — `sessions` 0, no days, no lifetime. */
function emptyMetrics(): MetricsSnapshot {
  return {
    schema: 1,
    firstSeen: daysAgo(0),
    sessions: 0,
    days: [],
    lifetime: { counters: {}, durations: {}, errors: {}, sessionMs: 0 },
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
    // §6.10 harness seam: `?obsInfoFail=1` makes EVERY read fail — the one on
    // mount, the 2 s poll and the delete dialog's re-read — so the page's `info`
    // stays `null` and the unknown-count copy (R8a's hint, R8b's archives line,
    // R3/R11's lead line) becomes visible in a browser. Without it that whole
    // family is unreachable: this handler never threw and returned a zero-filled
    // record with Dev mode off, which is a KNOWN ZERO, a different state.
    // The message mirrors the backend VERBATIM: `log_session_info` can only fail
    // in `obs::logs_dir`/`exports_dir`, as `cannot resolve app config dir: {e}`
    // over a `tauri::Error::UnknownPath` ("unknown path"). Every caller
    // (`DevCategory.refresh`, `DevModePill`, `batcher.refreshSalt`) swallows it,
    // so the flag degrades the copy, it does not break the page.
    if (query('obsInfoFail') === '1') {
      throw new Error('cannot resolve app config dir: unknown path');
    }
    const dev = readUiSettings().dev;
    const { records, dropped } = ringStats();
    const redaction = dev.includeRawNames ? 'raw' : 'strict';
    // ~180 bytes per JSONL line is what a real strict-mode record measures.
    // §6.11.5: `?obsLogFiles=N` / `?obsExports=N` drive BOTH this read and the
    // delete, from one fixture, so the reported and the deleted counts cannot
    // disagree.
    const state = logFixtureState(dev.enabled, records * 180);
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
        // Log files OUTLIVE the toggle: `?obsLogFiles=3` with Dev mode off is a
        // real state, and the row hint / confirm dialog must count it.
        totalFiles: state.logFiles,
        totalBytes: state.logBytes,
        droppedParts: 0,
        writeFailed: false,
        exportFiles: state.exportZips,
        exportBytes: state.exportBytes,
      };
    }
    const truncated = query('obsTruncated') === '1';
    return {
      sessionId: 'smock0001',
      dir: MOCK_DIR,
      // `?obsLogFiles=0` with Dev mode ON is §6.11.8 AC5's state: records live in
      // the ring, nothing flushed to disk yet, so there is no part to name.
      files: state.hasActivePart ? [MOCK_SESSION_FILE] : [],
      bytes: state.activePartBytes,
      records,
      anomalies: ringAnomalies().length,
      // §16.8 independence proof: the `dev-truncated` fixture forces a nonzero
      // `dropped` alongside `droppedParts`, so the in-memory backpressure line and
      // the on-disk truncation line must render as two distinct rows.
      dropped: truncated ? Math.max(dropped, 42) : dropped,
      redaction,
      salt: MOCK_SALT,
      totalFiles: state.logFiles,
      totalBytes: state.logBytes,
      // §6.3: a truncated session (`?obsTruncated=1`) exercises the Dev-page cap
      // warning; the default session is untruncated.
      droppedParts: truncated ? 3 : 0,
      // §8.4/§16.9: `?obsWriteFail=1` drives the "Not writing" disk-error state.
      writeFailed: query('obsWriteFail') === '1',
      // §6.2: exports live in <config>/exports and are inside the delete scope.
      // §6.11.5: `?obsExports=N` is what makes every export-bearing string —
      // the outcome's " and N exports" clause and the confirm dialog's
      // "This includes N exported log archives." — reachable in a browser.
      exportFiles: state.exportZips,
      exportBytes: state.exportBytes,
    };
  },

  async logsDeleteAll(): Promise<LogsDeleteResult> {
    await delay(80);
    // §6.8 R7 / §6.11.5: the counts are DERIVED from the same fixture state
    // `logSessionInfo` reports, never hard-coded. Fixed numbers made the harness
    // show "Deleted 3 log files and 1 export" to a user with none — the harness
    // contradicting the copy it exists to verify — and that fiction is what hid
    // §6.8 R5's `logParts === 0` branch from view for an entire increment.
    const dev = readUiSettings().dev.enabled;
    // §6.11.5 — `?obsDeleteFail=throw` reaches the THROWN path (`deleteErrorText`
    // plus "Nothing was deleted."), which no harness flag could reach before.
    // It returns FIRST, before the ring and the aggregate are touched: on every
    // reachable backend rejection zero files were removed and the usage counts
    // were never cleared (`obs_delete.rs`'s all-or-nothing note), and a mock that
    // wiped them here would make that announcement a lie.
    if (deleteFailMode() === 'throw') throw new Error(DELETE_THROW_MESSAGE);
    // Read the ring BEFORE clearing it: these are the bytes being reclaimed.
    const state = logFixtureState(dev, ringStats().records * 180);
    const result = deleteOutcome(state, dev);
    // Mirrors the backend: purging clears every prior record. The ring is the
    // mock's on-disk stand-in, so emptying it is the roll-then-purge equivalent.
    ringClear();
    // §F6: the delete covers the usage counts too, and clearing the fixture is the
    // mock's half of the in-memory reset — without it a following
    // `metricsSnapshot()` would still return the aggregate the toast just claimed
    // was cleared, which is exactly the "button does nothing" failure. Reset even
    // when `metricsCleared` is false: `metrics_clear.rs` replaces the in-memory
    // file FIRST and only then touches the disk, so a failed on-disk half still
    // leaves an empty aggregate behind.
    mockMetrics = emptyMetrics();
    return result;
  },

  async logRevealDir(): Promise<void> {
    await delay(30);
    // No OS file manager in a browser; the harness only proves the wiring.
    console.info(`[mock] reveal logs folder: ${MOCK_DIR}`);
  },

  async logExportSession(): Promise<string> {
    await delay(60);
    // Mirrors the backend refusal VERBATIM (`commands/obs.rs`): with no session
    // recorded and nothing on disk there is nothing to zip, and the harness must
    // exercise that copy path rather than resolving with a fictional archive.
    if (!readUiSettings().dev.enabled && ringStats().records === 0) {
      throw new Error('there are no log files to export');
    }
    // §12: `?obsExportFail=space|permission|other` reaches the three
    // `exportErrorText()` branches the "no log files" refusal above cannot.
    // Each message mirrors the shape `commands/obs.rs::export_session` actually
    // produces (`AppError::Io("cannot …: {os error}")`), so the substring the
    // mapper keys off is the one the backend would really emit.
    const exportFail = query('obsExportFail');
    if (exportFail === 'space') {
      throw new Error('cannot write log part to the zip: no space left on device (os error 28)');
    }
    if (exportFail === 'permission') {
      throw new Error('cannot create export file: permission denied (os error 13)');
    }
    if (exportFail !== null && exportFail !== '') {
      throw new Error('cannot finalize the zip: zip writer failed');
    }
    // §6.2: the destination is ALWAYS exports/, NEVER logs/ and never a
    // caller-supplied path — the command takes no `dest` (audit F4).
    return `${MOCK_EXPORTS_DIR}/bonsai-2026-08-27T14-03-11-smock0001.zip`;
  },

  async metricsSnapshot(): Promise<MetricsSnapshot> {
    await delay(20);
    // Deep-ish clone so a caller can't mutate the fixture backing the ring.
    return structuredClone(mockMetrics);
  },

  async metricsReset(): Promise<void> {
    await delay(20);
    // Mirrors the backend: clears every aggregate to an empty file. `firstSeen`
    // is reset and `sessions` returns to 0. Same shape `logsDeleteAll` leaves
    // behind — one implementation, as in Rust.
    mockMetrics = emptyMetrics();
  },
} satisfies Partial<IpcApi>;
