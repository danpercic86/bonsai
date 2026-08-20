/**
 * P69k — settings search (UI §3).
 *
 * The load-bearing claim is §3.1's: a result is the REAL row, live and editable
 * in place, not a link to it. Several tests below therefore edit a control from
 * inside the result list and assert the change reaches the panel's `onChange` —
 * that is the property a jump-and-highlight implementation could not satisfy.
 */
import { describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, within } from '@testing-library/react';

import { SettingsPanel } from '../SettingsPanel';
import type { SettingsPanelProps } from '../SettingsPanel';
import { FIXTURE_CONFIG_VIEW, FIXTURE_PROFILES, MAXIMAL, MINIMAL } from './coverageFixtures';
import { mockIpc } from '../../ipc/mock';

function renderPanel(over: Partial<SettingsPanelProps> = {}) {
  const props: SettingsPanelProps = {
    open: true,
    onClose: vi.fn(),
    requestSeq: 0,
    onChange: vi.fn(),
    onToggleTheme: vi.fn(),
    onToggleListView: vi.fn(),
    onRequestEnableAi: vi.fn(),
    onSetMcpEnabled: vi.fn(),
    onRequestEnableMcp: vi.fn(),
    onSetMcpAllowWrite: vi.fn(),
    onRequestEnableMcpWrite: vi.fn(),
    onRegisterMcp: vi.fn(async () => {}),
    onShowOnboarding: vi.fn(),
    onOpenRepository: vi.fn(),
    onCheckUpdate: vi.fn(),
    onOpenUpdateDialog: vi.fn(),
    ...MINIMAL,
    ...over,
  };
  return { ...render(<SettingsPanel {...props} />), props };
}

const box = (): HTMLElement => screen.getByRole('searchbox', { name: 'Search settings' });
const type = (value: string): void => {
  fireEvent.change(box(), { target: { value } });
};
const tab = (name: string) => screen.getByRole('tab', { name });
const railItem = (id: string): HTMLElement => {
  const el = document.getElementById(`settings-tab-${id}`);
  if (el === null) throw new Error(`no rail item ${id}`);
  return el;
};

async function settle(): Promise<void> {
  await act(async () => {
    await new Promise((resolve) => {
      setTimeout(resolve, 0);
    });
  });
}

describe('settings search — cross-category results (§3.1)', () => {
  it('replaces the category pane with matches from EVERY category, grouped', () => {
    renderPanel();
    // "density" lives in Appearance (Panel density) and Commit graph (row
    // height / compact rows, via keywords) — the whole point of D3.
    type('density');

    const results = document.querySelectorAll('.settings-results-title');
    expect([...results].map((r) => r.textContent)).toEqual(['Appearance', 'Commit graph']);

    // Real controls, not links.
    expect(screen.getByRole('radiogroup', { name: 'Panel density' })).toBeInTheDocument();
    expect(screen.getByRole('spinbutton', { name: 'Row height' })).toBeInTheDocument();
    // A non-matching row of a matching category is gone.
    expect(screen.queryByRole('radiogroup', { name: 'Theme' })).toBeNull();
    // …and so is the category pane header it replaced.
    expect(document.querySelector('.settings-pane-header')).toBeNull();
  });

  it('a result row is editable in place', () => {
    const { props } = renderPanel();
    type('density');
    fireEvent.click(screen.getByRole('radio', { name: 'Compact' }));
    expect(props.onChange).toHaveBeenCalledWith({ panelDensity: 'compact' });
  });

  it('drops group headers whose rows all filtered out', () => {
    renderPanel();
    type('density');
    const groups = [...document.querySelectorAll('.settings-group-title')].map((g) => g.textContent);
    expect(groups).toContain('Geometry');
    expect(groups).not.toContain('Row details');
    expect(groups).not.toContain('Badges');
  });

  it('reaches the one catalogued row that lives in the pane header', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(FIXTURE_CONFIG_VIEW);
    renderPanel({ ...MAXIMAL });
    type('scope');
    await settle();
    expect(screen.getByRole('radiogroup', { name: 'Scope' })).toBeInTheDocument();
  });
});

/**
 * P69k review A9 — one negative control per stamper.
 *
 * FOUR components stamp `data-setting-id` (see `SettingsSearchContext.ts`), and
 * each one's self-filter is a production line that must turn a test red when it
 * is reverted. Before A9 only `SettingsRow`'s was covered: the identity action
 * cells were never searched at all, and the scope switch was only asserted to
 * APPEAR. Every test below fails if its `useSettingsRowVisible` /
 * `useSettingsGroupVisible` guard is deleted.
 */
