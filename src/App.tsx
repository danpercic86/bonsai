import { useCallback, useEffect, useRef, useState } from 'react';
import { StatusPanel } from './components/StatusPanel';
import { GraphCanvas } from './graph/GraphCanvas';
import { ipc } from './ipc';
import type { AppError, GraphLayout, HeadInfo, RepoInfo, StatusSnapshot, Unsubscribe } from './ipc';

function isAppError(e: unknown): e is AppError {
  return (
    typeof e === 'object' &&
    e !== null &&
    'kind' in e &&
    'message' in e &&
    typeof (e as { message: unknown }).message === 'string'
  );
}

function errorMessage(e: unknown): string {
  if (isAppError(e)) return e.message;
  if (e instanceof Error) return e.message;
  return String(e);
}

function folderName(path: string): string {
  const segments = path.split(/[\\/]/).filter(Boolean);
  return segments[segments.length - 1] ?? path;
}

function shortOid(oid: string): string {
  return oid.slice(0, 7);
}

function HeadSummary({ head }: { head: HeadInfo }) {
  if (head.unborn) {
    return (
      <span className="head-summary">
        <span className="head-branch">{head.branchName ?? '?'}</span>
        <span className="pill pill-unborn">no commits yet</span>
      </span>
    );
  }
  if (head.detached) {
    return (
      <span className="head-summary">
        <span className="head-branch">
          HEAD detached @ <span className="mono">{shortOid(head.oid)}</span>
        </span>
        <span className="pill pill-detached">detached</span>
      </span>
    );
  }
  return (
    <span className="head-summary">
      <span className="head-branch">
        {'⎇ '}
        {head.branchName ?? '?'} @ <span className="mono">{shortOid(head.oid)}</span>
      </span>
    </span>
  );
}

function isUsableRepo(info: RepoInfo): boolean {
  return info.isRepo && !info.bare;
}

