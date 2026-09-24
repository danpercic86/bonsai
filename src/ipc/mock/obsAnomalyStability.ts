/**
 * P120 §5 — HARNESS-ONLY mirror of the Rust `unprompted-shift` and
 * `focus-steal` rules (`src-tauri/src/obs/anomaly/stability.rs`, the source of
 * truth for every constant and detail string below).
 *
 * It replays the ring in order through the same bounded state machine the Rust
 * detector runs (causes ≤256 / 6 s, held losses ≤64 / 5 s, prune keyed on each
 * record's `ts`), then drains the losses still held, because a dump is the
 * mock's session end. Returned anomalies carry `seq: 0`; `analyzeAnomalies`
 * stamps them.
 */
import type { LogRecord } from '../types';
import { isUnpromptedShift } from '../../obs/stability/classify';
import type { AnomalySeverity, FocusLostPayload, LayoutShiftPayload } from '../../obs/types';

const FOCUS_WATCHER_LOOKBACK_MS = 1_000;
const REFRESH_PRE_SLACK_MS = 50;
const REFRESH_POST_MS = 250;
const FOCUS_HOLD_MS = 5_000;
const FOCUS_PENDING_CAP = 64;
const CAUSE_HISTORY_MS = 6_000;
const CAUSE_HISTORY_CAP = 256;

interface Cause {
  kind: 'refresh' | 'watcher';
  seq: number;
  start: number;
  end: number;
  traces: string[];
}

interface Loss {
  seq: number;
  ts: number;
  p: FocusLostPayload;
  trace?: string;
}

function anomaly(rule: string, severity: AnomalySeverity, detail: string, refs: number[], traces: string[], ts: number): LogRecord {
  return { seq: 0, ts, mono: 0, src: 'rust', lvl: severity, kind: 'anomaly', rule, severity, detail, refs, traces };
}

function traceOf(rec: LogRecord): string | undefined {
  return typeof rec.trace === 'string' ? rec.trace : undefined;
}

function strings(v: unknown): string[] {
  return Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string') : [];
}

function matches(c: Cause, loss: Loss): boolean {
  const traceHit = loss.trace !== undefined && c.traces.includes(loss.trace);
  const at = loss.p.lostAt;
  const windowHit =
    c.kind === 'watcher'
      ? c.start <= at && at <= c.start + FOCUS_WATCHER_LOOKBACK_MS
      : c.start - REFRESH_PRE_SLACK_MS <= at && at <= c.end + REFRESH_POST_MS;
  return traceHit || windowHit;
}

function lossPrefix(l: Loss): string {
  return `focus lost from ${l.p.fromRegion} (${l.p.via}, ${l.p.phase})`;
}

function warn(loss: Loss, hits: Cause[]): LogRecord {
  const nRefresh = hits.filter((c) => c.kind === 'refresh').length;
  const nWatcher = hits.length - nRefresh;
  const refs = [loss.seq, ...hits.map((c) => c.seq).sort((a, b) => a - b)];
  const traces: string[] = [];
  for (const t of [...hits.flatMap((c) => c.traces), ...(loss.trace ? [loss.trace] : [])]) {
    if (!traces.includes(t)) traces.push(t);
  }
  const detail = `${lossPrefix(loss)}; causes: ${nRefresh} refresh, ${nWatcher} watcher`;
  return anomaly('focus-steal', 'warn', detail, refs, traces, loss.ts);
}

function info(loss: Loss): LogRecord {
  const detail = `${lossPrefix(loss)}; no refresh or watcher round found`;
  return anomaly('focus-steal', 'info', detail, [loss.seq], loss.trace ? [loss.trace] : [], loss.ts);
}

function unpromptedShift(rec: LogRecord): LogRecord | null {
  const p = rec as unknown as LayoutShiftPayload;
  if (!isUnpromptedShift(p)) return null;
  const detail = `${p.region}: ${p.shifts} shift(s), score ${p.value.toFixed(4)}, over ${p.spanMs} ms`;
  const trace = traceOf(rec);
  return anomaly('unprompted-shift', 'warn', detail, [rec.seq], trace ? [trace] : [], rec.ts);
}

/** Batch pass over the ring; returns the derived stability anomalies (seq 0). */
export function analyzeStability(records: readonly LogRecord[]): LogRecord[] {
  const out: LogRecord[] = [];
  let causes: Cause[] = [];
  let pending: Loss[] = [];
  const pushCause = (c: Cause): void => {
    causes.push(c);
    if (causes.length > CAUSE_HISTORY_CAP) causes.shift();
  };

  for (const rec of records) {
    const ts = rec.ts;
    if (rec.kind === 'layout.shift') {
      const a = unpromptedShift(rec);
      if (a) out.push(a);
    } else if (rec.kind === 'focus.lost') {
      const loss: Loss = { seq: rec.seq, ts, p: rec as unknown as FocusLostPayload, trace: traceOf(rec) };
      const hits = causes.filter((c) => matches(c, loss));
      if (hits.length > 0) out.push(warn(loss, hits));
      else {
        pending.push(loss);
        const oldest = pending.length > FOCUS_PENDING_CAP ? pending.shift() : undefined;
        if (oldest) out.push(info(oldest));
      }
    } else if (rec.kind === 'refresh') {
      const start = ts - Math.round(Number(rec.ms ?? 0));
      const c: Cause = { kind: 'refresh', seq: rec.seq, start, end: ts, traces: strings(rec.contributingTraces) };
      pushCause(c);
      pending = pending.filter((loss) => {
        if (!matches(c, loss)) return true;
        const watchers = causes.filter((w) => w.kind === 'watcher' && matches(w, loss));
        out.push(warn(loss, [c, ...watchers]));
        return false;
      });
    } else if (rec.kind === 'watcher' && rec.fired === true && rec.suppressed !== true) {
      const traces = [traceOf(rec), typeof rec.causedBy === 'string' ? rec.causedBy : undefined];
      pushCause({ kind: 'watcher', seq: rec.seq, start: ts, end: ts, traces: traces.filter((t): t is string => !!t) });
    }
    // prune(ts) — runs after every record, exactly as `observe()` does.
    causes = causes.filter((c) => c.end >= ts - CAUSE_HISTORY_MS);
    pending = pending.filter((loss) => {
      const expired = ts > loss.p.lostAt + FOCUS_HOLD_MS;
      if (expired) out.push(info(loss));
      return !expired;
    });
  }
  for (const loss of pending) out.push(info(loss));
  return out;
}
