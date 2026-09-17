/** ui-reference §12.14 — "a note clears when ANY operation reporting into its
 *  slot begins". Since P114 R4 the remove's leftover reports into the SECTION
 *  slot, so `confirmRemove` is one of those operations and calls
 *  `begin(SECTION_SLOT)`. That single line had no coverage: deleting it left all
 *  739 tests green, because every other section-slot test either never
 *  establishes a standing note first or ends in a `report` that overwrites it.
 *
 *  Own file (CLAUDE.md file-size discipline): the sibling suites are already
 *  large and this one guards a different invariant — slot HYGIENE, not copy.
 *
 *  Case chosen: the remove FAILURE path, which is §12.14's own rationale — a
 *  stale success note must not sit beside a fresh failure — and the only
 *  unambiguous one. On the R4 leftover path `report(SECTION_SLOT, …)` overwrites
 *  the note whether or not `begin` ran, so that path cannot see the line at all.
 *  The clean-success path (`leftover === null`) can, and is the reviewer's
 *  scope correction ("the wipe fires on EVERY attempt"), so it is pinned too as
 *  a second case. */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

import { ipc } from '../../ipc';
import type { ForgeAccount } from '../../ipc';
import { REMOVE_KEYCHAIN_FAIL_MESSAGE } from '../../ipc/mock/handlers/forgeRemoveFailure';
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

function sectionNote(): HTMLElement {
  const el = document.querySelector<HTMLElement>('[data-outcome-note="accounts.add"]');
  if (el === null) throw new Error('the section slot note must be mounted');
  return el;
}

function announcer(): HTMLElement {
  const el = document.querySelector<HTMLElement>('[role="status"][aria-live="polite"]');
  if (el === null) throw new Error('the section announcer must be mounted');
  return el;
}

/** Establish the standing section note by adding a token for a new host —
 *  `Connected to <host> as <login>.`, written into SECTION_SLOT. */
async function establishSectionNote(): Promise<void> {
  fireEvent.click(await screen.findByRole('button', { name: 'Add a token for a host' }));
  fireEvent.change(screen.getByLabelText('Host'), { target: { value: 'gitlab.com' } });
  fireEvent.change(screen.getByLabelText('Personal access token'), {
    target: { value: 'glpat_valid' },
  });
  fireEvent.click(screen.getByRole('button', { name: 'Add account' }));
  // Not a formality: without this the test would pass vacuously on a note that
  // was never written.
  await waitFor(() => expect(sectionNote()).toHaveTextContent('Connected to gitlab.com as octocat.'));
  expect(announcer().textContent).toBe('Connected to gitlab.com as octocat.');
}

/** Open the Remove dialog for the seeded account and click Remove. */
async function attemptRemove(): Promise<void> {
  fireEvent.click(await screen.findByRole('button', { name: /Actions for octocat/i }));
  fireEvent.click(screen.getByRole('menuitem', { name: 'Remove account' }));
  await screen.findByRole('dialog');
  fireEvent.click(screen.getByRole('button', { name: 'Remove' }));
}

describe('Accounts — a remove attempt wipes the standing section note (§12.14)', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(ipc, 'openUrl').mockResolvedValue(undefined);
    vi.spyOn(ipc, 'forgeAddAccount').mockResolvedValue({ login: 'octocat', avatarUrl: null });
    // Stable list: the github.com row stays through the add's refetch, so it is
    // there to be removed while the gitlab.com success note still stands.
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([GH_ACCOUNT]);
  });

  it('a failing remove clears it, so the stale success cannot sit beside the failure', async () => {
    vi.spyOn(ipc, 'forgeRemoveAccount').mockRejectedValue({
      kind: 'other',
      message: REMOVE_KEYCHAIN_FAIL_MESSAGE,
    });
    render(<SettingsAccountsSection />);

    await establishSectionNote();
    await attemptRemove();

    // The failure lands in the dialog's own alert…
    await waitFor(() =>
      expect(document.querySelector('.dialog-error')).toHaveTextContent(
        REMOVE_KEYCHAIN_FAIL_MESSAGE,
      ),
    );
    // …and nothing reports into the section slot on this path, so the note is
    // gone only because the operation's start wiped it.
    expect(sectionNote().textContent).toBe('');
    // `begin` clears the section's one announcement unconditionally too.
    expect(announcer().textContent).toBe('');
  });

  it('a clean success clears it as well — the wipe is per ATTEMPT, not per failure', async () => {
    vi.spyOn(ipc, 'forgeRemoveAccount').mockResolvedValue({ leftover: null });
    render(<SettingsAccountsSection />);

    await establishSectionNote();
    await attemptRemove();

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(sectionNote().textContent).toBe('');
    expect(announcer().textContent).toBe('');
  });
});
