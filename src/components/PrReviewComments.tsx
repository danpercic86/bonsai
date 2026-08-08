import type { ReviewComment } from '../ipc';
import { SkeletonRows } from './CommitPanel';

// P62c: presentational comment thread for a PR — merged review (diff-line) +
// conversation comments, already sorted oldest→newest by the backend. No IPC.

function formatDate(iso: string): string {
  const t = Date.parse(iso);
  return Number.isNaN(t) ? iso : new Date(t).toLocaleString();
}

export interface PrReviewCommentsProps {
  comments: ReviewComment[];
  loading: boolean;
  error: string | null;
}

export function PrReviewComments({ comments, loading, error }: PrReviewCommentsProps) {
  return (
    <section className="pr-comments">
      <div className="section-header section-label">
        <span>{`Comments${loading ? '' : ` (${comments.length})`}`}</span>
      </div>

      {error !== null && (
        <div className="error-banner error-banner-dismissible pr-error" role="alert">
          <span className="error-banner-text">{error}</span>
        </div>
      )}

      {loading ? (
        <div className="pr-comments-loading">
          <SkeletonRows />
        </div>
      ) : comments.length === 0 && error === null ? (
        <p className="pane-empty pr-empty">No comments yet.</p>
      ) : (
        <ul className="pr-comment-list">
          {comments.map((c) => (
            <li key={c.id} className={`pr-comment pr-comment-${c.kind}`}>
              <div className="pr-comment-head">
                <span className="pr-comment-author">{c.author}</span>
                <span className={`pr-comment-kind pr-comment-kind-${c.kind}`}>
                  {c.kind === 'review' ? 'Review' : 'Comment'}
                </span>
                <span className="pr-comment-date">{formatDate(c.createdAt)}</span>
              </div>
              {c.path !== null && (
                <div className="pr-comment-loc mono" title={c.path}>
                  {c.line !== null ? `${c.path}:${c.line}` : c.path}
                </div>
              )}
              <div className="pr-comment-body">{c.body}</div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
