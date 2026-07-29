import { useMemo, useState } from 'react';
import { relativeDate } from '../graph/draw';
import type { CommitDiff, FileDiffHeader, FileStatus, GraphNode, ListView } from '../ipc';
import { buildPathTree } from '../utils/pathTree';
import type { DiffSlot } from './DiffView';
import { Tree } from './Tree';

// Mode B (M4 contract §4.3): shown INSTEAD of StatusPanel + CommitBox when a
// graph commit is selected. Presentational — App owns all fetching.

const BADGES: Record<FileStatus, string> = {
  added: 'A',
  modified: 'M',
  deleted: 'D',
  renamed: 'R',
  typechange: 'T',
  untracked: 'U',
  conflicted: 'C',
};

const BODY_COLLAPSE_LINES = 8;

function shortOid(oid: string): string {
  return oid.slice(0, 7);
}

function splitPath(path: string): { dir: string | null; name: string } {
  const idx = path.lastIndexOf('/');
  if (idx === -1) return { dir: null, name: path };
  return { dir: path.slice(0, idx + 1), name: path.slice(idx + 1) };
}

/** Body = message minus its first line and the following blank separator lines
 * (P1 §4.3 — the summary is always derived from line 1 by git2, so an
 * unconditional first-line strip can neither cut mid-line nor duplicate). */
function messageBody(message: string): string {
  const nl = message.indexOf('\n');
  if (nl === -1) return '';
  return message.slice(nl + 1).replace(/^(\r?\n)+/, '').replace(/^\r/, '');
}

function MessageBody({ body }: { body: string }) {
  const [showAll, setShowAll] = useState(false);
  const lines = body.split('\n');
  const collapsed = !showAll && lines.length > BODY_COLLAPSE_LINES;
  const visible = collapsed ? lines.slice(0, BODY_COLLAPSE_LINES).join('\n') : body;
  return (
    <div className="commit-msg-body">
      <pre className="commit-msg-text">{visible}</pre>
      {lines.length > BODY_COLLAPSE_LINES && (
        <button type="button" className="section-action" onClick={() => setShowAll(!showAll)}>
          {collapsed ? 'Show more' : 'Show less'}
        </button>
      )}
    </div>
  );
}

export function FileHeaderRow({
  file,
  expanded,
  onToggle,
  treeMode = false,
}: {
  file: FileDiffHeader;
  expanded: boolean;
  onToggle: () => void;
  /** P3b: the tree supplies directory context — render only the basename
   *  (renames keep the full `orig → path` text; tooltips keep full paths). */
  treeMode?: boolean;
}) {
  const isRename = file.origPath !== null;
  const title = isRename ? `${file.origPath} → ${file.path}` : file.path;
  const { dir, name } = splitPath(file.path);
  return (
    <li
      className={`file-row file-status-${file.status}${expanded ? ' file-row-expanded' : ''}`}
      title={title}
    >
      <button type="button" className="file-row-main" aria-expanded={expanded} onClick={onToggle}>
        <span className={`file-chevron${expanded ? ' file-chevron-open' : ''}`}>{'›'}</span>
        <span className="file-badge mono">{BADGES[file.status]}</span>
        {isRename ? (
          <span className="file-path mono file-rename">
            {file.origPath} {'→'} {file.path}
          </span>
        ) : (
          <span className="file-path">
            {!treeMode && dir !== null && <span className="file-dir">{dir}</span>}
            <span className="file-name">{name}</span>
          </span>
        )}
        <span className="file-counts mono">
          {file.binary ? (
            <span className="file-count-bin">bin</span>
          ) : (
            <>
              <span className="file-count-add">+{file.additions}</span>
              <span className="file-count-del">−{file.deletions}</span>
            </>
          )}
        </span>
      </button>
    </li>
  );
}

export function SkeletonRows() {
  return (
    <div className="skeleton-group" aria-hidden="true">
      {Array.from({ length: 4 }, (_, i) => (
        <div key={i} className="skeleton-row" />
      ))}
    </div>
  );
}

