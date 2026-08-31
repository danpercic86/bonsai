import { useEffect, useRef } from 'react';
import type { IndexProgress } from '../ipc';
import type { UseHistorySearch } from './repoWorkspace/useHistorySearch';
import { HistoryResultsList } from './HistoryResultsList';
import { SummarizeIcon } from './menuIcons';

export interface HistorySearchPanelProps {
  historySearch: UseHistorySearch;
  /** Reveal a clicked hit in the graph (shared P50 reveal path). */
  revealCommitByOid(oid: string): void;
}

/** Human phase label for the build progress bar. */
function progressLabel(p: IndexProgress | null): string {
  if (p === null) return 'Preparing…';
  switch (p.phase) {
    case 'counting':
      return 'Counting commits…';
    case 'extracting':
      return p.total > 0
        ? `Indexing commits ${p.processed.toLocaleString()}/${p.total.toLocaleString()}`
        : 'Indexing commits…';
    case 'writing':
      return 'Saving index…';
    case 'done':
      return 'Done';
  }
}

/** Fill percent for the build progress bar (0 total ⇒ an indeterminate stub). */
function progressPct(p: IndexProgress | null): number {
  if (p === null) return 8;
  if (p.phase === 'writing' || p.phase === 'done') return 100;
  if (p.total <= 0) return 8;
  return Math.max(4, Math.min(100, Math.round((p.processed / p.total) * 100)));
}

/** P57c: the "Ask history" overlay — an index-status line (prepare / progress /
 *  indexed+rebuild), a question input, Search (retrieval) + Ask AI (synthesis)
 *  actions, inline error, and the ranked results list. Presentational: all state
 *  lives in `useHistorySearch` (no direct ipc). Esc closes when the input is
 *  focused (capture-phase, so the workspace Esc-layering never fires under it). */
export function HistorySearchPanel({ historySearch, revealCommitByOid }: HistorySearchPanelProps) {
  const {
    status,
    building,
    progress,
    build,
    query,
    setText,
    hits,
    searching,
    error,
    searched,
    search,
    canAsk,
    askAi,
    close,
  } = historySearch;
  const inputRef = useRef<HTMLInputElement>(null);

  // Autofocus on mount.
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Capture-phase Escape (CommitSearchBar idiom): close only when the input is
  // focused, so it beats the workspace Esc-layering (which bails on inputs).
  useEffect(() => {
    const onWindowKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (document.activeElement !== inputRef.current) return;
      e.stopPropagation();
      e.stopImmediatePropagation();
      e.preventDefault();
      close();
    };
    window.addEventListener('keydown', onWindowKeyDown, true);
    return () => window.removeEventListener('keydown', onWindowKeyDown, true);
  }, [close]);

  const built = status !== null && status.built;
  const emptyQuery = query.text.trim() === '';

  const onInputKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      search();
    }
    // Escape handled by the capture-phase window listener above.
  };

  return (
    <div className="history-search" role="search" aria-label="Ask history">
      <div className="history-search-status">
        {building ? (
          <div className="index-progress">
            <span className="index-progress-label">{progressLabel(progress)}</span>
            <span className="index-progress-track" aria-hidden="true">
              <span className="index-progress-fill" style={{ width: `${progressPct(progress)}%` }} />
            </span>
          </div>
        ) : status !== null && status.built ? (
          <span className="history-search-indexed">
            Indexed {status.indexedCommits.toLocaleString()} commits
            {status.stale && (
              <>
                {' · '}
                {status.newCommits > 0 ? `${status.newCommits.toLocaleString()} new` : 'stale'}{' '}
                <button type="button" className="history-rebuild" onClick={() => build()}>
                  Rebuild
                </button>
              </>
            )}
          </span>
        ) : (
          <button type="button" className="btn-secondary history-prepare" onClick={() => build()}>
            Prepare history search
          </button>
        )}
      </div>

      <div className="commit-search-bar history-search-bar">
        <span className="commit-search-icon" aria-hidden="true">
          <SummarizeIcon />
        </span>
        <input
          ref={inputRef}
          className="commit-search-input"
          type="text"
          spellCheck={false}
          autoComplete="off"
          placeholder="Ask about the history…"
          aria-label="Ask about the history"
          value={query.text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={onInputKeyDown}
        />
        <button
          type="button"
          className="search-run"
          title="Find the most relevant commits"
          disabled={!built || searching || emptyQuery}
          onClick={() => search()}
        >
          {searching ? '…' : 'Search'}
        </button>
        <button
          type="button"
          className="search-run history-ask"
          title={canAsk ? 'Ask AI to answer from the history' : 'Build the index and enable AI first'}
          disabled={!canAsk || emptyQuery}
          onClick={() => askAi()}
        >
          <SummarizeIcon />
          <span>Ask AI</span>
        </button>
        <button type="button" className="search-close" title="Close (Esc)" onClick={() => close()}>
          ✕
        </button>
      </div>

      {error !== null && <div className="commit-search-error">{error}</div>}

      <HistoryResultsList hits={hits} searched={searched} onSelect={revealCommitByOid} />
    </div>
  );
}