describe('settings search — every stamped row self-filters (§3.1)', () => {
  const stamped = (id: string): NodeListOf<Element> =>
    document.querySelectorAll(`[data-setting-id="${id}"]`);

  it('drops the identity action cells when the hit is another row of the card', () => {
    renderPanel({ ...MAXIMAL });
    // "nickname" is a keyword of `identities.profile-label` and nothing else.
    type('nickname');
    expect(stamped('identities.profile-label')).toHaveLength(FIXTURE_PROFILES.length);
    expect(stamped('identities.apply')).toHaveLength(0);
    expect(stamped('identities.delete')).toHaveLength(0);
  });

  it('keeps an identity action cell when IT is the hit', () => {
    renderPanel({ ...MAXIMAL });
    // "confirm" is a keyword of `identities.delete` and nothing else.
    type('confirm');
    expect(stamped('identities.delete')).toHaveLength(FIXTURE_PROFILES.length);
    expect(stamped('identities.profile-label')).toHaveLength(0);
    expect(stamped('identities.apply')).toHaveLength(0);
  });

  it('drops the pane-header scope switch when git-config was hit for another reason', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(FIXTURE_CONFIG_VIEW);
    renderPanel({ ...MAXIMAL });
    // "husky" is a keyword of `git-config.run-hooks` and nothing else.
    type('husky');
    await settle();
    expect(
      screen.getByRole('checkbox', { name: 'Run git hooks for this repository' }),
    ).toBeInTheDocument();
    expect(screen.queryByRole('radiogroup', { name: 'Scope' })).toBeNull();
    expect(stamped('git-config.scope')).toHaveLength(0);
  });

  it('renders only the Advanced block that matched, with the disclosure forced open', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(FIXTURE_CONFIG_VIEW);
    renderPanel({ ...MAXIMAL });
    // "autocrlf" is in `git-config.behaviour`'s help and nothing else.
    type('autocrlf');
    await settle();
    expect(stamped('git-config.behaviour')).toHaveLength(1);
    expect(stamped('git-config.custom-keys')).toHaveLength(0);
    // …so the add-entry form, which lives in the Custom keys block, went too.
    expect(screen.queryByRole('button', { name: 'Add entry' })).toBeNull();
    // A hit behind a collapsed disclosure is an invisible result.
    const details = document.querySelector('details.settings-config-advanced-details');
    expect(details).toBeInstanceOf(HTMLDetailsElement);
    expect((details as HTMLDetailsElement).open).toBe(true);
  });

  it('drops the Behaviour block when only the Custom keys block matched', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(FIXTURE_CONFIG_VIEW);
    renderPanel({ ...MAXIMAL });
    // "section.key" is in `git-config.custom-keys`' help and nothing else — the
    // mirror image of the test above, so BOTH block guards have a control.
    type('section.key');
    await settle();
    expect(stamped('git-config.custom-keys')).toHaveLength(1);
    expect(stamped('git-config.behaviour')).toHaveLength(0);
    expect(screen.getByRole('button', { name: 'Add entry' })).toBeInTheDocument();
  });

  it('drops the whole Advanced disclosure when neither of its blocks matched', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(FIXTURE_CONFIG_VIEW);
    renderPanel({ ...MAXIMAL });
    type('husky');
    await settle();
    expect(document.querySelector('details.settings-config-advanced-details')).toBeNull();
    expect(stamped('git-config.behaviour')).toHaveLength(0);
    expect(stamped('git-config.custom-keys')).toHaveLength(0);
  });

  it('renders nothing from the AI-run fieldset when none of its groups was hit', () => {
    renderPanel(); // MINIMAL: AI off, MCP stopped.
    // "mcp" hits the AI access group only; the run fieldset spans Runs, Limits
    // and Bulk resolve, so it must not render an empty bordered box.
    type('mcp');
    expect(screen.getByRole('checkbox', { name: 'Enable MCP server' })).toBeInTheDocument();
    expect(document.querySelector('.settings-fieldset')).toBeNull();
    expect(document.getElementById('ai-run-gate-note')).toBeNull();
  });
});

