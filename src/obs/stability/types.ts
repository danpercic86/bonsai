/**
 * P120 §3.1 — the shared stability-detector events. Part a (lab counters) and
 * part b (dev-mode `logRecord` sink) both attach to `StabilitySink`.
 *
 * The three vocabulary types live with the log-record payloads in
 * `src/obs/types.ts` (one definition) and are re-exported here. They are
 * closed unions: never a path or user string.
 */
import type { FocusVia, StabilityPhase, StabilityRegion } from '../types';

export type { FocusVia, StabilityPhase, StabilityRegion };

export interface ShiftEvent {
  type: 'shift';
  region: StabilityRegion;
  phase: StabilityPhase;
  hadRecentInput: boolean;
  resizing: boolean;
  /** Whole-entry score (A9) — not exclusive to this region; never sum across regions. */
  value: number;
  /** Epoch ms. */
  at: number;
}

export interface FocusLostEvent {
  type: 'focus-lost';
  fromRegion: StabilityRegion;
  phase: StabilityPhase;
  via: FocusVia;
  /** Epoch ms. */
  at: number;
}

export type StabilityEvent = ShiftEvent | FocusLostEvent;
export type StabilitySink = (e: StabilityEvent) => void;

/** §3.2 constants — the one place they live. */
export const INPUT_WINDOW_MS = 500;
export const RESIZE_WINDOW_MS = 500;
export const FOCUS_SETTLE_MS = 50;
export const SHIFT_BUCKET_MS = 1000;
export const BOOT_BUCKET_MAX_MS = 30_000;
