// Security-audit MEDIUM-1 FU — the reachable failure seams of
// `forge_clear_token_for_host` (sign out of a whole host).
//
// Own file (CLAUDE.md file-size discipline: `forge.ts` sits at the 500-line soft
// limit) and own module because these strings are CONTRACT text: each one is the
// backend's `format!` output verbatim, so the harness renders exactly what a
// user would see. Nothing here is invented copy — and that is enforced:
// `forge_clear_host_tests::mock_copy_mirrors_the_rust_copy` include_str!s THIS
// file and asserts the fixed halves of every Rust message (and the separator
// that joins multiple causes) appear in it, so
// editing the backend copy without editing here turns that test red.
//
// FIDELITY NOTE, verified against `forge_clear_token_for_host_inner_with`
// (`src-tauri/src/commands/forge_clear_host.rs`): as of the 2026-09-17 user
// ruling the command no longer swallows its keychain deletes. It attempts all
// N+1 keys (every account's `keychain_key` plus the legacy bare-host key) and,
// if ANY is refused, returns `Err` and mutates NOTHING — the accounts stay
// listed so the sign-out is retryable.
// Reachable rejections:
//   - `cannot resolve app config dir: {e}` — from `settings::settings_file` in
//     the outer command, before the core runs (seam key `'config-dir'`).
//   Outcome 1. the keychain refused one or more deletes — no settings or UI
//     state changed (not even the cached viewer, which is still accurate because
//     the tokens are still live), so the rows stay listed and sign-out is
//     retryable in place. Every cause is reported, joined with `'; '`.
//   Outcome 1', the empty-host half: the host has no accounts, so only the
//     best-effort legacy bare-host key could be refused — and then there are NO
//     rows listed, so the copy must not claim "the accounts are still listed"
//     (same overstatement `e583f11` removed from the sibling).
//   Outcome 3. the credentials WERE all deleted but `settings::update` failed —
//     a different user-facing fact (the accounts may still be listed until the
//     save lands), which is why the two have distinguishable copy.
//   Outcome 3', the empty-host half: no account on the host named a credential,
//     so `delete_token` was only ever called for the best-effort legacy
//     bare-host key. Its copy must NOT claim credentials were removed (that
//     false claim is the bug `e583f11` fixed in the per-account sibling).
//   - `task join error: {e}` — a panicking blocking task; not modelled.
// A key that is NOT in the keychain is SUCCESS, not a failure
// (`crates/bonsai-forge/src/auth.rs:53` folds `keyring::Error::NoEntry` into
// `Ok(())`), which is what keeps the command re-runnable after outcome 1 — the
// `keychain-then-ok` seam below is that retry, end to end.
import type { AppError } from '../../types';

/** `?forgeClearHostFail=` values this module understands. */
export type ForgeClearHostFailSeam =
  | 'config-dir'
  | 'keychain'
  | 'keychain-all'
  | 'keychain-no-account'
  | 'settings'
  | 'settings-no-credential'
  | 'keychain-then-ok'
  | null;

/** The `{e}` half of outcome 1: one refused delete, real Credential-Manager text. */
const KEYCHAIN_CAUSE =
  'keychain error: Platform secure storage failure: Access is denied. (os error 5)';

/** Outcome 1 — verbatim `forge_clear_host.rs` keychain-refusal text (k of N). */
export const CLEAR_HOST_KEYCHAIN_FAIL_MESSAGE = `could not remove the credentials from the OS keychain: ${KEYCHAIN_CAUSE}. Nothing was changed — the accounts are still listed, so you can try again.`;

/** Outcome 1, all N+1 refused: every cause is reported, joined with `'; '`. */
export const CLEAR_HOST_KEYCHAIN_FAIL_ALL_MESSAGE = `could not remove the credentials from the OS keychain: ${KEYCHAIN_CAUSE}; ${KEYCHAIN_CAUSE}; ${KEYCHAIN_CAUSE}. Nothing was changed — the accounts are still listed, so you can try again.`;

/** Outcome 1' — verbatim `forge_clear_host.rs` text for a host with NO accounts,
 *  where only the leftover legacy credential could be refused, so the copy does
 *  not claim any rows are still listed. */
export const CLEAR_HOST_KEYCHAIN_FAIL_NO_ACCOUNT_MESSAGE = `could not remove the credentials from the OS keychain: ${KEYCHAIN_CAUSE}. Nothing was changed — this host has no accounts listed; a leftover credential for it could not be removed, so you can try again.`;

/** The `{e}` half of outcome 3: `settings::save_to`'s real tmp-write failure. */
const SETTINGS_CAUSE =
  'io error: write C:\\Users\\dev\\AppData\\Roaming\\com.bonsai.app\\settings.json.9184.0.tmp: Access is denied. (os error 5)';

/** Outcome 3 — verbatim `forge_clear_host.rs` settings-save text. */
export const CLEAR_HOST_SETTINGS_FAIL_MESSAGE = `the credentials were removed from the OS keychain, but the account list could not be saved: ${SETTINGS_CAUSE}. The accounts may still appear until settings can be written.`;

/** Outcome 3' — verbatim `forge_clear_host.rs` settings-save text for a host with
 *  no accounts, where NO credential was named, so the copy says only that the
 *  list could not be saved. */
export const CLEAR_HOST_SETTINGS_FAIL_NO_CREDENTIAL_MESSAGE = `the account list could not be saved: ${SETTINGS_CAUSE}.`;

const other = (message: string): AppError => ({ kind: 'other', message });

/** 1-based attempt count per host, so `keychain-then-ok` can fail only the first. */
const attempts = new Map<string, number>();

/**
 * The rejection (if any) a `forgeClearTokenForHost(host)` call should throw for
 * `seam`. `keychain-then-ok` fails the FIRST attempt on that host and succeeds
 * the second — the idempotent retry the ruling depends on. `null` ⇒ let the
 * sign-out proceed.
 */
export function clearHostRejection(seam: ForgeClearHostFailSeam, host: string): AppError | null {
  const attempt = (attempts.get(host) ?? 0) + 1;
  attempts.set(host, attempt);
  switch (seam) {
    case 'config-dir':
      return other('cannot resolve app config dir: unknown path');
    case 'keychain':
      return other(CLEAR_HOST_KEYCHAIN_FAIL_MESSAGE);
    case 'keychain-all':
      return other(CLEAR_HOST_KEYCHAIN_FAIL_ALL_MESSAGE);
    case 'keychain-no-account':
      return other(CLEAR_HOST_KEYCHAIN_FAIL_NO_ACCOUNT_MESSAGE);
    case 'settings':
      return other(CLEAR_HOST_SETTINGS_FAIL_MESSAGE);
    case 'settings-no-credential':
      return other(CLEAR_HOST_SETTINGS_FAIL_NO_CREDENTIAL_MESSAGE);
    case 'keychain-then-ok':
      return attempt === 1 ? other(CLEAR_HOST_KEYCHAIN_FAIL_MESSAGE) : null;
    default:
      return null;
  }
}
