/**
 * P69d — the Git-config section after the Advanced split.
 *
 * The hard constraint this suite exists for: the `configMissing` deep link
 * (App opens Settings with `configInitialFocus='identity'` after a commit fails)
 * scrolls to the Identity sub-section and focuses `user.name`. That effect must keep
 * firing after the split, which is only true while Identity stays in THIS file — so
 * the structural assertion "Identity is outside the collapsed Advanced <details>" is
 * part of the contract, not decoration (a control inside a closed <details> is not
 * focusable and the deep link would land nowhere).
 *
 * The rest covers the behaviour that moved into `settings/GitConfigAdvanced.tsx` and
 * `settings/CuratedConfigControl.tsx`: commit-on-blur, enum commit-on-change, remove,
 * and the add row with its client-side key-shape pre-check.
 */
import { describe, expect, it, vi, afterEach } from 'vitest';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';

import { SettingsGitConfigSection } from './SettingsGitConfigSection';
import { GitConfigScopeContext } from './settings/GitConfigScopeContext';
import { mockIpc } from '../ipc/mock';
import {
  resetEffectiveIdentityForTests,
  useEffectiveIdentity,
} from '../hooks/useEffectiveIdentity';
import type { ConfigLevelArg, ConfigView } from '../ipc';
import { useState, type ReactNode } from 'react';

const REPO = '/repo/a';

function makeView(overrides: Partial<ConfigView> = {}): ConfigView {
  return {
    targetLevel: 'local',
    curated: [
      {
        key: 'user.name',
        kind: 'text',
        enumValues: [],
        effectiveValue: 'Ada Local',
        effectiveLevel: 'local',
        targetValue: 'Ada Local',
      },
      {
        key: 'user.email',
        kind: 'text',
        enumValues: [],
        effectiveValue: 'ada@global.dev',
        effectiveLevel: 'global',
        targetValue: null,
      },
      {
        key: 'pull.ff',
        kind: 'enum',
        enumValues: ['true', 'false', 'only'],
        effectiveValue: 'only',
        effectiveLevel: 'local',
        targetValue: 'only',
      },
    ],
    advanced: [{ name: 'custom.thing', value: 'v1', level: 'local' }],
    ...overrides,
  };
}

afterEach(() => {
  vi.restoreAllMocks();
  resetEffectiveIdentityForTests();
});

/**
 * P69h — the Local | Global switch moved OUT of this component and into the pane
 * header (UI §1.1), reaching it through `GitConfigScopeContext`. This harness is
 * the switch's stand-in: the assertions below are about how the SECTION reacts to
 * a scope change, and `GitConfigScope.test.tsx` owns the control itself.
 */
function ScopeHarness({ children }: { children: ReactNode }) {
  const [level, setLevel] = useState<ConfigLevelArg>('local');
  return (
    <GitConfigScopeContext.Provider value={{ level, setLevel }}>
      <button type="button" onClick={() => setLevel('global')}>
        Global
      </button>
      <button type="button" onClick={() => setLevel('local')}>
        Repo scope
      </button>
      {children}
    </GitConfigScopeContext.Provider>
  );
}

/** A second consumer of the shared identity store, mounted alongside the pane. */
function IdentityProbe({ repoId }: { repoId: string }) {
  const identity = useEffectiveIdentity(repoId);
  return <span data-testid="probe">{identity.email ?? 'none'}</span>;
}

