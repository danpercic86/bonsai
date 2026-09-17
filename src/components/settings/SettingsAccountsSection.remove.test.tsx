/** The three user-ruled outcomes of `forge_remove_account` (2026-09-17), as the
 *  Accounts section renders them. Own file (the sibling suite already covers the
 *  happy paths; CLAUDE.md file-size discipline) because these assert CONTRACT
 *  text: the strings are imported from the mock module that mirrors the Rust
 *  `format!` output verbatim. This suite only guards the MOCK↔UI half — Rust
 *  and TS share no constant, so the cross-language half is guarded in Rust by
 *  `forge_remove_account_tests::mock_copy_mirrors_the_rust_copy`, which fails if
 *  a `format!` literal in `forge_remove_account.rs` is edited without updating
 *  `src/ipc/mock/handlers/forgeRemoveFailure.ts`.
 *
 *  All three land in the dialog's own `.dialog-error` — `confirmRemove` never
 *  clears `removeTarget` on rejection, so the dialog stays open and Remove is
 *  retryable in place. Outcome R4 (P114 Addendum A) is the exception and is
 *  covered below: it is a SUCCESS carrying a leftover, so it closes the dialog,
 *  refetches, and renders a section-level `--warn` note instead. */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

import { ipc } from '../../ipc';
import type { ForgeAccount } from '../../ipc';
import {
  REMOVE_CONFIG_DIR_FAIL_MESSAGE,
  REMOVE_KEYCHAIN_FAIL_MESSAGE,
  REMOVE_LEGACY_LEFTOVER_MESSAGE,
  REMOVE_SETTINGS_FAIL_MESSAGE,
  REMOVE_SETTINGS_FAIL_NO_CREDENTIAL_MESSAGE,
  REMOVE_TASK_JOIN_FAIL_MESSAGE,
  removeAccountLeftover,
  removeAccountRejection,
} from '../../ipc/mock/handlers/forgeRemoveFailure';
import { SettingsAccountsSection } from './SettingsAccountsSection';

const GH_ACCOUNT: ForgeAccount = {
  accountId: 'gitHub:github.com:octocat',
  host: 'github.com',
  kind: 'gitHub',
  login: 'octocat',
  avatarUrl: null,
  connected: true,
  isHostDefault: true,
};

/** Open the Remove dialog for the seeded account and click Remove. */
async function attemptRemove(): Promise<HTMLElement> {
  render(<SettingsAccountsSection />);
  fireEvent.click(await screen.findByRole('button', { name: /Actions for octocat/i }));
  fireEvent.click(screen.getByRole('menuitem', { name: 'Remove account' }));
  const dialog = await screen.findByRole('dialog');
  fireEvent.click(screen.getByRole('button', { name: 'Remove' }));
  return dialog;
}

async function expectDialogError(dialog: HTMLElement, text: string): Promise<void> {
  await waitFor(() => expect(dialog.querySelector('.dialog-error')).toHaveTextContent(text));
  // Still open, still retryable — the removal did not "succeed".
  expect(screen.getByRole('dialog')).toBe(dialog);
  expect(screen.getByRole('button', { name: 'Remove' })).toBeEnabled();
}

