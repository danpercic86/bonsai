// P125-ui §4.2 / §4.3 — the populated body: the summary/filter bar, the
// report-wide lead, the calm up-to-date line, the sticky column header and
// the scrolling list of groups. Filtering is pure (`depsView`); this only lays
// it out.
import type { DepReport, DepRow, DepUpdateKind } from '../../ipc/types/depTracker';
import {
  CLEAR_FILTERS,
  COLUMNS,
  NO_MATCH,
  UP_TO_DATE,
  UP_TO_DATE_SOME_UNCHECKED,
  reportWideLead,
} from './depsCopy';
import { DepsGroup } from './DepsGroup';
import { DepsSummaryBar } from './DepsSummaryBar';
import type { DepsFilter } from './depsView';
import {
  NO_FILTER,
  ecosystemLabel,
  isFilterActive,
  reportWideConfigLead,
  someUnchecked,
  visibleGroups,
} from './depsView';

export interface DepsBodyListProps {
  report: DepReport;
  upToDate: boolean;
  filter: DepsFilter;
  idStem: string;
  /** Collapsed group paths (groups are expanded by default). */
  collapsed: ReadonlySet<string>;
  shownAll: ReadonlySet<string>;
  /** `${manifestPath}\u0001${rowKey}` of each expanded row. */
  expandedRows: ReadonlySet<string>;
  onFilter(next: DepsFilter): void;
  onToggleGroup(manifestPath: string): void;
  onShowAll(manifestPath: string): void;
  onToggleRow(manifestPath: string, key: string): void;
  onOpenUrl(url: string, host: string): void;
  onRowMenu(row: DepRow, x: number, y: number): void;
}

/** The sticky, `aria-hidden` column header — each group table has its own
 *  visually hidden `<thead>` with the same labels. */
function ColumnHeader() {
  return (
    <div className="deps-colhead" aria-hidden="true">
      <span className="deps-colhead-name">{COLUMNS.package}</span>
      <span className="deps-colhead-ver">{COLUMNS.installed}</span>
      <span className="deps-colhead-ver">{COLUMNS.latest}</span>
      <span className="deps-colhead-upd">{COLUMNS.update}</span>
      <span className="deps-colhead-vuln">{COLUMNS.vulnerability}</span>
      <span className="deps-colhead-act" />
    </div>
  );
}

export function DepsBodyList({
  report,
  upToDate,
  filter,
  idStem,
  collapsed,
  shownAll,
  expandedRows,
  onFilter,
  onToggleGroup,
  onShowAll,
  onToggleRow,
  onOpenUrl,
  onRowMenu,
}: DepsBodyListProps) {
  // §8.3 / R3: from the report's `networkProfile` flag only, never inferred.
  const wideEcosystems = reportWideConfigLead(report.groups, report.networkProfile);
  const reportWide = wideEcosystems !== null;
  const groups = visibleGroups(report.groups, filter);
  const noMatch =
    isFilterActive(filter) &&
    groups.every((g) => g.rows.length === 0);
  const toggleKind = (k: DepUpdateKind) => {
    const kinds = new Set(filter.kinds);
    if (kinds.has(k)) kinds.delete(k);
    else kinds.add(k);
    onFilter({ ...filter, kinds });
  };

  return (
    <>
      <DepsSummaryBar
        summary={report.summary}
        filter={filter}
        onToggleVulnerable={() => onFilter({ ...filter, vulnerable: !filter.vulnerable })}
        onToggleKind={toggleKind}
        onName={(name) => onFilter({ ...filter, name })}
      />
      {wideEcosystems !== null && (
        <p className="deps-lead">{reportWideLead(wideEcosystems.map(ecosystemLabel))}</p>
      )}
      {upToDate && (
        <p className="deps-calm">
          {someUnchecked(report.groups, report.summary)
            ? `${UP_TO_DATE} ${UP_TO_DATE_SOME_UNCHECKED}`
            : UP_TO_DATE}
        </p>
      )}
      <div className="deps-body">
        <ColumnHeader />
        {noMatch && (
          <p className="deps-no-match">
            {NO_MATCH}{' '}
            <button type="button" className="deps-link" onClick={() => onFilter(NO_FILTER)}>
              {CLEAR_FILTERS}
            </button>
          </p>
        )}
        {groups.map(({ group, rows }, i) => (
          <DepsGroup
            key={group.manifestPath}
            group={group}
            rows={rows}
            idStem={`${idStem}-g${i}`}
            expanded={!collapsed.has(group.manifestPath)}
            showAll={shownAll.has(group.manifestPath)}
            reportWide={reportWide}
            expandedRows={
              new Set(
                [...expandedRows]
                  .filter((k) => k.startsWith(`${group.manifestPath}\u0001`))
                  .map((k) => k.slice(group.manifestPath.length + 1)),
              )
            }
            onToggleGroup={() => onToggleGroup(group.manifestPath)}
            onShowAll={() => onShowAll(group.manifestPath)}
            onToggleRow={(key) => onToggleRow(group.manifestPath, key)}
            onOpenUrl={onOpenUrl}
            onRowMenu={onRowMenu}
          />
        ))}
      </div>
    </>
  );
}
