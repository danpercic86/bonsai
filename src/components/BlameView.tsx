import { useMemo } from 'react';
import type { BlameLine } from '../ipc';
import { relativeDate } from '../graph/draw';

/** Short 7-char oid for gutter pills. */
function shortOid(oid: string): string {
  return oid.slice(0, 7);
}

/** One contiguous run of lines sharing a commit — the gutter label renders once
 *  per run (GitHub/GitKraken blame look). */
interface BlameBlock {
  oid: string;
  authorName: string;
  authorTs: number;
  summary: string;
  lines: BlameLine[];
}

/** Collapse consecutive lines with the same commit oid into blocks. */
function groupBlocks(lines: BlameLine[]): BlameBlock[] {
  const blocks: BlameBlock[] = [];
  for (const line of lines) {
    const last = blocks[blocks.length - 1];
    if (last !== undefined && last.oid === line.oid) {
      last.lines.push(line);
    } else {
      blocks.push({
        oid: line.oid,
        authorName: line.authorName,
        authorTs: line.authorTs,
        summary: line.summary,
        lines: [line],
      });
    }
  }
  return blocks;
}

export interface BlameViewProps {
  path: string;
  lines: BlameLine[];
  loading: boolean;
  error: string | null;
  onClose(): void;
  /** Reveal (select + scroll) the commit for a gutter block in the graph. */
  onRevealCommit(oid: string): void;
}

/** Read-only per-line blame overlay (P23d §11.1). Layered over the graph pane
 *  exactly like the diff overlay; presentation-only — RepoWorkspace owns the
 *  fetch + reveal. */
export function BlameView({ path, lines, loading, error, onClose, onRevealCommit }: BlameViewProps) {
  const now = Math.floor(Date.now() / 1000);
  const blocks = useMemo(() => groupBlocks(lines), [lines]);

  return (
    <div className="diff-overlay blame-view" role="region" aria-label={`Blame: ${path}`}>
      <div className="diff-overlay-header">
        <span className="diff-overlay-path mono" title={path}>
          {path}
        </span>
        <span className="diff-overlay-kind">Blame</span>
        <button
          type="button"
          className="btn-icon diff-overlay-close"
          aria-label="Close blame"
          title="Close (Esc)"
          onClick={onClose}
        >
          {'×'}
        </button>
      </div>
      <div className="diff-overlay-body">
        {error !== null ? (
          <div className="diff-placeholder">{error}</div>
        ) : loading && lines.length === 0 ? (
          <div className="diff-slot-loading skeleton-group" aria-hidden="true">
            {Array.from({ length: 6 }, (_, i) => (
              <div key={i} className="skeleton-row" />
            ))}
          </div>
        ) : lines.length === 0 ? (
          <div className="diff-placeholder">No blame data</div>
        ) : (
          <div className={loading ? 'diff-scroll diff-stale' : 'diff-scroll'}>
            <div className="blame-grid">
              {blocks.map((block, bi) => (
                <div key={`${block.oid}:${bi}`} className="blame-block">
                  <button
                    type="button"
                    className="blame-gutter"
                    title={`${shortOid(block.oid)} — ${block.summary}\nClick to reveal in graph`}
                    onClick={() => onRevealCommit(block.oid)}
                  >
                    <span className="blame-oid mono">{shortOid(block.oid)}</span>
                    <span className="blame-author">{block.authorName}</span>
                    <span className="blame-date">{relativeDate(block.authorTs, now)}</span>
                  </button>
                  <div className="blame-code">
                    {block.lines.map((line) => (
                      <div key={line.finalLineNo} className="blame-line">
                        <span className="blame-lineno mono">{line.finalLineNo}</span>
                        <span className="blame-text mono">
                          {line.lineText === '' ? ' ' : line.lineText}
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
