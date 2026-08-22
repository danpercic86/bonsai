/** P-a11y §D: sidebar keyboard accessibility — the composite role="tree",
 *  roving tabindex, D.3 movement, D.5 activation, and D.6 context-menu focus
 *  restore. Renders the whole Sidebar (integration) so the tree root + provider
 *  + rows are wired exactly as in production; the restore test mounts a REAL
 *  ContextMenu so focus restore is exercised end-to-end. */
import { describe, it, expect, vi } from 'vitest';
import { useState } from 'react';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import { Sidebar } from '../Sidebar';
import type { SidebarProps } from '../Sidebar';
import { Tree } from '../Tree';
import { ContextMenu } from '../ContextMenu';
import type { ContextMenuItem } from '../ContextMenu';
import type { BranchInfo, BranchesSnapshot } from '../../ipc';
import type { TreeLeaf, TreeNode } from '../../utils/pathTree';

function branch(name: string, over: Partial<BranchInfo> = {}): BranchInfo {
  return { name, isHead: false, upstream: null, ahead: null, behind: null, tip: 'a'.repeat(40), ...over };
}

function snapshot(over: Partial<BranchesSnapshot> = {}): BranchesSnapshot {
  return {
    local: [
      branch('main', { isHead: true }),
      branch('dev'),
      branch('feature/one'),
      branch('feature/two'),
    ],
    remote: [{ name: 'origin/main', tip: 'b'.repeat(40) }],
    tags: ['v1.0'],
    head: { branchName: 'main', oid: 'c'.repeat(40), detached: false, unborn: false },
    ...over,
  };
}