describe('Accounts — remove-account failure copy', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(ipc, 'openUrl').mockResolvedValue(undefined);
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([GH_ACCOUNT]);
  });

  /** Outcome 1 — the keychain refused, so nothing changed and the row stays. */
  it('a keychain refusal is reported verbatim and the account stays listed', async () => {
    const remove = vi
      .spyOn(ipc, 'forgeRemoveAccount')
      .mockRejectedValue({ kind: 'other', message: REMOVE_KEYCHAIN_FAIL_MESSAGE });
    const dialog = await attemptRemove();
    // P114 rule 1: verbatim — no `Could not remove <host>: ` prefix.
    await expectDialogError(dialog, REMOVE_KEYCHAIN_FAIL_MESSAGE);
    expect(remove).toHaveBeenCalledWith(GH_ACCOUNT.accountId);
    // The copy must say the account is still there — that is the retry cue.
    expect(dialog.querySelector('.dialog-error')?.textContent).toContain('still listed');
  });

  /** Outcome 3 — the credential IS gone; only the list write failed. The two
   *  messages must be distinguishable, which is the whole point of the ruling. */
  it('a settings-save failure says the credential is gone but the list was not saved', async () => {
    vi.spyOn(ipc, 'forgeRemoveAccount').mockRejectedValue({
      kind: 'other',
      message: REMOVE_SETTINGS_FAIL_MESSAGE,
    });
    const dialog = await attemptRemove();
    await expectDialogError(dialog, REMOVE_SETTINGS_FAIL_MESSAGE);
    const text = dialog.querySelector('.dialog-error')?.textContent ?? '';
    // P114 rule 2 (state, not act): "is no longer in" survives a retry, where
    // the `NoEntry` fold means nothing is deleted and "was removed" is false.
    expect(text).toContain('is no longer in the OS keychain');
    expect(text).not.toContain('was removed');
    expect(text).toContain('Try again to finish removing it');
    expect(text).not.toContain('Nothing was changed');
  });

  /** Outcome 2 — a key that is no longer in the keychain is SUCCESS, so the
   *  retry after outcome 1 closes the dialog. Uses the mock's own
   *  `keychain-then-ok` seam so the harness knob and this suite agree. */
  it('the retry after a keychain refusal succeeds and closes the dialog', async () => {
    let attempt = 0;
    vi.spyOn(ipc, 'forgeRemoveAccount').mockImplementation(() => {
      attempt += 1;
      const rejection = removeAccountRejection('keychain-then-ok', attempt);
      return rejection === null
        ? Promise.resolve({ leftover: null })
        : Promise.reject(rejection);
    });
    const dialog = await attemptRemove();
    // P114 rule 1: verbatim — no `Could not remove <host>: ` prefix.
    await expectDialogError(dialog, REMOVE_KEYCHAIN_FAIL_MESSAGE);
    fireEvent.click(screen.getByRole('button', { name: 'Remove' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(attempt).toBe(2);
  });
});

/** Outcome R4 (P114 Addendum A) — the account WAS removed and the host's
 *  leftover legacy credential was refused. It is a SUCCESS with a payload, so
 *  the dialog must CLOSE, the list must refetch, and the sentence must render
 *  as a section-level `--warn` note plus the announcer — never in the dialog's
 *  `.dialog-error`, which stays open for a retry that is impossible here. */
describe('Accounts — remove leaves a legacy leftover (outcome R4)', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(ipc, 'openUrl').mockResolvedValue(undefined);
  });

  it('closes the dialog, refetches, and reports the leftover as a warn note', async () => {
    const list = vi
      .spyOn(ipc, 'forgeListAccounts')
      .mockResolvedValueOnce([GH_ACCOUNT])
      .mockResolvedValue([]);
    vi.spyOn(ipc, 'forgeRemoveAccount').mockResolvedValue({
      leftover: REMOVE_LEGACY_LEFTOVER_MESSAGE,
    });
    await attemptRemove();
    // The dialog CLOSES: its subject no longer exists, and its Remove button
    // would be a silent no-op on a second click.
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    // And the list REFETCHES, so the removed row cannot linger behind it.
    await waitFor(() => expect(list).toHaveBeenCalledTimes(2));
    // The message renders VERBATIM in the `--warn` recipe (`--warning`, not
    // `--danger`: nothing was lost), at SECTION level — the host group has just
    // been emptied and unmounted, so a note keyed to it would clear with it.
    const note = await waitFor(() => {
      const el = document.querySelector<HTMLElement>('.settings-row-note--warn');
      if (el === null) throw new Error('the leftover must render as a warn note');
      return el;
    });
    expect(note).toHaveTextContent(REMOVE_LEGACY_LEFTOVER_MESSAGE);
    expect(document.querySelector('.dialog-error')).toBeNull();
    // One utterance, from the same `report` call that wrote the note.
    const live = document.querySelector<HTMLElement>('[role="status"][aria-live="polite"]');
    expect(live?.textContent).toBe(REMOVE_LEGACY_LEFTOVER_MESSAGE);
  });

  it('reports NO note when the removal leaves nothing behind', async () => {
    vi.spyOn(ipc, 'forgeListAccounts')
      .mockResolvedValueOnce([GH_ACCOUNT])
      .mockResolvedValue([]);
    vi.spyOn(ipc, 'forgeRemoveAccount').mockResolvedValue({ leftover: null });
    await attemptRemove();
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(document.querySelector('.settings-row-note--warn')).toBeNull();
  });
});

