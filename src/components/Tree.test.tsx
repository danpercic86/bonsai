/** T3.3a — Tree primitive: expand/collapse, defaultCollapsed seeding,
 *  dir double-click activation, and dir-level action rendering. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Tree } from './Tree';
import type { TreeLeaf, TreeNode } from '../utils/pathTree';

type Item = { path: string };
const leaf = (path: string): TreeLeaf<Item> => ({
  kind: 'leaf',
  name: path.split('/').pop()!,
  path,
  item: { path },
});

/** src/{a.ts,b.ts} + src/git/lib.rs + README.md */
const nodes: TreeNode<Item>[] = [
  {
    kind: 'dir',
    name: 'src',
    fullPrefix: 'src',
    children: [
      {
        kind: 'dir',
        name: 'git',
        fullPrefix: 'src/git',
        children: [leaf('src/git/lib.rs')],
      },
      leaf('src/a.ts'),
      leaf('src/b.ts'),
    ],
  },
  leaf('README.md'),
];

function renderTree(over: Partial<Parameters<typeof Tree<Item>>[0]> = {}) {
  const onActivateDir = vi.fn();
  const utils = render(
    <Tree<Item>
      nodes={nodes}
      renderLeaf={(l) => <li data-testid="leaf">{l.name}</li>}
      leafKey={(l) => l.path}
      onActivateDir={onActivateDir}
      {...over}
    />,
  );
  return { ...utils, onActivateDir };
}

const dirToggle = (name: string) =>
  screen.getAllByRole('button').find((b) => b.textContent!.includes(name))!;

describe('Tree', () => {
  it('renders everything expanded by default (leaves + nested dirs visible)', () => {
    renderTree();
    expect(screen.getAllByTestId('leaf')).toHaveLength(4);
    const dirs = screen.getAllByRole('treeitem');
    for (const d of dirs) expect(d).toHaveAttribute('aria-expanded', 'true');
  });

  it('clicking a dir toggle collapses (hides descendants) and re-expands', () => {
    renderTree();
    fireEvent.click(dirToggle('src'));
    // Only README.md survives; src/git and its leaf are hidden too.
    expect(screen.getAllByTestId('leaf')).toHaveLength(1);
    expect(screen.getByText('README.md')).toBeInTheDocument();
    fireEvent.click(dirToggle('src'));
    expect(screen.getAllByTestId('leaf')).toHaveLength(4);
  });

  it('collapsing a nested dir keeps siblings visible', () => {
    renderTree();
    fireEvent.click(dirToggle('git'));
    expect(screen.queryByText('lib.rs')).not.toBeInTheDocument();
    expect(screen.getByText('a.ts')).toBeInTheDocument();
  });

  it('defaultCollapsed starts every dir closed except initiallyExpanded seeds', () => {
    renderTree({ defaultCollapsed: true, initiallyExpanded: ['src'] });
    // src open, src/git closed → a.ts/b.ts/README visible, lib.rs hidden.
    expect(screen.getByText('a.ts')).toBeInTheDocument();
    expect(screen.queryByText('lib.rs')).not.toBeInTheDocument();
    expect(screen.getByText('README.md')).toBeInTheDocument();
  });

  it('double-clicking a dir hands ALL descendant leaves to onActivateDir', () => {
    const { onActivateDir } = renderTree();
    fireEvent.doubleClick(dirToggle('src'));
    expect(onActivateDir).toHaveBeenCalledTimes(1);
    const leaves = onActivateDir.mock.calls[0][0] as TreeLeaf<Item>[];
    expect(leaves.map((l) => l.path).sort()).toEqual(['src/a.ts', 'src/b.ts', 'src/git/lib.rs']);
  });

  it('renderDirActions renders per-dir controls with that dir’s leaves', () => {
    renderTree({
      renderDirActions: (leaves) => (
        <button type="button" data-testid="dir-action">
          stage {leaves.length}
        </button>
      ),
    });
    const actions = screen.getAllByTestId('dir-action');
    expect(actions.map((a) => a.textContent)).toContain('stage 3'); // src
    expect(actions.map((a) => a.textContent)).toContain('stage 1'); // src/git
  });
});
