/** Audit MEDIUM-2 source A — the `?forgeAddFail=` mock seams.
 *
 *  Own file (the settings section's own suites are already large) and a thin
 *  one: the Rust side owns the copy, and `forge_add_account_tests::
 *  mock_copy_mirrors_the_rust_copy` is what pins these literals to it. What
 *  this adds is the TS-side contract — every documented URL value maps to the
 *  message the backend would send, an unknown/absent value lets the add
 *  proceed, and the three outcomes stay DISTINGUISHABLE (the whole point of the
 *  pre-read discriminator is that a re-add and a new account do not read the
 *  same).
 */
import { describe, expect, it } from 'vitest';
import {
  ADD_CREDENTIAL_KEPT_MESSAGE,
  ADD_ROLLBACK_FAILED_MESSAGE,
  ADD_ROLLED_BACK_MESSAGE,
  addAccountRejection,
} from './forgeAddFailure';

describe('addAccountRejection — mock seams', () => {
  it('covers every documented `?forgeAddFail` value and defaults to success', () => {
    expect(addAccountRejection(null)).toBeNull();
    expect(addAccountRejection('rolled-back')?.message).toBe(ADD_ROLLED_BACK_MESSAGE);
    expect(addAccountRejection('kept')?.message).toBe(ADD_CREDENTIAL_KEPT_MESSAGE);
    expect(addAccountRejection('rollback-failed')?.message).toBe(ADD_ROLLBACK_FAILED_MESSAGE);
    for (const seam of ['rolled-back', 'kept', 'rollback-failed'] as const) {
      expect(addAccountRejection(seam)?.kind).toBe('other');
    }
  });

  it('keeps the three outcomes distinguishable, cause-last', () => {
    const messages = [
      ADD_ROLLED_BACK_MESSAGE,
      ADD_CREDENTIAL_KEPT_MESSAGE,
      ADD_ROLLBACK_FAILED_MESSAGE,
    ];
    expect(new Set(messages).size).toBe(3);
    for (const m of messages) {
      // P114 §3: every human sentence precedes the interpolated cause.
      expect(m).toContain('Details: ');
      expect(m.indexOf('Details: ')).toBeGreaterThan(0);
      expect(m).toContain('the OS keychain');
      expect(m).not.toContain('token');
      expect(m).not.toContain('PAT');
    }
    // The refused-rollback outcome is the only one carrying TWO causes.
    expect(ADD_ROLLBACK_FAILED_MESSAGE).toContain('; keychain error:');
  });
});
