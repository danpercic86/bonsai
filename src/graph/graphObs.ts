import { createFrameRecorder, type FrameStats } from './frameStats';
import { useRenderCount } from '../obs/react';
import { obsEnabled } from '../obs/enabled';
import { logRecord } from '../obs/log';

/** P91 §9.3 — route a completed frame-timing window to a `frame` log record when
 *  Dev mode is on. `paint` and `gap` are separate recorders (§4.7), so each maps
 *  its own dimension; the other stays 0. `worstMs` is the window's max. */
function emitFrameRecord(kind: 'paint' | 'gap', s: FrameStats): void {
  if (!obsEnabled()) return;
  logRecord({
    kind: 'frame',
    paintMs: kind === 'paint' ? s.avgMs : 0,
    gapMs: kind === 'gap' ? s.avgMs : 0,
    over33: s.over33,
    over100: s.over100,
    worstMs: s.maxMs,
  });
}

// Two recorders (P1 §4.7): paint durations and scroll inter-frame gaps are
// different quantities — mixing them made `avg` meaningless.

/** Paint-window recorder wired to emit `paint` frame records (P91 §9.3). */
export function newPaintRecorder() {
  return createFrameRecorder((s) => emitFrameRecord('paint', s));
}

/** Scroll inter-frame-gap recorder wired to emit `gap` frame records (P91 §9.3). */
export function newGapRecorder() {
  return createFrameRecorder((s) => emitFrameRecord('gap', s));
}

/** P91 §9.2 surface 3 — canvas render churn (each mode). Thin wrapper so the
 *  container calls one hook; hook order stays unconditional at the call site. */
export function useGraphRenderCount(props: Record<string, unknown>) {
  useRenderCount('GraphCanvas', props);
}
