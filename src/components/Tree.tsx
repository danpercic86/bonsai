import { useState } from 'react';
import type { JSX, ReactNode } from 'react';
import type { TreeLeaf, TreeNode } from '../utils/pathTree';
import { useSidebarTreeItem } from './sidebar/useSidebarTreeItem';

// P3b contract §4 — recursive collapsible tree renderer. Display-only; leaf
// rows are supplied whole by the caller via renderLeaf (Tree never inspects
// leaf content). Collapse state is local and ephemeral (contract §7.5): a Set
// of COLLAPSED fullPrefix keys — default everything expanded; data refreshes
// keep the set (stale keys are harmless), unmount discards it.

export interface TreeProps<T> {
  nodes: TreeNode<T>[];
  /** Renders a COMPLETE <li> for a leaf (reuse existing FileRow / BranchRow /
   *  tag-row markup unchanged — Tree never inspects leaf content). `level` is the
   *  1-based aria-level for the leaf (P-a11y §D.8); file-tree callers ignore it. */
  renderLeaf(leaf: TreeLeaf<T>, level: number): ReactNode;
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
  /** P3f §1: double-click a dir row → caller applies its section action to all
   *  descendant leaves. Display-only; Tree stays generic (no "stage" knowledge). */
  onActivateDir?(leaves: TreeLeaf<T>[]): void;
  /** P3f §1: title on the dir toggle button for discoverability. */
  dirActionHint?: string;
  /** Optional inline folder-level action buttons rendered inside the dir row
   *  (after the toggle). Given the folder's descendant leaves; the caller
   *  decides what to render (e.g. stage/discard-all). Tree stays generic —
   *  branch/tag Trees pass nothing. Revealed on hover via CSS. */
  renderDirActions?(leaves: TreeLeaf<T>[]): ReactNode;
  /** P-a11y §D.8: embedded in the sidebar's composite `role="tree"`. Root <ul>
   *  becomes `role="group"` (no nested trees) and dir/leaf rows join the roving
   *  tabindex + Arrow/Enter wiring. Off (default) ⇒ the status file tree, byte
   *  identical to pre-P-a11y (`role="tree"`, no roving/aria-level). */
  asGroup?: boolean;
  /** P-a11y §D.8: aria-level of the top-level tree rows (sidebar sections put
   *  their content at level 2, under the level-1 header). Default 2. */
  baseLevel?: number;
}

function collectLeaves<T>(node: Extract<TreeNode<T>, { kind: 'dir' }>, out: TreeLeaf<T>[]): void {
  for (const c of node.children) {
    if (c.kind === 'leaf') out.push(c);
    else collectLeaves(c, out);
  }
}

function collectDirPrefixes<T>(nodes: TreeNode<T>[], out: string[]): void {
  for (const n of nodes) {
    if (n.kind === 'dir') {
      out.push(n.fullPrefix);
      collectDirPrefixes(n.children, out);
    }
  }
}

/** A directory row. Split into its own component so the P-a11y treeitem hook can
 *  run per-dir; the hook is inert (returns {}) unless `asGroup` embeds this Tree
 *  in the sidebar tree, keeping the status file tree unchanged. */
function TreeDir<T>({
  node,
  level,
  expanded,
  treeProps,
  collapsed,
  toggle,
}: {
  node: Extract<TreeNode<T>, { kind: 'dir' }>;
  level: number;
  expanded: boolean;
  treeProps: TreeProps<T>;
  collapsed: Set<string>;
  toggle(prefix: string): void;
}) {
  const wired = treeProps.asGroup === true;
  const item = useSidebarTreeItem({
    treeKey: `dir:${node.fullPrefix}`,
    level,
    kind: 'group',
    enabled: wired,
    expanded,
    onToggle: () => toggle(node.fullPrefix),
  });
  return (
    <li {...item} role="treeitem" aria-expanded={expanded} className="tree-dir">
      <div className="tree-dir-row">
        <button
          type="button"
          className="tree-dir-toggle"
          title={treeProps.dirActionHint}
          // Roving tabindex owner is the <li>; the toggle stays click-focusable
          // but leaves the Tab cycle so the sidebar tree has one Tab stop (§D.1).
          tabIndex={wired ? -1 : undefined}
          onClick={() => toggle(node.fullPrefix)}
          onDoubleClick={
            treeProps.onActivateDir
              ? () => {
                  const leaves: TreeLeaf<T>[] = [];
                  collectLeaves(node, leaves);
                  treeProps.onActivateDir!(leaves);
                }
              : undefined
          }
        >
          <span className={`file-chevron${expanded ? ' file-chevron-open' : ''}`}>{'›'}</span>
          <span className="tree-dir-name" title={node.fullPrefix}>
            {node.name}
          </span>
        </button>
        {treeProps.renderDirActions !== undefined &&
          (() => {
            const leaves: TreeLeaf<T>[] = [];
            collectLeaves(node, leaves);
            return treeProps.renderDirActions(leaves);
          })()}
      </div>
      {expanded && (
        <ul
          role="group"
          className={
            treeProps.groupClassName !== undefined
              ? `tree-group ${treeProps.groupClassName}`
              : 'tree-group'
          }
        >
          {renderNodes(node.children, treeProps, collapsed, toggle, level + 1)}
        </ul>
      )}
    </li>
  );
}

function renderNodes<T>(
  nodes: TreeNode<T>[],
  props: TreeProps<T>,
  collapsed: Set<string>,
  toggle: (prefix: string) => void,
  level: number,
): ReactNode {
  return nodes.map((node) => {
    if (node.kind === 'leaf') {
      // renderLeaf returns a complete <li>; key it via a fragment wrapper-free
      // approach is not possible, so we rely on the caller's <li> being the
      // only child and key the array position with leafKey.
      return (
        <TreeLeafSlot key={props.leafKey(node)}>{props.renderLeaf(node, level)}</TreeLeafSlot>
      );
    }
    const expanded = !collapsed.has(node.fullPrefix);
    return (
      <TreeDir
        key={node.fullPrefix}
        node={node}
        level={level}
        expanded={expanded}
        treeProps={props}
        collapsed={collapsed}
        toggle={toggle}
      />
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
  const baseLevel = props.baseLevel ?? 2;
  return (
    <ul className="tree" role={props.asGroup === true ? 'group' : 'tree'}>
      {renderNodes(props.nodes, props, collapsed, toggle, baseLevel)}
    </ul>
  );
}
