import type { ReactNode } from 'react';
import type { PrDetail, PrState } from '../ipc';

// P62c: presentational PR detail — header, meta, labels, mergeable, +/- stat,
// and the (markdown-ish) body, plus a slot for the comments component. No IPC.

const STATE_LABEL: Record<PrState, string> = {
  open: 'Open',
  merged: 'Merged',
  closed: 'Closed',
};

function formatDate(iso: string): string {
  const t = Date.parse(iso);
  return Number.isNaN(t) ? iso : new Date(t).toLocaleDateString();
}

function mergeableLabel(mergeable: boolean | null): { text: string; cls: string } {
  if (mergeable === null) return { text: 'Mergeability pending', cls: 'pending' };
  return mergeable
    ? { text: 'No conflicts', cls: 'clean' }
    : { text: 'Has conflicts', cls: 'conflict' };
}

export interface PrDetailViewProps {
  detail: PrDetail;
  onBack(): void;
  /** Comments component (or its loading/error state) rendered under the body. */
  children?: ReactNode;
}

export function PrDetailView({ detail, onBack, children }: PrDetailViewProps) {
  const { summary } = detail;
  const merge = mergeableLabel(detail.mergeable);
  return (
    <div className="pr-detail">
      <div className="pr-detail-header">
        <div className="pr-detail-title-row">
          <button
            type="button"
            className="section-action pr-back-button"
            onClick={onBack}
          >
            {'← Pull requests'}
          </button>
          <a
            className="section-action pr-open-link"
            href={summary.url}
            target="_blank"
            rel="noreferrer noopener"
          >
            Open in browser ↗
          </a>
        </div>
        <div className="pr-detail-title">
          <span className={`pr-state-pill pr-state-${summary.state}`}>
            {STATE_LABEL[summary.state]}
          </span>
          {summary.isDraft && <span className="pr-draft-tag">Draft</span>}
          <span className="pr-detail-title-text">{summary.title}</span>
          <span className="pr-detail-num mono">{`#${summary.number}`}</span>
        </div>
        <div className="pr-detail-meta">
          <span className="pr-detail-author">{summary.author}</span>
          <span className="pr-detail-branches mono">
            {`${summary.sourceBranch} → ${summary.targetBranch}`}
          </span>
          <span className="pr-detail-date">{`opened ${formatDate(summary.createdAt)}`}</span>
        </div>
        <div className="pr-detail-stats">
          {summary.state === 'open' && (
            <span className={`pr-mergeable pr-mergeable-${merge.cls}`}>{merge.text}</span>
          )}
          <span className="pr-stat-add">{`+${detail.additions}`}</span>
          <span className="pr-stat-del">{`−${detail.deletions}`}</span>
          <span className="pr-stat-files">
            {`${detail.changedFiles} file${detail.changedFiles === 1 ? '' : 's'}`}
          </span>
        </div>
        {detail.labels.length > 0 && (
          <div className="pr-labels">
            {detail.labels.map((label) => (
              <span key={label} className="pr-label">
                {label}
              </span>
            ))}
          </div>
        )}
      </div>

      {detail.body !== '' ? (
        <div className="pr-body">{detail.body}</div>
      ) : (
        <p className="pane-empty pr-empty">No description provided.</p>
      )}

      {children}
    </div>
  );
}
