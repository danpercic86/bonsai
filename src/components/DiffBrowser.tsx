import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from 'react';
import { ipc } from '../ipc';
import type { FileDiff, FileDiffHeader, FileStatus, ListView } from '../ipc';
import { buildPathTree } from '../utils/pathTree';
import type { TreeNode } from '../utils/pathTree';
import { errorMessage } from '../utils/errors';
import { SkeletonRows } from './CommitPanel';
import { DiffView } from './DiffView';

// P11g §6: Azure-DevOps-style all-files diff view. A file tree on the left
// filters a right-hand vertical scroll of stacked per-file diffs. Replaces the
// single-file DiffOverlay interaction for compare + commit-selected modes only
// (the working-dir StatusPanel keeps its own diffSlot/DiffOverlay unchanged).
//
// DiffBrowser owns its per-file fetching (IntersectionObserver + a bounded
// concurrency queue + a component-local cache), a deliberate, localized
// exception to App's "diffSlot owns all diff fetching" pattern (§8.4).

const BADGES: Record<FileStatus, string> = {
  added: 'A',
  modified: 'M',
  deleted: 'D',
  renamed: 'R',
  typechange: 'T',
  untracked: 'U',
  conflicted: 'C',
};

/** §6.4: at most 4 per-file hunk fetches in flight; a small queue drains as
 *  each resolves. Keeps IPC/memory proportional to what the user looks at. */
const MAX_CONCURRENCY = 4;

// P11g §6.2: the left-tree selection. `dir.prefix` is a TreeDir.fullPrefix
// (no trailing '/'); `file.path` is a FileDiffHeader.path.
export type DiffScope =
  | { kind: 'root' }
  | { kind: 'dir'; prefix: string }
  | { kind: 'file'; path: string };

/** Per-card fetch state, cached by `${source.oid}:${header.path}`. `idle` =
 *  queued but not yet fetched (renders the same skeleton as `loading`). */
type CardState =
  | { state: 'idle' }
  | { state: 'loading' }
  | { state: 'ready'; diff: FileDiff }
  | { state: 'error'; error: string };

export interface DiffBrowserProps {
  repoId: string;
  /** Which commands to call + how to label the header (§6.2). */
  source:
    | { mode: 'commit'; oid: string; title: string }
    | { mode: 'compare'; oid: string; fromLabel: string; toLabel: string };
  /** Header list (already fetched by RepoWorkspace: CommitDiff/CompareDiff files). */
  files: FileDiffHeader[];
  listView: ListView;
  /** §6.5: initial scope — {kind:'file',path} when a specific row was clicked;
   *  defaults to {kind:'root'} (all files). */
  initialScope?: DiffScope;
  onClose(): void;
}

