// P113 FU — the reachable failure seams of `forge_remove_account`.
//
// Own file (CLAUDE.md file-size discipline: `forge.ts` sits at the 500-line
// soft limit) and own module because these strings are CONTRACT text: each one
// is the backend's `format!` output verbatim, so the harness renders exactly
// what a user would see. Nothing here is invented copy — and that is enforced:
// `forge_remove_account_tests::mock_copy_mirrors_the_rust_copy` include_str!s
// THIS file and asserts each whole Rust HEAD (plus `CAUSE_LEAD`) appears here as
// ONE quoted literal on a non-comment line, so editing the backend copy without
// editing here turns that test red.
//
// P114 rule 1: these messages are OUTCOMES and own the whole sentence —
// `SettingsAccountsSection.confirmRemove` renders them verbatim with no prefix,
// so what is written below is literally what the user reads in the dialog.
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
//   - a panicking blocking task (seam key `'task-join'`). P114 N2 wraps the
//     `JoinError` into sentence form in the command file, and its copy is the
//     one that must NOT promise either way about the state.
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
  | 'task-join'
  | 'keychain-then-ok'
  | null;


/** The `{e}` half of outcome 1: real Windows Credential-Manager text. */
const KEYCHAIN_CAUSE =
  'keychain error: Platform secure storage failure: Access is denied. (os error 5)';

/** The `{e}` half of outcome 3: `settings::save_to`'s real tmp-write failure
 *  (`settings.rs:407`). */
const SETTINGS_CAUSE =
  'io error: write C:\\Users\\dev\\AppData\\Roaming\\com.bonsai.app\\settings.json.9184.0.tmp: Access is denied. (os error 5)';

/** The `{e}` half of N1: `settings::settings_file`'s own bare lowercase cause,
 *  which N1 wraps rather than edits (it is shared by many commands). */
const CONFIG_DIR_CAUSE = 'cannot resolve app config dir: unknown path';

/** The `{e}` half of N2: a tokio `JoinError`'s `Display`. */
const TASK_JOIN_CAUSE = 'task 42 panicked with message "boom"';

/** Verbatim `forge_remove_account.rs` HEAD + `CAUSE_LEAD` — i.e. ALL of a
 *  message's fixed text, as ONE literal. That is what lets the cross-language
 *  guard match a WHOLE message rather than a prefix and a suffix that could
 *  have come from two different strings. Cause-last mirrors Rust: the
 *  interpolated cause is appended behind `Details: `, so a `role="alert"`
 *  utterance speaks the action first. Backtick literals because every one
 *  contains an apostrophe, and the guard compares the Rust const byte-for-byte
 *  against this source, where a backslash-escaped quote would not match. */
const KEYCHAIN_FAIL_TEXT = `Couldn't remove the account's credential from the OS keychain. Nothing was changed — the account is still listed, so you can try again. Details: `;
const SETTINGS_FAIL_TEXT = `The credential is no longer in the OS keychain, but the account list couldn't be saved. Try again to finish removing it. Details: `;
const SETTINGS_FAIL_NO_CREDENTIAL_TEXT = `The account list couldn't be saved. Try again to finish removing it. Details: `;
const CONFIG_DIR_FAIL_TEXT = `Couldn't remove the account — Bonsai can't reach its settings folder. Details: `;
const TASK_JOIN_FAIL_TEXT = `Couldn't remove the account. It may or may not have been removed — check the list and try again. Details: `;

/** Outcome 1 — the keychain refused the delete. */
export const REMOVE_KEYCHAIN_FAIL_MESSAGE = `${KEYCHAIN_FAIL_TEXT}${KEYCHAIN_CAUSE}`;

/** Outcome 3 — the credential is gone; only the list write failed. */
export const REMOVE_SETTINGS_FAIL_MESSAGE = `${SETTINGS_FAIL_TEXT}${SETTINGS_CAUSE}`;

/** Outcome 3' — an unknown / already-removed id, where NO credential was
 *  deleted, so the copy says only that the list could not be saved. */
export const REMOVE_SETTINGS_FAIL_NO_CREDENTIAL_MESSAGE = `${SETTINGS_FAIL_NO_CREDENTIAL_TEXT}${SETTINGS_CAUSE}`;

/** N1 — the settings folder is unreachable, wrapped into sentence form. */
export const REMOVE_CONFIG_DIR_FAIL_MESSAGE = `${CONFIG_DIR_FAIL_TEXT}${CONFIG_DIR_CAUSE}`;

/** N2 — the blocking task panicked, so the state is genuinely unknown. */
export const REMOVE_TASK_JOIN_FAIL_MESSAGE = `${TASK_JOIN_FAIL_TEXT}${TASK_JOIN_CAUSE}`;

/** §14's pathological `{e}`: a ~330-char space-free UNC path. */
const LONG_CAUSE = `\\\\?\\UNC\\corp-file-cluster-07.ad.internal.example.com\\redirected-profiles$\\${'very-long-segment-'.repeat(12)}AppData\\Roaming\\com.bonsai.app\\settings.json`;

/** §14 — N1 again, with the pathological cause `overflow-wrap: anywhere` must
 *  wrap in `.dialog-error`. */
export const REMOVE_CONFIG_DIR_LONG_FAIL_MESSAGE = `${CONFIG_DIR_FAIL_TEXT}cannot resolve app config dir: ${LONG_CAUSE}`;

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
      return other(REMOVE_CONFIG_DIR_FAIL_MESSAGE);
    case 'long':
      return other(REMOVE_CONFIG_DIR_LONG_FAIL_MESSAGE);
    case 'task-join':
      return other(REMOVE_TASK_JOIN_FAIL_MESSAGE);
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
