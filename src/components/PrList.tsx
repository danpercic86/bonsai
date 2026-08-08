import type { PrStateFilter, PrSummary } from '../ipc';
import { SkeletonRows } from './CommitPanel';
import { PrListItem } from './PrListItem';

// P62c: presentational PR list — a small header (state filter + refresh + new
// PR) over the selectable rows. All state + IPC live in PrPanel.

const FILTERS: { value: PrStateFilter; label: string }[] = [
  { value: 'open', label: 'Open' },
  { value: 'closed', label: 'Closed' },
  { value: 'all', label: 'All' },
];

export interface PrListProps {
  items: PrSummary[];
  selectedNumber: number | null;
  loading: boolean;
  error: string | null;
  filter: PrStateFilter;
  onChangeFilter(filter: PrStateFilter): void;
  onSelect(number: number): void;
  onRefresh(): void;
  onCreate(): void;
}

export function PrList({
  items,
  selectedNumber,
  loading,
  error,
  filter,
  onChangeFilter,
  onSelect,
  onRefresh,
  onCreate,
}: PrListProps) {
  return (
    <div className="pr-list">
      <div className="pr-list-header">
        <div className="diff-view-toggle" role="group" aria-label="Filter pull requests by state">
          {FILTERS.map((f) => (
            <button
              key={f.value}
              type="button"
              className={filter === f.value ? 'active' : ''}
              aria-pressed={filter === f.value}
              onClick={() => onChangeFilter(f.value)}
            >
              {f.label}
            </button>
          ))}
        </div>
        <div className="pr-list-header-actions">
          <button
            type="button"
            className="section-action"
            disabled={loading}
            onClick={onRefresh}
          >
            Refresh
          </button>
          <button type="button" className="btn-primary pr-new-button" onClick={onCreate}>
            New pull request
          </button>
        </div>
      </div>

      {error !== null && (
        <div className="error-banner error-banner-dismissible pr-error" role="alert">
          <span className="error-banner-text">{error}</span>
          <button type="button" className="section-action" onClick={onRefresh}>
            Retry
          </button>
        </div>
      )}

      {loading ? (
        <div className="pr-list-loading">
          <SkeletonRows />
        </div>
      ) : items.length === 0 && error === null ? (
        <p className="pane-empty pr-empty">{`No ${filter === 'all' ? '' : `${filter} `}pull requests.`}</p>
      ) : (
        <ul className="pr-rows">
          {items.map((pr) => (
            <li key={pr.number}>
              <PrListItem
                pr={pr}
                selected={pr.number === selectedNumber}
                onSelect={onSelect}
              />
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