export function DiffBrowser({
  repoId,
  source,
  files,
  listView,
  initialScope,
  onClose,
}: DiffBrowserProps) {
  const [scope, setScope] = useState<DiffScope>(initialScope ?? { kind: 'root' });

  // §6.4: component-local cache + a bounded fetch queue. The cache is a ref so
  // async resolutions mutate it in place; `bump` forces the re-render that shows
  // the new content. Reading the ref during render is safe — every mutation is
  // paired with a bump.
  const cacheRef = useRef<Map<string, CardState>>(new Map());
  const [, bump] = useReducer((n: number) => n + 1, 0);
  const queueRef = useRef<string[]>([]);
  const inFlightRef = useRef(0);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [observer, setObserver] = useState<IntersectionObserver | null>(null);
  // §6.4: set true on unmount so in-flight `.finally` re-pumps and observer
  // callbacks short-circuit — no fetches writing into a discarded cache after
  // the browser closes mid-load.
  const cancelledRef = useRef(false);

  // Read the latest props inside the stable pump/enqueue callbacks without
  // making them churn (which would re-arm the observer effect).
  const sourceRef = useRef(source);
  sourceRef.current = source;
  const filesRef = useRef(files);
  filesRef.current = files;
  const repoIdRef = useRef(repoId);
  repoIdRef.current = repoId;

  // §6.4: drain the queue up to the concurrency cap. Fetches only entries still
  // marked `idle`; re-calls itself as each settles so the queue keeps flowing.
  const pump = useCallback(() => {
    if (cancelledRef.current) return;
    while (inFlightRef.current < MAX_CONCURRENCY && queueRef.current.length > 0) {
      const path = queueRef.current.shift();
      if (path === undefined) break;
      const src = sourceRef.current;
      const key = `${src.oid}:${path}`;
      const entry = cacheRef.current.get(key);
      if (entry === undefined || entry.state !== 'idle') continue; // superseded
      const header = filesRef.current.find((f) => f.path === path);
      if (header === undefined) continue;
      cacheRef.current.set(key, { state: 'loading' });
      inFlightRef.current += 1;
      bump();
      const request =
        src.mode === 'commit'
          ? ipc.getCommitFileDiff(repoIdRef.current, src.oid, header.path, header.origPath)
          : ipc.compareWithHeadFileDiff(repoIdRef.current, src.oid, header.path, header.origPath);
      void request
        .then(
          (diff) => {
            cacheRef.current.set(key, { state: 'ready', diff });
          },
          (e: unknown) => {
            cacheRef.current.set(key, { state: 'error', error: errorMessage(e) });
          },
        )
        .finally(() => {
          inFlightRef.current -= 1;
          bump();
          pump();
        });
    }
  }, []);

  // §6.4: a card entered view → queue it once (idempotent per cache key). Binary
  // files never reach here (their cards are not observed).
  const enqueue = useCallback(
    (path: string) => {
      if (cancelledRef.current) return;
      const key = `${sourceRef.current.oid}:${path}`;
      if (cacheRef.current.has(key)) return; // already queued/loading/ready/error
      cacheRef.current.set(key, { state: 'idle' });
      queueRef.current.push(path);
      pump();
    },
    [pump],
  );

  const retry = useCallback(
    (path: string) => {
      cacheRef.current.delete(`${sourceRef.current.oid}:${path}`);
      enqueue(path);
    },
    [enqueue],
  );

  // §6.4: one IntersectionObserver rooted on the scroll container, ~200px
  // rootMargin so a card starts loading just before it scrolls into view.
  useEffect(() => {
    const root = scrollRef.current;
    if (root === null) return;
    const obs = new IntersectionObserver(
      (entries) => {
        for (const en of entries) {
          if (!en.isIntersecting) continue;
          const path = en.target.getAttribute('data-diff-path');
          if (path !== null) enqueue(path);
        }
      },
      { root, rootMargin: '200px 0px' },
    );
    setObserver(obs);
    return () => obs.disconnect();
  }, [enqueue]);

  // §6.4: on unmount, cancel so any in-flight fetch's `.finally` re-pump and
  // late observer callbacks stop draining the queue into a discarded cache.
  useEffect(
    () => () => {
      cancelledRef.current = true;
    },
    [],
  );

  // §6.3: filter the header list to the current scope WITHOUT refetching — the
  // cache persists across scope changes for the browser's lifetime.
  const visibleFiles = useMemo(() => {
    if (scope.kind === 'root') return files;
    if (scope.kind === 'dir') {
      const prefix = scope.prefix;
      return files.filter((f) => f.path === prefix || f.path.startsWith(`${prefix}/`));
    }
    return files.filter((f) => f.path === scope.path);
  }, [files, scope]);

  return (
    <div className="diff-browser" role="region" aria-label="All changes">
      <div className="diff-browser-header">
        <div className="diff-browser-title mono">
          {source.mode === 'compare' ? (
            <>
              <span className="diff-browser-endpoint">{source.fromLabel}</span>
              <span className="compare-arrow" aria-hidden="true">
                {' → '}
              </span>
              <span className="diff-browser-endpoint">{source.toLabel}</span>
            </>
          ) : (
            <span className="diff-browser-endpoint">{source.title}</span>
          )}
        </div>
        <button
          type="button"
          className="btn-icon diff-browser-close"
          aria-label="Close all-changes view"
          title="Close (Esc)"
          onClick={onClose}
        >
          {'×'}
        </button>
      </div>
      <div className="diff-browser-body">
        <div className="diff-browser-tree">
          <DiffFileTree files={files} listView={listView} scope={scope} onSelect={setScope} />
        </div>
        <div className="diff-browser-scroll" ref={scrollRef}>
          {visibleFiles.length === 0 ? (
            <div className="pane-empty">No changes</div>
          ) : (
            visibleFiles.map((f) => (
              <DiffCard
                key={f.path}
                header={f}
                entry={f.binary ? undefined : cacheRef.current.get(`${source.oid}:${f.path}`)}
                observer={observer}
                onRetry={retry}
              />
            ))
          )}
        </div>
      </div>
    </div>
  );
}

