// Security-audit MEDIUM-2 (2026-09-17), source A — the failure seams of
// `forge_add_account` that were previously INVISIBLE, because the command did
// `let _ = settings::update(…)` and returned `Ok(viewer)` with the credential
// in the OS keychain and no record naming it.
//
// Own file (CLAUDE.md file-size discipline: `forge.ts` sits near the 500-line
// soft limit) and own module because these strings are CONTRACT text: each one
// is the backend's `format!` output verbatim, so the harness renders exactly
// what a user would see. Nothing here is invented copy — and that is enforced:
// `forge_add_account_tests::mock_copy_mirrors_the_rust_copy` include_str!s THIS
// file and asserts each whole Rust HEAD (plus `CAUSE_LEAD`) appears here as ONE
// quoted literal on a non-comment line, so editing the backend copy without
// editing here turns that test red.
//
// P114 rule 1: these messages are OUTCOMES and own the whole sentence. FOUR
// render sites pass `kind: 'other'` through verbatim, since the backend's
// `forge_set_token` now delegates to the same core:
// `SettingsAccountAddForm.addError`'s `default` arm returns `e.message`, and
// `SettingsAccountCard.submit`, `PrPanel.handleConnect` and
// `ChecksPanel.handleConnect` use `errorMessage(e)` — so what is written below
// is literally what the user reads under the token field. (The two panels used
// to ALSO toast `Could not connect: {message}`, which double-framed the
// sentence and announced it twice; P114 Addendum B deleted that toast — no
// single prefix can be right for a command that rejects with both cause-shaped
// and outcome-shaped messages, and `ForgeConnect`'s `role="alert"` banner IS
// the notification.)
//
// Reachable from both `?forgeAddFail=` (the Accounts settings section) and
// `?forgeSetTokenFail=` (the per-repo Connect field) — one helper, because the
// backend is one code path.
//
// FIDELITY NOTE, verified against `forge_add_account_inner_with`
// (`src-tauri/src/commands/forge_add_account.rs`): which message a failing
// settings write produces depends on a settings PRE-READ, because
// `TokenStore::set` OVERWRITES (`crates/bonsai-forge/src/auth.rs:113-118` →
// `auth.rs:47-51`, `keyring::Entry::set_password`) and the keychain key is
// derived deterministically from `(kind, host, login)`:
//   - `rolled-back` — no record named that key before the store, so the new
//     credential was referenced by nothing and is withdrawn again.
//   - `kept` — a record ALREADY named that key, so the store overwrote a live
//     credential. It is deliberately NOT deleted: the surviving record still
//     points at it, and deleting would render the account `connected: false`.
//     This is the discriminating outcome — an unconditional rollback would
//     destroy a working credential here.
//   - `rollback-failed` — case `rolled-back`, but the compensating delete was
//     itself refused. Carries BOTH causes, settings first.
import type { AppError } from '../../types';

/** `?forgeAddFail=` values this module understands. */
export type ForgeAddFailSeam = 'rolled-back' | 'kept' | 'rollback-failed' | null;

/** The `{e}` half of the settings write: `settings::save_to`'s real tmp-write
 *  failure (`settings.rs:407`). */
const SETTINGS_CAUSE =
  'io error: write C:\\Users\\dev\\AppData\\Roaming\\com.bonsai.app\\settings.json.9184.0.tmp: Access is denied. (os error 5)';

/** The second `{e}` of `rollback-failed`: real Windows Credential-Manager text
 *  from the refused delete. */
const DELETE_CAUSE = 'keychain error: Platform secure storage failure: Access is denied. (os error 5)';

/** Verbatim `forge_add_account.rs` HEAD + `CAUSE_LEAD` — i.e. ALL of a
 *  message's fixed text, as ONE literal. That is what lets the cross-language
 *  guard match a WHOLE message rather than a prefix and a suffix that could
 *  have come from two different strings. Cause-last mirrors Rust, so a
 *  `role="alert"`-adjacent utterance speaks the action first. Backtick literals
 *  because these contain apostrophes and the guard compares the Rust const
 *  byte-for-byte against this source. */
const ROLLED_BACK_TEXT = `The account couldn't be saved, and its credential is not in the OS keychain. Add it again. Details: `;
const CREDENTIAL_KEPT_TEXT = `The credential in the OS keychain is up to date, but the account details couldn't be saved. Try again to finish adding the account. Details: `;
const ROLLBACK_FAILED_TEXT = `The credential is in the OS keychain, but the account couldn't be saved. Try again to finish adding it. Details: `;

/** Verbatim `forge_add_account::CAUSE_JOIN` — the separator between the two
 *  causes of `rollback-failed`. */
const CAUSE_JOIN = `; `;

/** The store wrote a NEW credential; the failed save withdrew it again. */
export const ADD_ROLLED_BACK_MESSAGE = `${ROLLED_BACK_TEXT}${SETTINGS_CAUSE}`;

/** A re-add OVERWROTE a live credential, so it is kept. The discriminator. */
export const ADD_CREDENTIAL_KEPT_MESSAGE = `${CREDENTIAL_KEPT_TEXT}${SETTINGS_CAUSE}`;

/** The save failed AND the compensating delete was refused — both causes. */
export const ADD_ROLLBACK_FAILED_MESSAGE = `${ROLLBACK_FAILED_TEXT}${SETTINGS_CAUSE}${CAUSE_JOIN}${DELETE_CAUSE}`;

const other = (message: string): AppError => ({ kind: 'other', message });

/**
 * The rejection (if any) a `forgeAddAccount` call should throw for `seam`.
 * `null` ⇒ let the add proceed.
 */
export function addAccountRejection(seam: ForgeAddFailSeam): AppError | null {
  switch (seam) {
    case 'rolled-back':
      return other(ADD_ROLLED_BACK_MESSAGE);
    case 'kept':
      return other(ADD_CREDENTIAL_KEPT_MESSAGE);
    case 'rollback-failed':
      return other(ADD_ROLLBACK_FAILED_MESSAGE);
    default:
      return null;
  }
}
