import { useCallback, useMemo, useState } from 'react';
import type { FileDiffHeader, FileStatus, ListView } from '../ipc';
import { buildPathTree } from '../utils/pathTree';
import type { TreeNode } from '../utils/pathTree';

// P11g-rev §2: shared scope navigator. Extracted verbatim from DiffBrowser so
// BOTH ComparePanel and CommitPanel render it as their file list, driving a
// single lifted `scope` (root/dir/file) in RepoWorkspace. This is the canonical
// home of `DiffScope` now (DiffBrowser + RepoWorkspace import it from here).
//
// Purpose-built single-click tree over buildPathTree data (§8.3): the shared
// Tree binds dir-click to collapse, so it cannot express single-click
// select-folder. This reuses buildPathTree for STRUCTURE only. Do NOT modify
// Tree.tsx.

const BADGES: Record<FileStatus, string> = {
  added: 'A',
  modified: 'M',
  deleted: 'D',
  renamed: 'R',
  typechange: 'T',
  untracked: 'U',
  conflicted: 'C',
};

// P11g §6.2: the tree selection. `dir.prefix` is a TreeDir.fullPrefix
// (no trailing '/'); `file.path` is a FileDiffHeader.path.
export type DiffScope =
  | { kind: 'root' }
  | { kind: 'dir'; prefix: string }
  | { kind: 'file'; path: string };

export interface DiffFileTreeProps {
  files: FileDiffHeader[];
  listView: ListView;
  scope: DiffScope;
  onSelect(scope: DiffScope): void;
}

export function DiffFileTree({ files, listView, scope, onSelect }: DiffFileTreeProps) {
  const nodes = useMemo(
    () => (listView === 'tree' ? buildPathTree(files, (f) => f.path) : null),
    [listView, files],
  );
  // Local ephemeral collapse state (fullPrefix keys); independent of selection.
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());
  const toggle = useCallback((prefix: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(prefix)) next.delete(prefix);
      else next.add(prefix);
      return next;
    });
  }, []);

  return (
    <div className="diff-tree">
      <button
        type="button"
        className={`diff-tree-root${scope.kind === 'root' ? ' diff-tree-selected' : ''}`}
        onClick={() => onSelect({ kind: 'root' })}
      >
        <span className="diff-tree-root-label">All files</span>
        <span className="diff-tree-count mono">{files.length}</span>
      </button>
      {nodes !== null ? (
        <ul className="tree" role="tree">
          <DiffTreeNodes nodes={nodes} scope={scope} onSelect={onSelect} collapsed={collapsed} toggle={toggle} />
        </ul>
      ) : (
        <ul className="file-list diff-tree-flat">
          {files.map((f) => (
            <li key={f.path}>
              <DiffTreeFileRow
                file={f}
                selected={scope.kind === 'file' && scope.path === f.path}
                onSelect={() => onSelect({ kind: 'file', path: f.path })}
              />
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function DiffTreeNodes({
  nodes,
  scope,
  onSelect,
  collapsed,
  toggle,
}: {
  nodes: TreeNode<FileDiffHeader>[];
  scope: DiffScope;
  onSelect(scope: DiffScope): void;
  collapsed: Set<string>;
  toggle(prefix: string): void;
}) {
  return (
    <>
      {nodes.map((node) => {
        if (node.kind === 'leaf') {
          const selected = scope.kind === 'file' && scope.path === node.item.path;
          return (
            <li key={node.path}>
              <DiffTreeFileRow
                file={node.item}
                name={node.name}
                treeMode
                selected={selected}
                onSelect={() => onSelect({ kind: 'file', path: node.item.path })}
              />
            </li>
          );
        }
        const expanded = !collapsed.has(node.fullPrefix);
        const selected = scope.kind === 'dir' && scope.prefix === node.fullPrefix;
        return (
          <li key={node.fullPrefix} role="treeitem" aria-expanded={expanded} className="tree-dir">
            <div className={`tree-dir-row diff-tree-dir-row${selected ? ' diff-tree-selected' : ''}`}>
              <button
                type="button"
                className="diff-tree-chevron"
                aria-label={expanded ? 'Collapse folder' : 'Expand folder'}
                onClick={() => toggle(node.fullPrefix)}
              >
                <span className={`file-chevron${expanded ? ' file-chevron-open' : ''}`}>{'›'}</span>
              </button>
              <button
                type="button"
                className="diff-tree-dir-name-btn"
                title={node.fullPrefix}
                onClick={() => onSelect({ kind: 'dir', prefix: node.fullPrefix })}
              >
                <span className="tree-dir-name">{node.name}</span>
              </button>
            </div>
            {expanded && (
              <ul role="group" className="tree-group">
                <DiffTreeNodes
                  nodes={node.children}
                  scope={scope}
                  onSelect={onSelect}
                  collapsed={collapsed}
                  toggle={toggle}
                />
              </ul>
            )}
          </li>
        );
      })}
    </>
  );
}

function DiffTreeFileRow({
  file,
  name,
  treeMode = false,
  selected,
  onSelect,
}: {
  file: FileDiffHeader;
  /** Basename supplied by the tree (tree mode renders only the segment). */
  name?: string;
  treeMode?: boolean;
  selected: boolean;
  onSelect(): void;
}) {
  const isRename = file.origPath !== null;
  const title = isRename ? `${file.origPath} → ${file.path}` : file.path;
  const display = treeMode ? (name ?? file.path) : file.path;
  return (
    <button
      type="button"
      className={`file-row diff-tree-file file-status-${file.status}${selected ? ' diff-tree-selected' : ''}`}
      title={title}
      onClick={onSelect}
    >
      <span className="file-badge mono">{BADGES[file.status]}</span>
      {isRename ? (
        <span className="file-path mono file-rename">
          {file.origPath} {'→'} {file.path}
        </span>
      ) : (
        <span className="file-path">{display}</span>
      )}
      <span className="file-counts mono">
        {file.binary ? (
          <span className="file-count-bin">bin</span>
        ) : (
          <>
            <span className="file-count-add">+{file.additions}</span>
            <span className="file-count-del">−{file.deletions}</span>
          </>
        )}
      </span>
    </button>
  );
}
