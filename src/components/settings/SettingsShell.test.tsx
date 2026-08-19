/**
 * P69g — the two-pane shell: tab semantics, keyboard, seeding, close paths.
 *
 * The two rules worth pinning hardest:
 *   * activation is MANUAL (shell contract D-5) — arrowing across `Git config`
 *     must not select it, or every keypress would fire a `getConfig`;
 *   * the category is seeded on every OPEN, because `SettingsPanel` unmounts the
 *     shell while closed. That is what makes a second deep link in one session
 *     land correctly with no request counter.
 */
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { SettingsPanel } from '../SettingsPanel';
import type { SettingsPanelProps } from '../SettingsPanel';
import { MINIMAL } from './coverageFixtures';

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

const tab = (name: string) => screen.getByRole('tab', { name });

describe('SettingsShell — roles and structure', () => {
  it('keeps the frozen dialog name and adds aria-modal', () => {
    renderPanel();
    const dialog = screen.getByRole('dialog', { name: 'Settings' });
    expect(dialog).toHaveAttribute('aria-modal', 'true');
    expect(dialog).toHaveAttribute('aria-labelledby', 'settings-title');
  });

  it('is a vertical tablist of seven tabs wired to one tabpanel', () => {
    renderPanel();
    const list = screen.getByRole('tablist', { name: 'Settings categories' });
    expect(list).toHaveAttribute('aria-orientation', 'vertical');
    const tabs = screen.getAllByRole('tab');
    expect(tabs).toHaveLength(7);
    for (const t of tabs) expect(t).toHaveAttribute('aria-controls', 'settings-pane');
    const pane = screen.getByRole('tabpanel');
    expect(pane).toHaveAttribute('id', 'settings-pane');
    expect(pane).toHaveAttribute('aria-labelledby', 'settings-tab-general');
    expect(pane).toHaveAttribute('tabindex', '-1');
  });

  it('folds the repo pill into the git-config tab NAME, not just its colour', () => {
    renderPanel();
    expect(tab('Git config, repository')).toBeInTheDocument();
  });

  it('roving tabindex: exactly one rail tab stop, on the selected item', () => {
    renderPanel();
    const stops = screen.getAllByRole('tab').filter((t) => t.getAttribute('tabindex') === '0');
    expect(stops).toHaveLength(1);
    expect(stops[0]).toBe(tab('General'));
  });

  // APG: the stop follows FOCUS, not selection — otherwise arrowing to a category
  // without activating it, then tabbing out and back, silently loses the position.
  it('the rail tab stop follows focus, not selection', () => {
    renderPanel();
    tab('General').focus();
    fireEvent.keyDown(tab('General'), { key: 'ArrowDown' });
    expect(document.activeElement).toBe(tab('Appearance'));
    expect(tab('General')).toHaveAttribute('aria-selected', 'true'); // unchanged
    const stops = screen.getAllByRole('tab').filter((t) => t.getAttribute('tabindex') === '0');
    expect(stops).toEqual([tab('Appearance')]);
  });
});

describe('SettingsShell — selection', () => {
  it('a rail click swaps the pane and moves aria-selected', () => {
    renderPanel();
    expect(screen.getByRole('heading', { name: 'General', level: 3 })).toBeInTheDocument();
    fireEvent.click(tab('Appearance'));
    expect(tab('Appearance')).toHaveAttribute('aria-selected', 'true');
    expect(tab('General')).toHaveAttribute('aria-selected', 'false');
    expect(screen.getByRole('radio', { name: 'Dark' })).toBeInTheDocument();
    expect(screen.getByRole('tabpanel')).toHaveAttribute(
      'aria-labelledby',
      'settings-tab-appearance',
    );
  });

  it('arrows MOVE focus without activating (D-5), Enter selects', () => {
    renderPanel();
    tab('General').focus();
    fireEvent.keyDown(tab('General'), { key: 'ArrowDown' });
    expect(document.activeElement).toBe(tab('Appearance'));
    // The crux: focus moved, selection did NOT.
    expect(tab('General')).toHaveAttribute('aria-selected', 'true');
    expect(tab('Appearance')).toHaveAttribute('aria-selected', 'false');

    fireEvent.click(tab('Appearance')); // Enter/Space on a <button> IS a click
    expect(tab('Appearance')).toHaveAttribute('aria-selected', 'true');
  });

  it('arrow navigation wraps, and Home/End jump to the ends', () => {
    renderPanel();
    tab('General').focus();
    fireEvent.keyDown(tab('General'), { key: 'ArrowUp' });
    expect(document.activeElement).toBe(tab('About'));
    fireEvent.keyDown(tab('About'), { key: 'ArrowDown' });
    expect(document.activeElement).toBe(tab('General'));
    fireEvent.keyDown(tab('General'), { key: 'End' });
    expect(document.activeElement).toBe(tab('About'));
    fireEvent.keyDown(tab('About'), { key: 'Home' });
    expect(document.activeElement).toBe(tab('General'));
  });

  it('→ hands focus to the pane', () => {
    renderPanel();
    tab('General').focus();
    fireEvent.keyDown(tab('General'), { key: 'ArrowRight' });
    expect(document.activeElement).toBe(screen.getByRole('tabpanel'));
  });
});

