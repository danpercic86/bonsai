// P113 FU — the reachable failure seams of `forge_remove_account`.
//
// Own file (CLAUDE.md file-size discipline: `forge.ts` sits at the 500-line
// soft limit) and own module because these strings are CONTRACT text: each one
// is the backend's `format!` output verbatim, so the harness renders exactly
// what a user would see. Nothing here is invented copy — and that is enforced:
// `forge_remove_account_tests::mock_copy_mirrors_the_rust_copy` include_str!s
// THIS file and asserts the fixed halves of all three Rust messages appear in
// it, so editing the backend copy without editing here turns that test red.
//
// FIDELITY NOTE, verified against `forge_remove_account_inner_with`
// (`src-tauri/src/commands/forge_remove_account.rs`): as of the 2026-09-17 user
// ruling, `forge_remove_account` no longer swallows its two substantive
// failures. Numbering below follows the ruling's OUTCOME numbers (1 = keychain
// refusal, 3 = settings save failed); the unnumbered entries are failures that
// happen outside the three ruled outcomes. NB the `'1'` seam KEY below is a URL
// knob name, not outcome 1 — it selects the config-dir failure.
// Reachable rejections:
//   - `cannot resolve app config dir: {e}` — from `settings::settings_file` in
//     the outer command, before the core runs (seam key `'1'`).
//   Outcome 1. the keychain refused the delete — no settings or UI state
//     changed, so the row stays listed and Remove is retryable in place.
//   Outcome 3. the credential WAS deleted but `settings::update` failed — a
//     different user-facing fact (the account may still be listed until settings
//     save), which is why the two have distinguishable copy.
//   Outcome 3', the `rec == None` half: the id was unknown or already removed, so
//     `delete_token` never ran and `settings::update` still failed. Its copy must
//     NOT claim a credential was deleted (that would be false).
//   - `task join error: {e}` — a panicking blocking task; not modelled.
// A key that is NOT in the keychain is SUCCESS, not a failure
// (`crates/bonsai-forge/src/auth.rs:53` folds `keyring::Error::NoEntry` into
// `Ok(())`), which is what keeps the command re-runnable after outcome 1 — the
// `keychain-then-ok` seam below is that retry, end to end.
import type { AppError } from '../../types';

/** `?forgeRemoveFail=` values this module understands. */
export type ForgeRemoveFailSeam =
  | '1'
  | 'long'
  | 'keychain'
  | 'settings'
  | 'settings-no-credential'
  | 'keychain-then-ok'
  | null;

/** Outcome 1 — verbatim `forge_remove_account.rs` keychain-refusal text. */
export const REMOVE_KEYCHAIN_FAIL_MESSAGE =
  'could not remove the credential from the OS keychain: keychain error: Platform secure storage failure: Access is denied. (os error 5). Nothing was changed — the account is still listed, so you can try again.';

/** Outcome 3 — verbatim `forge_remove_account.rs` settings-save text. The `{e}`
 *  half is `settings::save_to`'s real tmp-write failure (`settings.rs:407`). */
export const REMOVE_SETTINGS_FAIL_MESSAGE =
  'the credential was removed from the OS keychain, but the account list could not be saved: io error: write C:\\Users\\dev\\AppData\\Roaming\\com.bonsai.app\\settings.json.9184.0.tmp: Access is denied. (os error 5). The account may still appear until settings can be written.';

/** Outcome 3' — verbatim `forge_remove_account.rs` settings-save text for an
 *  unknown / already-removed id, where NO credential was deleted, so the copy
 *  says only that the list could not be saved. */
export const REMOVE_SETTINGS_FAIL_NO_CREDENTIAL_MESSAGE =
  'the account list could not be saved: io error: write C:\\Users\\dev\\AppData\\Roaming\\com.bonsai.app\\settings.json.9184.0.tmp: Access is denied. (os error 5).';

/** §14's pathological `{e}`: a ~330-char space-free UNC path. */
const LONG_CAUSE = `\\\\?\\UNC\\corp-file-cluster-07.ad.internal.example.com\\redirected-profiles$\\${'very-long-segment-'.repeat(12)}AppData\\Roaming\\com.bonsai.app\\settings.json`;

const other = (message: string): AppError => ({ kind: 'other', message });

/**
 * The rejection (if any) a `forgeRemoveAccount` call should throw for `seam`.
 *
 * `attempt` is the 1-based call count for that account, so `keychain-then-ok`
 * fails the FIRST attempt and succeeds the second — the idempotent retry the
 * ruling depends on. `null` ⇒ let the removal proceed.
 */
export function removeAccountRejection(seam: ForgeRemoveFailSeam, attempt: number): AppError | null {
  switch (seam) {
    case '1':
      return other('cannot resolve app config dir: unknown path');
    case 'long':
      return other(`cannot resolve app config dir: ${LONG_CAUSE}`);
    case 'keychain':
      return other(REMOVE_KEYCHAIN_FAIL_MESSAGE);
    case 'settings':
      return other(REMOVE_SETTINGS_FAIL_MESSAGE);
    case 'settings-no-credential':
      return other(REMOVE_SETTINGS_FAIL_NO_CREDENTIAL_MESSAGE);
    case 'keychain-then-ok':
      return attempt === 1 ? other(REMOVE_KEYCHAIN_FAIL_MESSAGE) : null;
    default:
      return null;
  }
}