// ---------- per-file card (§6.4) ----------

function DiffCard({
  header,
  entry,
  observer,
  onRetry,
}: {
  header: FileDiffHeader;
  /** undefined for binary headers (never fetched) and not-yet-observed files. */
  entry: CardState | undefined;
  observer: IntersectionObserver | null;
  onRetry(path: string): void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  // Non-binary cards register with the observer; binary cards render their
  // placeholder without ever fetching (§6.4 binary short-circuit).
  useEffect(() => {
    const el = ref.current;
    if (el === null || observer === null || header.binary) return;
    observer.observe(el);
    return () => observer.unobserve(el);
  }, [observer, header.binary]);

  const isRename = header.origPath !== null;
  const title = isRename ? `${header.origPath} → ${header.path}` : header.path;

  return (
    <div ref={ref} data-diff-path={header.path} className="diff-card">
      <div className={`diff-card-header file-status-${header.status}`} title={title}>
        <span className="file-badge mono">{BADGES[header.status]}</span>
        {isRename ? (
          <span className="diff-card-path mono file-rename">
            {header.origPath} {'→'} {header.path}
          </span>
        ) : (
          <span className="diff-card-path mono">{header.path}</span>
        )}
        <span className="file-counts mono">
          {header.binary ? (
            <span className="file-count-bin">bin</span>
          ) : (
            <>
              <span className="file-count-add">+{header.additions}</span>
              <span className="file-count-del">−{header.deletions}</span>
            </>
          )}
        </span>
      </div>
      <div className="diff-card-body">
        <DiffCardBody header={header} entry={entry} onRetry={onRetry} />
      </div>
    </div>
  );
}

function DiffCardBody({
  header,
  entry,
  onRetry,
}: {
  header: FileDiffHeader;
  entry: CardState | undefined;
  onRetry(path: string): void;
}) {
  if (header.binary) return <div className="diff-placeholder">Binary file</div>;
  if (entry === undefined || entry.state === 'idle' || entry.state === 'loading') {
    return (
      <div className="diff-card-loading skeleton-group" aria-hidden="true">
        <SkeletonRows />
      </div>
    );
  }
  if (entry.state === 'error') {
    return (
      <div className="error-banner error-banner-dismissible diff-slot-error" role="alert">
        <span className="error-banner-text">{entry.error}</span>
        <button type="button" className="section-action" onClick={() => onRetry(header.path)}>
          Retry
        </button>
      </div>
    );
  }
  // §6.4: DiffView already handles binary / tooLarge / empty-hunks placeholders.
  return <DiffView diff={entry.diff} />;
}

// ---------- left tree (§6.2 DiffFileTree) ----------

// Purpose-built single-click tree over buildPathTree data (§8.3): the shared
// Tree binds dir-click to collapse, so it cannot express single-click
// select-folder. This reuses buildPathTree for STRUCTURE only.

function DiffFileTree({
  files,
  listView,
  scope,
  onSelect,
}: {
  files: FileDiffHeader[];
  listView: ListView;
  scope: DiffScope;
  onSelect(scope: DiffScope): void;
}) {
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