describe('SettingsShell — seeding and close', () => {
  it('seeds from initialCategory', () => {
    renderPanel({ initialCategory: 'about' });
    expect(tab('About')).toHaveAttribute('aria-selected', 'true');
  });

  it('a configMissing deep link lands on Git config with no App-side plumbing', () => {
    // `configInitialFocus='identity'` is the only signal App sends today; deriving
    // the category from it here is what keeps App.tsx at its size ratchet.
    renderPanel({ configInitialFocus: 'identity' });
    expect(tab('Git config, repository')).toHaveAttribute('aria-selected', 'true');
  });

  it('re-seeds on every OPEN, so a second deep link still lands', () => {
    const { rerender, props } = renderPanel();
    fireEvent.click(tab('About'));
    // Close, then re-open deep-linked. The shell unmounts in between.
    rerender(<SettingsPanel {...props} open={false} />);
    rerender(<SettingsPanel {...props} open configInitialFocus="identity" />);
    expect(tab('Git config, repository')).toHaveAttribute('aria-selected', 'true');
  });

  /**
   * P69h §5.4 — the case a fresh mount cannot cover. A commit fails while
   * Settings is already open on another category: `requestSeq` changes, nothing
   * unmounts, and the pane must still move to Git config.
   */
  it('a deep link that arrives while Settings is ALREADY open moves the category', () => {
    const { rerender, props } = renderPanel();
    fireEvent.click(tab('About'));
    expect(tab('About')).toHaveAttribute('aria-selected', 'true');

    rerender(
      <SettingsPanel {...props} initialCategory="git-config" requestSeq={props.requestSeq + 1} />,
    );
    expect(tab('Git config, repository')).toHaveAttribute('aria-selected', 'true');
  });

  it('an open request that names NO category leaves the user where they were', () => {
    // The plain ⚙ click bumps the seq too (it must, or the next deep link would
    // be indistinguishable from a repeat) — but it must not yank the pane.
    const { rerender, props } = renderPanel();
    fireEvent.click(tab('About'));
    rerender(<SettingsPanel {...props} requestSeq={props.requestSeq + 1} />);
    expect(tab('About')).toHaveAttribute('aria-selected', 'true');
  });

  it('✕ and backdrop close; a click inside the card does not', () => {
    const { props, container } = renderPanel();
    fireEvent.mouseDown(screen.getByRole('dialog', { name: 'Settings' }));
    expect(props.onClose).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(props.onClose).toHaveBeenCalledTimes(1);
    fireEvent.mouseDown(container.querySelector('.dialog-overlay')!);
    expect(props.onClose).toHaveBeenCalledTimes(2);
  });

  it('restores focus to the element that was active before opening', () => {
    const trigger = document.createElement('button');
    document.body.appendChild(trigger);
    trigger.focus();

    const { rerender, props } = renderPanel();
    expect(document.activeElement).toBe(tab('General'));
    rerender(<SettingsPanel {...props} open={false} />);
    expect(document.activeElement).toBe(trigger);
    trigger.remove();
  });
});
