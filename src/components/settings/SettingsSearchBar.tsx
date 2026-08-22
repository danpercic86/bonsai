// P69k — UI §2.2 / §7.1: the search bar above the pane.
//
// Thin wrapper over the shared `ListFilterInput` (already `role="searchbox"`),
// which also brings the capture-phase Escape idiom for free: the first Escape
// clears a non-empty query and swallows the event, an empty one bubbles up and
// closes the dialog — exactly UI §7.2's two-press behaviour, no new mechanism.
//
// It owns one other thing: the visually-hidden `role="status"` line, so the
// result count is announced without the sighted user seeing a duplicate of what
// the rail counts already say.

import { ListFilterInput } from '../ListFilterInput';

export function SettingsSearchBar({
  query,
  matchCount,
  onChange,
}: {
  query: string;
  /** Total matches across every category. Ignored while the query is blank. */
  matchCount: number;
  onChange(query: string): void;
}) {
  const searching = query.trim() !== '';
  const status = !searching
    ? ''
    : matchCount === 0
      ? 'No settings match'
      : matchCount === 1
        ? '1 setting matches'
        : `${String(matchCount)} settings match`;

  return (
    <div className="settings-search">
      <ListFilterInput
        value={query}
        onChange={onChange}
        placeholder="Search settings"
        ariaLabel="Search settings"
      />
      <p className="settings-search-status sr-only" role="status" aria-live="polite">
        {status}
      </p>
    </div>
  );
}
