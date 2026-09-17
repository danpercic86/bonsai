// Security-audit MEDIUM-1 FU — the reachable failure seams of
// `forge_clear_token_for_host` (sign out of a whole host).
//
// Own file (CLAUDE.md file-size discipline: `forge.ts` sits at the 500-line soft
// limit) and own module because these strings are CONTRACT text: each one is the
// backend's `format!` output verbatim, so the harness renders exactly what a
// user would see. Nothing here is invented copy — and that is enforced:
// `forge_clear_host_tests::mock_copy_mirrors_the_rust_copy` include_str!s THIS
// file and asserts each whole Rust message text (and the separator that joins
// multiple causes) appears here as ONE quoted literal on a non-comment line, so
// editing the backend copy without editing here turns that test red.
//
// P114 rule 1: these messages are OUTCOMES and own the whole sentence — a
// caller would render them verbatim, with no prefix. They are console-only
// today because the command is dormant (see below).
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
//   - `task join error: {e}` — a panicking blocking task; not modelled, and
//     (unlike the reachable sibling's N2) not in P114's scope while dormant.
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

/** The `{e}` half of outcome 3: `settings::save_to`'s real tmp-write failure. */
const SETTINGS_CAUSE =
  'io error: write C:\\Users\\dev\\AppData\\Roaming\\com.bonsai.app\\settings.json.9184.0.tmp: Access is denied. (os error 5)';

/** Verbatim `forge_clear_host.rs` HEAD + `CAUSE_LEAD` — i.e. ALL of a message's
 *  fixed text, as ONE literal, so the cross-language guard can match a WHOLE
 *  message instead of a prefix and a suffix that could have come from two
 *  different strings. Three of them name the host: `${host}` sits exactly where
 *  the Rust const carries its `{host}` placeholder, and the guard renders the
 *  placeholder to that form before matching. Backtick literals throughout
 *  because each contains an apostrophe (an escaped quote would not match the
 *  Rust const byte-for-byte). */
const KEYCHAIN_FAIL_TEXT = `Couldn't remove this host's credentials from the OS keychain. Nothing was changed — the accounts are still listed, so you can try again. Details: `;
const keychainFailNoAccountText = (host: string) =>
  `A leftover credential for ${host} couldn't be removed from the OS keychain. Nothing was changed, so you can try again. Details: `;
const settingsFailText = (host: string) =>
  `The credentials are no longer in the OS keychain, but the account list couldn't be saved. Try again to finish signing out of ${host}. Details: `;
const settingsFailNoCredentialText = (host: string) =>
  `The account list couldn't be saved. Try again to finish signing out of ${host}. Details: `;

/** Outcome 1 — the keychain refused one of the deletes (k of N). */
export const CLEAR_HOST_KEYCHAIN_FAIL_MESSAGE = `${KEYCHAIN_FAIL_TEXT}${KEYCHAIN_CAUSE}`;

/** Outcome 1, all N+1 refused: every cause is reported, joined with `'; '`. */
export const CLEAR_HOST_KEYCHAIN_FAIL_ALL_MESSAGE = `${KEYCHAIN_FAIL_TEXT}${KEYCHAIN_CAUSE}; ${KEYCHAIN_CAUSE}; ${KEYCHAIN_CAUSE}`;

/** Outcome 1' — a host with NO accounts, where only the leftover legacy
 *  credential could be refused, so the copy names it and claims nothing about
 *  rows that are still listed. */
export const clearHostKeychainFailNoAccountMessage = (host: string): string =>
  `${keychainFailNoAccountText(host)}${KEYCHAIN_CAUSE}`;

/** Outcome 3 — the credentials are gone; only the list write failed. */
export const clearHostSettingsFailMessage = (host: string): string =>
  `${settingsFailText(host)}${SETTINGS_CAUSE}`;

/** Outcome 3' — no account on the host named a credential, so the copy says
 *  only that the list could not be saved. */
export const clearHostSettingsFailNoCredentialMessage = (host: string): string =>
  `${settingsFailNoCredentialText(host)}${SETTINGS_CAUSE}`;

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
      return other(clearHostKeychainFailNoAccountMessage(host));
    case 'settings':
      return other(clearHostSettingsFailMessage(host));
    case 'settings-no-credential':
      return other(clearHostSettingsFailNoCredentialMessage(host));
    case 'keychain-then-ok':
      return attempt === 1 ? other(CLEAR_HOST_KEYCHAIN_FAIL_MESSAGE) : null;
    default:
      return null;
  }
}
