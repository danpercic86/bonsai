// P77 §1.2/§2.5: a small count/status pill for a SectionHeader's `extra` slot,
// so a collapsed section can still surface a problem. Status-agnostic and
// reusable (Branches/Remotes could adopt it): callers pass the divergence
// `count`, a `busy` flag + its `label`, and the accessible name. Reuses the
// existing `.submodule-badge-*` recipes (no new token).

/** Renders (in priority order):
 *  - busy → a hueless `checking…`-style muted pill with `aria-busy` (no spinner);
 *    wins over `refreshing`;
 *  - count > 0 → a `⚠ {count}` warning pill (`role="img"`, named by `ariaLabel`);
 *    when `refreshing`, the SAME pill plus `aria-busy` (static dashed outline —
 *    P123 "Re-check pill"), so a re-check never swaps the verdict away;
 *  - otherwise → nothing (clean section: no badge), refreshing or not. */
export function SectionRollupBadge({
  count,
  busy,
  refreshing = false,
  label,
  ariaLabel,
  title,
}: {
  count: number;
  busy: boolean;
  /** A verdict exists and a re-check is in flight (P123). The caller folds the
   *  re-check into `ariaLabel`/`title`; the badge stays status-agnostic. */
  refreshing?: boolean;
  /** Text shown while `busy` (e.g. "checking…"). */
  label: string;
  /** Accessible name for the count pill (`⚠ {N}` alone is ambiguous to a reader). */
  ariaLabel: string;
  /** Hover tooltip for the count pill. Kept separate from `ariaLabel` so callers
   *  can surface an actionable hint (§5) without changing the screen-reader name;
   *  falls back to `ariaLabel` when omitted. */
  title?: string;
}) {
  if (busy) {
    return (
      <span className="branch-badge submodule-badge-muted" aria-busy="true">
        <span>{label}</span>
      </span>
    );
  }
  if (count > 0) {
    return (
      <span
        className="branch-badge submodule-badge-warn"
        role="img"
        aria-busy={refreshing ? 'true' : undefined}
        aria-label={ariaLabel}
        title={title ?? ariaLabel}
      >
        <span className="submodule-badge-glyph" aria-hidden="true">
          {'⚠'}
        </span>
        <span aria-hidden="true">{count}</span>
      </span>
    );
  }
  return null;
}
