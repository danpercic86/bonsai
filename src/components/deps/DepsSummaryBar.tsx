// P125-ui §4.2 — the summary IS the filter bar: each count is a toggle, so the
// counts and the filters cannot disagree. Plus the secondary counts line and
// the name filter (`ListFilterInput`, reused).
import type { DepSummary, DepUpdateKind } from '../../ipc/types/depTracker';
import { ListFilterInput } from '../ListFilterInput';
import { FILTER_ARIA, FILTER_KIND, FILTER_PLACEHOLDER, FILTER_VULNERABLE } from './depsCopy';
import type { DepsFilter } from './depsView';
import { secondaryLine } from './depsView';

const KINDS: readonly DepUpdateKind[] = ['major', 'minor', 'patch'];

export interface DepsSummaryBarProps {
  summary: DepSummary;
  filter: DepsFilter;
  onToggleVulnerable(): void;
  onToggleKind(kind: DepUpdateKind): void;
  onName(name: string): void;
}

export function DepsSummaryBar({ summary, filter, onToggleVulnerable, onToggleKind, onName }: DepsSummaryBarProps) {
  const vuln = summary.vulnerable;
  const second = secondaryLine(summary);
  return (
    <div className="deps-summary">
      <div className="deps-summary-bar">
        <button
          type="button"
          className={`search-toggle deps-filter-toggle${filter.vulnerable ? ' is-active' : ''}`}
          aria-pressed={filter.vulnerable}
          disabled={vuln === 0}
          onClick={onToggleVulnerable}
        >
          {vuln > 0 && (
            <span className="deps-filter-glyph" aria-hidden="true">
              ⊘
            </span>
          )}
          {FILTER_VULNERABLE}
          <span className="deps-pill deps-pill--neutral deps-filter-count">{vuln}</span>
        </button>
        {KINDS.map((k) => (
          <button
            key={k}
            type="button"
            className={`search-toggle deps-filter-toggle${filter.kinds.has(k) ? ' is-active' : ''}`}
            aria-pressed={filter.kinds.has(k)}
            disabled={summary[k] === 0}
            onClick={() => onToggleKind(k)}
          >
            {FILTER_KIND[k]}
            <span className="deps-pill deps-pill--neutral deps-filter-count">{summary[k]}</span>
          </button>
        ))}
        <span className="deps-summary-spacer" />
        <div className="deps-filter">
          <ListFilterInput
            value={filter.name}
            onChange={onName}
            ariaLabel={FILTER_ARIA}
            placeholder={FILTER_PLACEHOLDER}
          />
        </div>
      </div>
      {second !== '' && <p className="deps-summary-secondary">{second}</p>}
    </div>
  );
}
