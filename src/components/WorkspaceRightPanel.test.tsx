/** P67b §7.1 — WorkspaceRightPanel: the single merged actions row that replaced
 *  the stacked stash split button + amend affordance. Asserts the `⋯` overflow
 *  menu carries ALL THREE stash scopes with the SAME per-scope gating the deleted
 *  StashSplitButton had, that it closes on outside-mousedown and Escape, when the
 *  row is suppressed, and that no `.stash-split` element survives anywhere
 *  (deletion regression guard). Presentational only — props in, callbacks out. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { WorkspaceRightPanel } from './WorkspaceRightPanel';
import type { WorkspaceRightPanelProps } from './WorkspaceRightPanel';
import type { BranchInfo, HeadInfo, StatusEntry, StatusSnapshot } from '../ipc';

function entry(path: string): StatusEntry {
  return { path, origPath: null, status: 'modified' };
}

function snapshot(over: Partial<StatusSnapshot> = {}): StatusSnapshot {
  return { staged: [], unstaged: [], untracked: [], conflicted: [], ...over };
}

const HEAD: HeadInfo = { branchName: 'main', oid: 'a'.repeat(40), detached: false, unborn: false };

function branch(over: Partial<BranchInfo> = {}): BranchInfo {
  return {
    name: 'main',
    isHead: true,
    upstream: null,
    ahead: null,
    behind: null,
    tip: 'a'.repeat(40),
    ...over,
  };
}

function renderPanel(over: Partial<WorkspaceRightPanelProps> = {}) {
  const props: WorkspaceRightPanelProps = {
    rightPanelWidth: 360,
    repoId: 'r1',
    rightPaneTab: 'work',
    onSelectRightPaneTab: vi.fn(),
    prDefaultHead: 'main',
    prDefaultBase: 'main',
    prBaseOptions: [],
    prCompareOptions: [],
    prNav: null,
    checksTarget: null,
    checksRefreshSeq: 0,
    opState: { kind: 'none' },
    conflicts: [],
    mutating: false,
    onCommitMerge: vi.fn(),
    onRebaseContinue: vi.fn(),
    onRebaseSkip: vi.fn(),
    onCherrypickContinue: vi.fn(),
    onRevertContinue: vi.fn(),
    onAbort: vi.fn(),
    onBisectMark: vi.fn(),
    onBisectSkip: vi.fn(),
    bisectSummaries: {},
    compare: null,
    compareData: null,
    compareLoading: false,
    compareError: null,
    headBranch: branch(),
    listView: 'flat',
    panelDensity: 'cozy',
    primaryCommitAction: 'commit',
    scope: { kind: 'root' },
    setScope: vi.fn(),
    clearCompare: vi.fn(),
    selectedIndex: null,
    graph: null,
    commitDiff: null,
    commitDiffLoading: false,
    commitDiffError: null,
    setCommitBrowserOpen: vi.fn(),
    onSelectParent: vi.fn(),
    setSelectedIndex: vi.fn(),
    aiEligible: false,
    runAnalyze: vi.fn(),
    status: snapshot({ staged: [entry('a.ts')], unstaged: [entry('b.ts')] }),
    statusLoading: false,
    statusError: null,
    diffSlot: null,
    aiRows: {},
    aiAtCapacity: false,
    aiPanelLoading: false,
    onStage: vi.fn(),
    onUnstage: vi.fn(),
    onDiscard: vi.fn(),
    onDiscardForce: vi.fn(),
    onToggleDiff: vi.fn(),
    onResolveConflict: vi.fn(),
    onToggleConflictView: vi.fn(),
    onAiResolve: vi.fn(),
    onAiReview: vi.fn(),
    onBlame: vi.fn(),
    onFileHistory: vi.fn(),
    onCreateStash: vi.fn(),
    head: HEAD,
    amend: false,
    onToggleAmend: vi.fn(),
    amendMessage: null,
    commitBoxRef: { current: null },
    onCommitAmend: vi.fn(async () => {}),
    onCommitMergeSubmit: vi.fn(async () => {}),
    onCommit: vi.fn(async () => {}),
    onCommitAndPush: vi.fn(async () => {}),
    onGenerate: vi.fn(async () => ''),
    workingDirty: true,
    onCompose: vi.fn(),
    onOpenIdentitySettings: vi.fn(),
    signingStatus: null,
    commitSignature: null,
    ...over,
  };
  return { ...render(<WorkspaceRightPanel {...props} />), props };
}

// P80 §2b: the actions row is gone — the trigger is CommitBox's `⋯` toolbar
// button ("Commit options"), and amend/sign/skip/stash fold into its menu.
const overflowBtn = () => screen.getByRole('button', { name: 'Commit options' });

describe('WorkspaceRightPanel commit-options menu (P80 §2b)', () => {
  it('the `⋯` trigger lives in the commit toolbar; amend is a menuitemcheckbox', () => {
    const { container } = renderPanel();
    // The old dedicated actions row is gone.
    expect(container.querySelectorAll('.rp-actions-row')).toHaveLength(0);
    expect(container.querySelector('.commit-msg-toolbar')?.contains(overflowBtn())).toBe(true);
    fireEvent.click(overflowBtn());
    expect(
      screen.getByRole('menuitemcheckbox', { name: /Amend last commit/ }),
    ).toBeInTheDocument();
  });

  it('no .stash-split element survives (deletion regression guard)', () => {
    const { container } = renderPanel();
    expect(container.querySelectorAll('[class*="stash-split"]')).toHaveLength(0);
  });

  it('the amend menuitemcheckbox reports its new value and respects `mutating`', () => {
    const { props } = renderPanel();
    fireEvent.click(overflowBtn());
    fireEvent.click(screen.getByRole('menuitemcheckbox', { name: /Amend last commit/ }));
    expect(props.onToggleAmend).toHaveBeenCalledWith(true);
    renderPanel({ mutating: true });
    // The second panel's own trigger.
    const triggers = screen.getAllByRole('button', { name: 'Commit options' });
    fireEvent.click(triggers[1]);
    // The first panel's menu may still be open (checkbox clicks keep it open);
    // the second panel's amend item is the newest one and must be disabled.
    const amendItems = screen.getAllByRole('menuitemcheckbox', { name: /Amend last commit/ });
    expect(amendItems[amendItems.length - 1]).toBeDisabled();
  });

  it('⋯ opens a menu with the three stash scopes, each firing onCreateStash', () => {
    const { props } = renderPanel({
      status: snapshot({ staged: [entry('a.ts')], untracked: [entry('c.ts')] }),
    });
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
    fireEvent.click(overflowBtn());
    const menu = screen.getByRole('menu');
    const items = screen.getAllByRole('menuitem');
    expect(items.map((i) => i.textContent)).toEqual([
      'Stash all',
      'Stash all + untracked',
      'Stash staged only',
    ]);
    expect(menu.contains(items[0])).toBe(true);

    fireEvent.click(items[0]);
    expect(props.onCreateStash).toHaveBeenCalledWith('all');
    // Choosing a stash scope closes the menu.
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();

    fireEvent.click(overflowBtn());
    fireEvent.click(screen.getAllByRole('menuitem')[1]);
    expect(props.onCreateStash).toHaveBeenCalledWith('allWithUntracked');

    fireEvent.click(overflowBtn());
    fireEvent.click(screen.getAllByRole('menuitem')[2]);
    expect(props.onCreateStash).toHaveBeenCalledWith('staged');
    expect(props.onCreateStash).toHaveBeenCalledTimes(3);
  });

  it('per-scope gating: staged needs staged files, all needs tracked changes', () => {
    // Untracked only → `all` disabled, `allWithUntracked` enabled, `staged` disabled.
    renderPanel({ status: snapshot({ untracked: [entry('c.ts')] }) });
    fireEvent.click(overflowBtn());
    let items = screen.getAllByRole('menuitem');
    expect(items[0]).toBeDisabled();
    expect(items[1]).toBeEnabled();
    expect(items[2]).toBeDisabled();

    // Unstaged tracked edit only → `all` + `allWithUntracked` enabled, `staged` not.
    const second = renderPanel({ status: snapshot({ unstaged: [entry('b.ts')] }) });
    fireEvent.click(second.getAllByRole('button', { name: 'Commit options' }).slice(-1)[0]);
    items = Array.from(second.container.querySelectorAll('.rp-overflow-item[role="menuitem"]'));
    expect(items[0]).toBeEnabled();
    expect(items[1]).toBeEnabled();
    expect(items[2]).toBeDisabled();
  });

  it('the `⋯` trigger is enabled while mutating; its stash items are disabled', () => {
    renderPanel({ mutating: true, status: snapshot({ staged: [entry('a.ts')] }) });
    expect(overflowBtn()).toBeEnabled();
    fireEvent.click(overflowBtn());
    for (const it of screen.getAllByRole('menuitem')) expect(it).toBeDisabled();
  });

  it('outside mousedown and Escape close the menu', () => {
    renderPanel();
    fireEvent.click(overflowBtn());
    expect(screen.getByRole('menu')).toBeInTheDocument();
    fireEvent.mouseDown(document.body);
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();

    fireEvent.click(overflowBtn());
    expect(screen.getByRole('menu')).toBeInTheDocument();
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
  });

  it('a mousedown INSIDE the overflow keeps the menu open', () => {
    const { container } = renderPanel();
    fireEvent.click(overflowBtn());
    fireEvent.mouseDown(container.querySelector('.rp-overflow-menu') as HTMLElement);
    expect(screen.getByRole('menu')).toBeInTheDocument();
  });

  it('the amend item is absent on an unborn HEAD / no HEAD (canAmend gate)', () => {
    const unborn = renderPanel({ head: { ...HEAD, unborn: true } });
    fireEvent.click(unborn.getByRole('button', { name: 'Commit options' }));
    expect(screen.queryByRole('menuitemcheckbox', { name: /Amend last commit/ })).toBeNull();

    const noHead = renderPanel({ head: null });
    fireEvent.click(noHead.getAllByRole('button', { name: 'Commit options' }).slice(-1)[0]);
    expect(screen.queryByRole('menuitemcheckbox', { name: /Amend last commit/ })).toBeNull();
  });

  it('hides the stash items during a merge (git stash refuses on unmerged paths)', () => {
    // canStash mirrors canAmend: opState 'none' + born HEAD. In merge mode the
    // `⋯` trigger stays enabled (blocked is false), but stash must NOT appear.
    renderPanel({
      opState: { kind: 'merge', incoming: 'dev', message: 'Merge dev' },
      status: snapshot({ staged: [entry('a.ts')] }),
    });
    fireEvent.click(screen.getAllByRole('button', { name: 'Commit options' }).slice(-1)[0]);
    expect(screen.queryByRole('menuitem', { name: /^Stash/ })).toBeNull();
  });

  it('the amend push warning shows only when amend + upstream set + ahead 0', () => {
    const pushed = branch({ upstream: 'origin/main', ahead: 0, behind: 0 });
    const on = renderPanel({ amend: true, headBranch: pushed });
    expect(on.container.querySelector('.commit-note')).not.toBeNull();

    const off = renderPanel({ amend: false, headBranch: pushed });
    expect(off.container.querySelector('.commit-note')).toBeNull();

    const ahead = renderPanel({
      amend: true,
      headBranch: branch({ upstream: 'origin/main', ahead: 2, behind: 0 }),
    });
    expect(ahead.container.querySelector('.commit-note')).toBeNull();

    const noUpstream = renderPanel({ amend: true, headBranch: branch() });
    expect(noUpstream.container.querySelector('.commit-note')).toBeNull();
  });

  it('keeps both right-pane tabs and the always-mounted work wrapper', () => {
    // P62c: `.right-panel-work` is `hidden`, never unmounted, so a half-typed
    // commit message survives a Working↔PRs toggle. P67b must not regress that.
    const { container } = renderPanel();
    expect(screen.getByRole('tab', { name: 'Working' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Pull requests' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Checks' })).toBeInTheDocument();
    const work = container.querySelector('.right-panel-work');
    expect(work).not.toBeNull();
    expect(work).not.toHaveAttribute('hidden');
  });

  // P67c §7.1 (the case P67b deferred): density is a PROP rendered as an
  // attribute on the `<aside>` (D7), never `documentElement.dataset` — the
  // whole compact override block hangs off this selector, so an unconditional
  // attribute in BOTH densities is what makes the CSS cascade work.
  it('carries data-density from the prop in both densities', () => {
    const cozy = renderPanel().container.querySelector('.right-panel');
    expect(cozy).toHaveAttribute('data-density', 'cozy');
    const compact = renderPanel({ panelDensity: 'compact' }).container.querySelector(
      '.right-panel',
    );
    expect(compact).toHaveAttribute('data-density', 'compact');
  });

  it('the Checks tab selects via onSelectRightPaneTab and mounts only when active', () => {
    // P90: the third tab. Inactive ⇒ ChecksPanel not mounted (no work for a
    // hidden panel); clicking the tab requests the switch.
    const { props, container } = renderPanel();
    expect(container.querySelector('.checks-panel')).toBeNull();
    fireEvent.click(screen.getByRole('tab', { name: 'Checks' }));
    expect(props.onSelectRightPaneTab).toHaveBeenCalledWith('checks');
    // When active, the panel mounts (idle empty state, no target).
    const active = renderPanel({ rightPaneTab: 'checks' });
    expect(active.container.querySelector('.checks-panel')).not.toBeNull();
  });

  it('keeps the work wrapper MOUNTED (only `hidden`) while the PRs tab is active', () => {
    // This is the half of P62c that actually preserves a half-typed commit
    // message: a regression to `{rightPaneTab === 'work' && …}` still renders
    // fine on the Working tab, so only the 'prs' case catches it.
    const { container } = renderPanel({ rightPaneTab: 'prs' });
    const work = container.querySelector('.right-panel-work');
    expect(work).not.toBeNull();
    expect(work).toHaveAttribute('hidden');
    // …and the commit box inside it is still in the tree, not unmounted.
    expect(work?.querySelector('.commit-box')).not.toBeNull();
  });
});