export interface CommitPanelProps {
  /** Selected node (immediate summary/oid while details load). */
  node: GraphNode;
  data: CommitDiff | null; // null while loading
  loading: boolean;
  error: string | null;
  /** Currently open diff (key = `commit:${path}`) — drives row expanded state;
   * the diff itself renders in App's center-pane DiffOverlay (P3a). */
  diffSlot: DiffSlot | null;
  /** P3b: flat vs directory-tree file list (display-only). */
  listView: ListView;
  onToggleDiff(file: FileDiffHeader): void;
  /** Parent short-oid clicked; App maps to a row via node.parents indices. */
  onSelectParent(parentOrdinal: number): void;
  /** "×" button -> deselect. */
  onClose(): void;
}

export function CommitPanel({
  node,
  data,
  loading,
  error,
  diffSlot,
  listView,
  onToggleDiff,
  onSelectParent,
  onClose,
}: CommitPanelProps) {
  const files = data?.files;
  // P3b §5.2: directory tree of the commit's file list (tree mode only).
  const fileNodes = useMemo(
    () =>
      listView === 'tree' && files !== undefined ? buildPathTree(files, (f) => f.path) : null,
    [listView, files],
  );
  const details = data?.details ?? null;
  const now = Math.floor(Date.now() / 1000);
  const body = details !== null ? messageBody(details.message) : '';

  return (
    <div className="commit-panel">
      <div className="commit-panel-header">
        <div className="commit-panel-title">
          <div className="commit-summary">{details?.summary ?? node.summary}</div>
          <button
            type="button"
            className="btn-icon commit-close"
            aria-label="Close commit details"
            title="Close"
            onClick={onClose}
          >
            {'×'}
          </button>
        </div>
        <div className="commit-oid mono" title={details?.oid ?? node.id}>
          {shortOid(details?.oid ?? node.id)}
        </div>
        {details !== null && (
          <>
            <div className="commit-author">
              {details.authorName}{' '}
              <span className="commit-author-email">{'<'}{details.authorEmail}{'>'}</span>
            </div>
            <div className="commit-date">
              {relativeDate(details.authorTs, now)}
              {' · '}
              {new Date(details.authorTs * 1000).toLocaleString()}
            </div>
            {details.parents.length > 0 && (
              <div className="commit-parents">
                <span className="commit-parents-label">Parents:</span>
                {details.parents.map((p, i) =>
                  node.parents[i] !== undefined ? (
                    <button
                      key={p}
                      type="button"
                      className="commit-parent-link mono"
                      title={p}
                      onClick={() => onSelectParent(i)}
                    >
                      {shortOid(p)}
                    </button>
                  ) : (
                    // Parent truncated out of the layout: plain text, no jump.
                    <span key={p} className="commit-parent-plain mono" title={p}>
                      {shortOid(p)}
                    </span>
                  ),
                )}
              </div>
            )}
            {details.parents.length > 1 && (
              <div className="commit-merge-note">Showing changes vs first parent</div>
            )}
          </>
        )}
      </div>

      {error !== null && (
        <div className="error-banner error-banner-dismissible commit-panel-error" role="alert">
          <span className="error-banner-text">{error}</span>
        </div>
      )}

      {details !== null && body !== '' && <MessageBody body={body} />}

      {loading && data === null ? (
        <div className="commit-panel-loading">
          <SkeletonRows />
        </div>
      ) : (
        data !== null && (
          <section className="status-section commit-files">
            <div className="section-header section-label">
              <span>Changes ({data.files.length})</span>
            </div>
            {fileNodes !== null ? (
              <Tree
                nodes={fileNodes}
                leafKey={(l) => `commit:${l.item.path}`}
                renderLeaf={(l) => (
                  <FileHeaderRow
                    file={l.item}
                    expanded={diffSlot !== null && diffSlot.key === `commit:${l.item.path}`}
                    onToggle={() => onToggleDiff(l.item)}
                    treeMode
                  />
                )}
              />
            ) : (
              <ul className="file-list">
                {data.files.map((file) => {
                  const key = `commit:${file.path}`;
                  const expanded = diffSlot !== null && diffSlot.key === key;
                  return (
                    <FileHeaderRow
                      key={key}
                      file={file}
                      expanded={expanded}
                      onToggle={() => onToggleDiff(file)}
                    />
                  );
                })}
              </ul>
            )}
          </section>
        )
      )}
    </div>
  );
}
