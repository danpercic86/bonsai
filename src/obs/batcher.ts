/**
 * P91 §6 "Frontend → file" — the sink client.
 *
 * ONE concern: getting stamped records to `logAppend` at **≤1 call per 500 ms**
 * (§11) without ever blocking a caller and without logging itself.
 *
 * Self-amplification is prevented STRUCTURALLY: the sink handle captured here is
 * the *raw* (un-proxied) api that `instrumentIpc` hands over, so a flush cannot
 * re-enter the instrumentation no matter what the proxy's exclusion list says.
 * The exclusion list is the second, independent guard.
 */
import type { LogRecord } from './types';
import { obsEnabled, onObsConfigChange } from './enabled';
import { clearSessionSalt, setSessionSalt } from './redact';

/** §6 — the batcher's own cap; oldest is dropped first and reported. */
export const BATCH_CAP = 5000;
/** §11 — at most one `logAppend` per window. */
export const FLUSH_MS = 500;
/** §6 — a full batch flushes early. */
export const FLUSH_RECORDS = 100;

/** The slice of `IpcApi` the sink needs. Both the real and the mock api satisfy
 *  it, so the harness exercises this file unchanged. */
export interface LogSink {
  logAppend(records: LogRecord[]): Promise<void>;
  logSessionInfo(): Promise<{ salt: string }>;
}

const START = Date.now();
let buffer: LogRecord[] = [];
let dropped = 0;
let droppedSinceSeq = 0;
let sink: LogSink | null = null;
let timer: ReturnType<typeof setTimeout> | null = null;
let inFlight: Promise<void> = Promise.resolve();
let appendCalls = 0;
let unsubscribeConfig: (() => void) | null = null;
let restart: Promise<void> = Promise.resolve();
let pageHooksInstalled = false;

/** Ms since this module was loaded — the `mono` field (§3). */
export function sessionMono(): number {
  return Date.now() - START;
}

/**
 * Attaches the sink and starts the salt lifecycle. Called once by
 * `instrumentIpc` with the RAW api (see the module header).
 */
export function attachSink(next: LogSink): void {
  sink = next;
  unsubscribeConfig?.();
  unsubscribeConfig = onObsConfigChange((dev, sessionRestarted) => {
    // §11: the page hooks are attached on the FIRST enable, not at module load,
    // so a session that never turns Dev mode on adds no listeners at all.
    if (dev) installPageHooks();
    if (!dev) {
      clearSessionSalt();
      // §6: nothing already buffered is thrown away — flush it, then stop.
      void flushNow();
      return;
    }
    // §12 row 2 / constraint 4: `level` and `includeRawNames` restart the Rust
    // session, which mints a NEW salt. Re-fetching is mandatory, otherwise every
    // later `argsHash` is computed against a stale salt and `dup-ipc` breaks.
    if (!sessionRestarted) return;
    // Order matters, and all three lines are load-bearing:
    //  1. flush, so records captured under the OLD mode land in the old file —
    //     §7.3 requires that a file never mixes redaction modes. `configureObs`
    //     is synchronous, so it cannot await this: the caller that flips the
    //     setting (increment 4/7) MUST await `pendingRestart()` before telling
    //     Rust to roll the session, or the tail of the old batch can land in
    //     the new file;
    //  2. clear SYNCHRONOUSLY, so the window until the fetch resolves is INERT
    //     rather than hashing with a stale salt into the new session's file;
    //  3. then re-fetch.
    restart = flushNow().then(() => {
      clearSessionSalt();
      return refreshSalt().then(() => undefined);
    });
    clearSessionSalt();
  });
  // Defensive: if `configureObs` ran before the sink was attached (module init
  // order is not guaranteed forever), the enable event was missed and the
  // pipeline would stay silently inert.
  if (obsEnabled()) restart = refreshSalt().then(() => undefined);
}

/** Fetches the session salt (§7.2). Never throws: a failed fetch leaves the
 *  redactor unset, which keeps the pipeline inert rather than emitting
 *  unsalted hashes. */
export async function refreshSalt(): Promise<boolean> {
  if (!sink) return false;
  try {
    const info = await sink.logSessionInfo();
    return setSessionSalt(info.salt ?? '');
  } catch {
    clearSessionSalt();
    return false;
  }
}

function installPageHooks(): void {
  if (pageHooksInstalled || typeof document === 'undefined') return;
  pageHooksInstalled = true;
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') void flushNow();
  });
  // Intentionally never removed: these are process-lifetime hooks on the sink,
  // not component listeners. There is exactly one batcher per document.
  window.addEventListener('beforeunload', () => {
    void flushNow();
  });
}

/** Enqueues one already-stamped record. Cheap and synchronous — the only work on
 *  a caller's thread is a push and, at most, one `setTimeout`. */
export function enqueue(record: LogRecord): void {
  if (buffer.length >= BATCH_CAP) {
    const lost = buffer.shift();
    dropped += 1;
    if (lost) droppedSinceSeq = lost.seq;
  }
  buffer.push(record);
  if (buffer.length >= FLUSH_RECORDS) {
    void flushNow();
    return;
  }
  if (timer === null) {
    timer = setTimeout(() => {
      timer = null;
      void flushNow();
    }, FLUSH_MS);
  }
}

/** Flushes immediately. Serialized: a second call while one `logAppend` is in
 *  flight queues behind it, so the ≤1-per-500 ms budget is never breached by
 *  overlap. */
export function flushNow(): Promise<void> {
  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }
  if (!sink || buffer.length === 0) return inFlight;
  if (dropped > 0) {
    // §6: the loss is reported as a record, never as backpressure.
    buffer.push({
      seq: 0,
      ts: Date.now(),
      mono: sessionMono(),
      src: 'ui',
      lvl: 'error',
      kind: 'drop',
      dropped,
      sinceSeq: droppedSinceSeq,
    });
    dropped = 0;
    droppedSinceSeq = 0;
  }
  const batch = buffer;
  buffer = [];
  const target = sink;
  appendCalls += 1;
  inFlight = inFlight
    .catch(() => undefined)
    .then(() => target.logAppend(batch))
    .catch(() => {
      // A failed append must never surface to a git or render path; the records
      // are lost by design (re-queuing would grow unbounded during an outage).
    });
  return inFlight;
}

/** Resolves once the in-progress session restart (flush → re-salt) is done.
 *  The settings caller must await this before rolling the Rust session. */
export function pendingRestart(): Promise<void> {
  return restart;
}

/** TEST ONLY — how many `logAppend` calls have been made (§11 budget check). */
export function appendCallCount(): number {
  return appendCalls;
}

/** TEST ONLY — drops the sink, the buffer and the config subscription. */
export function resetBatcherForTests(): void {
  if (timer !== null) clearTimeout(timer);
  timer = null;
  buffer = [];
  dropped = 0;
  droppedSinceSeq = 0;
  appendCalls = 0;
  sink = null;
  inFlight = Promise.resolve();
  restart = Promise.resolve();
  unsubscribeConfig?.();
  unsubscribeConfig = null;
}
