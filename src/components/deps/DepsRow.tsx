// P125-ui §4.4 — one package row (32px, never wraps) plus its optional
// advisory-details row. Presentational: the container owns expansion, the
// link opener and the context menu.
import type { KeyboardEvent, MouseEvent } from 'react';

import type { DepRow as DepRowData } from '../../ipc/types/depTracker';
import { ExternalLinkIcon } from '../appIcons';
import { SEVERITY_LABEL, morePill, openOnHost, rowToggleName } from './depsCopy';
import { DepsAdvisoryList } from './DepsAdvisoryList';
import { AvailabilityCell, NotVerifiedPill, RowTag, SeverityPill, UpdateKindPill } from './DepsPills';
import { ecosystemLabel, hostLabel, notVerifiedCause, rowTags } from './depsView';

export interface DepsRowProps {
  row: DepRowData;
  detailsId: string;
  expanded: boolean;
  /** The inferred report-wide config lead is showing (§8.3). */
  reportWide: boolean;
  onToggle(): void;
  onOpenUrl(url: string, host: string): void;
  /** Right-click, Shift+F10 or the Menu key (§5.3). */
  onMenu(x: number, y: number): void;
}

/** Vulnerability cell (§4.4): first advisory + `+n`, empty when checked-clean,
 *  `not verified` when OSV could not be asked. */
function VulnCell({ row }: { row: DepRowData }) {
  if (row.vulns.length > 0) {
    const first = row.vulns[0];
    return (
      <span className="deps-vuln">
        <SeverityPill severity={first.severity} />
        <span className="mono deps-vuln-id" title={first.id}>
          {first.id}
        </span>
        {row.vulns.length > 1 && <RowTag>{morePill(row.vulns.length - 1)}</RowTag>}
      </span>
    );
  }
  if (row.vulnsChecked) return null;
  return <NotVerifiedPill title={notVerifiedCause(row)} />;
}

export function DepsRow({ row, detailsId, expanded, reportWide, onToggle, onOpenUrl, onMenu }: DepsRowProps) {
  const kind = row.availability.kind;
  const offline = kind === 'local' || kind === 'private';
  const version = row.current ?? row.declared ?? '';
  const tags = rowTags(row);
  // No `, vulnerabilities not verified` here: every such row renders the
  // `not verified` pill, which SR users already hear in its own cell.
  const srSuffix = `, ${[ecosystemLabel(row.ecosystem), ...tags].join(', ')}`;
  const host = hostLabel(row.ecosystem);

  const onContextMenu = (e: MouseEvent) => {
    e.preventDefault();
    onMenu(e.clientX, e.clientY);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) {
      e.preventDefault();
      const r = (e.target as HTMLElement).getBoundingClientRect();
      onMenu(r.left, r.bottom);
    }
  };

  const name = (
    <span className="deps-name" title={row.name}>
      {row.name}
    </span>
  );

  return (
    <>
      <tr
        className={`deps-row${expanded ? ' is-expanded' : ''}`}
        onContextMenu={onContextMenu}
        onKeyDown={onKeyDown}
      >
        <td className="deps-cell deps-cell-name">
          <span className="deps-name-wrap">
            {row.vulns.length > 0 ? (
              <button
                type="button"
                className="deps-row-toggle"
                aria-expanded={expanded}
                aria-controls={detailsId}
                aria-label={rowToggleName(
                  row.name,
                  SEVERITY_LABEL[row.vulns[0].severity],
                  row.vulns[0].id,
                  row.vulns.length - 1,
                )}
                onClick={onToggle}
              >
                <span className="deps-chevron" aria-hidden="true">
                  ▸
                </span>
                {name}
              </button>
            ) : (
              <span className="deps-name-plain">{name}</span>
            )}
            {tags.map((t) => (
              <span key={t} aria-hidden="true" className="deps-tag">
                <RowTag>{t}</RowTag>
              </span>
            ))}
            <span className="sr-only">{srSuffix}</span>
          </span>
        </td>
        <td className="deps-cell mono" title={version}>
          {version}
        </td>
        {offline ? (
          <td className="deps-cell" colSpan={3}>
            <AvailabilityCell row={row} reportWide={reportWide} />
          </td>
        ) : kind === 'unknown' ? (
          <>
            <td className="deps-cell" colSpan={2}>
              <AvailabilityCell row={row} reportWide={reportWide} />
            </td>
            <td className="deps-cell">
              <VulnCell row={row} />
            </td>
          </>
        ) : (
          <>
            <td
              className={`deps-cell mono ${row.update !== null ? 'deps-latest--newer' : 'deps-latest--same'}`}
              title={row.latest ?? undefined}
            >
              {row.latest ?? ''}
            </td>
            <td className="deps-cell">{row.update !== null && <UpdateKindPill kind={row.update} />}</td>
            <td className="deps-cell">
              <VulnCell row={row} />
            </td>
          </>
        )}
        <td className="deps-cell deps-cell-action">
          {row.registryUrl !== null && !offline && (
            <button
              type="button"
              className="btn-icon deps-row-action"
              aria-label={openOnHost(row.name, host)}
              title={openOnHost(row.name, host)}
              onClick={() => {
                if (row.registryUrl !== null) onOpenUrl(row.registryUrl, host);
              }}
            >
              <ExternalLinkIcon />
            </button>
          )}
        </td>
      </tr>
      {expanded && row.vulns.length > 0 && (
        <tr id={detailsId} className="deps-details">
          <td colSpan={6}>
            <DepsAdvisoryList advisories={row.vulns} onOpen={onOpenUrl} />
          </td>
        </tr>
      )}
    </>
  );
}