describe('removeAccountRejection — mock seams', () => {
  it('covers every documented `?forgeRemoveFail` value and defaults to success', () => {
    expect(removeAccountRejection(null, 1)).toBeNull();
    expect(removeAccountRejection('1', 1)?.message).toBe(REMOVE_CONFIG_DIR_FAIL_MESSAGE);
    // P114 N1/N2: the wrappers turn `settings::settings_file`'s bare lowercase
    // cause into a sentence, with the cause last behind `Details: `.
    expect(removeAccountRejection('1', 1)?.message).toMatch(
      /^Couldn't remove the account — Bonsai can't reach its settings folder\. Details: cannot resolve app config dir: /,
    );
    expect(removeAccountRejection('long', 1)?.message).toMatch(
      /^Couldn't remove the account — Bonsai can't reach its settings folder\. Details: cannot resolve app config dir: /,
    );
    expect(removeAccountRejection('task-join', 1)?.message).toBe(REMOVE_TASK_JOIN_FAIL_MESSAGE);
    expect((removeAccountRejection('long', 1)?.message ?? '').length).toBeGreaterThan(300);
    expect(removeAccountRejection('keychain', 9)?.message).toBe(REMOVE_KEYCHAIN_FAIL_MESSAGE);
    // P114 Addendum A: the legacy seam is NOT a rejection any more — the sweep
    // is best effort, so the removal succeeds and the leftover rides the
    // fulfilled value.
    expect(removeAccountRejection('legacy-keychain', 9)).toBeNull();
    expect(removeAccountLeftover('legacy-keychain')).toBe(REMOVE_LEGACY_LEFTOVER_MESSAGE);
    expect(removeAccountLeftover(null)).toBeNull();
    expect(removeAccountLeftover('keychain')).toBeNull();
    // It must not reuse the account-credential copy (a different key), must
    // carry no retry cue (there is nothing left to retry), and must name the
    // bare HOST rather than the login (nothing named `login` is left behind).
    expect(REMOVE_LEGACY_LEFTOVER_MESSAGE).not.toBe(REMOVE_KEYCHAIN_FAIL_MESSAGE);
    expect(REMOVE_LEGACY_LEFTOVER_MESSAGE).not.toContain('try again');
    expect(REMOVE_LEGACY_LEFTOVER_MESSAGE).not.toContain('Nothing was changed');
    expect(REMOVE_LEGACY_LEFTOVER_MESSAGE).toContain('for github.com');
    expect(removeAccountRejection('settings', 9)?.message).toBe(REMOVE_SETTINGS_FAIL_MESSAGE);
    expect(removeAccountRejection('settings-no-credential', 9)?.message).toBe(
      REMOVE_SETTINGS_FAIL_NO_CREDENTIAL_MESSAGE,
    );
    // Outcome 3' must NOT claim a credential was deleted — that is the whole
    // reason it is a separate message (`rec == None` never calls `delete_token`).
    expect(REMOVE_SETTINGS_FAIL_NO_CREDENTIAL_MESSAGE).not.toContain('keychain');
    // And the harness must render single backslashes, as `io::Error` Display does.
    expect(REMOVE_SETTINGS_FAIL_MESSAGE).toContain('C:\\Users\\dev\\AppData');
    expect(REMOVE_SETTINGS_FAIL_MESSAGE).not.toContain('C:\\\\Users');
    expect(removeAccountRejection('keychain-then-ok', 1)?.message).toBe(
      REMOVE_KEYCHAIN_FAIL_MESSAGE,
    );
    expect(removeAccountRejection('keychain-then-ok', 2)).toBeNull();
  });
});
