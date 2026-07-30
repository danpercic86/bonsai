import { useMemo } from 'react';
import type { CompareDiff, FileDiffHeader, ListView } from '../ipc';
import { buildPathTree } from '../utils/pathTree';
import { FileHeaderRow, SkeletonRows } from './CommitPanel';
import { Tree } from './Tree';

// P5 §5.7: Compare right-panel mode. Shown INSTEAD of CommitPanel /
// StatusPanel + CommitBox while a HEAD → commit comparison is active.
// P11g §6.5: now a SUMMARY panel — the file list + "View all changes" open the
// all-files DiffBrowser over the graph pane (single-file diffSlot retired here).
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
  /** P11g §6.5: open the all-files DiffBrowser at the root scope. */
  onViewAll(): void;
  /** P11g §6.5: open the DiffBrowser scoped to the clicked file. */
  onOpenFile(file: FileDiffHeader): void;
  onClose(): void;
}

export function ComparePanel({
  data,
  loading,
  error,
  headBranchName,
  listView,
  onViewAll,
  onOpenFile,
  onClose,
}: ComparePanelProps) {
  const files = data?.files;
  const fileNodes = useMemo(
    () =>
      listView === 'tree' && files !== undefined ? buildPathTree(files, (f) => f.path) : null,
    [listView, files],
  );

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
              <button type="button" className="section-action" onClick={onViewAll}>
                View all changes
              </button>
            </div>
            {fileNodes !== null ? (
              <Tree
                nodes={fileNodes}
                leafKey={(l) => `compare:${l.item.path}`}
                renderLeaf={(l) => (
                  <FileHeaderRow
                    file={l.item}
                    expanded={false}
                    onToggle={() => onOpenFile(l.item)}
                    treeMode
                  />
                )}
              />
            ) : (
              <ul className="file-list">
                {data.files.map((file) => (
                  <FileHeaderRow
                    key={`compare:${file.path}`}
                    file={file}
                    expanded={false}
                    onToggle={() => onOpenFile(file)}
                  />
                ))}
              </ul>
            )}
          </section>
        )
      ) : null}
    </div>
  );
}
