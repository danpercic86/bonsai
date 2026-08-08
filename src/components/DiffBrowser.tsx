import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from 'react';
import { ipc } from '../ipc';
import type { FileDiff, FileDiffHeader, FileStatus, ListView } from '../ipc';
import { errorMessage } from '../utils/errors';
import { buildPathTree, flattenTreeLeaves } from '../utils/pathTree';
import { SkeletonRows } from './CommitPanel';
import type { DiffScope } from './DiffFileTree';
import { DiffView } from './DiffView';

// P11g §6 (revised by P11g-revision A + D): the all-files diff view. NO longer
// owns an internal file tree — it is now just a header + a vertical scroll of
// stacked per-file DiffCards, filling the graph/main pane. The scope navigator
// lives in the right-hand ComparePanel/CommitPanel (shared DiffFileTree), which
// drives the `scope` prop lifted to RepoWorkspace.
//
// DiffBrowser owns its per-file fetching (a bounded concurrency queue + a
// component-local cache), a deliberate, localized exception to App's "diffSlot
// owns all diff fetching" pattern (§8.4). Change D: the loader no longer depends
// on IntersectionObserver/visibility — it eagerly enqueues the current scope's
// non-binary files on mount + every scope change.

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
  /** Header list for the active source (RepoWorkspace: CommitDiff/CompareDiff files). */
  files: FileDiffHeader[];
  /** P11g-rev §1: current scope (lifted to RepoWorkspace). Drives which cards render. */
  scope: DiffScope;
  /** P11g-rev §2: current list view (lifted to RepoWorkspace). In 'tree' mode the
   *  stacked card order mirrors the Changes-panel tree; in 'flat' it is raw order. */
  listView: ListView;
  onClose(): void;
}

