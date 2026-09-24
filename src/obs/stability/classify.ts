/** P120 §3.2 — the SINGLE "unprompted shift" predicate (dev sink level, mock
 *  analyzer, and part a's `layoutShiftsUnprompted` counter all use it). */
import type { ShiftEvent } from './types';

export function isUnpromptedShift(e: Pick<ShiftEvent, 'phase' | 'hadRecentInput' | 'resizing'>): boolean {
  return e.phase === 'interactive' && !e.hadRecentInput && !e.resizing;
}
