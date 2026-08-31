// P77 §1.2/§2.5: a small count/status pill for a SectionHeader's `extra` slot,
// so a collapsed section can still surface a problem. Status-agnostic and
// reusable (Branches/Remotes could adopt it): callers pass the divergence
// `count`, a `busy` flag + its `label`, and the accessible name. Reuses the
// existing `.submodule-badge-*` recipes (no new token).

/** Renders (in priority order):
 *  - busy → a hueless `checking…`-style muted pill with `aria-busy` (no spinner);
 *  - count > 0 → a `⚠ {count}` warning pill named by `ariaLabel`;
 *  - otherwise → nothing (clean section: no badge). */
export function SectionRollupBadge({
  count,
  busy,
  label,
  ariaLabel,
  title,
}: {
  count: number;
  busy: boolean;
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
