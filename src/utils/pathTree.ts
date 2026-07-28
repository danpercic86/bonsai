// P3b contract §3 — generic path-tree builder. Pure display math; the backend
// keeps returning flat lists.

export interface TreeLeaf<T> {
  kind: 'leaf';
  /** Basename (segment after the last '/'), display-only. */
  name: string;
  /** The full original path — stable React key material for callers. */
  path: string;
  item: T;
}

export interface TreeDir<T> {
  kind: 'dir';
  /** Display label. After chain-collapsing this may contain '/'
   *  (e.g. "src/git" for a collapsed src -> git chain). */
  name: string;
  /** Full prefix from the root INCLUDING trailing content up to this dir,
   *  WITHOUT a trailing '/' (e.g. "src/git"). Unique within one tree —
   *  used as the React key and the collapse-state key. */
  fullPrefix: string;
  children: TreeNode<T>[];
}

export type TreeNode<T> = TreeLeaf<T> | TreeDir<T>;

/** Transient mutable shape used only during the build. */
interface BuildNode<T> {
  childrenByName: Map<string, BuildNode<T>>;
  leaves: TreeLeaf<T>[];
}

function newBuildNode<T>(): BuildNode<T> {
  return { childrenByName: new Map(), leaves: [] };
}

/** §3.3 locked sort: case-insensitive code-unit compare, case-sensitive
 *  tiebreak. NOT localeCompare — locale-independent determinism. */
function compareNames(a: string, b: string): number {
  const al = a.toLowerCase();
  const bl = b.toLowerCase();
  if (al < bl) return -1;
  if (al > bl) return 1;
  if (a < b) return -1;
  if (a > b) return 1;
  return 0;
}

/**
 * Splits each item's path on '/' into a nested tree, collapses single-child
 * directory chains, and sorts deterministically (§3.3). Pure; O(n · depth).
 * Items whose paths are duplicated produce duplicate leaves (callers'
 * status lists never contain duplicates within one section — not defended).
 * Empty segments from leading/trailing/double slashes are skipped defensively.
 */
export function buildPathTree<T>(
  items: readonly T[],
  getPath: (item: T) => string,
): TreeNode<T>[] {
  const root = newBuildNode<T>();
  for (const item of items) {
    const path = getPath(item);
    const segments = path.split('/').filter((s) => s !== '');
    if (segments.length === 0) continue; // defensive; never expected
    let node = root;
    for (let i = 0; i < segments.length - 1; i++) {
      const seg = segments[i];
      let child = node.childrenByName.get(seg);
      if (child === undefined) {
        child = newBuildNode<T>();
        node.childrenByName.set(seg, child);
      }
      node = child;
    }
    node.leaves.push({
      kind: 'leaf',
      name: segments[segments.length - 1],
      path,
      item,
    });
  }

  function finalize(node: BuildNode<T>, name: string, prefix: string): TreeDir<T> {
    const dirs: TreeDir<T>[] = [];
    for (const [childName, child] of node.childrenByName) {
      let d = finalize(child, childName, prefix === '' ? childName : `${prefix}/${childName}`);
      // Chain-collapse: a dir with EXACTLY ONE child, and that child is a dir
      // (i.e. the dir has no leaves of its own), merges into the child.
      while (d.children.length === 1 && d.children[0].kind === 'dir') {
        const only = d.children[0];
        d = {
          kind: 'dir',
          name: `${d.name}/${only.name}`,
          fullPrefix: only.fullPrefix,
          children: only.children,
        };
      }
      dirs.push(d);
    }
    dirs.sort((a, b) => compareNames(a.name, b.name));
    const leaves = [...node.leaves].sort((a, b) => compareNames(a.name, b.name));
    return { kind: 'dir', name, fullPrefix: prefix, children: [...dirs, ...leaves] };
  }

  return finalize(root, '', '').children;
}
