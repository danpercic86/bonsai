/**
 * P113 phase 2, fixes S1 and S2 — the two Accounts defects the code review found,
 * each measured on the property that is actually broken.
 *
 * **S2 (AC15, amended scope → AC17).** Rows 9/10 call `report` with no preceding
 * clear, so re-adding the same login to the same host writes a byte-identical
 * announcement, React bails on the identical state, and the SECOND outcome is
 * SILENT — the exact bug §8.1 exists to kill, surviving in two of the ten
 * phase-1 sites.
 *
 * The fix used to be a LOCAL `flushSync(() => begin(key))` in this section. It
 * is now `useOutcomeNotes.report` itself, keyed on TEXT identity for every key
 * (AC17, `outcomeNotesAc17.test.tsx`): the local workaround covered these two
 * sites and left the other eight one identical string away from the same
 * silence. This suite is unchanged and still fails if the hook loses the
 * guarantee — which is exactly what it is for.
 *
 * **Asserting the final text would prove nothing**, because the bug is an
 * ABSENT change: the announcer reads correctly in both the broken and the fixed
 * version. So this suite watches the live region's text node with a
 * `MutationObserver` and counts transitions. That is the whole point — this
 * increment exists because a check that measures the wrong property returned a
 * confident wrong answer twice.
 *
 * **S1.** `useOutcomeNotes` prunes only via `begin(key)`, and nothing on the
 * remove/refetch path touches the hook, so a host's stale note outlives the
 * group that showed it and revives when a token is re-added for the same host.
 * `begin(host)` at the top of the global form's `onSuccess` — the moment the
 * host is (re)created — is the fix.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

import { ipc } from '../../ipc';
import type { ForgeAccount } from '../../ipc';
import { SettingsAccountsSection } from './SettingsAccountsSection';

const VIEWER = { login: 'octocat', avatarUrl: null };

const GH_ACCOUNT: ForgeAccount = {
  accountId: 'gitHub:github.com:octocat',
  host: 'github.com',
  kind: 'gitHub',
  login: 'octocat',
  avatarUrl: null,
  connected: true,
  isHostDefault: true,
};
/** A second account on the same host: the Default radiogroup only renders when
 *  the host has more than one, which is the only way to reach `setDefault`. */
const GH_ALT: ForgeAccount = {
  accountId: 'gitHub:github.com:danpercic86',
  host: 'github.com',
  kind: 'gitHub',
  login: 'danpercic86',
  avatarUrl: null,
  connected: true,
  isHostDefault: false,
};

/** Records every text the section's ONE live region has held, in order.
 *  `characterData` + `childList`, because React replaces the text NODE when the
 *  string goes empty and mutates it in place otherwise. */
function watchAnnouncer(): { texts: string[]; stop(): void } {
  const region = document.querySelector<HTMLElement>('[role="status"][aria-live="polite"]');
  expect(region, 'the section announcer must be mounted before the first outcome').not.toBeNull();
  if (region === null) throw new Error('unreachable');
  const texts: string[] = [region.textContent ?? ''];
  const push = (): void => {
    const next = region.textContent ?? '';
    if (next !== texts[texts.length - 1]) texts.push(next);
  };
  const observer = new MutationObserver(push);
  observer.observe(region, { characterData: true, childList: true, subtree: true });
  return { texts, stop: () => observer.disconnect() };
}

async function submitAdd(host: string, token: string): Promise<void> {
  fireEvent.click(await screen.findByRole('button', { name: 'Add a token for a host' }));
  fireEvent.change(screen.getByLabelText('Host'), { target: { value: host } });
  fireEvent.change(screen.getByLabelText('Personal access token'), { target: { value: token } });
  fireEvent.click(screen.getByRole('button', { name: 'Add account' }));
}

function outcomeNote(slot: string): HTMLElement | null {
  return document.querySelector<HTMLElement>(`[data-outcome-note="${slot}"]`);
}

describe('P113 S2 — the same outcome twice announces twice (AC15, rows 9/10)', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(ipc, 'openUrl').mockResolvedValue(undefined);
  });

  it('re-connecting the SAME login to the SAME host produces a second transition', async () => {
    vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([]);
    vi.spyOn(ipc, 'forgeAddAccount').mockResolvedValue(VIEWER);
    render(<SettingsAccountsSection />);
    await screen.findByRole('button', { name: 'Add a token for a host' });
    const watch = watchAnnouncer();

    const expected = 'Connected to gitlab.com as octocat.';
    await submitAdd('gitlab.com', 'glpat_valid');
    await waitFor(() => expect(outcomeNote('accounts.add')).toHaveTextContent(expected));

    // Identical host + identical login: byte-identical announcement text.
    await submitAdd('gitlab.com', 'glpat_valid');
    await waitFor(() => expect(outcomeNote('accounts.add')).toHaveTextContent(expected));
    watch.stop();

    // '' -> text -> '' -> text. Before the fix this was '' -> text and stopped:
    // the second `report` set the same string, React skipped it, and a screen
    // reader heard the outcome once for two operations.
    expect(watch.texts).toEqual(['', expected, '', expected]);
  });
});

describe('P113 S1 — a stale host note does not survive the host', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(ipc, 'openUrl').mockResolvedValue(undefined);
  });

  it('drops the previous note for a host when a token is re-added for it', async () => {
    const list = vi.spyOn(ipc, 'forgeListAccounts').mockResolvedValue([GH_ACCOUNT, GH_ALT]);
    vi.spyOn(ipc, 'forgeSetHostDefault').mockRejectedValue({
      kind: 'other',
      message: 'account is not on the given host',
    });
    vi.spyOn(ipc, 'forgeAddAccount').mockResolvedValue(VIEWER);
    render(<SettingsAccountsSection />);

    // 1. A failed setDefault leaves an error note keyed to `github.com`.
    await screen.findByText('danpercic86');
    fireEvent.click(screen.getAllByRole('radio', { name: 'Default' })[1]);
    await waitFor(() =>
      expect(outcomeNote('github.com')).toHaveTextContent(
        'Could not set the default account for github.com',
      ),
    );

    // 2. Adding a token for the SAME host is the moment the host is (re)created.
    //    In the user-visible sequence the accounts are removed in between and the
    //    group unmounts, but that is incidental: the remove path never touches
    //    the note hook, so the defect and its fix both live HERE — the key is
    //    either dropped at (re)creation or it is not. Keeping the group mounted
    //    throughout makes the note observable before and after, which is what
    //    lets this assert the clear rather than a remount.
    await submitAdd('github.com', 'ghp_valid');
    await waitFor(() =>
      expect(outcomeNote('accounts.add')).toHaveTextContent('Connected to github.com as octocat.'),
    );

    // Something HAS happened since the error, so §7's "a stale error is still
    // true, because nothing has happened" no longer holds: the note must be gone.
    expect(outcomeNote('github.com')).toHaveTextContent('');
    expect(list).toHaveBeenCalled();
  });
});
