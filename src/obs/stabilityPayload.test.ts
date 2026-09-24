/** P120 T-F11 — the `UiPayload` members for `layout.shift` / `focus.lost` are
 *  checked by `tsc --noEmit` (this file is in the tsconfig `include`). The
 *  `@ts-expect-error` lines FAIL the type check if the kinds ever fall back to
 *  the `Record<string, unknown>` catch-all. */
import { describe, expect, it } from 'vitest';
import type { LogKind } from '../ipc/types/obs';
import type { UiRecordInput } from './types';

const shift: UiRecordInput = {
  kind: 'layout.shift',
  lvl: 'info',
  region: 'graph',
  phase: 'interactive',
  hadRecentInput: false,
  resizing: false,
  shifts: 3,
  value: 0.0412,
  firstAt: 1,
  spanMs: 412,
};

const lost: UiRecordInput = { kind: 'focus.lost', fromRegion: 'details', phase: 'boot', via: 'focusout', lostAt: 1 };

// @ts-expect-error — `region` is required.
const missingRegion: UiRecordInput = { kind: 'layout.shift', phase: 'interactive', hadRecentInput: false, resizing: false, shifts: 1, value: 0, firstAt: 1, spanMs: 0 };

// @ts-expect-error — `region` is a closed vocabulary, never a path.
const pathRegion: UiRecordInput = { kind: 'focus.lost', fromRegion: 'C:\\x', phase: 'boot', via: 'focusout', lostAt: 1 };

const kinds: LogKind[] = ['layout.shift', 'focus.lost'];

describe('UiPayload stability members (T-F11)', () => {
  it('type-checks the valid payloads (the real assertion is tsc)', () => {
    expect([shift.kind, lost.kind, missingRegion.kind, pathRegion.kind]).toEqual([...kinds, 'layout.shift', 'focus.lost']);
  });
});
