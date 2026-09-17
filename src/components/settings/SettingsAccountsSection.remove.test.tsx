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
 *  All three land in the dialog's own `.dialog-error` — `confirmRemove`
 *  (`SettingsAccountsSection.tsx:137-149`) never clears `removeTarget` on
 *  rejection, so the dialog stays open and Remove is retryable in place. */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

import { ipc } from '../../ipc';
import type { ForgeAccount } from '../../ipc';
import {
  REMOVE_KEYCHAIN_FAIL_MESSAGE,
  REMOVE_SETTINGS_FAIL_MESSAGE,
  REMOVE_SETTINGS_FAIL_NO_CREDENTIAL_MESSAGE,
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
    await expectDialogError(
      dialog,
      `Could not remove github.com: ${REMOVE_KEYCHAIN_FAIL_MESSAGE}`,
    );
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
    await expectDialogError(
      dialog,
      `Could not remove github.com: ${REMOVE_SETTINGS_FAIL_MESSAGE}`,
    );
    const text = dialog.querySelector('.dialog-error')?.textContent ?? '';
    expect(text).toContain('was removed from the OS keychain');
    expect(text).toContain('may still appear');
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
      return rejection === null ? Promise.resolve() : Promise.reject(rejection);
    });
    const dialog = await attemptRemove();
    await expectDialogError(
      dialog,
      `Could not remove github.com: ${REMOVE_KEYCHAIN_FAIL_MESSAGE}`,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Remove' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(attempt).toBe(2);
  });
});

describe('removeAccountRejection — mock seams', () => {
  it('covers every documented `?forgeRemoveFail` value and defaults to success', () => {
    expect(removeAccountRejection(null, 1)).toBeNull();
    expect(removeAccountRejection('1', 1)?.message).toBe(
      'cannot resolve app config dir: unknown path',
    );
    expect(removeAccountRejection('long', 1)?.message).toMatch(/^cannot resolve app config dir: /);
    expect((removeAccountRejection('long', 1)?.message ?? '').length).toBeGreaterThan(300);
    expect(removeAccountRejection('keychain', 9)?.message).toBe(REMOVE_KEYCHAIN_FAIL_MESSAGE);
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
