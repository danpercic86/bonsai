/** T3.5 — Sidebar: section rendering from props, current-branch highlight,
 *  context-menu wiring, the per-section filter box, ahead/behind badges,
 *  create-branch flow, and empty/loading states. Presentational only — every
 *  assertion is against props-in / callbacks-out. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import { Sidebar } from './Sidebar';
import type { SidebarProps } from './Sidebar';
import type { BranchInfo, BranchesSnapshot } from '../ipc';

function branch(name: string, over: Partial<BranchInfo> = {}): BranchInfo {
  return { name, isHead: false, upstream: null, ahead: null, behind: null, tip: 'a'.repeat(40), ...over };
}

function snapshot(over: Partial<BranchesSnapshot> = {}): BranchesSnapshot {
  return {
    local: [branch('main', { isHead: true }), branch('dev')],
    remote: [{ name: 'origin/main', tip: 'b'.repeat(40) }],
    tags: ['v1.0'],
    head: { branchName: 'main', oid: 'c'.repeat(40), detached: false, unborn: false },
    ...over,
  };
}

function renderSidebar(over: Partial<SidebarProps> = {}) {
  const props: SidebarProps = {
    data: snapshot(),
    loading: false,
    error: null,
    onDismissError: vi.fn(),
    busy: false,
    opActive: false,
    currentBranch: 'main',
    onCheckout: vi.fn(),
    onContextMenu: vi.fn(),
    onCreateBranch: vi.fn(async () => {}),
    width: 240,
    listView: 'flat',
    stashes: [],
    onCreateStash: vi.fn(),
    onStashContextMenu: vi.fn(),
    submodules: [],
    onSubmoduleContextMenu: vi.fn(),
    submoduleBusy: null,
    onNewSubmodule: vi.fn(),
    worktrees: [],
    onWorktreeContextMenu: vi.fn(),
    onNewWorktree: vi.fn(),
    onTagContextMenu: vi.fn(),
    tagSyncReport: null,
    tagSyncState: 'idle',
    tagSyncRemote: null,
    tagSyncCheckedAt: null,
    onTagsExpand: vi.fn(),
    remotes: [{ name: 'origin', url: 'https://example.com/r.git' }],
    onRemoteContextMenu: vi.fn(),
    onAddRemote: vi.fn(),
    ...over,
  };
  return { ...render(<Sidebar {...props} />), props };
}

describe('Sidebar sections', () => {
  it('renders branches, remotes, tags, and section empty states', () => {
    renderSidebar();
    expect(screen.getByText('main')).toBeInTheDocument();
    expect(screen.getByText('dev')).toBeInTheDocument();
    expect(screen.getByText('origin')).toBeInTheDocument();
    expect(screen.getByText('origin/main')).toBeInTheDocument();
    // Tags start collapsed by default; expanding shows the tag.
    expect(screen.queryByText('v1.0')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('treeitem', { name: /Tags/ }));
    expect(screen.getByText('v1.0')).toBeInTheDocument();
    expect(screen.getByText('No stashes')).toBeInTheDocument();
    expect(screen.getByText('No submodules')).toBeInTheDocument();
    expect(screen.getByText('No worktrees')).toBeInTheDocument();
  });

  it('null data + loading shows only skeletons; null data without loading shows nothing', () => {
    const { container } = renderSidebar({ data: null, loading: true });
    expect(container.querySelector('.skeleton-group')).toBeInTheDocument();
    expect(screen.queryByText('Branches')).not.toBeInTheDocument();
  });

  it('highlights the HEAD branch row and puts it first in flat mode', () => {
    const { container } = renderSidebar({
      data: snapshot({ local: [branch('aaa'), branch('zzz', { isHead: true })] }),
    });
    const rows = container.querySelectorAll('.branch-row');
    expect(rows[0]).toHaveClass('branch-row-head');
    expect(rows[0].textContent).toContain('zzz');
  });

  it('double-click checks out a non-HEAD branch; HEAD row and busy state do not', () => {
    const { props, container } = renderSidebar();
    const dev = screen.getByText('dev').closest('li')!;
    fireEvent.doubleClick(dev);
    expect(props.onCheckout).toHaveBeenCalledWith('dev');
    const head = container.querySelector('.branch-row-head')!;
    fireEvent.doubleClick(head);
    expect(props.onCheckout).toHaveBeenCalledTimes(1);
  });

  it('busy blocks double-click checkout and disables the header actions', () => {
    const { props } = renderSidebar({ busy: true, onCleanupBranches: vi.fn() });
    fireEvent.doubleClick(screen.getByText('dev').closest('li')!);
    expect(props.onCheckout).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Create branch' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Clean up branches…' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Add remote' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Stash changes' })).toBeDisabled();
  });

  it('right-click wiring: local branch, remote-tracking, configured remote, tag', () => {
    const { props } = renderSidebar();
    fireEvent.contextMenu(screen.getByText('dev').closest('li')!, { clientX: 10, clientY: 20 });
    expect(props.onContextMenu).toHaveBeenCalledWith('dev', 'localBranch', 10, 20);
    fireEvent.contextMenu(screen.getByText('origin/main').closest('li')!, { clientX: 1, clientY: 2 });
    expect(props.onContextMenu).toHaveBeenCalledWith('origin/main', 'remoteBranch', 1, 2);
    fireEvent.contextMenu(screen.getByText('origin').closest('li')!, { clientX: 3, clientY: 4 });
    expect(props.onRemoteContextMenu).toHaveBeenCalledWith('origin', 3, 4);
    fireEvent.click(screen.getByRole('treeitem', { name: /Tags/ }));
    fireEvent.contextMenu(screen.getByText('v1.0').closest('li')!, { clientX: 5, clientY: 6 });
    expect(props.onTagContextMenu).toHaveBeenCalledWith('v1.0', 5, 6);
  });

  it('ahead/behind badge renders ↑/↓ only when diverged with an upstream', () => {
    renderSidebar({
      data: snapshot({
        local: [
          branch('sync', { upstream: 'origin/sync', ahead: 0, behind: 0 }),
          branch('div', { upstream: 'origin/div', ahead: 2, behind: 1 }),
          branch('nouv'),
        ],
      }),
    });
    expect(screen.getByText('↑2 ↓1')).toBeInTheDocument();
    expect(screen.getByText('↑2 ↓1')).toHaveAttribute('title', 'vs origin/div');
    // in-sync and upstream-less rows carry no badge
    expect(screen.queryByText(/↑0|↓0/)).not.toBeInTheDocument();
  });

  it('detached HEAD renders the short-oid row', () => {
    renderSidebar({
      data: snapshot({
        local: [],
        head: { branchName: null, oid: 'deadbeefcafe' + '0'.repeat(28), detached: true, unborn: false },
      }),
      currentBranch: null,
    });
    expect(screen.getByText(/HEAD detached @/)).toBeInTheDocument();
    expect(screen.getByText('deadbee')).toBeInTheDocument();
  });

  it('unborn HEAD hides the create-branch and stash header buttons', () => {
    renderSidebar({
      data: snapshot({
        local: [],
        head: { branchName: null, oid: '', detached: false, unborn: true },
      }),
    });
    expect(screen.queryByRole('button', { name: 'Create branch' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Stash changes' })).not.toBeInTheDocument();
    expect(screen.getByText('No branches yet')).toBeInTheDocument();
  });

  it('filter box appears only at ≥6 rows, filters, and shows the no-match text', () => {
    renderSidebar(); // 2 branches -> no filter box
    expect(screen.queryByLabelText('Filter branches')).not.toBeInTheDocument();
    renderSidebar({
      data: snapshot({
        local: ['alpha', 'beta', 'gamma', 'delta', 'epsilon', 'zeta'].map((n) => branch(n)),
      }),
    });
    const box = screen.getByLabelText('Filter branches');
    fireEvent.change(box, { target: { value: 'et' } });
    expect(screen.getByText('beta')).toBeInTheDocument();
    expect(screen.getByText('zeta')).toBeInTheDocument();
    expect(screen.queryByText('alpha')).not.toBeInTheDocument();
    fireEvent.change(box, { target: { value: 'nope' } });
    expect(screen.getByText("No branches match 'nope'")).toBeInTheDocument();
  });

  it('create-branch: + opens the input, Enter submits, success closes it', async () => {
    const onCreateBranch = vi.fn(async () => {});
    renderSidebar({ onCreateBranch });
    fireEvent.click(screen.getByRole('button', { name: 'Create branch' }));
    const input = screen.getByPlaceholderText('new-branch-name');
    fireEvent.change(input, { target: { value: '  feat/x  ' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(onCreateBranch).toHaveBeenCalledWith('feat/x');
    await vi.waitFor(() =>
      expect(screen.queryByPlaceholderText('new-branch-name')).not.toBeInTheDocument(),
    );
  });

  it('create-branch rejection shows the inline error and keeps the input open', async () => {
    const onCreateBranch = vi.fn(async () => {
      throw { kind: 'invalidName', message: 'bad ref name' };
    });
    renderSidebar({ onCreateBranch });
    fireEvent.click(screen.getByRole('button', { name: 'Create branch' }));
    const input = screen.getByPlaceholderText('new-branch-name');
    fireEvent.change(input, { target: { value: 'x y' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(await screen.findByText('bad ref name')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('new-branch-name')).toBeInTheDocument();
  });

  it('error banner renders with a working dismiss button', () => {
    const { props } = renderSidebar({ error: 'checkout failed' });
    expect(screen.getByRole('alert')).toHaveTextContent('checkout failed');
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss error' }));
    expect(props.onDismissError).toHaveBeenCalledTimes(1);
  });

  it('stash rows render label + message and wire the context menu by index', () => {
    const { props } = renderSidebar({
      stashes: [
        { index: 0, message: 'WIP on main', oid: 'a'.repeat(40), baseOid: 'b'.repeat(40), ts: 1_700_000_000 },
      ],
    });
    expect(screen.getByText('stash@{0}')).toBeInTheDocument();
    expect(screen.getByText('WIP on main')).toBeInTheDocument();
    fireEvent.contextMenu(screen.getByText('stash@{0}').closest('li')!, { clientX: 7, clientY: 8 });
    // F-A6-B: the row forwards the oid it rendered alongside index + coords.
    expect(props.onStashContextMenu).toHaveBeenCalledWith(0, 'a'.repeat(40), 7, 8);
  });

  it('submodule rows render the status badge; worktree rows render their badge pills', () => {
    const { props } = renderSidebar({
      submodules: [
        {
          name: 'libfoo', path: 'vendor/libfoo', absPath: '/r/vendor/libfoo', url: null,
          headOid: null, indexOid: null, wtOid: null, status: 'outOfSync',
        },
      ],
      worktrees: [
        {
          name: 'hotfix', absPath: '/wt/hotfix', relPath: null, branch: 'hotfix-1',
          headOid: 'a'.repeat(40), locked: true, lockReason: 'busy', isMain: false,
          isCurrent: true, prunable: false, valid: true,
        },
      ],
    });
    expect(screen.getByText('out of sync')).toBeInTheDocument();
    expect(screen.getByText('current')).toBeInTheDocument();
    expect(screen.getByText('locked')).toHaveAttribute('title', 'busy');
    expect(screen.getByText('hotfix-1')).toBeInTheDocument();
    fireEvent.contextMenu(screen.getByText('libfoo').closest('li')!, { clientX: 1, clientY: 1 });
    expect(props.onSubmoduleContextMenu).toHaveBeenCalledWith('libfoo', 1, 1);
    fireEvent.contextMenu(screen.getByText('hotfix').closest('li')!, { clientX: 2, clientY: 2 });
    expect(props.onWorktreeContextMenu).toHaveBeenCalledWith('hotfix', 2, 2);
  });

  it('header + buttons fire onAddRemote / onNewSubmodule / onNewWorktree / onCreateStash', () => {
    const { props } = renderSidebar();
    fireEvent.click(screen.getByRole('button', { name: 'Add remote' }));
    expect(props.onAddRemote).toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Add submodule' }));
    expect(props.onNewSubmodule).toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'New worktree' }));
    expect(props.onNewWorktree).toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Stash changes' }));
    expect(props.onCreateStash).toHaveBeenCalled();
  });

  it('collapsing a section hides its rows', () => {
    renderSidebar();
    const toggle = screen.getByRole('treeitem', { name: /Branches/ });
    expect(toggle).toHaveAttribute('aria-expanded', 'true');
    fireEvent.click(toggle);
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByText('dev')).not.toBeInTheDocument();
  });

  it('tree mode groups slash names and keeps full-name semantics on the leaf', () => {
    const { props } = renderSidebar({
      listView: 'tree',
      currentBranch: 'feature/one',
      data: snapshot({
        local: [branch('feature/one', { isHead: true }), branch('feature/two'), branch('main')],
      }),
    });
    // The HEAD-ancestor folder starts expanded; its leaves show basenames.
    expect(screen.getByText('feature')).toBeInTheDocument();
    const leaf = screen.getByText('two');
    expect(within(leaf.closest('li')!).getByTitle('feature/two')).toBeInTheDocument();
    fireEvent.doubleClick(leaf.closest('li')!);
    expect(props.onCheckout).toHaveBeenCalledWith('feature/two');
  });
});
