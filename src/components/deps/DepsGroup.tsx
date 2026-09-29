// P125-ui §4.3 — one manifest group: the disclosure header, its state lines,
// the group `<table>` and the "Show all" cap. `notTracked` and `parseError`
// groups are a single line with no header button and no table.
import type { DepManifestGroup, DepRow as DepRowData } from '../../ipc/types/depTracker';
import {
  COLUMNS,
  NO_LOCKFILE_LINE,
  groupCount,
  notTrackedSuffix,
  parseErrorLine,
  showAll as showAllLabel,
} from './depsCopy';
import { DepsRow } from './DepsRow';
import { GROUP_ROW_CAP, ecosystemLabel, groupPrivateLead, groupTitle, rowKey, vulnerableRows } from './depsView';

export interface DepsGroupProps {
  group: DepManifestGroup;
  /** Rows after the filters (order unchanged). */
  rows: readonly DepRowData[];
  /** Stable DOM-id stem for this group (index-based; paths are not id-safe). */
  idStem: string;
  expanded: boolean;
  showAll: boolean;
  reportWide: boolean;
  expandedRows: ReadonlySet<string>;
  onToggleGroup(): void;
  onShowAll(): void;
  onToggleRow(key: string): void;
  onOpenUrl(url: string, host: string): void;
  onRowMenu(row: DepRowData, x: number, y: number): void;
}

/** The visually hidden per-table header (the sticky header above is aria-hidden). */
function HiddenHead() {
  return (
    <thead className="deps-thead">
      <tr>
        <th scope="col">{COLUMNS.package}</th>
        <th scope="col">{COLUMNS.installed}</th>
        <th scope="col">{COLUMNS.latest}</th>
        <th scope="col">{COLUMNS.update}</th>
        <th scope="col">{COLUMNS.vulnerability}</th>
        <th scope="col">{COLUMNS.actions}</th>
      </tr>
    </thead>
  );
}

export function DepsGroup({
  group,
  rows,
  idStem,
  expanded,
  showAll,
  reportWide,
  expandedRows,
  onToggleGroup,
  onShowAll,
  onToggleRow,
  onOpenUrl,
  onRowMenu,
}: DepsGroupProps) {
  const state = group.state;
  if (state.kind === 'notTracked') {
    return (
      <div className="deps-group-line deps-group-single">
        <span className="mono deps-group-path">{group.manifestPath}</span>
        <span className="deps-group-meta">{notTrackedSuffix(state.label)}</span>
      </div>
    );
  }
  if (state.kind === 'parseError') {
    return (
      <p className="deps-group-line deps-group-single deps-group-wrap" title={state.message}>
        {parseErrorLine(state.file)}
      </p>
    );
  }

  const btnId = `${idStem}-h`;
  const bodyId = `${idStem}-b`;
  const lead = groupPrivateLead(group, reportWide);
  const visible = showAll ? rows : rows.slice(0, GROUP_ROW_CAP);
  const vulnerable = vulnerableRows(group);

  return (
    <section className="deps-group">
      <h3 className="deps-group-h">
        <button
          type="button"
          id={btnId}
          className="deps-group-toggle"
          aria-expanded={expanded}
          aria-controls={bodyId}
          title={groupTitle(group)}
          onClick={onToggleGroup}
        >
          <span className="deps-chevron" aria-hidden="true">
            ▸
          </span>
          <span className="mono deps-group-path">{group.manifestPath}</span>
          {group.ecosystem !== null && (
            <span className="deps-group-meta deps-group-eco">{` · ${ecosystemLabel(group.ecosystem)}`}</span>
          )}
          {group.lockfilePath !== null && (
            <span className="mono deps-group-meta deps-group-lock">{` · ${group.lockfilePath}`}</span>
          )}
          <span className="deps-group-count">{groupCount(group.rows.length, vulnerable)}</span>
        </button>
      </h3>
      {expanded && (
        <div id={bodyId}>
          {state.kind === 'noLockfile' && <p className="deps-group-line deps-group-wrap">{NO_LOCKFILE_LINE}</p>}
          {lead !== null && <p className="deps-group-line deps-group-wrap">{lead}</p>}
          <table className="deps-table" aria-labelledby={btnId}>
            <colgroup>
              <col />
              <col className="deps-col-ver" />
              <col className="deps-col-ver" />
              <col className="deps-col-upd" />
              <col className="deps-col-vuln" />
              <col className="deps-col-act" />
            </colgroup>
            <HiddenHead />
            <tbody>
              {visible.map((row, i) => {
                const key = rowKey(row);
                return (
                  <DepsRow
                    key={key}
                    row={row}
                    detailsId={`${idStem}-r${i}`}
                    expanded={expandedRows.has(key)}
                    reportWide={reportWide}
                    onToggle={() => onToggleRow(key)}
                    onOpenUrl={onOpenUrl}
                    onMenu={(x, y) => onRowMenu(row, x, y)}
                  />
                );
              })}
            </tbody>
          </table>
          {visible.length < rows.length && (
            <button type="button" className="deps-link deps-show-all" onClick={onShowAll}>
              {showAllLabel(rows.length)}
            </button>
          )}
        </div>
      )}
    </section>
  );
}
