// P125-ui §4.4 — the expanded advisory details under a vulnerable row.
// `summary` is plain text (never markdown/HTML [LOW-3]); URLs come from Rust.
import type { DepAdvisory } from '../../ipc/types/depTracker';
import { ADVISORY_HOST, NO_SUMMARY, OPEN_ADVISORY, cvss, openAdvisory } from './depsCopy';
import { SeverityPill } from './DepsPills';

export function DepsAdvisoryList({
  advisories,
  onOpen,
}: {
  advisories: readonly DepAdvisory[];
  onOpen(url: string, host: string): void;
}) {
  return (
    <ul className="deps-advisories">
      {advisories.map((a) => (
        <li key={a.id} className="deps-advisory">
          <div className="deps-advisory-head">
            <SeverityPill severity={a.severity} />
            <span className="mono deps-advisory-id" title={a.id}>
              {a.id}
            </span>
            {a.cvssScore !== null && <span className="deps-advisory-cvss">{cvss(a.cvssScore)}</span>}
            {a.aliases.length > 0 && (
              <span className="mono deps-advisory-aliases" title={a.aliases.join(', ')}>
                {a.aliases.join(', ')}
              </span>
            )}
          </div>
          {a.summary !== null ? (
            <p className="deps-advisory-summary" title={a.summary}>
              {a.summary}
            </p>
          ) : (
            <p className="deps-advisory-summary deps-advisory-summary--none">{NO_SUMMARY}</p>
          )}
          <button
            type="button"
            className="deps-link"
            aria-label={openAdvisory(a.id)}
            onClick={() => onOpen(a.url, ADVISORY_HOST)}
          >
            {OPEN_ADVISORY}
          </button>
        </li>
      ))}
    </ul>
  );
}
