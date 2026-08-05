// P29c §8: the read-only repo-health overlay ("📊 Health"). Fetches the full
// RepoHealth payload on open / repoId change / repo-changed, and renders the
// four Section<T> envelopes INDEPENDENTLY: a section with `error` shows an
// inline error row while its siblings render data. Rust owns all metrics; this
// component only renders + formats. No mutations of any kind.

import { useCallback, useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { ipc } from '../ipc';
import type {
  BranchesSection,
  RepoHealth,
  RepoOpState,
  Section,
  StatsSection,
  StructureSection,
  Unsubscribe,
  WorkingStateSection,
} from '../ipc';
import { errorMessage } from '../utils/errors';
import { formatBytes } from '../utils/format';

export interface RepoHealthPanelProps {
  open: boolean;
  onClose(): void;
  repoId: string;
}

/** "generated 12s ago" caption text from an epoch-seconds stamp. */
function relativeTime(epochSec: number, nowMs: number): string {
  const secs = Math.max(0, Math.floor(nowMs / 1000) - epochSec);
  if (secs < 5) return 'just now';
  if (secs < 60) return `${secs}s ago`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  return `${Math.floor(mins / 60)}h ago`;
}

/** Human label for an in-progress operation (RepoOpState, reused verbatim). */
function opStateLabel(op: RepoOpState): string | null {
  switch (op.kind) {
    case 'none':
      return null;
    case 'merge':
      return `merge in progress (${op.incoming})`;
    case 'rebase':
      return `rebase in progress (${op.currentStep}/${op.totalSteps})`;
    case 'cherryPick':
      return 'cherry-pick in progress';
    case 'revert':
      return 'revert in progress';
    case 'bisect':
      return op.firstBad === null
        ? `bisect in progress (${op.revisionsRemaining} left)`
        : 'bisect found first bad commit';
  }
}

/** A number that may be a floor: capped values render `≥ n` + a muted chip. */
function CappedNum({ value, capped }: { value: number | string; capped: boolean }) {
  return (
    <>
      {capped ? `≥ ${value}` : value}
      {capped && <span className="asset-chip asset-chip-muted">capped</span>}
    </>
  );
}

/** Amber chip when the count signals a warning, green "ok" chip otherwise. */
function WarnCount({ value, label }: { value: number; label: string }) {
  if (value > 0) {
    return (
      <span className="asset-chip asset-chip-drifted">
        {value} {label}
      </span>
    );
  }
  return <span className="asset-chip asset-chip-sync">0 {label}</span>;
}

/** One label/value row in a section's metric grid. */
function Metric({
  label,
  children,
  title,
}: {
  label: string;
  children: ReactNode;
  title?: string;
}) {
  return (
    <>
      <span className="health-metric-label" title={title}>
        {label}
      </span>
      <span className="health-metric-value">{children}</span>
    </>
  );
}

/** Per-section wrapper (contract D4/§8): renders the title always, then a
 *  skeleton while loading, an inline error row on `section.error`, or the
 *  section body — never hiding sibling sections. */
function HealthSection<T>({
  title,
  section,
  loading,
  render,
}: {
  title: string;
  section: Section<T> | null;
  loading: boolean;
  render(data: T): ReactNode;
}) {
  return (
    <section className="settings-section">
      <div className="health-section-head">
        <h3 className="settings-section-title">{title}</h3>
        {section !== null && (
          <span className="health-elapsed">{section.elapsedMs} ms</span>
        )}
      </div>
      {section === null ? (
        <p className="settings-ai-status">{loading ? 'Loading…' : '—'}</p>
      ) : section.error !== null ? (
        <div className="error-banner health-section-error" role="alert">
          {section.error}
        </div>
      ) : section.data !== null ? (
        render(section.data)
      ) : (
        // Defensive: the contract guarantees exactly one of data/error is set.
        <p className="settings-ai-status">No data.</p>
      )}
    </section>
  );
}

function StatsBody({ data }: { data: StatsSection }) {
  return (
    <>
      <div className="health-grid">
        <Metric label="Commits (HEAD)">
          <CappedNum value={data.commitCount} capped={data.commitCountCapped} />
        </Metric>
        <Metric label="Commits (30d)">{data.commitsLast30d}</Metric>
        <Metric label="Authors (30d)">{data.authorsLast30d}</Metric>
        <Metric label="Authors (total)">
          <CappedNum value={data.authorsTotal} capped={data.commitCountCapped} />
        </Metric>
        <Metric label="Objects">
          <CappedNum value={data.objectCount} capped={data.objectScanCapped} />
        </Metric>
        <Metric
          label="Working tree"
          title="Excludes gitignored files (node_modules, target, dist, …)"
        >
          <CappedNum
            value={`${data.workdirFileCount} files, ${formatBytes(data.workdirBytes)}`}
            capped={data.workdirScanCapped}
          />
        </Metric>
        <Metric label="Large files (≥ 10 MiB)">
          <WarnCount value={data.largeFileCount} label="large" />
        </Metric>
        <Metric label=".git size">
          <CappedNum
            value={formatBytes(data.gitDirBytes)}
            capped={data.gitDirScanCapped}
          />
        </Metric>
      </div>
      {data.largestFiles.length > 0 && (
        <>
          <h4 className="health-subtitle">Largest files</h4>
          <ul className="health-stat-list">
            {data.largestFiles.map((f) => (
              <li className="health-stat-row" key={f.path}>
                <span className="mono health-stat-name">{f.path}</span>
                <span className="health-stat-size">{formatBytes(f.size)}</span>
              </li>
            ))}
          </ul>
        </>
      )}
      {data.largestBlobs.length > 0 && (
        <>
          <h4 className="health-subtitle">Largest blobs</h4>
          <ul className="health-stat-list">
            {data.largestBlobs.map((b) => (
              <li className="health-stat-row" key={b.oid}>
                <span className="mono health-stat-name">blob {b.oid.slice(0, 7)}</span>
                <span className="health-stat-size">{formatBytes(b.size)}</span>
              </li>
            ))}
          </ul>
        </>
      )}
    </>
  );
}

function BranchesBody({ data }: { data: BranchesSection }) {
  return (
    <div className="health-grid">
      <Metric label="Local branches">{data.localCount}</Metric>
      <Metric label="Remote branches">{data.remoteCount}</Metric>
      <Metric label="Tags">{data.tagCount}</Metric>
      <Metric label="HEAD">
        {data.unborn ? (
          <span className="asset-chip asset-chip-drifted">unborn</span>
        ) : data.detached ? (
          <span className="asset-chip asset-chip-drifted">detached</span>
        ) : (
          <span className="mono">{data.currentBranch ?? '—'}</span>
        )}
      </Metric>
      <Metric label="Upstream">
        {data.upstream === null ? (
          <span className="asset-chip asset-chip-muted">none</span>
        ) : (
          <>
            <span className="mono">{data.upstream}</span>
            {data.ahead !== null && data.ahead > 0 && (
              <span className="asset-chip asset-chip-drifted">↑{data.ahead} ahead</span>
            )}
            {data.behind !== null && data.behind > 0 && (
              <span className="asset-chip asset-chip-drifted">↓{data.behind} behind</span>
            )}
            {data.ahead === 0 && data.behind === 0 && (
              <span className="asset-chip asset-chip-sync">in sync</span>
            )}
            {(data.ahead === null || data.behind === null) && (
              <span className="asset-chip asset-chip-muted">unknown</span>
            )}
          </>
        )}
      </Metric>
      <Metric label="Stale branches">
        {data.stale === null ? (
          <span className="asset-chip asset-chip-muted">
            {data.staleError ?? 'unavailable'}
          </span>
        ) : (
          <>
            <WarnCount value={data.stale.mergedCount} label="merged" />
            <WarnCount value={data.stale.goneUpstreamCount} label="gone upstream" />
            <span className="health-stale-base">
              vs <span className="mono">{data.stale.base}</span>
            </span>
          </>
        )}
      </Metric>
    </div>
  );
}

function WorkingStateBody({ data }: { data: WorkingStateSection }) {
  const opLabel = opStateLabel(data.opState);
  return (
    <div className="health-grid">
      <Metric label="Staged">{data.staged}</Metric>
      <Metric label="Unstaged">{data.unstaged}</Metric>
      <Metric label="Untracked">{data.untracked}</Metric>
      <Metric label="Conflicted">
        <WarnCount value={data.conflicted} label="conflicted" />
      </Metric>
      <Metric label="Operation">
        {opLabel === null ? (
          <span className="asset-chip asset-chip-sync">none</span>
        ) : (
          <span className="asset-chip asset-chip-drifted">{opLabel}</span>
        )}
      </Metric>
      <Metric label="Stashes">{data.stashCount}</Metric>
      <Metric label=".gitignore">
        {data.hasGitignore ? (
          <span className="asset-chip asset-chip-sync">present</span>
        ) : (
          <span className="asset-chip asset-chip-muted">missing</span>
        )}
      </Metric>
    </div>
  );
}

function StructureBody({ data }: { data: StructureSection }) {
  return (
    <div className="health-grid">
      <Metric label="Submodules">
        {data.submoduleCount}
        {data.submoduleCount > 0 && (
          <>
            <WarnCount value={data.submodulesUninitialized} label="uninitialized" />
            <WarnCount value={data.submodulesOutOfSync} label="out of sync" />
            <WarnCount value={data.submodulesModified} label="modified" />
          </>
        )}
      </Metric>
      <Metric label="Worktrees">
        {data.worktreeCount}
        <WarnCount value={data.worktreesLocked} label="locked" />
        <WarnCount value={data.worktreesPrunable} label="prunable" />
        <WarnCount value={data.worktreesInvalid} label="invalid" />
      </Metric>
      <Metric label="AI assets">
        {data.assetsInSync ? (
          <span className="asset-badge asset-badge-ok">In sync</span>
        ) : (
          <span className="asset-badge asset-badge-warn">
            {data.assetDriftedCount} file{data.assetDriftedCount === 1 ? '' : 's'} drifted
          </span>
        )}
      </Metric>
    </div>
  );
}

export function RepoHealthPanel({ open, onClose, repoId }: RepoHealthPanelProps) {
  const [health, setHealth] = useState<RepoHealth | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  // Monotonic request id: a fetch whose id no longer matches the latest issued
  // one is stale (repoId changed via Ctrl+Tab while the panel was open, or a
  // newer Refresh superseded it) and must NOT write state. Same discipline as
  // AiAssetsPanel.
  const fetchIdRef = useRef(0);

  const refresh = useCallback(async (): Promise<void> => {
    const id = (fetchIdRef.current += 1);
    setLoading(true);
    setError(null);
    try {
      const result = await ipc.getRepoHealth(repoId);
      if (fetchIdRef.current !== id) return;
      setHealth(result);
    } catch (e) {
      if (fetchIdRef.current !== id) return;
      setError(errorMessage(e));
      setHealth(null);
    } finally {
      if (fetchIdRef.current === id) setLoading(false);
    }
  }, [repoId]);

  // Fetch on open + whenever the repo changes while open. Drop the previous
  // repo's payload immediately on repoId change so stale numbers never show.
  useEffect(() => {
    if (!open) return;
    setHealth(null);
    void refresh();
  }, [open, refresh]);

  // Auto-refresh on the repo-changed event for THIS repo while open (D11) —
  // the scan is heavier than status, so it is NEVER fetched while closed.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    const unsubs: Unsubscribe[] = [];
    void (async () => {
      const off = await ipc.onRepoChanged((p) => {
        if (p.repoId === repoId) void refresh();
      });
      if (cancelled) {
        off();
        return;
      }
      unsubs.push(off);
    })();
    return () => {
      cancelled = true;
      for (const unsub of unsubs) unsub();
    };
  }, [open, repoId, refresh]);

  if (!open) return null;

  return (
    <div
      className="dialog-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="dialog-card ai-assets-card" role="dialog" aria-label="Repo Health">
        <div className="shortcut-header">
          <h2 className="dialog-title shortcut-title">Repo Health</h2>
          <div className="asset-header-actions">
            {health !== null && (
              <span className="health-caption">
                generated {relativeTime(health.generatedAt, Date.now())}
              </span>
            )}
            <button
              type="button"
              className="btn-secondary settings-toggle-btn"
              disabled={loading}
              onClick={() => void refresh()}
            >
              {loading ? 'Refreshing…' : 'Refresh'}
            </button>
            <button
              type="button"
              className="btn-icon shortcut-close"
              aria-label="Close"
              title="Close"
              onClick={onClose}
            >
              {'×'}
            </button>
          </div>
        </div>

        {error !== null && (
          <div className="error-banner" role="alert">
            {error}
          </div>
        )}

        <HealthSection
          title="Stats"
          section={health?.stats ?? null}
          loading={loading}
          render={(data) => <StatsBody data={data} />}
        />
        <HealthSection
          title="Branches"
          section={health?.branches ?? null}
          loading={loading}
          render={(data) => <BranchesBody data={data} />}
        />
        <HealthSection
          title="Working state"
          section={health?.workingState ?? null}
          loading={loading}
          render={(data) => <WorkingStateBody data={data} />}
        />
        <HealthSection
          title="Structure"
          section={health?.structure ?? null}
          loading={loading}
          render={(data) => <StructureBody data={data} />}
        />
      </div>
    </div>
  );
}