export function DiffBrowser({ repoId, source, files, scope, listView, onClose }: DiffBrowserProps) {
  // P17d §0.4/§5.2: File/Diff view toggle (locked "toggle everywhere"). Local
  // state — the browser is its own surface and does not share
  // RepoWorkspace.diffViewMode. `diff` (3-context hunks) is the default/current
  // behavior; `file` fetches whole-file (fullContext) diffs. The mode is folded
  // into the cache key so switching modes refetches instead of serving a
  // stale-context payload; a modeRef lets the stable pump/enqueue callbacks read
  // it without churning.
  const [mode, setMode] = useState<'diff' | 'file'>('diff');
  const modeRef = useRef(mode);
  modeRef.current = mode;

  // §6.4: component-local cache + a bounded fetch queue. The cache is a ref so
  // async resolutions mutate it in place; `bump` forces the re-render that shows
  // the new content. Reading the ref during render is safe — every mutation is
  // paired with a bump. Keyed `${oid}:${path}:${mode}` (P17d) so diff-vs-file
  // context variants never collide.
  const cacheRef = useRef<Map<string, CardState>>(new Map());
  const [, bump] = useReducer((n: number) => n + 1, 0);
  const queueRef = useRef<string[]>([]);
  const inFlightRef = useRef(0);
  // §6.4: set true on unmount so in-flight `.finally` re-pumps short-circuit —
  // no fetches writing into a discarded cache after the browser closes mid-load.
  const cancelledRef = useRef(false);

  // Collapse state is component-local + ephemeral (same lifetime as the cache):
  // keyed by file path, an EMPTY set means all files are expanded (the default).
  // Collapsing a card UNMOUNTS its `.diff-card-body` (see DiffCard) rather than
  // hiding it with CSS, so huge DiffViews are removed from the DOM entirely.
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());
  const toggleCollapsed = useCallback((path: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }, []);

  // Read the latest props inside the stable pump/enqueue callbacks without
  // making them churn.
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
      const fullContext = modeRef.current === 'file';
      const key = `${src.oid}:${path}:${modeRef.current}`;
      const entry = cacheRef.current.get(key);
      if (entry === undefined || entry.state !== 'idle') continue; // superseded
      const header = filesRef.current.find((f) => f.path === path);
      if (header === undefined) continue;
      cacheRef.current.set(key, { state: 'loading' });
      inFlightRef.current += 1;
      bump();
      const request =
        src.mode === 'commit'
          ? ipc.getCommitFileDiff(
              repoIdRef.current,
              src.oid,
              header.path,
              header.origPath,
              fullContext,
              false, // P61a: intraline emphasis is an overlay-only toggle
            )
          : ipc.compareWithHeadFileDiff(
              repoIdRef.current,
              src.oid,
              header.path,
              header.origPath,
              fullContext,
              false, // P61a: intraline emphasis is an overlay-only toggle
            );
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

  // §6.4: queue a file once (idempotent per cache key). Binary files never reach
  // here (they are filtered out by the enqueue effect below).
  const enqueue = useCallback(
    (path: string) => {
      if (cancelledRef.current) return;
      const key = `${sourceRef.current.oid}:${path}:${modeRef.current}`;
      if (cacheRef.current.has(key)) return; // already queued/loading/ready/error
      cacheRef.current.set(key, { state: 'idle' });
      queueRef.current.push(path);
      pump();
    },
    [pump],
  );

  const retry = useCallback(
    (path: string) => {
      cacheRef.current.delete(`${sourceRef.current.oid}:${path}:${modeRef.current}`);
      enqueue(path);
    },
    [enqueue],
  );

  // §6.4: on unmount, cancel so any in-flight fetch's `.finally` re-pump stops
  // draining the queue into a discarded cache. Reset to false on (re)mount so a
  // React.StrictMode simulated unmount→remount (dev only) does not leave the flag
  // stuck true — that would permanently short-circuit pump()/enqueue().
  useEffect(() => {
    cancelledRef.current = false;
    return () => {
      cancelledRef.current = true;
    };
  }, []);

  // P11g-rev §2: order the header list to match the Changes panel. In 'tree' view
  // the panel renders buildPathTree(files) (dirs-first, deterministic alpha), so
  // we flatten that tree to its visual leaf order; in 'flat' view the panel maps
  // `files` directly, so raw order already matches.
  const orderedFiles = useMemo(
    () =>
      listView === 'tree' ? flattenTreeLeaves(buildPathTree(files, (f) => f.path)) : files,
    [listView, files],
  );

  // §6.3: filter the ordered header list to the current scope WITHOUT refetching —
  // the cache persists across scope changes for the browser's lifetime. Scope now
  // comes from props (lifted to RepoWorkspace); order mirrors the Changes-panel
  // tree in tree view.
  const visibleFiles = useMemo(() => {
    if (scope.kind === 'root') return orderedFiles;
    if (scope.kind === 'dir') {
      const prefix = scope.prefix;
      return orderedFiles.filter((f) => f.path === prefix || f.path.startsWith(`${prefix}/`));
    }
    return orderedFiles.filter((f) => f.path === scope.path);
  }, [orderedFiles, scope]);

  // Change D: no visibility events. Eagerly enqueue every non-binary file in the
  // current scope (top-to-bottom order == visibleFiles order == the Changes-panel
  // order: tree pre-order in 'tree' view, raw file order in 'flat' view),
  // so the first cards paint first. `enqueue` is idempotent per cache key, so
  // re-running on scope change never double-fetches already-loaded/queued files;
  // narrowing to a folder/file simply enqueues fewer.
  //
  // Tradeoff: at root scope this issues one bounded (max 4 in-flight) fetch per
  // file. The per-file MAX_FILE_DIFF_LINES = 5000 cap keeps each response cheap,
  // and scoping to a folder/file naturally reduces load. Acceptable at
  // desktop-repo scale (the user's 125-file case is fine); a batched command
  // remains a future additive optimization, out of scope here.
  //
  // The trailing pump() resumes a drain that stalled across a React.StrictMode
  // remount: the queue may already hold `idle` entries pushed during the first
  // pass, so enqueue() short-circuits on them (cache already has the key) and
  // never re-pumps. pump() is a stable useCallback([]) — calling it here is safe
  // and idempotent.
  //
  // P17d: `mode` is a dependency so toggling File/Diff re-enqueues every visible
  // file under its new `${oid}:${path}:${mode}` cache key — a genuine refetch
  // (whole-file vs 3-context payloads differ). Old-mode entries stay cached (and
  // are simply looked up under the other key), so toggling back is instant.
  useEffect(() => {
    for (const f of visibleFiles) {
      if (!f.binary) enqueue(f.path);
    }
    pump(); // resume any drain stalled across a StrictMode remount
  }, [visibleFiles, enqueue, pump, mode]);

  // The collapse-all/expand-all toggle operates on the CURRENT scope's visible
  // files: it reads as "all collapsed" only when every visible file is collapsed,
  // and its click flips every visible path in one setState.
  const allCollapsed =
    visibleFiles.length > 0 && visibleFiles.every((f) => collapsed.has(f.path));
  const toggleAll = useCallback(() => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (allCollapsed) {
        for (const f of visibleFiles) next.delete(f.path); // expand all visible
      } else {
        for (const f of visibleFiles) next.add(f.path); // collapse all visible
      }
      return next;
    });
  }, [allCollapsed, visibleFiles]);

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
        {visibleFiles.length > 0 && (
          <button
            type="button"
            className="section-action diff-browser-collapse-all"
            title={allCollapsed ? 'Expand all' : 'Collapse all'}
            onClick={toggleAll}
          >
            {allCollapsed ? 'Expand all' : 'Collapse all'}
          </button>
        )}
        <div className="diff-view-toggle" role="group" aria-label="View mode">
          <button
            type="button"
            className={mode === 'file' ? 'active' : ''}
            aria-pressed={mode === 'file'}
            onClick={() => setMode('file')}
          >
            File
          </button>
          <button
            type="button"
            className={mode === 'diff' ? 'active' : ''}
            aria-pressed={mode === 'diff'}
            onClick={() => setMode('diff')}
          >
            Diff
          </button>
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
      <div className="diff-browser-scroll">
        {visibleFiles.length === 0 ? (
          <div className="pane-empty">No changes</div>
        ) : (
          visibleFiles.map((f) => (
            <DiffCard
              key={f.path}
              header={f}
              entry={f.binary ? undefined : cacheRef.current.get(`${source.oid}:${f.path}:${mode}`)}
              viewMode={mode}
              onRetry={retry}
              collapsed={collapsed.has(f.path)}
              onToggle={toggleCollapsed}
            />
          ))
        )}
      </div>
    </div>
  );
}

