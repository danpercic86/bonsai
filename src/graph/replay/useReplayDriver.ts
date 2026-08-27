// Spec-007 decision 4: ONE self-terminating rAF loop (useSway pattern) —
// scheduled ONLY while `status === 'playing'`; cancels on pause/finish/exit/
// unmount and suspends while `document.hidden`. Paused/finished/scrubbing
// states schedule ZERO frames — a scrub while paused paints exactly once per
// change (via ReplayMode's layout effect on `state`).
//
// Deliberate departure from GraphCanvas's ref-only pattern: the transport bar
// genuinely needs per-frame React state (slider fill + progress label), so the
// driver holds ReplayState in useState and the canvas repaints in a layout
// effect keyed on it — one paint per state change, still zero rAF when idle.
import { useCallback, useEffect, useRef, useState } from 'react';
import {
  initialState,
  keyStepRows,
  pause,
  play,
  scrubTo,
  setFollow,
  setSpeed,
  stepRows,
  tick,
} from './replayModel';
import type { ReplayModel, ReplaySpeed, ReplayState } from './replayModel';
import { prunePulses } from './replayPulse';
import type { ReplayPulse } from './replayPulse';

/** Hard cap on tracked pulses (4× over dense history keeps paints bounded). */
const MAX_TRACKED_PULSES = 200;

export interface ReplayDriver {
  state: ReplayState;
  /** Live frontier pulses — read by the paint path (mutated in effects only). */
  pulsesRef: { readonly current: readonly ReplayPulse[] };
  togglePlay(): void;
  scrub(fraction: number): void;
  /** ←/→ scrub by rows (positive = reveal more); `xl` = Shift's 10× step. */
  stepBy(direction: 1 | -1, xl: boolean): void;
  toStart(): void;
  toEnd(): void;
  setSpeedTo(sp: ReplaySpeed): void;
  /** User wheel/drag on the overlay scroller disengages auto-follow. */
  disengageFollow(): void;
}

export function useReplayDriver(model: ReplayModel): ReplayDriver {
  const [state, setState] = useState<ReplayState>(() => initialState(model));
  const pulsesRef = useRef<readonly ReplayPulse[]>([]);

  // rAF loop — mounted only while playing; self-terminates when `tick` lands on
  // 'finished' (the status dep flips and the cleanup cancels). document.hidden
  // suspends scheduling; visibilitychange resumes with a fresh dt anchor.
  useEffect(() => {
    if (state.status !== 'playing') {
      pulsesRef.current = []; // pulses only live during playback
      return;
    }
    let raf = 0;
    let last: number | null = null;
    const frame = (ts: number): void => {
      raf = 0;
      if (document.hidden) {
        last = null;
        return; // suspended — visibilitychange re-arms
      }
      if (last !== null) {
        const dt = ts - last;
        setState((s) => tick(model, s, dt));
      }
      last = ts;
      raf = requestAnimationFrame(frame);
    };
    raf = requestAnimationFrame(frame);
    const onVisibility = (): void => {
      if (!document.hidden && raf === 0) {
        last = null;
        raf = requestAnimationFrame(frame);
      }
    };
    document.addEventListener('visibilitychange', onVisibility);
    return () => {
      if (raf !== 0) cancelAnimationFrame(raf);
      document.removeEventListener('visibilitychange', onVisibility);
    };
  }, [state.status, model]);

  // Frontier-pulse bookkeeping: rows revealed since the previous commit get a
  // pulse (playback only — scrubs reveal silently). Runs once per commit, so
  // StrictMode's double-invoked updaters can't double-record. Note: pulses
  // recorded by the tick that LANDS on 'finished' are dead bookkeeping — the
  // paint path drops pulses whenever status !== 'playing' (clean final frame),
  // and the rAF-effect cleanup clears the ref right after. Harmless by design.
  const prevCutoffRef = useRef(state.cutoff);
  useEffect(() => {
    const prev = prevCutoffRef.current;
    prevCutoffRef.current = state.cutoff;
    if (state.status === 'paused' || state.cutoff >= prev) return;
    const now = performance.now();
    const next = prunePulses(pulsesRef.current, now).slice();
    for (let row = state.cutoff; row < prev && next.length < MAX_TRACKED_PULSES; row += 1) {
      next.push({ row, start: now });
    }
    pulsesRef.current = next;
  }, [state.cutoff, state.status]);

  const togglePlay = useCallback(() => {
    setState((s) => (s.status === 'playing' ? pause(s) : play(model, s)));
  }, [model]);
  const scrub = useCallback(
    (fraction: number) => setState((s) => scrubTo(model, s, fraction)),
    [model],
  );
  const stepBy = useCallback(
    (direction: 1 | -1, xl: boolean) => {
      const rows = keyStepRows(model.n) * (xl ? 10 : 1);
      setState((s) => stepRows(model, s, direction * rows));
    },
    [model],
  );
  const toStart = useCallback(() => setState((s) => scrubTo(model, s, 0)), [model]);
  const toEnd = useCallback(() => setState((s) => scrubTo(model, s, 1)), [model]);
  const setSpeedTo = useCallback((sp: ReplaySpeed) => setState((s) => setSpeed(s, sp)), []);
  const disengageFollow = useCallback(() => setState((s) => setFollow(s, false)), []);

  return { state, pulsesRef, togglePlay, scrub, stepBy, toStart, toEnd, setSpeedTo, disengageFollow };
}
