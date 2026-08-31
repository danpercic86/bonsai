// P69k — UI §3.2: `<mark class="settings-match">` around the matched substrings
// of a result row's LABEL.
//
// Labels only. Help text is not highlighted (too noisy at 12px), and a term that
// only matched `keywords` — which are never displayed — simply produces no mark:
// the row still appears, it just has nothing visible to point at.
//
// Overlapping hits from different terms are merged before wrapping, so `set spend`
// over "Set a spend limit per run" yields two marks, never a nested pair.

import type { ReactNode } from 'react';

type Range = { start: number; end: number };

/** Every occurrence of every term, merged into non-overlapping ranges. */
function matchRanges(lower: string, terms: readonly string[]): Range[] {
  const found: Range[] = [];
  for (const term of terms) {
    if (term === '') continue;
    let at = lower.indexOf(term);
    while (at !== -1) {
      found.push({ start: at, end: at + term.length });
      at = lower.indexOf(term, at + term.length);
    }
  }
  found.sort((a, b) => a.start - b.start || a.end - b.end);

  const merged: Range[] = [];
  for (const range of found) {
    const last = merged[merged.length - 1];
    if (last !== undefined && range.start <= last.end) {
      last.end = Math.max(last.end, range.end);
    } else {
      merged.push({ ...range });
    }
  }
  return merged;
}

/**
 * `text` with each matched substring wrapped in `<mark class="settings-match">`.
 * Returns the plain string when nothing matches, so a non-search render stays
 * byte-identical to what it was before search shipped.
 */
export function highlightTerms(text: string, terms: readonly string[]): ReactNode {
  if (terms.length === 0) return text;
  const ranges = matchRanges(text.toLowerCase(), terms);
  if (ranges.length === 0) return text;

  const parts: ReactNode[] = [];
  let cursor = 0;
  for (const [i, range] of ranges.entries()) {
    if (range.start > cursor) parts.push(text.slice(cursor, range.start));
    parts.push(
      <mark className="settings-match" key={`m${String(i)}`}>
        {text.slice(range.start, range.end)}
      </mark>,
    );
    cursor = range.end;
  }
  if (cursor < text.length) parts.push(text.slice(cursor));
  return parts;
}
