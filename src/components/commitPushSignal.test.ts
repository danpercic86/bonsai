import { describe, expect, it } from 'vitest';

import { COMMIT_HOOK_CANCELED, COMMIT_PUSH_CANCELED } from './commitPushSignal';

describe('commit cancellation sentinels', () => {
  it('both are symbols with stable descriptions', () => {
    expect(typeof COMMIT_PUSH_CANCELED).toBe('symbol');
    expect(typeof COMMIT_HOOK_CANCELED).toBe('symbol');
    expect(COMMIT_PUSH_CANCELED.description).toBe('commitPushCanceled');
    expect(COMMIT_HOOK_CANCELED.description).toBe('commitHookCanceled');
  });

  it('the two sentinels are distinct (flows must not be confusable)', () => {
    expect(COMMIT_PUSH_CANCELED).not.toBe(COMMIT_HOOK_CANCELED);
  });

  it('sentinels are not Errors and not registered global symbols', () => {
    expect(COMMIT_PUSH_CANCELED instanceof Object).toBe(false);
    expect(Symbol.keyFor(COMMIT_PUSH_CANCELED)).toBeUndefined();
    expect(Symbol.keyFor(COMMIT_HOOK_CANCELED)).toBeUndefined();
  });
});
