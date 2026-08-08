import type { HistoryHit } from '../ipc';
import { relativeDate } from '../graph/draw';

export interface HistoryResultsListProps {
  hits: HistoryHit[];
  /** True once a retrieval has run — drives the "No relevant commits" empty
   *  state (a pristine panel shows nothing). */
  searched: boolean;
  /** Click a row → reveal that commit in the graph. */
  onSelect(oid: string): void;
}

/** P57c: relevance-ranked history hits (short-oid · summary · author · rel-date ·
 *  a BM25 score bar). Presentational — click reveals in the graph (reuses the
 *  P50 reveal path). Styled like `SearchResultsList`. */
export function HistoryResultsList({ hits, searched, onSelect }: HistoryResultsListProps) {
  if (hits.length === 0) {
    if (!searched) return null;
    return (
      <div className="search-results history-results" role="listbox" aria-label="History results">
        <div className="search-results-empty">No relevant commits</div>
      </div>
    );
  }
  const now = Math.floor(Date.now() / 1000);
  // Scores are BM25, descending → the top hit anchors the bar's full width.
  const maxScore = hits[0].score > 0 ? hits[0].score : 1;
  return (
    <div className="search-results history-results" role="listbox" aria-label="History results">
      {hits.map((h) => {
        const pct = Math.max(4, Math.min(100, Math.round((h.score / maxScore) * 100)));
        return (
          <button
            type="button"
            key={h.oid}
            role="option"
            aria-selected={false}
            className="search-result history-hit"
            onClick={() => onSelect(h.oid)}
          >
            <span className="search-result-oid">{h.oid.slice(0, 7)}</span>
            <span className="search-result-summary" title={h.summary}>
              {h.summary}
            </span>
            <span className="search-result-author" title={h.authorName}>
              {h.authorName}
            </span>
            <span className="search-result-date">{relativeDate(h.authorTs, now)}</span>
            <span className="history-score-bar" aria-hidden="true" title={`score ${h.score.toFixed(2)}`}>
              <span className="history-score-fill" style={{ width: `${pct}%` }} />
            </span>
          </button>
        );
      })}
    </div>
  );
}
