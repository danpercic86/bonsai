import { useState } from 'react';
import type { JSX, ReactNode } from 'react';
import type { TreeLeaf, TreeNode } from '../utils/pathTree';

// P3b contract §4 — recursive collapsible tree renderer. Display-only; leaf
// rows are supplied whole by the caller via renderLeaf (Tree never inspects
// leaf content). Collapse state is local and ephemeral (contract §7.5): a Set
// of COLLAPSED fullPrefix keys — default everything expanded; data refreshes
// keep the set (stale keys are harmless), unmount discards it.

export interface TreeProps<T> {
  nodes: TreeNode<T>[];
  /** Renders a COMPLETE <li> for a leaf (reuse existing FileRow / BranchRow /
   *  tag-row markup unchanged — Tree never inspects leaf content). */
  renderLeaf(leaf: TreeLeaf<T>): ReactNode;
  /** React key for a leaf <li>'s wrapper position; must be unique per list
   *  (e.g. `${entry.status}:${entry.path}` for status rows, branch name for refs). */
  leafKey(leaf: TreeLeaf<T>): string;
  /** Optional extra class on nested <ul role="group"> levels (styling hook). */
  groupClassName?: string;
  /** P4d: when true, every dir starts COLLAPSED except those in initiallyExpanded.
   *  When falsy (default), all dirs start expanded — legacy behavior. */
  defaultCollapsed?: boolean;
  /** P4d: dir fullPrefixes to leave expanded on first render (seed). Applied
   *  once by the useState initializer; callers reseed by changing the React key. */
  initiallyExpanded?: readonly string[];
}

function collectDirPrefixes<T>(nodes: TreeNode<T>[], out: string[]): void {
  for (const n of nodes) {
    if (n.kind === 'dir') {
      out.push(n.fullPrefix);
      collectDirPrefixes(n.children, out);
    }
  }
}

function renderNodes<T>(
  nodes: TreeNode<T>[],
  props: TreeProps<T>,
  collapsed: Set<string>,
  toggle: (prefix: string) => void,
): ReactNode {
  return nodes.map((node) => {
    if (node.kind === 'leaf') {
      // renderLeaf returns a complete <li>; key it via a fragment wrapper-free
      // approach is not possible, so we rely on the caller's <li> being the
      // only child and key the array position with leafKey.
      return <TreeLeafSlot key={props.leafKey(node)}>{props.renderLeaf(node)}</TreeLeafSlot>;
    }
    const expanded = !collapsed.has(node.fullPrefix);
    return (
      <li key={node.fullPrefix} role="treeitem" aria-expanded={expanded} className="tree-dir">
        <div className="tree-dir-row">
          <button
            type="button"
            className="tree-dir-toggle"
            onClick={() => toggle(node.fullPrefix)}
          >
            <span className={`file-chevron${expanded ? ' file-chevron-open' : ''}`}>{'›'}</span>
            <span className="tree-dir-name" title={node.fullPrefix}>
              {node.name}
            </span>
          </button>
        </div>
        {expanded && (
          <ul
            role="group"
            className={
              props.groupClassName !== undefined
                ? `tree-group ${props.groupClassName}`
                : 'tree-group'
            }
          >
            {renderNodes(node.children, props, collapsed, toggle)}
          </ul>
        )}
      </li>
    );
  });
}

/** Keyed pass-through so renderLeaf's <li> can sit directly in the list while
 *  the Tree owns the React key. */
function TreeLeafSlot({ children }: { children: ReactNode }) {
  return <>{children}</>;
}

export function Tree<T>(props: TreeProps<T>): JSX.Element {
  const [collapsed, setCollapsed] = useState<Set<string>>(() => {
    if (props.defaultCollapsed !== true) return new Set(); // legacy: all expanded
    const all: string[] = [];
    collectDirPrefixes(props.nodes, all);
    const s = new Set(all);
    for (const p of props.initiallyExpanded ?? []) s.delete(p);
    return s;
  });
  const toggle = (prefix: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(prefix)) next.delete(prefix);
      else next.add(prefix);
      return next;
    });
  };
  return (
    <ul className="tree" role="tree">
      {renderNodes(props.nodes, props, collapsed, toggle)}
    </ul>
  );
}
