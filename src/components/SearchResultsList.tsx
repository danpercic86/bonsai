import type { SearchMatch } from '../ipc';
import { relativeDate } from '../graph/draw';

export interface SearchResultsListProps {
  matches: SearchMatch[];
  /** Index of the current match (highlighted row), or -1. */
  currentMatch: number;
  /** Click a row → jump to that match (reveals in graph + becomes current). */
  onSelect(index: number): void;
}

/** P50b: compact commit-search results overlay (a dropdown under the search
 *  bar). Presentational — one row per match with the short oid, summary, author,
 *  relative date, and a matched-field badge (plus the path snippet for path
 *  mode). The current row is highlighted; clicking reveals it in the graph. */
export function SearchResultsList({ matches, currentMatch, onSelect }: SearchResultsListProps) {
  if (matches.length === 0) {
    return (
      <div className="search-results" role="listbox" aria-label="Search results">
        <div className="search-results-empty">No matches</div>
      </div>
    );
  }
  const now = Math.floor(Date.now() / 1000);
  return (
    <div className="search-results" role="listbox" aria-label="Search results">
      {matches.map((m, i) => (
        <button
          type="button"
          key={m.oid}
          role="option"
          aria-selected={i === currentMatch}
          className={'search-result' + (i === currentMatch ? ' is-current' : '')}
          onClick={() => onSelect(i)}
        >
          <span className="search-result-oid">{m.oid.slice(0, 7)}</span>
          <span className="search-result-summary" title={m.summary}>
            {m.summary}
          </span>
          {m.snippet !== undefined && (
            <span className="search-result-snippet" title={m.snippet}>
              {m.snippet}
            </span>
          )}
          <span className="search-result-author" title={m.authorName}>
            {m.authorName}
          </span>
          <span className="search-result-date">{relativeDate(m.authorTs, now)}</span>
          <span className="search-result-badge">{m.matched}</span>
        </button>
      ))}
    </div>
  );
}