describe('settings search — unavailable rows are not matches (review A3)', () => {
  it('never counts a row the pane cannot render', () => {
    renderPanel(); // MINIMAL: no repo, MCP stopped, no profiles.
    // "bearer" is only in `ai.mcp-token`, which requires a running server.
    type('bearer');
    expect(screen.getByRole('status')).toHaveTextContent('No settings match');
    expect(document.querySelector('.settings-empty')).not.toBeNull();
    expect(railItem('ai').querySelector('.settings-rail-count')?.textContent).toBe('0');
  });

  it('counts it once the requirement holds', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(FIXTURE_CONFIG_VIEW);
    renderPanel({ ...MAXIMAL });
    type('bearer');
    await settle();
    expect(screen.getByRole('status')).toHaveTextContent('1 setting matches');
    expect(screen.getByRole('button', { name: 'Copy bearer token' })).toBeInTheDocument();
  });
});

describe('settings search — no idref may dangle (ui-reference §12.3.3)', () => {
  // "most" appears only in the spend-limit row's help, so the SWITCH row that
  // owns the stateful note is filtered out while the slider survives.
  it('drops a borrowed hint idref when the row that owns the note is gone', () => {
    renderPanel();
    type('most');
    expect(document.querySelector('[data-setting-id="ai.budget-enabled"]')).toBeNull();
    const slider = screen.getByRole('spinbutton', { name: 'Spend limit' });
    expect(slider.getAttribute('aria-describedby')).toBe('ai.budget-usd-help');
    expect(document.getElementById('ai.budget-usd-help')).not.toBeNull();
  });

  it('keeps the AI gate note with the fieldset when the Runs group filters out', () => {
    renderPanel(); // MINIMAL: AI off, so the whole run fieldset is inert.
    type('most');
    const fieldset = document.querySelector('.settings-fieldset');
    expect(fieldset).toHaveAttribute('aria-describedby', 'ai-run-gate-note');
    const note = document.getElementById('ai-run-gate-note');
    expect(note?.textContent).toBe('Turn on “Enable AI features” above to change these.');
    expect(document.querySelectorAll('#ai-run-gate-note')).toHaveLength(1);
  });
});

describe('settings search — matching (§3.2)', () => {
  it('ANDs whitespace-split terms, case-insensitively', () => {
    renderPanel();
    type('LANE width');
    expect(screen.getByRole('spinbutton', { name: 'Lane width' })).toBeInTheDocument();
    expect(screen.queryByRole('spinbutton', { name: 'Row height' })).toBeNull();
  });

  it('matches never-displayed keywords', () => {
    renderPanel();
    // "spend" is the label word; "budget" only exists in `keywords`.
    type('budget');
    expect(screen.getByRole('checkbox', { name: 'Set a spend limit per run' })).toBeInTheDocument();
  });
});

describe('settings search — highlighting (§3.2)', () => {
  it('marks matched substrings in the label and nothing in the help', () => {
    renderPanel();
    type('row height');
    const row = document.querySelector('[data-setting-id="graph.row-height"]');
    expect(row).not.toBeNull();
    const marks = [...(row?.querySelectorAll('mark.settings-match') ?? [])];
    expect(marks.map((m) => m.textContent)).toEqual(['Row', 'height']);
    expect(row?.querySelector('.settings-row-help mark')).toBeNull();
  });

  it('leaves a keyword-only match unmarked rather than inventing a hit', () => {
    renderPanel();
    type('budget');
    const row = document.querySelector('[data-setting-id="ai.budget-enabled"]');
    expect(row?.querySelectorAll('mark.settings-match')).toHaveLength(0);
  });
});

describe('settings search — group headers (§3.2)', () => {
  it('“Go to {Category}” clears the query and selects that category', () => {
    renderPanel();
    type('density');
    fireEvent.click(screen.getByRole('button', { name: 'Go to Commit graph' }));
    expect(box()).toHaveValue('');
    expect(tab('Commit graph')).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('heading', { name: 'Commit graph', level: 3 })).toBeInTheDocument();
  });
});

describe('settings search — the rail (§3.2)', () => {
  it('shows per-category counts and emphasises the ones with hits, disabling none', () => {
    renderPanel();
    type('density');
    expect(railItem('appearance').querySelector('.settings-rail-count')?.textContent).toBe('1');
    expect(railItem('graph').querySelector('.settings-rail-count')?.textContent).toBe('2');
    expect(railItem('about').querySelector('.settings-rail-count')?.textContent).toBe('0');
    // Inverted emphasis, not a dim: the zero-count item keeps the resting colour
    // (a dimmed clickable item was under 4.5:1 in both themes).
    expect(railItem('appearance').className).toContain('is-hit');
    expect(railItem('about').className).not.toContain('is-hit');
    expect(railItem('about')).not.toBeDisabled();
    // The count is in the accessible NAME too, so it is not a sighted-only cue.
    expect(railItem('about')).toHaveAttribute('aria-label', 'About, 0 matches');
    expect(railItem('appearance')).toHaveAttribute('aria-label', 'Appearance, 1 match');
  });

  it('never emphasises anything while the query is empty', () => {
    renderPanel();
    expect(document.querySelectorAll('.settings-rail-item.is-hit')).toHaveLength(0);
  });

  it('carries no counts at all when the query is empty', () => {
    renderPanel();
    expect(document.querySelectorAll('.settings-rail-count')).toHaveLength(0);
    expect(railItem('about')).not.toHaveAttribute('aria-label');
  });

  it('clicking any rail item — even a zero-count one — clears the query', () => {
    renderPanel();
    type('density');
    fireEvent.click(railItem('about'));
    expect(box()).toHaveValue('');
    expect(tab('About')).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('heading', { name: 'About', level: 3 })).toBeInTheDocument();
  });
});

