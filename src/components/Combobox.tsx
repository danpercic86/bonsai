import { useEffect, useId, useMemo, useRef, useState } from 'react';

export interface ComboboxOption {
  value: string;
  label: string;
  disabled?: boolean;
  hint?: string;
}

export interface ComboboxProps {
  options: ComboboxOption[];
  value: string;
  onChange(value: string): void;
  /** false (default) = strict select: the input reverts to the selected option's
   *  label on blur/Escape and free text is never committed. true = free-text with
   *  suggestions: `onChange` fires on every keystroke. */
  allowFreeInput?: boolean;
  placeholder?: string;
  disabled?: boolean;
  ariaLabel?: string;
  /** Focus the input on mount (the worktree dialog opens focused here). */
  autoFocus?: boolean;
  id?: string;
}

/** First index in `list` that isn't disabled, or -1. */
function firstEnabled(list: ComboboxOption[]): number {
  return list.findIndex((o) => !o.disabled);
}

/**
 * Reusable searchable single-select combobox (filter-as-you-type). Strict mode
 * (worktree branch picker) never commits free text; free mode (WhatChanged ref
 * fields) reports every keystroke and treats options as suggestions. Disabled
 * options stay visible but greyed (e.g. "checked out" branches). When the
 * popover is open, a capture-phase window Escape listener closes only the
 * dropdown and stops the event, so it beats (and doesn't reach) a parent
 * dialog's own capture-phase Escape→cancel; when closed, Escape bubbles up to
 * cancel the parent dialog.
 */
export function Combobox({
  options,
  value,
  onChange,
  allowFreeInput = false,
  placeholder,
  disabled = false,
  ariaLabel,
  autoFocus = false,
  id,
}: ComboboxProps) {
  const reactId = useId();
  const baseId = id ?? reactId;
  const listId = `${baseId}-listbox`;

  const wrapperRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  // Strict mode only: null → show the selected option's label; a string → the
  // user is typing (filter query). Ignored in free mode (input shows `value`).
  const [query, setQuery] = useState<string | null>(null);
  const [highlight, setHighlight] = useState(-1);

  const selectedLabel = useMemo(
    () => options.find((o) => o.value === value)?.label ?? '',
    [options, value],
  );

  // The text shown in the input.
  const inputValue = allowFreeInput ? value : query ?? selectedLabel;
  // The text driving the filter.
  const filterText = allowFreeInput ? value : query ?? '';

  const filtered = useMemo(() => {
    const needle = filterText.trim().toLowerCase();
    if (needle === '') return options;
    return options.filter((o) => o.label.toLowerCase().includes(needle));
  }, [options, filterText]);

  // Reset the highlight to the first enabled row whenever the popover opens or
  // the filtered set changes; arrow keys then move it without a reset.
  useEffect(() => {
    if (open) setHighlight(firstEnabled(filtered));
  }, [open, filtered]);

  // Close on outside mousedown; revert the strict-mode query to the label.
  useEffect(() => {
    if (!open) return;
    const onDocMouseDown = (e: MouseEvent) => {
      if (wrapperRef.current?.contains(e.target as Node)) return;
      setOpen(false);
      setQuery(null);
    };
    document.addEventListener('mousedown', onDocMouseDown);
    return () => document.removeEventListener('mousedown', onDocMouseDown);
  }, [open]);

  // While the popover is open, intercept Escape in the CAPTURE phase on window so
  // it closes only the dropdown — not a parent dialog whose own Escape→cancel is
  // also a capture-phase window listener. Child effects run before parent effects,
  // so this child-registered capture listener fires first and stops the event
  // before the parent sees it. When closed we don't register, so Escape bubbles up
  // to cancel the parent dialog as usual.
  useEffect(() => {
    if (!open) return;
    const onWindowKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.stopPropagation();
      e.stopImmediatePropagation();
      e.preventDefault();
      setOpen(false);
      setQuery(null); // strict mode: revert the query to the selected label
    };
    window.addEventListener('keydown', onWindowKeyDown, true);
    return () => window.removeEventListener('keydown', onWindowKeyDown, true);
  }, [open]);

  const select = (opt: ComboboxOption) => {
    if (opt.disabled) return;
    onChange(opt.value);
    setQuery(null);
    setOpen(false);
  };

  const moveHighlight = (dir: 1 | -1) => {
    if (filtered.length === 0) return;
    let i = highlight;
    for (let step = 0; step < filtered.length; step += 1) {
      i = (i + dir + filtered.length) % filtered.length;
      if (!filtered[i]?.disabled) {
        setHighlight(i);
        return;
      }
    }
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        if (!open) setOpen(true);
        else moveHighlight(1);
        break;
      case 'ArrowUp':
        e.preventDefault();
        if (!open) setOpen(true);
        else moveHighlight(-1);
        break;
      case 'Enter': {
        if (!open) break;
        const opt = filtered[highlight];
        if (opt !== undefined && !opt.disabled) {
          e.preventDefault();
          select(opt);
        }
        break;
      }
      // Escape is handled by a capture-phase window listener while the popover is
      // open (see the useEffect above) so it beats the parent dialog's own
      // capture-phase Escape→cancel; nothing to do here.
      default:
        break;
    }
  };

  const onInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const text = e.target.value;
    if (!open) setOpen(true);
    if (allowFreeInput) onChange(text);
    else setQuery(text);
  };

  const onBlur = () => {
    setOpen(false);
    setQuery(null);
  };

  const activeId = highlight >= 0 ? `${baseId}-opt-${highlight}` : undefined;

  return (
    <div className="combobox" ref={wrapperRef}>
      <input
        id={baseId}
        className="dialog-input"
        type="text"
        role="combobox"
        aria-expanded={open}
        aria-controls={listId}
        aria-autocomplete="list"
        aria-label={ariaLabel}
        aria-activedescendant={activeId}
        autoFocus={autoFocus}
        autoComplete="off"
        spellCheck={false}
        disabled={disabled}
        placeholder={placeholder}
        value={inputValue}
        onChange={onInputChange}
        onFocus={(e) => {
          setOpen(true);
          // Select the pre-filled text so the first keystroke replaces it
          // (search from scratch) instead of appending to the selected label.
          e.currentTarget.select();
        }}
        onKeyDown={onKeyDown}
        onBlur={onBlur}
      />
      {open && (
        <ul className="combobox-popover" role="listbox" id={listId}>
          {filtered.length === 0 ? (
            <li
              className="combobox-option combobox-option--disabled"
              role="option"
              aria-disabled={true}
              aria-selected={false}
            >
              No matches
            </li>
          ) : (
            filtered.map((opt, i) => (
              <li
                key={opt.value}
                id={`${baseId}-opt-${i}`}
                role="option"
                aria-selected={opt.value === value}
                aria-disabled={opt.disabled === true}
                className={
                  'combobox-option' +
                  (i === highlight ? ' combobox-option--active' : '') +
                  (opt.disabled ? ' combobox-option--disabled' : '')
                }
                // preventDefault keeps focus on the input so onBlur doesn't fire
                // (and revert) before the click registers.
                onMouseDown={(e) => {
                  e.preventDefault();
                  select(opt);
                }}
                onMouseEnter={() => {
                  if (!opt.disabled) setHighlight(i);
                }}
              >
                <span className="combobox-option-label">{opt.label}</span>
                {opt.hint !== undefined && (
                  <span className="combobox-option-hint">{opt.hint}</span>
                )}
              </li>
            ))
          )}
        </ul>
      )}
    </div>
  );
}
