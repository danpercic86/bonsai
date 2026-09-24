import { describe, expect, it } from 'vitest';
import { isUnpromptedShift } from './classify';
import type { StabilityPhase } from './types';

describe('isUnpromptedShift (T-F12)', () => {
  const phases: StabilityPhase[] = ['boot', 'interactive'];
  for (const phase of phases) {
    for (const hadRecentInput of [false, true]) {
      for (const resizing of [false, true]) {
        const expected = phase === 'interactive' && !hadRecentInput && !resizing;
        it(`${phase} input=${hadRecentInput} resizing=${resizing} → ${expected}`, () => {
          expect(isUnpromptedShift({ phase, hadRecentInput, resizing })).toBe(expected);
        });
      }
    }
  }
});
