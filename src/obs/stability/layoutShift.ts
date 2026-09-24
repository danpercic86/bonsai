/**
 * P120 §3.4 — layout-shift capture (Chromium/WebView2 only). On WebKit the entry
 * type is absent and this installs NOTHING (and never throws).
 *
 * No coalescing here (A10 moves it to part b's dev sink): one event per
 * (entry, distinct region), each carrying the WHOLE-entry score (A9).
 */
import { inputWithin, resizeWithin } from './input';
import { phaseAt } from './phase';
import { regionOf } from './regions';
import { INPUT_WINDOW_MS, RESIZE_WINDOW_MS } from './types';
import type { StabilityRegion, StabilitySink } from './types';

/** The Layout Instability API entry (not in lib.dom). */
interface LayoutShiftSource {
  node: Node | null;
}
interface LayoutShiftEntry extends PerformanceEntry {
  value: number;
  hadRecentInput: boolean;
  sources?: readonly LayoutShiftSource[];
}

function isLayoutShift(e: PerformanceEntry): e is LayoutShiftEntry {
  return e.entryType === 'layout-shift';
}

export function layoutShiftSupported(): boolean {
  return (
    typeof PerformanceObserver !== 'undefined' &&
    PerformanceObserver.supportedEntryTypes?.includes('layout-shift') === true
  );
}

/** `first` is true only for the page's first attach (A7: buffered replay then
 *  only, and only while still booting). */
export function attachLayoutShift(emit: StabilitySink, first: boolean): () => void {
  if (!layoutShiftSupported()) return () => {};
  const attachedAt = performance.now();
  const buffered = first && phaseAt(attachedAt) === 'boot';
  const handle = (e: LayoutShiftEntry): void => {
    if (!buffered && e.startTime < attachedAt) return;
    const regions = new Set<StabilityRegion>();
    for (const s of e.sources ?? []) regions.add(regionOf(s.node));
    if (regions.size === 0) regions.add('other');
    const phase = phaseAt(e.startTime);
    const hadRecentInput = e.hadRecentInput || inputWithin(e.startTime, INPUT_WINDOW_MS);
    const resizing = resizeWithin(e.startTime, RESIZE_WINDOW_MS);
    const value = Number.isFinite(e.value) ? e.value : 0;
    const at = Math.round(performance.timeOrigin + e.startTime);
    for (const region of regions) {
      emit({ type: 'shift', region, phase, hadRecentInput, resizing, value, at });
    }
  };
  let obs: PerformanceObserver;
  try {
    obs = new PerformanceObserver((list) => {
      for (const e of list.getEntries()) if (isLayoutShift(e)) handle(e);
    });
    obs.observe({ type: 'layout-shift', buffered });
  } catch {
    return () => {};
  }
  return () => obs.disconnect();
}
