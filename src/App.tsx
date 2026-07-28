import { useCallback, useEffect, useRef, useState } from 'react';
import { CommitBox } from './components/CommitBox';
import { CommitPanel } from './components/CommitPanel';
import { Sidebar } from './components/Sidebar';
import { StatusPanel } from './components/StatusPanel';
import type { DiffSlot, WorkdirSection } from './components/StatusPanel';
import { GraphCanvas } from './graph/GraphCanvas';
import { ipc } from './ipc';
import type {
  AppError,
  BranchesSnapshot,
  CommitDiff,
  FileDiff,
  FileDiffHeader,
  GraphLayout,
  HeadInfo,
  RepoInfo,
  StatusEntry,
  StatusSnapshot,
  Unsubscribe,
} from './ipc';

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
  // Single flag for stage/unstage/commit (M3 §4.4): pessimistic UI — controls
  // disable in flight, state comes back via refetch.
  const [mutating, setMutating] = useState(false);

  const [branches, setBranches] = useState<BranchesSnapshot | null>(null);
  const [branchesError, setBranchesError] = useState<string | null>(null);
  const [branchesLoading, setBranchesLoading] = useState(false);

  // M6 §4.1: remote-op feedback. Notice = transient success/warning line under
  // the header; error = dismissible banner. Never routed to statusError.
  const [remoteNotice, setRemoteNotice] = useState<{ text: string; tone: 'ok' | 'warn' } | null>(
    null,
  );
  const [remoteError, setRemoteError] = useState<string | null>(null);
  // Which remote op is in flight — drives the per-button busy label.
  const [remoteOp, setRemoteOp] = useState<'fetch' | 'pull' | 'push' | null>(null);
  const noticeId = useRef(0);

  const [graph, setGraph] = useState<GraphLayout | null>(null);
  const [graphError, setGraphError] = useState<string | null>(null);
  const [graphLoading, setGraphLoading] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);

  // M4: commit details (mode B) + the shared per-file diff expansion slot.
  const [commitDiff, setCommitDiff] = useState<CommitDiff | null>(null);
  const [commitDiffLoading, setCommitDiffLoading] = useState(false);
  const [commitDiffError, setCommitDiffError] = useState<string | null>(null);
  const [diffSlot, setDiffSlot] = useState<DiffSlot | null>(null); // shared by both modes

  // Request-id last-wins guards: only the latest in-flight request may apply
  // its result (M1 contract §5 — no frontend debounce beyond this).
  const statusReqId = useRef(0);
  const graphReqId = useRef(0);
  const branchesReqId = useRef(0);
  const commitDiffReqId = useRef(0);
  const fileDiffReqId = useRef(0);
  // Current slot, readable from stable callbacks without re-subscribing.
  const diffSlotRef = useRef<DiffSlot | null>(null);
  diffSlotRef.current = diffSlot;

  const repoPath = repo !== null && isUsableRepo(repo) ? repo.path : null;

  /** Fetch (or re-fetch) the expanded diff for `key`; last-wins guarded. */
  const fetchDiffSlot = useCallback(async (key: string, fetcher: () => Promise<FileDiff>) => {
    const id = ++fileDiffReqId.current;
    setDiffSlot({ key, state: 'loading', diff: null, error: null });
    try {
      const diff = await fetcher();
      if (id !== fileDiffReqId.current) return;
      setDiffSlot({ key, state: 'ready', diff, error: null });
    } catch (e) {
      if (id !== fileDiffReqId.current) return;
      setDiffSlot({ key, state: 'error', diff: null, error: errorMessage(e) });
    }
  }, []);

  const collapseDiffSlot = useCallback(() => {
    fileDiffReqId.current += 1; // invalidate any in-flight fetch
    setDiffSlot(null);
  }, []);

  const refetchStatus = useCallback(async () => {
    const id = ++statusReqId.current;
    setStatusLoading(true);
    try {
      const snapshot = await ipc.getStatus();
      if (id !== statusReqId.current) return;
      setStatus(snapshot);
      setStatusError(null);
      // M4 §4.4: a new snapshot invalidates the mode-A expansion — entry gone
      // -> collapse; still present -> re-fetch (content may have changed).
      const slot = diffSlotRef.current;
      if (slot !== null && !slot.key.startsWith('commit:')) {
        const sep = slot.key.indexOf(':');
        const section = slot.key.slice(0, sep) as WorkdirSection;
        const path = slot.key.slice(sep + 1);
        const entry = snapshot[section].find((en) => en.path === path);
        if (entry === undefined) {
          collapseDiffSlot();
        } else {
          void fetchDiffSlot(slot.key, () =>
            ipc.getWorkdirFileDiff(entry.path, entry.origPath, section === 'staged'),
          );
        }
      }
    } catch (e) {
      if (id !== statusReqId.current) return;
      setStatusError(errorMessage(e));
    } finally {
      if (id === statusReqId.current) setStatusLoading(false);
    }
  }, [fetchDiffSlot, collapseDiffSlot]);

  const clearStatus = useCallback(() => {
    statusReqId.current += 1; // invalidate any in-flight request
    setStatus(null);
    setStatusError(null);
    setStatusLoading(false);
    collapseDiffSlot();
  }, [collapseDiffSlot]);

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

  const refetchBranches = useCallback(async () => {
    const id = ++branchesReqId.current;
    setBranchesLoading(true);
    try {
      const snapshot = await ipc.listBranches();
      if (id !== branchesReqId.current) return;
      setBranches(snapshot);
      setBranchesError(null);
    } catch (e) {
      if (id !== branchesReqId.current) return;
      setBranchesError(errorMessage(e));
    } finally {
      if (id === branchesReqId.current) setBranchesLoading(false);
    }
  }, []);

  const clearBranches = useCallback(() => {
    branchesReqId.current += 1; // invalidate any in-flight request
    setBranches(null);
    setBranchesError(null);
    setBranchesLoading(false);
  }, []);

  const clearGraph = useCallback(() => {
    graphReqId.current += 1; // invalidate any in-flight request
    setGraph(null);
    setGraphError(null);
    setGraphLoading(false);
    setSelectedIndex(null);
  }, []);

  // M4 §4.4: selection -> commit diff. Every selection change also resets the
  // shared expansion slot (its keys belong to the previous mode/commit).
  useEffect(() => {
    if (selectedIndex !== null && graph !== null) {
      fileDiffReqId.current += 1;
      setDiffSlot(null);
      const oid = graph.nodes[selectedIndex].id;
      const id = ++commitDiffReqId.current;
      setCommitDiff(null);
      setCommitDiffLoading(true);
      setCommitDiffError(null);
      ipc.getCommitDiff(oid).then(
        (cd) => {
          if (id !== commitDiffReqId.current) return;
          setCommitDiff(cd);
          setCommitDiffLoading(false);
        },
        (e: unknown) => {
          if (id !== commitDiffReqId.current) return;
          setCommitDiffError(errorMessage(e));
          setCommitDiffLoading(false);
        },
      );
    } else {
      commitDiffReqId.current += 1; // invalidate any in-flight commit diff
      setCommitDiff(null);
      setCommitDiffLoading(false);
      setCommitDiffError(null);
      if (diffSlotRef.current?.key.startsWith('commit:') === true) {
        fileDiffReqId.current += 1;
        setDiffSlot(null);
      }
    }
  }, [selectedIndex, graph]);

  // Esc deselects (back to mode A), except while typing in an input/textarea.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      const target = e.target as HTMLElement | null;
      if (target !== null && (target.tagName === 'TEXTAREA' || target.tagName === 'INPUT')) return;
      setSelectedIndex((cur) => (cur !== null ? null : cur));
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  // Subscriptions only (per React rules): repo-changed events + window focus
  // both trigger a status refetch while a usable repo is open.
  useEffect(() => {
    if (repoPath === null) return;
    let cancelled = false;
    const unsubs: Unsubscribe[] = [];

    const subscribe = async () => {
      const offChanged = await ipc.onRepoChanged(() => {
        console.debug('[bonsai] repo-changed → refetch status+graph+branches');
        void refetchStatus();
        void refetchGraph();
        void refetchBranches();
      });
      if (cancelled) {
        offChanged();
        return;
      }
      unsubs.push(offChanged);

      const offFocus = await ipc.onWindowFocus(() => {
        console.debug('[bonsai] window focus → refetch status+graph+branches');
        void refetchStatus();
        void refetchGraph();
        void refetchBranches();
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
  }, [repoPath, refetchStatus, refetchGraph, refetchBranches]);

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
        void refetchBranches();
      } else {
        clearStatus();
        clearGraph();
        clearBranches();
      }
    } catch (e) {
      setError(errorMessage(e));
      setRepo(null); // a failed open leaves no repo open (matches backend)
      clearStatus();
      clearGraph();
      clearBranches();
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
        await Promise.all([refetchStatus(), refetchGraph(), refetchBranches()]);
      } else {
        clearStatus();
        clearGraph();
        clearBranches();
      }
    } catch (e) {
      setStatusError(errorMessage(e));
    } finally {
      setRefreshing(false);
    }
  }

  async function handleStage(paths: string[]) {
    setMutating(true);
    try {
      await ipc.stage(paths);
      await refetchStatus();
    } catch (e) {
      setStatusError(errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleUnstage(paths: string[]) {
    setMutating(true);
    try {
      await ipc.unstage(paths);
      await refetchStatus();
    } catch (e) {
      setStatusError(errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // Commit errors are RETHROWN so CommitBox displays them inline; errors from
  // the post-commit refresh (commit already succeeded) go to statusError.
  async function handleCommit(message: string) {
    setMutating(true);
    try {
      await ipc.commit(message);
      try {
        // Post-commit refresh: openRepo updates the header HEAD oid and
        // self-heals the watcher (same as handleRefresh), then both refetches.
        if (repoPath !== null) {
          const info = await ipc.openRepo(repoPath);
          setRepo(info);
        }
        // Branches too: the commit moved the branch tip → ahead counts change.
        await Promise.all([refetchStatus(), refetchGraph(), refetchBranches()]);
      } catch (e) {
        setStatusError(errorMessage(e));
      }
    } finally {
      setMutating(false);
    }
  }

  // Errors RETHROWN so the Sidebar's create input shows them inline
  // (CommitBox pattern).
  async function handleCreateBranch(name: string) {
    setBranchesError(null);
    setMutating(true);
    try {
      await ipc.createBranch(name);
      await refetchBranches();
      void refetchGraph(); // new ref pill appears
    } finally {
      setMutating(false);
    }
  }

  async function handleCheckoutBranch(name: string) {
    setBranchesError(null);
    setMutating(true);
    try {
      await ipc.checkoutBranch(name);
      // Full refresh: openRepo updates the header HEAD and self-heals the
      // watcher (same as post-commit), then all three refetches.
      if (repoPath !== null) {
        setRepo(await ipc.openRepo(repoPath));
      }
      await Promise.all([refetchBranches(), refetchStatus(), refetchGraph()]);
    } catch (e) {
      setBranchesError(errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleDeleteBranch(name: string) {
    setBranchesError(null);
    setMutating(true);
    try {
      await ipc.deleteBranch(name);
      await Promise.all([refetchBranches(), refetchGraph()]);
    } catch (e) {
      setBranchesError(errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // ----- M6: remote operations (fetch / pull / push) -----

  /** Shows the transient notice; auto-clears after 5 s iff still current. */
  const showNotice = useCallback((text: string, tone: 'ok' | 'warn') => {
    const id = ++noticeId.current;
    setRemoteNotice({ text, tone });
    window.setTimeout(() => {
      if (id === noticeId.current) setRemoteNotice(null);
    }, 5000);
  }, []);

  const dismissNotice = useCallback(() => {
    noticeId.current += 1; // invalidate the pending auto-clear timeout
    setRemoteNotice(null);
  }, []);

  /** Common entry for every remote-op handler: clear feedback, mark busy. */
  function beginRemoteOp(op: 'fetch' | 'pull' | 'push') {
    setRemoteError(null);
    dismissNotice();
    setMutating(true);
    setRemoteOp(op);
  }

  function endRemoteOp() {
    setMutating(false);
    setRemoteOp(null);
  }

  async function handleFetch() {
    beginRemoteOp('fetch');
    try {
      const res = await ipc.fetch();
      const n = res.remotes.length;
      const k = res.remotes.reduce((sum, r) => sum + r.updatedRefs, 0);
      showNotice(
        `Fetched ${n} remote${n === 1 ? '' : 's'}` +
          (k > 0 ? ` — ${k} ref${k === 1 ? '' : 's'} updated` : ''),
        'ok',
      );
      await Promise.all([refetchBranches(), refetchGraph()]); // status unaffected
    } catch (e) {
      setRemoteError(errorMessage(e));
    } finally {
      endRemoteOp();
    }
  }

  async function handlePull() {
    beginRemoteOp('pull');
    try {
      const res = await ipc.pull();
      switch (res.kind) {
        case 'upToDate':
          showNotice('Already up to date', 'ok');
          break;
        case 'fastForwarded':
          showNotice(`Fast-forwarded ${res.branch} to ${shortOid(res.to)}`, 'ok');
          break;
        case 'wouldNotFastForward':
          showNotice(
            `Cannot fast-forward: '${res.branch}' has ${res.ahead} local commit(s) not on ` +
              'upstream. Bonsai v1 does not merge — push your commits or reconcile via the CLI.',
            'warn',
          );
          break;
      }
      // Full refresh: the branch tip may have moved (same as post-checkout).
      if (repoPath !== null) {
        setRepo(await ipc.openRepo(repoPath));
      }
      await Promise.all([refetchBranches(), refetchStatus(), refetchGraph()]);
    } catch (e) {
      setRemoteError(errorMessage(e));
    } finally {
      endRemoteOp();
    }
  }

  async function handlePush() {
    beginRemoteOp('push');
    try {
      const res = await ipc.push();
      if (res.kind === 'upToDate') {
        showNotice('Already up to date', 'ok');
      } else {
        showNotice(
          `Pushed ${res.branch} → ${res.remote}/${res.branch}` +
            (res.setUpstream ? ' (upstream set)' : ''),
          'ok',
        );
      }
      await Promise.all([refetchBranches(), refetchGraph()]); // ahead badge -> 0
    } catch (e) {
      setRemoteError(errorMessage(e));
    } finally {
      endRemoteOp();
    }
  }

  // Mode-A accordion toggle: staged rows -> staged diff; unstaged/untracked ->
  // unstaged diff (M4 §4.2).
  function handleToggleWorkdirDiff(section: WorkdirSection, entry: StatusEntry) {
    const key = `${section}:${entry.path}`;
    if (diffSlotRef.current?.key === key) {
      collapseDiffSlot();
      return;
    }
    void fetchDiffSlot(key, () =>
      ipc.getWorkdirFileDiff(entry.path, entry.origPath, section === 'staged'),
    );
  }

  // Mode-B accordion toggle: hunks for one file of the selected commit.
  function handleToggleCommitDiff(file: FileDiffHeader) {
    if (selectedIndex === null || graph === null) return;
    const oid = graph.nodes[selectedIndex].id;
    const key = `commit:${file.path}`;
    if (diffSlotRef.current?.key === key) {
      collapseDiffSlot();
      return;
    }
    void fetchDiffSlot(key, () => ipc.getCommitFileDiff(oid, file.path, file.origPath));
  }

  // Parent short-oid clicked: GraphNode.parents are node indices, ordinal-
  // matched to CommitDetails.parents (both first-parent-first).
  function handleSelectParent(parentOrdinal: number) {
    if (selectedIndex === null || graph === null) return;
    const parentIndex = graph.nodes[selectedIndex].parents[parentOrdinal];
    if (parentIndex !== undefined) setSelectedIndex(parentIndex);
  }

  const repoOpen = repoPath !== null;
  // Convenience gating only — the backend guards detached/unborn itself (§4.2).
  const canPullPush = repo?.head != null && !repo.head.detached && !repo.head.unborn;
  const headBranch = branches?.local.find((b) => b.isHead) ?? null;
  const pushTitle =
    headBranch === null
      ? 'Push'
      : headBranch.upstream !== null
        ? `Push ${headBranch.name} to ${headBranch.upstream}`
        : `Push ${headBranch.name} to origin/${headBranch.name} and set upstream`;

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
        <div className="header-toolbar">
          <button
            type="button"
            className="toolbar-btn"
            disabled={!repoOpen || refreshing || mutating}
            onClick={() => void handleFetch()}
            title="Fetch all remotes"
          >
            {remoteOp === 'fetch' ? 'Fetching…' : '↓ Fetch'}
          </button>
          <button
            type="button"
            className="toolbar-btn"
            disabled={!repoOpen || refreshing || mutating || !canPullPush}
            onClick={() => void handlePull()}
            title="Pull (fast-forward only)"
          >
            {remoteOp === 'pull' ? 'Pulling…' : '⇣ Pull'}
          </button>
          <button
            type="button"
            className="toolbar-btn"
            disabled={!repoOpen || refreshing || mutating || !canPullPush}
            onClick={() => void handlePush()}
            title={pushTitle}
          >
            {remoteOp === 'push' ? 'Pushing…' : '↑ Push'}
          </button>
          <button
            type="button"
            className="btn-icon"
            disabled={!repoOpen || refreshing || statusLoading || graphLoading || mutating}
            onClick={handleRefresh}
            title="Refresh"
            aria-label="Refresh"
          >
            {'⟳'}
          </button>
        </div>
      </header>

      {repoOpen && remoteError !== null && (
        <div className="error-banner error-banner-dismissible remote-error-banner">
          <span className="error-banner-text">{remoteError}</span>
          <button
            type="button"
            className="error-dismiss"
            onClick={() => setRemoteError(null)}
            aria-label="Dismiss"
          >
            {'✕'}
          </button>
        </div>
      )}
      {repoOpen && remoteError === null && remoteNotice !== null && (
        <div className={`remote-notice${remoteNotice.tone === 'warn' ? ' remote-notice-warn' : ''}`}>
          <span className="remote-notice-text">{remoteNotice.text}</span>
          <button
            type="button"
            className="notice-dismiss"
            onClick={dismissNotice}
            aria-label="Dismiss"
          >
            {'✕'}
          </button>
        </div>
      )}

      {repoOpen && repo !== null ? (
        <div className="panes">
          <Sidebar
            data={branches}
            loading={branchesLoading}
            error={branchesError}
            onDismissError={() => setBranchesError(null)}
            busy={mutating}
            onCheckout={(name) => void handleCheckoutBranch(name)}
            onDelete={(name) => void handleDeleteBranch(name)}
            onCreateBranch={handleCreateBranch}
          />
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
            {selectedIndex !== null && graph !== null ? (
              <CommitPanel
                node={graph.nodes[selectedIndex]}
                data={commitDiff}
                loading={commitDiffLoading}
                error={commitDiffError}
                diffSlot={diffSlot}
                onToggleDiff={handleToggleCommitDiff}
                onSelectParent={handleSelectParent}
                onClose={() => setSelectedIndex(null)}
              />
            ) : (
              <>
                <StatusPanel
                  snapshot={status}
                  loading={statusLoading}
                  error={statusError}
                  busy={mutating}
                  diffSlot={diffSlot}
                  onStage={(paths) => void handleStage(paths)}
                  onUnstage={(paths) => void handleUnstage(paths)}
                  onToggleDiff={handleToggleWorkdirDiff}
                />
                <CommitBox
                  stagedCount={status?.staged.length ?? 0}
                  busy={mutating}
                  onCommit={handleCommit}
                />
              </>
            )}
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
