import type { CompareDiff, ListView } from '../ipc';
import { SkeletonRows } from './CommitPanel';
import { DiffFileTree } from './DiffFileTree';
import type { DiffScope } from './DiffFileTree';

// P5 §5.7: Compare right-panel mode. Shown INSTEAD of CommitPanel /
// StatusPanel + CommitBox while a HEAD → commit comparison is active.
// P11g-rev §3.1: the file list is now the shared DiffFileTree, the SOLE scope
// navigator. Clicking root/folder/file drives the lifted `scope`; the compare
// DiffBrowser is already auto-open, so a click just refilters it.
// Presentational only (App owns all fetching + the compare state).

function shortOid(oid: string): string {
  return oid.slice(0, 7);
}

export interface ComparePanelProps {
  data: CompareDiff | null; // null while loading
  loading: boolean;
  error: string | null;
  /** HEAD branch name for the header ("HEAD (main)"); null when detached. */
  headBranchName: string | null;
  listView: ListView;
  /** P11g-rev §3.1: current diff scope (selection highlight) + its setter. */
  scope: DiffScope;
  onSelectScope(scope: DiffScope): void;
  onClose(): void;
}

export function ComparePanel({
  data,
  loading,
  error,
  headBranchName,
  listView,
  scope,
  onSelectScope,
  onClose,
}: ComparePanelProps) {
  const fromUnborn = data !== null && data.from.oid === '';
  const fromLabel = `HEAD${headBranchName !== null ? ` (${headBranchName})` : ''}`;

  return (
    <div className="commit-panel compare-panel">
      <div className="commit-panel-header">
        <div className="commit-panel-title">
          <div className="commit-summary">Comparing</div>
          <button
            type="button"
            className="btn-icon commit-close"
            aria-label="Close comparison"
            title="Close"
            onClick={onClose}
          >
            {'×'}
          </button>
        </div>
        {data !== null && (
          <div className="compare-endpoints">
            <span className="compare-endpoint" title={fromUnborn ? 'unborn HEAD' : data.from.oid}>
              <span className="compare-endpoint-label">{fromLabel}</span>
              {!fromUnborn && (
                <>
                  {' · '}
                  <span className="mono compare-endpoint-oid">{shortOid(data.from.oid)}</span>
                </>
              )}
            </span>
            <span className="compare-arrow" aria-hidden="true">
              {'→'}
            </span>
            <span className="compare-endpoint" title={data.to.oid}>
              <span className="mono compare-endpoint-oid">{shortOid(data.to.oid)}</span>
              {' · '}
              <span className="compare-endpoint-summary">{data.to.summary}</span>
            </span>
          </div>
        )}
      </div>

      {error !== null && (
        <div className="error-banner error-banner-dismissible commit-panel-error" role="alert">
          <span className="error-banner-text">{error}</span>
        </div>
      )}

      {loading && data === null ? (
        <div className="commit-panel-loading">
          <SkeletonRows />
        </div>
      ) : data !== null ? (
        data.files.length === 0 ? (
          <div className="pane-empty compare-empty">No differences</div>
        ) : (
          <section className="status-section commit-files">
            <div className="section-header section-label">
              <span>Changes ({data.files.length})</span>
            </div>
            <DiffFileTree
              files={data.files}
              listView={listView}
              scope={scope}
              onSelect={onSelectScope}
            />
          </section>
        )
      ) : null}
    </div>
  );
}
