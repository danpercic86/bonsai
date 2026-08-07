import { useEffect, useRef, useState } from 'react';

// P50d — a small inline type-to-filter box shown under a sidebar section header
// (Branches / Remotes / Tags). Purely presentational: the owning section holds
// the query string and applies `filterByName`/`filterTree` (see listFilter.ts).

export interface ListFilterInputProps {
  value: string;
  onChange(value: string): void;
  /** Placeholder text; defaults to "Filter…". */
  placeholder?: string;
  /** Accessible label — the box has no visible <label>. */
  ariaLabel?: string;
  /** Optional match count, rendered as a subtle hint while filtering. */
  count?: number;
}

/**
 * Reuses the Combobox capture-phase-Escape idiom (not the component): while the
 * input is focused, a capture-phase window Escape listener CLEARS a non-empty
 * filter and swallows the event so it never reaches the workspace Esc-layering
 * (Combobox.tsx registers its child capture listener first — same ordering).
 * An already-empty filter is left alone so Escape bubbles up to peel other
 * layers. The listener is scoped to focus, so a background filter that still
 * holds text never steals Escape from a dialog/overlay elsewhere.
 */
export function ListFilterInput({
  value,
  onChange,
  placeholder = 'Filter…',
  ariaLabel,
  count,
}: ListFilterInputProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [focused, setFocused] = useState(false);

  useEffect(() => {
    if (!focused) return;
    const onWindowKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      // Nothing to clear — let Escape bubble so it can close other layers.
      if (value === '') return;
      e.stopPropagation();
      e.stopImmediatePropagation();
      e.preventDefault();
      onChange('');
      inputRef.current?.blur();
    };
    window.addEventListener('keydown', onWindowKeyDown, true);
    return () => window.removeEventListener('keydown', onWindowKeyDown, true);
  }, [focused, value, onChange]);

  return (
    <div className="list-filter">
      <input
        ref={inputRef}
        className="list-filter-input"
        type="text"
        role="searchbox"
        aria-label={ariaLabel}
        placeholder={placeholder}
        autoComplete="off"
        spellCheck={false}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
      />
      {count !== undefined && value.trim() !== '' && (
        <span className="list-filter-count" aria-hidden="true">
          {count}
        </span>
      )}
      {value !== '' && (
        <button
          type="button"
          className="list-filter-clear"
          aria-label="Clear filter"
          title="Clear filter"
          // mousedown + preventDefault fires before the input's blur and keeps
          // focus on the input, so the box stays ready for the next keystroke.
          onMouseDown={(e) => {
            e.preventDefault();
            onChange('');
            inputRef.current?.focus();
          }}
        >
          {'×'}
        </button>
      )}
    </div>
  );
}
