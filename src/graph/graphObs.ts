import { createFrameRecorder, type FrameStats } from './frameStats';
import type { FrameDim } from '../obs/types';
import { useRenderCount } from '../obs/react';
import { obsCaptureFrames, obsLevelAllows } from '../obs/enabled';
import { logRecord } from '../obs/log';

/** P91 §9.3 — route a completed frame-timing window to a `frame` log record when
 *  Dev mode is on. `paint` and `gap` are separate recorders (§4.7), so each maps
 *  its own dimension; the other stays 0 and `dim` says which is which — a
 *  consumer must never read the filler `0` as "zero measured". `worstMs` is the
 *  window's max, in the dimension named by `dim`. */
function emitFrameRecord(dim: FrameDim, s: FrameStats): void {
  // P122e — gated on the `dev.captureFrames` switch (forced on at `level:
  // 'trace'`, §5 footnote). It used to gate on `obsEnabled()` alone and inherit
  // the kind's default `trace` level, so the switch did nothing below trace.
  // Under the switch the record is `debug`, like the ipc/event records
  // `captureIpc` gates; at trace it stays `trace`.
  if (!obsCaptureFrames()) return;
  logRecord({
    kind: 'frame',
    lvl: obsLevelAllows('trace') ? 'trace' : 'debug',
    dim,
    paintMs: dim === 'paint' ? s.avgMs : 0,
    gapMs: dim === 'gap' ? s.avgMs : 0,
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
