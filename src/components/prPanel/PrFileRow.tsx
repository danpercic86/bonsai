import type { FileDiffHeader, FileStatus } from '../../ipc';

// P89/P93: one changed-file row in the PR detail's changed-files section. A flat
// single-line row (status badge + path + ±counts) that OPENS the file's diff in
// the center DiffOverlay over the graph — the same interaction the working-dir
// Changes list has. Nothing expands inline any more (P93): loading, error and
// the hunks all live in the overlay, so this file is purely presentational and
// holds no fetch state at all.
//
// A `binary: true` header renders a NON-interactive <span> (mirroring
// StatusFileRow's non-expandable branch): there is no text diff to show, so the
// row is not clickable and not in the tab order.

const BADGES: Record<FileStatus, string> = {
  added: 'A',
  modified: 'M',
  deleted: 'D',
  renamed: 'R',
  typechange: 'T',
  untracked: 'U',
  conflicted: 'C',
};

export interface PrFileRowProps {
  header: FileDiffHeader;
  /** This file's diff is the one currently open in the center overlay. */
  active: boolean;
  onOpen(header: FileDiffHeader): void;
}

export function PrFileRow({ header, active, onOpen }: PrFileRowProps) {
  const isRename = header.origPath !== null;
  const title = isRename ? `${header.origPath} → ${header.path}` : header.path;

  const inner = (
    <>
      <span className="file-badge mono">{BADGES[header.status]}</span>
      {isRename ? (
        <span className="diff-card-path mono file-rename">
          {header.origPath} {'→'} {header.path}
        </span>
      ) : (
        <span className="diff-card-path mono">{header.path}</span>
      )}
      <span className="file-counts mono">
        {header.binary ? (
          <span className="file-count-bin">bin</span>
        ) : (
          <>
            <span className="file-count-add">+{header.additions}</span>
            <span className="file-count-del">−{header.deletions}</span>
          </>
        )}
      </span>
    </>
  );

  return (
    // P93 §10: `diff-card-collapsed` is STATIC — the row has no body, ever, so
    // it is permanently collapsed. The house rule drops the header's duplicate
    // bottom border and rounds every corner (diff-browser.css:197).
    <li
      className={`diff-card diff-card-collapsed pr-file-row${
        active ? ' pr-file-row-active' : ''
      }`}
    >
      {header.binary ? (
        <span
          className={`diff-card-header file-status-${header.status} pr-file-row-binary`}
          title="Binary file — no text diff"
        >
          {inner}
        </span>
      ) : (
        <button
          type="button"
          className={`diff-card-header file-status-${header.status}`}
          title={title}
          aria-expanded={active}
          onClick={() => onOpen(header)}
        >
          {inner}
        </button>
      )}
    </li>
  );
}
