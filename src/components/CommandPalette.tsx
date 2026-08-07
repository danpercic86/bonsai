import { Fragment, useEffect, useMemo, useRef, useState } from 'react';
import type { PaletteAction, PaletteGroup } from './paletteActions';
import { filterActions } from './paletteActions';

export interface CommandPaletteProps {
  open: boolean;
  actions: PaletteAction[];
  onClose(): void;
  /** The dynamic "Search commits for '<text>'" row. */
  onRunSearch(text: string): void;
  /** The dynamic "Jump to commit <hex>" row (input matches /^[0-9a-f]{4,40}$/). */
  onJumpToCommit(prefix: string): void;
}

/** Display order of the grouped result list (dynamic hex/search rows first, then
 *  the static registry groups). */
const GROUP_ORDER: PaletteGroup[] = ['commit', 'search', 'action', 'branch', 'tag'];
const GROUP_LABELS: Record<PaletteGroup, string> = {
  commit: 'Commit',
  search: 'Search',
  action: 'Actions',
  branch: 'Branches',
  tag: 'Tags',
};

const HEX_RE = /^[0-9a-f]{4,40}$/i;

/** First index in `list` that is enabled, or 0 (so an all-disabled list still has
 *  a defined highlight even though Enter is a no-op there). */
function firstEnabledIndex(list: PaletteAction[]): number {
  const i = list.findIndex((a) => a.disabled !== true);
  return i < 0 ? 0 : i;
}

/**
 * P50c: the Ctrl/Cmd-K command palette — a centered modal over the active repo.
 * Presentational: it receives the full `actions` registry + `onRun*` callbacks
 * and owns only its input text + highlight. Fuzzy-filters over title+keywords
 * (dynamic hex/search rows are always shown), ↑/↓ move (skipping disabled),
 * Enter runs the highlighted row then closes, and a capture-phase window Escape
 * closes ONLY the palette (Combobox idiom) so it beats the workspace Esc-layering
 * and any overlay beneath stays put. Destructive rows are never present (§6.1).
 */
export function CommandPalette({
  open,
  actions,
  onClose,
  onRunSearch,
  onJumpToCommit,
}: CommandPaletteProps) {
  const [text, setText] = useState('');
  const [highlight, setHighlight] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);

  // Reset the query + focus the input on every open (a fresh session each time).
  useEffect(() => {
    if (!open) return;
    setText('');
    inputRef.current?.focus();
  }, [open]);

  // Capture-phase Escape closes only the palette and stops the event, so the
  // workspace Esc-layering (a bubble-phase window listener) never fires under it.
  useEffect(() => {
    if (!open) return;
    const onWindowKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.stopPropagation();
      e.stopImmediatePropagation();
      e.preventDefault();
      onClose();
    };
    window.addEventListener('keydown', onWindowKeyDown, true);
    return () => window.removeEventListener('keydown', onWindowKeyDown, true);
  }, [open, onClose]);

  const trimmed = text.trim();

  // Dynamic rows built from the raw input (never fuzzy-filtered): a hex-looking
  // token offers a commit jump; any non-empty text offers a full commit search.
  const dynamicRows = useMemo<PaletteAction[]>(() => {
    const rows: PaletteAction[] = [];
    if (HEX_RE.test(trimmed)) {
      rows.push({
        id: '__jump',
        title: `Jump to commit ${trimmed}`,
        group: 'commit',
        run: () => onJumpToCommit(trimmed),
      });
    }
    if (trimmed !== '') {
      rows.push({
        id: '__search',
        title: `Search commits for “${trimmed}”`,
        group: 'search',
        run: () => onRunSearch(trimmed),
      });
    }
    return rows;
  }, [trimmed, onJumpToCommit, onRunSearch]);

  const filtered = useMemo(() => filterActions(actions, text), [actions, text]);

  // Flatten into a single ordered list (headers are derived at render time). The
  // highlight indexes into THIS list; header <li>s are extra and not counted.
  const flat = useMemo(() => {
    const all = [...dynamicRows, ...filtered];
    const ordered: PaletteAction[] = [];
    for (const g of GROUP_ORDER) {
      for (const a of all) if (a.group === g) ordered.push(a);
    }
    return ordered;
  }, [dynamicRows, filtered]);

  // Land the highlight on the first enabled row whenever the visible set changes
  // (mirrors Combobox's reset-on-filter-change behavior).
  useEffect(() => {
    setHighlight(firstEnabledIndex(flat));
  }, [flat]);

  // Keep the highlighted row visible when navigating with the keyboard.
  useEffect(() => {
    listRef.current
      ?.querySelector('.command-palette-option.is-active')
      ?.scrollIntoView({ block: 'nearest' });
  }, [highlight]);

  const runEntry = (a: PaletteAction) => {
    if (a.disabled === true) return;
    a.run();
    onClose();
  };

  const move = (dir: 1 | -1) => {
    if (flat.length === 0) return;
    let i = highlight;
    for (let step = 0; step < flat.length; step += 1) {
      i = (i + dir + flat.length) % flat.length;
      if (flat[i]?.disabled !== true) {
        setHighlight(i);
        return;
      }
    }
  };

  const onInputKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      move(1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      move(-1);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const a = flat[highlight];
      if (a !== undefined) runEntry(a);
    }
    // Escape is handled by the capture-phase window listener above.
  };

  if (!open) return null;

  return (
    <div
      className="dialog-overlay command-palette-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="command-palette" role="dialog" aria-modal="true" aria-label="Command palette">
        <input
          ref={inputRef}
          className="command-palette-input"
          type="text"
          role="combobox"
          aria-expanded={true}
          aria-autocomplete="list"
          spellCheck={false}
          autoComplete="off"
          placeholder="Type a command, branch, tag, or commit…"
          aria-label="Command palette"
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={onInputKeyDown}
        />
        <ul className="command-palette-list" role="listbox" ref={listRef}>
          {flat.length === 0 ? (
            <li className="command-palette-empty" role="option" aria-selected={false}>
              No matching commands
            </li>
          ) : (
            flat.map((a, i) => {
              const header =
                i === 0 || flat[i - 1].group !== a.group ? (
                  <li className="command-palette-group" aria-hidden="true">
                    {GROUP_LABELS[a.group]}
                  </li>
                ) : null;
              return (
                <Fragment key={a.id}>
                  {header}
                  <li
                    role="option"
                    aria-selected={i === highlight}
                    aria-disabled={a.disabled === true}
                    className={
                      'command-palette-option' +
                      (i === highlight ? ' is-active' : '') +
                      (a.disabled === true ? ' is-disabled' : '')
                    }
                    // preventDefault keeps focus in the input (so the palette does
                    // not blur-close before the click runs the entry).
                    onMouseDown={(e) => {
                      e.preventDefault();
                      runEntry(a);
                    }}
                    onMouseEnter={() => {
                      if (a.disabled !== true) setHighlight(i);
                    }}
                  >
                    <span className="command-palette-option-title">{a.title}</span>
                    {a.hint !== undefined && (
                      <span className="command-palette-option-hint">{a.hint}</span>
                    )}
                  </li>
                </Fragment>
              );
            })
          )}
        </ul>
      </div>
    </div>
  );
}
