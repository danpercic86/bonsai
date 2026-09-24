/**
 * P120 §3.5 — the dev-mode stability sink: turns detector events into
 * `layout.shift` / `focus.lost` log records.
 *
 * Shifts are coalesced into buckets keyed by (region, phase, hadRecentInput,
 * resizing) (A10), so a bucket never mixes prompted and unprompted shifts.
 * Interactive buckets close after `SHIFT_BUCKET_MS`. Boot buckets close at
 * `markInteractive()` (the boot summary, A11), or after `BOOT_BUCKET_MAX_MS`
 * if the app never becomes interactive. Levels follow A16: an unprompted
 * interactive bucket is `info`, every other bucket `debug`; `focus.lost` is
 * `info`. `logRecord` still applies the `obsEnabled()` + level gate. The phase
 * listener is held only while a boot bucket is open.
 */
import { logRecord } from '../log';
import { isUnpromptedShift } from './classify';
import { onPhaseChange } from './phase';
import { BOOT_BUCKET_MAX_MS, SHIFT_BUCKET_MS } from './types';
import type { FocusLostEvent, ShiftEvent, StabilityEvent, StabilitySink } from './types';

interface Bucket {
  region: ShiftEvent['region'];
  phase: ShiftEvent['phase'];
  hadRecentInput: boolean;
  resizing: boolean;
  shifts: number;
  value: number;
  first: number;
  last: number;
  timer: ReturnType<typeof setTimeout>;
}

export interface DevSink {
  sink: StabilitySink;
  flushAll(): void;
}

function round4(v: number): number {
  return Math.round(v * 1e4) / 1e4;
}

function keyOf(e: ShiftEvent): string {
  return `${e.region}|${e.phase}|${e.hadRecentInput ? 1 : 0}|${e.resizing ? 1 : 0}`;
}

export function createDevSink(): DevSink {
  const buckets = new Map<string, Bucket>();
  let unsubPhase: (() => void) | null = null;

  function syncPhaseSubscription(): void {
    const anyBoot = [...buckets.values()].some((b) => b.phase === 'boot');
    if (anyBoot && unsubPhase === null) {
      unsubPhase = onPhaseChange((p) => {
        if (p === 'interactive') flushWhere((b) => b.phase === 'boot');
      });
    } else if (!anyBoot && unsubPhase !== null) {
      unsubPhase();
      unsubPhase = null;
    }
  }

  function flush(key: string): void {
    const b = buckets.get(key);
    if (b === undefined) return;
    clearTimeout(b.timer);
    buckets.delete(key);
    logRecord({
      kind: 'layout.shift',
      lvl: isUnpromptedShift(b) ? 'info' : 'debug',
      region: b.region,
      phase: b.phase,
      hadRecentInput: b.hadRecentInput,
      resizing: b.resizing,
      shifts: b.shifts,
      value: round4(b.value),
      firstAt: b.first,
      spanMs: Math.max(0, b.last - b.first),
    });
  }

  function flushWhere(pred: (b: Bucket) => boolean): void {
    for (const [key, b] of [...buckets]) if (pred(b)) flush(key);
    syncPhaseSubscription();
  }

  function onShift(e: ShiftEvent): void {
    const key = keyOf(e);
    let b = buckets.get(key);
    if (b === undefined) {
      const ms = e.phase === 'interactive' ? SHIFT_BUCKET_MS : BOOT_BUCKET_MAX_MS;
      b = {
        region: e.region,
        phase: e.phase,
        hadRecentInput: e.hadRecentInput,
        resizing: e.resizing,
        shifts: 0,
        value: 0,
        first: e.at,
        last: e.at,
        timer: setTimeout(() => {
          flush(key);
          syncPhaseSubscription();
        }, ms),
      };
      buckets.set(key, b);
      syncPhaseSubscription();
    }
    b.shifts += 1;
    b.value += e.value;
    // Buffered boot replay can arrive out of order (A7): widen both ends.
    b.first = Math.min(b.first, e.at);
    b.last = Math.max(b.last, e.at);
  }

  function onFocusLost(e: FocusLostEvent): void {
    logRecord({ kind: 'focus.lost', lvl: 'info', fromRegion: e.fromRegion, phase: e.phase, via: e.via, lostAt: e.at });
  }

  return {
    sink: (e: StabilityEvent) => {
      if (e.type === 'shift') onShift(e);
      else onFocusLost(e);
    },
    flushAll: () => flushWhere(() => true),
  };
}
