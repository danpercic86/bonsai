/**
 * P120 §3.3 — boot/interactive phase as a transition log (A6): events are
 * labelled by THEIR timestamp, not by the phase at delivery, so a late observer
 * callback for a boot-time shift still reads `boot`.
 *
 * All times are `performance.now()`-based (same origin as `entry.startTime` and
 * `event.timeStamp`).
 */
import type { StabilityPhase } from './types';

interface Transition {
  at: number;
  phase: StabilityPhase;
}

const RING = 8;
let transitions: Transition[] = [{ at: 0, phase: 'boot' }];
const listeners = new Set<(p: StabilityPhase) => void>();

function last(): Transition | undefined {
  return transitions[transitions.length - 1];
}

function push(t: Transition): void {
  transitions.push(t);
  if (transitions.length > RING) transitions = transitions.slice(-RING);
  for (const l of listeners) l(t.phase);
}

/** Repo open/switch BEGINS — the graph re-streams, so it is boot-like. */
export function markBoot(): void {
  if (last()?.phase === 'boot') return;
  push({ at: performance.now(), phase: 'boot' });
}

/** First graph meta + first status round handled (or the no-repo empty state). */
export function markInteractive(): void {
  if (last()?.phase === 'interactive') return;
  push({ at: performance.now(), phase: 'interactive' });
}

/** Phase of the last transition with `at ≤ t` (default boot). */
export function phaseAt(t: DOMHighResTimeStamp): StabilityPhase {
  for (let i = transitions.length - 1; i >= 0; i--) {
    const tr = transitions[i];
    if (tr !== undefined && tr.at <= t) return tr.phase;
  }
  return 'boot';
}

export function onPhaseChange(fn: (p: StabilityPhase) => void): () => void {
  listeners.add(fn);
  return () => {
    listeners.delete(fn);
  };
}

export function resetPhaseForTests(): void {
  transitions = [{ at: 0, phase: 'boot' }];
  listeners.clear();
}
