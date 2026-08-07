import { useEffect, useRef, useState } from 'react';
import type { SearchField, SearchQuery, SearchResults } from '../ipc';
import { Combobox } from './Combobox';
import type { ComboboxOption } from './Combobox';
import { SearchResultsList } from './SearchResultsList';

const FIELD_OPTIONS: { value: SearchField; label: string }[] = [
  { value: 'all', label: 'All' },
  { value: 'message', label: 'Message' },
  { value: 'author', label: 'Author' },
  { value: 'path', label: 'Path' },
  { value: 'content', label: 'Content' },
];

export interface CommitSearchBarProps {
  query: SearchQuery;
  patchQuery(patch: Partial<SearchQuery>): void;
  submit(): void;
  close(): void;
  results: SearchResults | null;
  loading: boolean;
  error: string | null;
  currentMatch: number;
  needsSubmit: boolean;
  next(): void;
  prev(): void;
  goToMatch(index: number): void;
  /** Branch/ref scope options; `value: ''` == all refs. */
  scopeOptions: ComboboxOption[];
  /** Bumped on every openSearch() so the input refocuses even when already open. */
  openNonce: number;
}

/** P50b: the commit-search bar, rendered at the top of the graph pane while
 *  search is open. Presentational — all state comes from useCommitSearch. Regex
 *  is gated to content mode (OQ2); content is submit-only (Enter/Search button),
 *  cheap modes live-search via the hook's debounce. Enter jumps to the next
 *  match, Shift+Enter to the previous (wrap-around); Esc closes when the input
 *  is focused (capture-phase, so the workspace Esc-layering never fires under
 *  it), otherwise the workspace layering closes it. */
export function CommitSearchBar({
  query,
  patchQuery,
  submit,
  close,
  results,
  loading,
  error,
  currentMatch,
  needsSubmit,
  next,
  prev,
  goToMatch,
  scopeOptions,
  openNonce,
}: CommitSearchBarProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [showList, setShowList] = useState(false);

  const isContent = query.field === 'content';
  const total = results?.matches.length ?? 0;
  const hasMatches = total > 0;
  const position = total === 0 ? 0 : currentMatch + 1;

  // Focus + select on mount and on every re-open (openNonce bump) — so Ctrl/Cmd-F
  // while the bar is already open (focus in the graph) refocuses like native find.
  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, [openNonce]);

  // Capture-phase Escape (Combobox idiom): close only when the input is focused,
  // so it beats the workspace Esc-layering (which bails on inputs). When focus is
  // elsewhere, this is inert and the workspace layering closes search in order.
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

  const onInputKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      if (needsSubmit) {
        submit();
        return;
      }
      if (e.shiftKey) prev();
      else next();
    }
    // Escape handled by the capture-phase window listener above.
  };

  return (
    <div className="commit-search" role="search">
      <div className="commit-search-bar">
        <span className="commit-search-icon" aria-hidden="true">
          ⌕
        </span>
        <input
          ref={inputRef}
          className="commit-search-input"
          type="text"
          spellCheck={false}
          autoComplete="off"
          placeholder="Search commits…"
          aria-label="Search commits"
          value={query.text}
          onChange={(e) => patchQuery({ text: e.target.value })}
          onKeyDown={onInputKeyDown}
        />

        <select
          className="commit-search-field"
          aria-label="Search field"
          value={query.field}
          onChange={(e) => patchQuery({ field: e.target.value as SearchField })}
        >
          {FIELD_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>

        <button
          type="button"
          className={'search-toggle' + (query.caseSensitive ? ' is-active' : '')}
          aria-pressed={query.caseSensitive}
          title="Match case"
          onClick={() => patchQuery({ caseSensitive: !query.caseSensitive })}
        >
          Aa
        </button>
        <button
          type="button"
          className={'search-toggle' + (query.regex && isContent ? ' is-active' : '')}
          aria-pressed={isContent && query.regex}
          disabled={!isContent}
          title={isContent ? 'Regular expression (git -G)' : 'Regex applies to Content mode only'}
          onClick={() => patchQuery({ regex: !query.regex })}
        >
          .*
        </button>

        {scopeOptions.length > 1 && (
          <div className="commit-search-scope">
            <Combobox
              options={scopeOptions}
              value={query.scopeRef ?? ''}
              onChange={(v) => patchQuery({ scopeRef: v === '' ? null : v })}
              ariaLabel="Search scope"
            />
          </div>
        )}

        {isContent && (
          <button type="button" className="search-run" onClick={() => submit()}>
            Search
          </button>
        )}

        <span className="commit-search-count" aria-live="polite">
          {loading ? '…' : results === null ? '' : `${position}/${total}`}
        </span>
        {results?.truncated && (
          <span className="commit-search-truncated" title="More matches exist — refine your query">
            showing first 1000
          </span>
        )}

        <button
          type="button"
          className="search-nav"
          title="Previous match (Shift+Enter)"
          disabled={!hasMatches}
          onClick={() => prev()}
        >
          ↑
        </button>
        <button
          type="button"
          className="search-nav"
          title="Next match (Enter)"
          disabled={!hasMatches}
          onClick={() => next()}
        >
          ↓
        </button>
        <button
          type="button"
          className={'search-toggle' + (showList ? ' is-active' : '')}
          aria-pressed={showList}
          title="Toggle results list"
          disabled={results === null}
          onClick={() => setShowList((s) => !s)}
        >
          ☰
        </button>
        <button type="button" className="search-close" title="Close (Esc)" onClick={() => close()}>
          ✕
        </button>
      </div>

      {error !== null && <div className="commit-search-error">{error}</div>}

      {showList && results !== null && (
        <SearchResultsList matches={results.matches} currentMatch={currentMatch} onSelect={goToMatch} />
      )}
    </div>
  );
}
