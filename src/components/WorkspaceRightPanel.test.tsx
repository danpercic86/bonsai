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
    prNav: null,
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
    aiResolvingPath: null,
    aiPanelLoading: false,
    onStage: vi.fn(),
    onUnstage: vi.fn(),
    onDiscard: vi.fn(),
    onDiscardForce: vi.fn(),
    onToggleDiff: vi.fn(),
    onResolveConflict: vi.fn(),
    onToggleConflictView: vi.fn(),
    onAiResolve: vi.fn(),
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

const overflowBtn = () => screen.getByRole('button', { name: 'More actions' });

describe('WorkspaceRightPanel actions row', () => {
  it('renders exactly ONE merged row: the amend checkbox plus the ⋯ button', () => {
    const { container } = renderPanel();
    expect(container.querySelectorAll('.rp-actions-row')).toHaveLength(1);
    const row = container.querySelector('.rp-actions-row');
    expect(row).not.toBeNull();
    const amend = screen.getByRole('checkbox', { name: /Amend last commit/ });
    expect(row?.contains(amend)).toBe(true);
    expect(row?.contains(overflowBtn())).toBe(true);
  });

  it('no .stash-split element survives (deletion regression guard)', () => {
    const { container } = renderPanel();
    expect(container.querySelectorAll('[class*="stash-split"]')).toHaveLength(0);
  });

  it('the amend checkbox reports its new value and respects `mutating`', () => {
    const { props } = renderPanel();
    fireEvent.click(screen.getByRole('checkbox', { name: /Amend last commit/ }));
    expect(props.onToggleAmend).toHaveBeenCalledWith(true);
    renderPanel({ mutating: true });
    expect(screen.getAllByRole('checkbox', { name: /Amend last commit/ })[1]).toBeDisabled();
  });

  it('⋯ opens a menu with exactly the three stash scopes, each firing onCreateStash', () => {
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
    // Choosing closes the menu (same as the deleted split button).
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
    fireEvent.click(second.container.querySelector('.rp-overflow-btn') as HTMLButtonElement);
    items = Array.from(second.container.querySelectorAll('.rp-overflow-item'));
    expect(items[0]).toBeEnabled();
    expect(items[1]).toBeEnabled();
    expect(items[2]).toBeDisabled();
  });

  it('the ⋯ button is disabled while mutating, and with nothing to stash at all', () => {
    renderPanel({ mutating: true });
    expect(overflowBtn()).toBeDisabled();
    const clean = renderPanel({ status: snapshot() });
    expect(clean.container.querySelector('.rp-overflow-btn')).toBeDisabled();
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

  it('the whole row is absent during an operation and on an unborn HEAD', () => {
    const rebasing = renderPanel({
      opState: { kind: 'rebase', headName: 'main', onto: 'dev', currentStep: 1, totalSteps: 3 },
    });
    expect(rebasing.container.querySelector('.rp-actions')).toBeNull();

    const unborn = renderPanel({ head: { ...HEAD, unborn: true } });
    expect(unborn.container.querySelector('.rp-actions')).toBeNull();

    const noHead = renderPanel({ head: null });
    expect(noHead.container.querySelector('.rp-actions')).toBeNull();
  });

  it('the amend push warning shows only when amend + upstream set + ahead 0', () => {
    const pushed = branch({ upstream: 'origin/main', ahead: 0, behind: 0 });
    const on = renderPanel({ amend: true, headBranch: pushed });
    expect(on.container.querySelector('.amend-push-warning')).not.toBeNull();

    const off = renderPanel({ amend: false, headBranch: pushed });
    expect(off.container.querySelector('.amend-push-warning')).toBeNull();

    const ahead = renderPanel({
      amend: true,
      headBranch: branch({ upstream: 'origin/main', ahead: 2, behind: 0 }),
    });
    expect(ahead.container.querySelector('.amend-push-warning')).toBeNull();

    const noUpstream = renderPanel({ amend: true, headBranch: branch() });
    expect(noUpstream.container.querySelector('.amend-push-warning')).toBeNull();
  });

  it('keeps both right-pane tabs and the always-mounted work wrapper', () => {
    // P62c: `.right-panel-work` is `hidden`, never unmounted, so a half-typed
    // commit message survives a Working↔PRs toggle. P67b must not regress that.
    const { container } = renderPanel();
    expect(screen.getByRole('tab', { name: 'Working' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Pull requests' })).toBeInTheDocument();
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
