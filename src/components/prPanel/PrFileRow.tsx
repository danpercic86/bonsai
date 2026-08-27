import type { FileDiffHeader, FileStatus } from '../../ipc';

// P89 (revised): one changed-file row in the PR detail's changed-files section.
// Now a COMPACT, non-expanding row (status badge + path + ±counts) — the diff
// bodies render in the center-pane DiffBrowser (pr mode). Clicking a row simply
// (re)opens that browser; while the local diff is unresolved (`onOpen`
// undefined) the row renders as a plain, non-interactive line.

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
  /** (Re)open the center-pane PR diff browser. Undefined = not yet openable. */
  onOpen?(): void;
}

export function PrFileRow({ header, onOpen }: PrFileRowProps) {
  const isRename = header.origPath !== null;
  const title = isRename ? `${header.origPath} → ${header.path}` : header.path;

  const content = (
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
    <li className="pr-file-row">
      {onOpen !== undefined ? (
        <button
          type="button"
          className={`diff-card-header file-status-${header.status}`}
          title={`${title} — view diff in the center pane`}
          onClick={onOpen}
        >
          {content}
        </button>
      ) : (
        <div className={`diff-card-header file-status-${header.status}`} title={title}>
          {content}
        </div>
      )}
    </li>
  );
}