// ---------- per-file card (§6.4) ----------

function DiffCard({
  header,
  entry,
  viewMode,
  onRetry,
  collapsed,
  onToggle,
}: {
  header: FileDiffHeader;
  /** undefined for binary headers (never fetched). */
  entry: CardState | undefined;
  /** P17d: File/Diff render mode, forwarded read-only to the card's DiffView. */
  viewMode: 'diff' | 'file';
  onRetry(path: string): void;
  collapsed: boolean;
  onToggle(path: string): void;
}) {
  const isRename = header.origPath !== null;
  const title = isRename ? `${header.origPath} → ${header.path}` : header.path;

  return (
    <div className={`diff-card${collapsed ? ' diff-card-collapsed' : ''}`}>
      <button
        type="button"
        className={`diff-card-header file-status-${header.status}`}
        title={title}
        aria-expanded={!collapsed}
        onClick={() => onToggle(header.path)}
      >
        <span className={`file-chevron${collapsed ? '' : ' file-chevron-open'}`} aria-hidden="true">
          {'›'}
        </span>
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
      </button>
      {/* Collapsing UNMOUNTS the body (not display:none) so a giant DiffView is
          removed from the DOM entirely — the whole point of this feature. */}
      {!collapsed && (
        <div className="diff-card-body">
          <DiffCardBody header={header} entry={entry} viewMode={viewMode} onRetry={onRetry} />
        </div>
      )}
    </div>
  );
}

function DiffCardBody({
  header,
  entry,
  viewMode,
  onRetry,
}: {
  header: FileDiffHeader;
  entry: CardState | undefined;
  viewMode: 'diff' | 'file';
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
  // P17d: read-only (no stageable/onStageLines/onStageHunk) — only viewMode.
  return <DiffView diff={entry.diff} viewMode={viewMode} />;
}