describe('SettingsGitConfigSection — the configMissing deep link', () => {
  it('focuses the user.name input once the config view resolves', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(makeView());
    render(<SettingsGitConfigSection repoId={REPO} initialFocus="identity" />);

    await waitFor(() =>
      expect(document.activeElement).toBe(document.getElementById('cfg-user.name')),
    );
  });

  it('does NOT steal focus back after the user moves on (focusedOnce guard)', async () => {
    // A FRESH view object per call: an identical reference would let React bail out of
    // the re-render and the effect would never get the chance to re-fire.
    const getConfig = vi.spyOn(mockIpc, 'getConfig').mockImplementation(() =>
      Promise.resolve(makeView()),
    );
    render(
      <ScopeHarness>
        <SettingsGitConfigSection repoId={REPO} initialFocus="identity" />
      </ScopeHarness>,
    );
    await waitFor(() =>
      expect(document.activeElement).toBe(document.getElementById('cfg-user.name')),
    );

    const email = document.getElementById('cfg-user.email') as HTMLInputElement;
    email.focus();
    // A level switch refetches and replaces `view`, which is what re-runs the focus
    // effect — the guard, not a stale dependency, is what keeps focus put.
    const before = getConfig.mock.calls.length;
    fireEvent.click(screen.getByRole('button', { name: 'Global' }));
    await waitFor(() => expect(getConfig.mock.calls.length).toBeGreaterThan(before));
    await waitFor(() => expect(document.getElementById('cfg-user.email')).not.toBeNull());
    expect(document.activeElement).toBe(email);
  });

  it('without the deep link nothing is focused', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(makeView());
    render(<SettingsGitConfigSection repoId={REPO} />);

    await waitFor(() => expect(document.getElementById('cfg-user.name')).not.toBeNull());
    expect(document.activeElement).toBe(document.body);
  });

  it('Identity stays OUTSIDE the collapsed Advanced group; Behaviour is inside it', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(makeView());
    const { container } = render(<SettingsGitConfigSection repoId={REPO} />);
    await waitFor(() => expect(document.getElementById('cfg-user.name')).not.toBeNull());

    const details = container.querySelector('details');
    expect(details).not.toBeNull();
    // Each control exists EXACTLY once — a duplicate id would make the deep link's
    // getElementById a coin toss.
    for (const id of ['cfg-user.name', 'cfg-user.email', 'cfg-pull.ff']) {
      expect(container.querySelectorAll(`[id="${id}"]`)).toHaveLength(1);
    }
    expect(details!.querySelector('[id="cfg-user.name"]')).toBeNull();
    expect(details!.querySelector('[id="cfg-user.email"]')).toBeNull();
    expect(details!.querySelector('[id="cfg-pull.ff"]')).not.toBeNull();
    expect(details!.querySelector('[id="cfg-adv-custom.thing"]')).not.toBeNull();
  });
});

describe('SettingsGitConfigSection — writes through the extracted controls', () => {
  it('a changed identity field commits on blur; an unchanged one does not', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(makeView());
    const setConfig = vi.spyOn(mockIpc, 'setConfig').mockResolvedValue();
    render(<SettingsGitConfigSection repoId={REPO} />);
    const name = (await screen.findByLabelText('user.name')) as HTMLInputElement;

    fireEvent.blur(name);
    expect(setConfig).not.toHaveBeenCalled();

    fireEvent.change(name, { target: { value: 'Local Ada' } });
    fireEvent.blur(name);
    await waitFor(() =>
      expect(setConfig).toHaveBeenCalledWith(REPO, 'local', 'user.name', 'Local Ada'),
    );
  });

  it('clearing a set key unsets it instead of writing an empty value', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(makeView());
    const unsetConfig = vi.spyOn(mockIpc, 'unsetConfig').mockResolvedValue();
    render(<SettingsGitConfigSection repoId={REPO} />);
    const ff = (await screen.findByLabelText('pull.ff')) as HTMLSelectElement;

    fireEvent.change(ff, { target: { value: '' } });
    await waitFor(() => expect(unsetConfig).toHaveBeenCalledWith(REPO, 'local', 'pull.ff'));
  });

  it('Remove unsets a custom key', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(makeView());
    const unsetConfig = vi.spyOn(mockIpc, 'unsetConfig').mockResolvedValue();
    render(<SettingsGitConfigSection repoId={REPO} />);
    await screen.findByText('custom.thing');

    fireEvent.click(screen.getByRole('button', { name: 'Remove' }));
    await waitFor(() => expect(unsetConfig).toHaveBeenCalledWith(REPO, 'local', 'custom.thing'));
  });

  it('the add row rejects a malformed key client-side and never calls setConfig', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(makeView());
    const setConfig = vi.spyOn(mockIpc, 'setConfig').mockResolvedValue();
    render(<SettingsGitConfigSection repoId={REPO} />);
    await screen.findByPlaceholderText('section.key');

    fireEvent.change(screen.getByPlaceholderText('section.key'), { target: { value: 'nodot' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add entry' }));

    expect(await screen.findByText('config key must be section.key')).toBeInTheDocument();
    expect(setConfig).not.toHaveBeenCalled();
  });

  it('the add row writes a trimmed section.key = value and refetches', async () => {
    const getConfig = vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(makeView());
    const setConfig = vi.spyOn(mockIpc, 'setConfig').mockResolvedValue();
    render(<SettingsGitConfigSection repoId={REPO} />);
    await screen.findByPlaceholderText('section.key');
    // The section's own load plus SettingsHooksToggle's read — the delta is what matters.
    const before = getConfig.mock.calls.length;

    fireEvent.change(screen.getByPlaceholderText('section.key'), {
      target: { value: '  core.pager  ' },
    });
    fireEvent.change(screen.getByPlaceholderText('value'), { target: { value: ' less ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add entry' }));

    await waitFor(() =>
      expect(setConfig).toHaveBeenCalledWith(REPO, 'local', 'core.pager', 'less'),
    );
    await waitFor(() => expect(getConfig.mock.calls.length).toBe(before + 1));
  });

  it('a failed load shows the error instead of an empty form', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockRejectedValue(new Error('config unreadable'));
    render(<SettingsGitConfigSection repoId={REPO} />);

    expect(await screen.findByRole('note')).toHaveTextContent('config unreadable');
    expect(document.getElementById('cfg-user.name')).toBeNull();
  });
});