describe('settings search — the live region (§3.2)', () => {
  it('announces the count, and the zero case, politely', () => {
    renderPanel();
    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-live', 'polite');
    expect(status).toHaveTextContent('');

    type('density');
    expect(status).toHaveTextContent('3 settings match');
    type('zzzz');
    expect(status).toHaveTextContent('No settings match');
  });

  it('agrees with English in the singular', () => {
    renderPanel();
    // "colour" is the British spelling kept as a keyword on exactly one row.
    type('colour');
    expect(screen.getByRole('status')).toHaveTextContent('1 setting matches');
  });
});

describe('settings search — zero match (§3.3)', () => {
  it('shows the empty block, quoting the query, with a Clear search action', () => {
    renderPanel();
    type('zzzz');
    const empty = document.querySelector('.settings-empty');
    expect(empty).not.toBeNull();
    expect(empty?.querySelector('.settings-empty-title')?.textContent).toBe(
      'No settings match “zzzz”.',
    );
    expect(empty?.querySelector('.settings-empty-body')?.textContent).toBe(
      'Try a shorter word — for example graph, fetch, identity, or spend.',
    );

    fireEvent.click(within(empty as HTMLElement).getByRole('button', { name: 'Clear search' }));
    expect(box()).toHaveValue('');
    expect(screen.getByRole('heading', { name: 'General', level: 3 })).toBeInTheDocument();
  });
});

describe('settings search — focus and Escape (§7.2)', () => {
  it('takes initial focus, because a text field cannot be activated by accident', () => {
    renderPanel();
    expect(document.activeElement).toBe(box());
  });

  it('does NOT take focus from the configMissing deep link', () => {
    renderPanel({ configInitialFocus: 'identity' });
    expect(document.activeElement).not.toBe(box());
    expect(tab('Git config, repository')).toHaveAttribute('aria-selected', 'true');
  });

  it('a result block never steals focus into the identity field (review A2)', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(FIXTURE_CONFIG_VIEW);
    renderPanel({ ...MAXIMAL, configInitialFocus: 'identity' });
    await settle();
    const field = (): HTMLElement | null => document.getElementById('cfg-user.name');
    // The deep link itself still works — that is the commit-error linkage.
    expect(document.activeElement).toBe(field());

    // "whoami" is a keyword of `git-config.user-name`, so the field is present in
    // the result list. It must NOT be focused: the section's focus effect re-arms
    // on every fresh mount, and the field commits on blur, so a steal here would
    // write the rest of the query into the repository's real `user.name`.
    type('whoami');
    await settle();
    expect(field()).not.toBeNull();
    expect(document.activeElement).not.toBe(field());
  });

  it('Escape clears a non-empty query and leaves an empty one to the dialog', () => {
    renderPanel();
    type('density');
    fireEvent.focus(box());
    // `fireEvent` returns false when the event was cancelled. THAT is what
    // distinguishes the two presses: the first is swallowed by the box, the
    // second must travel on to App's overlay-Esc layering (which this panel
    // never sees, so asserting on `onClose` could not fail either way).
    expect(fireEvent.keyDown(window, { key: 'Escape' })).toBe(false);
    expect(box()).toHaveValue('');

    fireEvent.focus(box());
    expect(fireEvent.keyDown(window, { key: 'Escape' })).toBe(true);
  });
});

describe('settings search — deep links beat a live query', () => {
  it('a deep link arriving mid-search clears the query and lands on its category', () => {
    const { rerender, props } = renderPanel();
    type('density');
    rerender(
      <SettingsPanel {...props} initialCategory="about" requestSeq={props.requestSeq + 1} />,
    );
    expect(box()).toHaveValue('');
    expect(tab('About')).toHaveAttribute('aria-selected', 'true');
  });
});
