/**
 * P120 §3.2 — sink fan-out. The 0→1 sink transition attaches every observer and
 * tracker; 1→0 detaches them all, so with no sink registered nothing is
 * installed (zero cost when off, AC 12).
 */
import { attachFocusLoss } from './focusLoss';
import { attachInputTracker, attachResizeTracker } from './input';
import { attachLayoutShift } from './layoutShift';
import type { StabilityEvent, StabilitySink } from './types';

const sinks = new Set<StabilitySink>();
let detachAll: (() => void) | null = null;
/** Page-lifetime flag: only the first attach may replay buffered entries (A7). */
let attachedOnce = false;

function emit(e: StabilityEvent): void {
  for (const s of sinks) {
    try {
      s(e);
    } catch (err) {
      if (import.meta.env.DEV) console.error('[bonsai] stability sink threw', err);
    }
  }
}

function attach(): () => void {
  const first = !attachedOnce;
  attachedOnce = true;
  // Trackers first, so the observers' classification can read them.
  const parts = [attachInputTracker(), attachResizeTracker(), attachLayoutShift(emit, first), attachFocusLoss(emit)];
  return () => {
    for (const d of parts.reverse()) d();
  };
}

export function addStabilitySink(fn: StabilitySink): () => void {
  if (sinks.has(fn)) return () => {};
  sinks.add(fn);
  if (sinks.size === 1) detachAll = attach();
  let removed = false;
  return () => {
    if (removed) return;
    removed = true;
    sinks.delete(fn);
    if (sinks.size === 0 && detachAll !== null) {
      detachAll();
      detachAll = null;
    }
  };
}

export function stabilitySinkCountForTests(): number {
  return sinks.size;
}

/** Detach everything and forget the page-lifetime `first` flag (tests only). */
export function resetSinksForTests(): void {
  sinks.clear();
  detachAll?.();
  detachAll = null;
  attachedOnce = false;
}