describe('SettingsGitConfigSection — identity writes invalidate the shared store', () => {
  /** Editing user.email here must correct every identity surface. Before P69d the
   *  profiles section refetched on remount, so closing and reopening Settings healed a
   *  stale pill; with a module-level cache nothing would heal it, so the write has to
   *  say so explicitly (§5.1's exhaustive trigger list). */
  it('a user.* write refetches the effective identity for other consumers', async () => {
    let current = 'old@x.dev';
    const getConfig = vi.spyOn(mockIpc, 'getConfig').mockImplementation(() => {
      const v = makeView();
      v.curated[1].effectiveValue = current;
      v.curated[1].targetValue = current;
      return Promise.resolve(v);
    });
    vi.spyOn(mockIpc, 'setConfig').mockImplementation(() => {
      current = 'new@x.dev';
      return Promise.resolve();
    });
    render(
      <>
        <SettingsGitConfigSection repoId={REPO} />
        <IdentityProbe repoId={REPO} />
      </>,
    );
    await waitFor(() => expect(screen.getByTestId('probe')).toHaveTextContent('old@x.dev'));
    const reads = getConfig.mock.calls.length;

    const email = (await screen.findByLabelText('user.email')) as HTMLInputElement;
    fireEvent.change(email, { target: { value: 'new@x.dev' } });
    fireEvent.blur(email);

    await waitFor(() => expect(screen.getByTestId('probe')).toHaveTextContent('new@x.dev'));
    expect(getConfig.mock.calls.length).toBeGreaterThan(reads);
  });

  it('a NON-identity write does not invalidate the identity store', async () => {
    const getConfig = vi.spyOn(mockIpc, 'getConfig').mockImplementation(() =>
      Promise.resolve(makeView()),
    );
    const setConfig = vi.spyOn(mockIpc, 'setConfig').mockResolvedValue();
    render(
      <>
        <SettingsGitConfigSection repoId={REPO} />
        <IdentityProbe repoId={REPO} />
      </>,
    );
    await waitFor(() => expect(screen.getByTestId('probe')).toHaveTextContent('ada@global.dev'));

    const before = getConfig.mock.calls.length;

    const custom = document.getElementById('cfg-adv-custom.thing') as HTMLInputElement;
    fireEvent.change(custom, { target: { value: 'v2' } });
    fireEvent.blur(custom);
    await waitFor(() => expect(setConfig).toHaveBeenCalled());
    // The pane's own post-write refetch, and NOTHING else: an identity invalidation
    // would add a second read here.
    await waitFor(() => expect(getConfig.mock.calls.length).toBe(before + 1));
    await act(async () => {});
    await act(async () => {});
    expect(getConfig.mock.calls.length).toBe(before + 1);
  });
});

