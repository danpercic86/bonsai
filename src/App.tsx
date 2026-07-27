import { useState } from 'react';
import { ipc } from './ipc';
import type { AppError, HeadInfo, RepoInfo } from './ipc';

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

export default function App() {
  const [repo, setRepo] = useState<RepoInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

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
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setLoading(false);
    }
  }

  const repoOpen = repo !== null && repo.isRepo;

  return (
    <div className="app">
      <header className="header">
        <span className="app-name">Bonsai</span>
        {repoOpen && (
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
          disabled
          title="Refresh (coming soon)"
          aria-label="Refresh"
        >
          {'⟳'}
        </button>
      </header>

      {repoOpen ? (
        <div className="panes">
          <aside className="sidebar">
            <div className="section-label">Branches</div>
          </aside>
          <main className="graph-pane">
            {repo.head?.unborn ? (
              <p className="pane-empty">No commits yet</p>
            ) : (
              <p className="pane-empty">Commit graph</p>
            )}
          </main>
          <aside className="right-panel">
            <div className="section-label">Status</div>
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