function baseProps(over: Partial<SidebarProps> = {}): SidebarProps {
  return {
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
    stashes: [{ index: 0, message: 'WIP on main', oid: 'a'.repeat(40), baseOid: 'b'.repeat(40), ts: 1_700_000_000 }],
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
}

function renderSidebar(over: Partial<SidebarProps> = {}) {
  const props = baseProps(over);
  return { ...render(<Sidebar {...props} />), props };
}

const item = (root: ParentNode, key: string): HTMLElement | null =>
  root.querySelector<HTMLElement>(`[data-tree-key="${key}"]`);

const rovingZeros = (root: ParentNode): Element[] =>
  [...root.querySelectorAll('[data-tree-item]')].filter((e) => e.getAttribute('tabindex') === '0');

describe('sidebar tree — model + roles', () => {
  it('is a single role="tree" with roving tabindex on the HEAD row (Tab lands there)', () => {
    const { container } = renderSidebar();
    expect(screen.getByRole('tree', { name: 'Repository sidebar' })).toBeInTheDocument();
    const head = item(container, 'branch:main')!;
    expect(head).toHaveClass('branch-row-head');
    expect(head).toHaveAttribute('tabindex', '0');
    expect(head).toHaveAttribute('aria-current', 'true');
    // Exactly one item is in the Tab cycle.
    expect(rovingZeros(container)).toHaveLength(1);
  });

  it('sets aria-level: headers = 1, flat rows = 2', () => {
    const { container } = renderSidebar();
    expect(item(container, 'header:Branches')).toHaveAttribute('aria-level', '1');
    expect(item(container, 'branch:dev')).toHaveAttribute('aria-level', '2');
    expect(item(container, 'stash:0')).toHaveAttribute('aria-level', '2');
  });

  it('detached-HEAD renders a readable, action-less treeitem', () => {
    const { container } = renderSidebar({
      data: snapshot({
        local: [],
        head: { branchName: null, oid: 'deadbeefcafe' + '0'.repeat(28), detached: true, unborn: false },
      }),
      currentBranch: null,
    });
    const row = item(container, 'detached')!;
    expect(row).toHaveAttribute('role', 'treeitem');
    expect(row).toHaveAttribute('aria-disabled', 'true');
    // No current branch → the first section header is the default active item.
    expect(item(container, 'header:Branches')).toHaveAttribute('tabindex', '0');
  });
});

describe('sidebar tree — movement (D.3)', () => {
  it('ArrowDown/ArrowUp move across rows and keep exactly one roving 0', () => {
    const { container } = renderSidebar();
    const head = item(container, 'branch:main')!;
    head.focus();
    fireEvent.keyDown(head, { key: 'ArrowDown' });
    const dev = item(container, 'branch:dev')!;
    expect(dev).toHaveFocus();
    expect(dev).toHaveAttribute('tabindex', '0');
    expect(head).toHaveAttribute('tabindex', '-1');
    expect(rovingZeros(container)).toHaveLength(1);
    fireEvent.keyDown(dev, { key: 'ArrowUp' });
    expect(item(container, 'branch:main')).toHaveFocus();
  });

  it('Home/End jump to the first / last visible treeitem across sections', () => {
    const { container } = renderSidebar();
    const head = item(container, 'branch:main')!;
    head.focus();
    fireEvent.keyDown(head, { key: 'End' });
    // Worktrees is the last section (empty → its header is the last treeitem).
    expect(item(container, 'header:Worktrees')).toHaveFocus();
    fireEvent.keyDown(item(container, 'header:Worktrees')!, { key: 'Home' });
    expect(item(container, 'header:Branches')).toHaveFocus();
  });

  it('movement is ignored when focus is on a section action button (separate tab stop)', () => {
    const { container } = renderSidebar();
    const addRemote = screen.getByRole('button', { name: 'Add remote' });
    addRemote.focus();
    fireEvent.keyDown(addRemote, { key: 'ArrowDown' });
    // Focus stayed on the button — the tree root only moves when a row is focused.
    expect(addRemote).toHaveFocus();
    expect(container).toBeTruthy();
  });
});

describe('sidebar tree — structure keys (D.3)', () => {
  it('ArrowLeft collapses and ArrowRight expands a section header', () => {
    const { container } = renderSidebar();
    const header = item(container, 'header:Branches')!;
    header.focus();
    expect(header).toHaveAttribute('aria-expanded', 'true');
    fireEvent.keyDown(header, { key: 'ArrowLeft' });
    expect(header).toHaveAttribute('aria-expanded', 'false');
    expect(item(container, 'branch:dev')).toBeNull();
    fireEvent.keyDown(header, { key: 'ArrowRight' });
    expect(header).toHaveAttribute('aria-expanded', 'true');
    expect(item(container, 'branch:dev')).not.toBeNull();
  });

  it('ArrowRight on an expanded header moves to its first child', () => {
    const { container } = renderSidebar();
    const header = item(container, 'header:Branches')!;
    header.focus();
    fireEvent.keyDown(header, { key: 'ArrowRight' });
    expect(item(container, 'branch:main')).toHaveFocus();
  });

  it('ArrowRight/ArrowLeft expand/collapse a tree dir (tree mode); leaves get aria-level 3', () => {
    const { container } = renderSidebar({ listView: 'tree' });
    const dir = item(container, 'dir:feature')!;
    expect(dir).toHaveAttribute('aria-expanded', 'false');
    dir.focus();
    fireEvent.keyDown(dir, { key: 'ArrowRight' });
    expect(dir).toHaveAttribute('aria-expanded', 'true');
    const leaf = item(container, 'branch:feature/one')!;
    expect(leaf).not.toBeNull();
    expect(leaf).toHaveAttribute('aria-level', '3');
    fireEvent.keyDown(dir, { key: 'ArrowLeft' });
    expect(dir).toHaveAttribute('aria-expanded', 'false');
    expect(item(container, 'branch:feature/one')).toBeNull();
  });
});

describe('sidebar tree — activation (D.5)', () => {
  it('Enter checks out a non-HEAD branch and is a no-op on the HEAD row', () => {
    const onCheckout = vi.fn();
    const { container } = renderSidebar({ onCheckout });
    const dev = item(container, 'branch:dev')!;
    dev.focus();
    fireEvent.keyDown(dev, { key: 'Enter' });
    expect(onCheckout).toHaveBeenCalledWith('dev');
    const head = item(container, 'branch:main')!;
    head.focus();
    fireEvent.keyDown(head, { key: 'Enter' });
    expect(onCheckout).toHaveBeenCalledTimes(1); // HEAD Enter did nothing
  });

  it('Enter on the HEAD row does not open a menu (branchMenuItems is empty)', () => {
    const onContextMenu = vi.fn();
    const { container } = renderSidebar({ onContextMenu });
    const head = item(container, 'branch:main')!;
    head.focus();
    fireEvent.keyDown(head, { key: 'Enter' });
    fireEvent.keyDown(head, { key: 'F10', shiftKey: true });
    expect(onContextMenu).not.toHaveBeenCalled();
  });
});

/** Harness that wires every context-menu callback to a REAL ContextMenu, so the
 *  D.6 open + focus-restore path runs end to end. */
function Harness({ over }: { over?: Partial<SidebarProps> }) {
  const [menu, setMenu] = useState<{ x: number; y: number; items: ContextMenuItem[] } | null>(null);
  const items: ContextMenuItem[] = [
    { label: 'Apply', onSelect: vi.fn() },
    { label: 'Drop', onSelect: vi.fn() },
  ];
  const open = (x: number, y: number) => setMenu({ x, y, items });
  const props = baseProps({
    onContextMenu: (_n, _k, x, y) => open(x, y),
    onStashContextMenu: (_i, _o, x, y) => open(x, y),
    onRemoteContextMenu: (_n, x, y) => open(x, y),
    ...over,
  });
  return (
    <>
      <Sidebar {...props} />
      {menu !== null && (
        <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />
      )}
    </>
  );
}

describe('sidebar tree — context menu + focus restore (D.6)', () => {
  it('Enter on a menu-only row opens the menu; Esc restores focus to the row', async () => {
    const { container } = render(<Harness />);
    const stash = item(container, 'stash:0')!;
    stash.focus();
    fireEvent.keyDown(stash, { key: 'Enter' });
    const menu = await screen.findByRole('menu');
    await waitFor(() => expect(menu.contains(document.activeElement)).toBe(true));
    fireEvent.keyDown(menu, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
    await waitFor(() => expect(item(container, 'stash:0')).toHaveFocus());
  });

  it('Shift+F10 on a remote-tracking row opens the menu; Esc restores focus', async () => {
    const { container } = render(<Harness />);
    const remote = item(container, 'remote:origin/main')!;
    remote.focus();
    fireEvent.keyDown(remote, { key: 'F10', shiftKey: true });
    const menu = await screen.findByRole('menu');
    expect(menu).toBeInTheDocument();
    fireEvent.keyDown(menu, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
    await waitFor(() => expect(item(container, 'remote:origin/main')).toHaveFocus());
  });

  it('ContextMenu key opens the menu; activating an item restores focus to the row', async () => {
    const { container } = render(<Harness />);
    const stash = item(container, 'stash:0')!;
    stash.focus();
    fireEvent.keyDown(stash, { key: 'ContextMenu' });
    const menu = await screen.findByRole('menu');
    // Activate the first item (a dismiss path).
    fireEvent.click(within(menu).getByRole('menuitem', { name: 'Apply' }));
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
    await waitFor(() => expect(item(container, 'stash:0')).toHaveFocus());
  });
});

describe('Tree without asGroup (status file tree) is unaffected', () => {
  const leaf = (path: string): TreeLeaf<{ path: string }> => ({
    kind: 'leaf',
    name: path.split('/').pop()!,
    path,
    item: { path },
  });
  const nodes: TreeNode<{ path: string }>[] = [
    { kind: 'dir', name: 'src', fullPrefix: 'src', children: [leaf('src/a.ts')] },
  ];

  it('keeps role="tree" and un-wired dir rows (no roving/aria-level)', () => {
    render(
      <Tree
        nodes={nodes}
        renderLeaf={(l) => <li data-testid="file">{l.name}</li>}
        leafKey={(l) => l.path}
      />,
    );
    expect(screen.getByRole('tree')).toBeInTheDocument();
    const dir = screen.getByRole('treeitem');
    expect(dir).toHaveAttribute('aria-expanded', 'true');
    expect(dir).not.toHaveAttribute('data-tree-item');
    expect(dir).not.toHaveAttribute('aria-level');
  });
});