export default function App() {
  const [repo, setRepo] = useState<RepoInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const [status, setStatus] = useState<StatusSnapshot | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [statusLoading, setStatusLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);

  const [graph, setGraph] = useState<GraphLayout | null>(null);
  const [graphError, setGraphError] = useState<string | null>(null);
  const [graphLoading, setGraphLoading] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);

  // Request-id last-wins guards: only the latest in-flight request may apply
  // its result (M1 contract §5 — no frontend debounce beyond this).
  const statusReqId = useRef(0);
  const graphReqId = useRef(0);

  const repoPath = repo !== null && isUsableRepo(repo) ? repo.path : null;

  const refetchStatus = useCallback(async () => {
    const id = ++statusReqId.current;
    setStatusLoading(true);
    try {
      const snapshot = await ipc.getStatus();
      if (id !== statusReqId.current) return;
      setStatus(snapshot);
      setStatusError(null);
    } catch (e) {
      if (id !== statusReqId.current) return;
      setStatusError(errorMessage(e));
    } finally {
      if (id === statusReqId.current) setStatusLoading(false);
    }
  }, []);

  const clearStatus = useCallback(() => {
    statusReqId.current += 1; // invalidate any in-flight request
    setStatus(null);
    setStatusError(null);
    setStatusLoading(false);
  }, []);

  // Refetches keep showing the previous layout until the new one arrives.
  const refetchGraph = useCallback(async () => {
    const id = ++graphReqId.current;
    setGraphLoading(true);
    try {
      const layout = await ipc.getGraph();
      if (id !== graphReqId.current) return;
      setGraph(layout);
      setGraphError(null);
      setSelectedIndex(null); // indices are only valid within one layout
    } catch (e) {
      if (id !== graphReqId.current) return;
      setGraphError(errorMessage(e));
    } finally {
      if (id === graphReqId.current) setGraphLoading(false);
    }
  }, []);

  const clearGraph = useCallback(() => {
    graphReqId.current += 1; // invalidate any in-flight request
    setGraph(null);
    setGraphError(null);
    setGraphLoading(false);
    setSelectedIndex(null);
  }, []);

  // Subscriptions only (per React rules): repo-changed events + window focus
  // both trigger a status refetch while a usable repo is open.
  useEffect(() => {
    if (repoPath === null) return;
    let cancelled = false;
    const unsubs: Unsubscribe[] = [];

    const subscribe = async () => {
      const offChanged = await ipc.onRepoChanged(() => {
        console.debug('[bonsai] repo-changed → refetch status+graph');
        void refetchStatus();
        void refetchGraph();
      });
      if (cancelled) {
        offChanged();
        return;
      }
      unsubs.push(offChanged);

      const offFocus = await ipc.onWindowFocus(() => {
        console.debug('[bonsai] window focus → refetch status+graph');
        void refetchStatus();
        void refetchGraph();
      });
      if (cancelled) {
        offFocus();
        return;
      }
      unsubs.push(offFocus);
    };
    void subscribe();

    return () => {
      cancelled = true;
      for (const unsub of unsubs) unsub();
    };
  }, [repoPath, refetchStatus, refetchGraph]);

  async function handleOpenRepository() {
    setError(null);
    setLoading(true);
    try {
      const path = await ipc.pickFolder();
      if (path === null) {
        return; // user cancelled; keep current state
      }
      const info = await ipc.openRepo(path);
      setRepo(info);
      if (isUsableRepo(info)) {
        void refetchStatus();
        void refetchGraph();
      } else {
        clearStatus();
        clearGraph();
      }
    } catch (e) {
      setError(errorMessage(e));
      setRepo(null); // a failed open leaves no repo open (matches backend)
      clearStatus();
      clearGraph();
    } finally {
      setLoading(false);
    }
  }

  // Re-runs open_repo on the current path (refreshes HEAD in the header and
  // self-heals the watcher), then refetches status.
  async function handleRefresh() {
    if (repoPath === null || refreshing) return;
    setRefreshing(true);
    try {
      const info = await ipc.openRepo(repoPath);
      setRepo(info);
      if (isUsableRepo(info)) {
        await Promise.all([refetchStatus(), refetchGraph()]);
      } else {
        clearStatus();
        clearGraph();
      }
    } catch (e) {
      setStatusError(errorMessage(e));
    } finally {
      setRefreshing(false);
    }
  }

  const repoOpen = repoPath !== null;

  return (
    <div className="app">
      <header className="header">
        <span className="app-name">Bonsai</span>
        {repoOpen && repo !== null && (
          <div className="header-repo">
            <span className="repo-name">{folderName(repo.path)}</span>
            <span className="repo-path" title={repo.path}>
              {repo.path}
            </span>
            {repo.head && <HeadSummary head={repo.head} />}
          </div>
        )}
        <button
          type="button"
          className="btn-icon"
          disabled={!repoOpen || refreshing || statusLoading || graphLoading}
          onClick={handleRefresh}
          title="Refresh"
          aria-label="Refresh"
        >
          {'⟳'}
        </button>
      </header>

      {repoOpen && repo !== null ? (
        <div className="panes">
          <aside className="sidebar">
            <div className="section-label">Branches</div>
          </aside>
          <main className="graph-pane">
            {graphError !== null && (
              <div className="error-banner graph-error-banner">{graphError}</div>
            )}
            {repo.head?.unborn ? (
              <div className="graph-pane-empty">
                <p className="pane-empty">No commits yet</p>
              </div>
            ) : graph !== null ? (
              // Loading first layout: nothing over the canvas area (no spinners).
              <GraphCanvas
                layout={graph}
                selectedIndex={selectedIndex}
                onSelect={setSelectedIndex}
              />
            ) : null}
          </main>
          <aside className="right-panel">
            <StatusPanel snapshot={status} loading={statusLoading} error={statusError} />
          </aside>
        </div>
      ) : (
        <div className="empty-state">
          <h1 className="empty-title">Bonsai</h1>
          <p className="empty-tagline">A tidy Git client</p>
          {error !== null && <div className="error-banner">{error}</div>}
          {repo !== null && !repo.isRepo && (
            <div className="error-banner">
              Not a Git repository: <span className="mono">{repo.path}</span>
            </div>
          )}
          {repo !== null && repo.isRepo && repo.bare && (
            <div className="error-banner">
              Bare repositories are not supported: <span className="mono">{repo.path}</span>
            </div>
          )}
          <button
            type="button"
            className="btn-primary"
            onClick={handleOpenRepository}
            disabled={loading}
          >
            {loading ? 'Opening…' : 'Open repository'}
          </button>
        </div>
      )}
    </div>
  );
}