describe('SettingsGitConfigSection — one read serves the whole pane (P69h)', () => {
  /** The measurement this increment exists for: the pane used to cost THREE
   *  `getConfig(repoId,'local')` round-trips for one answer — its own view,
   *  `SettingsHooksToggle`'s private read of `bonsai.runHooks`, and
   *  `useEffectiveIdentity`'s read of `user.*`. All three come out of the same
   *  `ConfigView`, so the count is now ONE and this test is what holds it there. */
  it('reads getConfig exactly ONCE for the form, the hooks row and the identity store', async () => {
    const getConfig = vi.spyOn(mockIpc, 'getConfig').mockImplementation(() =>
      Promise.resolve(
        makeView({
          advanced: [
            { name: 'custom.thing', value: 'v1', level: 'local' },
            { name: 'bonsai.runHooks', value: 'false', level: 'local' },
          ],
        }),
      ),
    );
    render(
      <>
        <SettingsGitConfigSection repoId={REPO} />
        <IdentityProbe repoId={REPO} />
      </>,
    );

    // All three consumers are satisfied…
    await waitFor(() => expect(screen.getByTestId('probe')).toHaveTextContent('ada@global.dev'));
    expect(await screen.findByLabelText('user.name')).toBeInTheDocument();
    expect(
      screen.getByRole('checkbox', { name: 'Run git hooks for this repository' }),
    ).not.toBeChecked();
    // …by one read.
    await act(async () => {});
    expect(getConfig).toHaveBeenCalledTimes(1);
    expect(getConfig).toHaveBeenCalledWith(REPO, 'local');
  });

  it('the hooks switch writes bonsai.runHooks at the LOCAL level and refetches', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockImplementation(() => Promise.resolve(makeView()));
    const setConfig = vi.spyOn(mockIpc, 'setConfig').mockResolvedValue();
    render(<SettingsGitConfigSection repoId={REPO} />);

    const box = await screen.findByRole('checkbox', {
      name: 'Run git hooks for this repository',
    });
    expect(box).toBeChecked(); // unset ⇒ on (git's default)
    fireEvent.click(box);

    await waitFor(() =>
      expect(setConfig).toHaveBeenCalledWith(REPO, 'local', 'bonsai.runHooks', 'false'),
    );
  });

  it('an identity write at GLOBAL scope still invalidates the shared store', async () => {
    // At local scope the pane's own refetch re-primes the store, so no extra read
    // is needed. At global scope this view is NOT the identity's view, so the
    // store has to be told explicitly — otherwise every identity surface keeps
    // showing the pre-write value.
    let email = 'old@x.dev';
    const getConfig = vi.spyOn(mockIpc, 'getConfig').mockImplementation(() => {
      const v = makeView();
      v.curated[1].effectiveValue = email;
      v.curated[1].targetValue = email;
      return Promise.resolve(v);
    });
    vi.spyOn(mockIpc, 'setConfig').mockImplementation(() => {
      email = 'new@x.dev';
      return Promise.resolve();
    });
    render(
      <ScopeHarness>
        <SettingsGitConfigSection repoId={REPO} />
        <IdentityProbe repoId={REPO} />
      </ScopeHarness>,
    );
    await waitFor(() => expect(screen.getByTestId('probe')).toHaveTextContent('old@x.dev'));
    fireEvent.click(screen.getByRole('button', { name: 'Global' }));
    await waitFor(() => expect(getConfig).toHaveBeenCalledWith(REPO, 'global'));

    const field = (await screen.findByLabelText('user.email')) as HTMLInputElement;
    fireEvent.change(field, { target: { value: 'new@x.dev' } });
    fireEvent.blur(field);

    await waitFor(() => expect(screen.getByTestId('probe')).toHaveTextContent('new@x.dev'));
  });
});

describe('SettingsGitConfigSection — the Hooks row tells the truth (P69h)', () => {
  /** A deferred `getConfig` per call, so read ORDER is controllable. */
  function deferredConfig() {
    const pending: { level: string; resolve: (v: ConfigView) => void }[] = [];
    vi.spyOn(mockIpc, 'getConfig').mockImplementation(
      (_repo: string, level: string) =>
        new Promise<ConfigView>((resolve) => {
          pending.push({ level, resolve });
        }),
    );
    return pending;
  }

  const hooksBox = () => screen.getByRole('checkbox', { name: 'Run git hooks for this repository' });
  const runHooksView = (value: string) =>
    makeView({ advanced: [{ name: 'bonsai.runHooks', value, level: 'local' }] });

  it('is INERT until the local config has actually been read', async () => {
    // The bug this pins: rendering a confident ON over an unread config, which a
    // click would then write as a real `true` over the repo's real `false`.
    const pending = deferredConfig();
    render(<SettingsGitConfigSection repoId={REPO} />);

    expect(hooksBox()).toBeDisabled();
    await act(async () => {
      pending[0].resolve(runHooksView('false'));
    });
    await waitFor(() => expect(hooksBox()).toBeEnabled());
    expect(hooksBox()).not.toBeChecked();
  });

  it('stays inert and says why when the read FAILS', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockRejectedValue(new Error('config unreadable'));
    render(<SettingsGitConfigSection repoId={REPO} />);

    const hooks = await screen.findByRole('region', { name: 'Hooks' });
    await waitFor(() => expect(within(hooks).getByText('config unreadable')).toBeInTheDocument());
    expect(hooksBox()).toBeDisabled();
  });

  it('does not REVERT when an older local read resolves out of order', async () => {
    // local#1 → global#2 → local#3. #3 is the freshest local answer; #1 landing
    // late must not overwrite it (the Hooks row would silently flip back).
    const pending = deferredConfig();
    render(
      <ScopeHarness>
        <SettingsGitConfigSection repoId={REPO} />
      </ScopeHarness>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Global' }));
    fireEvent.click(screen.getByRole('button', { name: 'Repo scope' }));
    await waitFor(() => expect(pending.length).toBe(3));
    expect(pending.map((p) => p.level)).toEqual(['local', 'global', 'local']);

    await act(async () => {
      pending[2].resolve(runHooksView('false'));
    });
    await waitFor(() => expect(hooksBox()).not.toBeChecked());

    await act(async () => {
      pending[0].resolve(runHooksView('true')); // the stale one
    });
    expect(hooksBox()).not.toBeChecked();
  });
});
