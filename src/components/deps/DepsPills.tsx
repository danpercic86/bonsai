// P125-ui §4.5 — the dependency tracker's pills. Existing hues only, and the
// label is always the word: a pill is never `role="img"` (ui-reference §11).
import type { DepRow, DepSeverity, DepUpdateKind } from '../../ipc/types/depTracker';
import { NOT_VERIFIED, SEVERITY_LABEL, UPDATE_LABEL } from './depsCopy';
import { availabilityWords } from './depsView';

/** Critical is SOLID; high a danger verdict (⊘); medium/low a warning verdict
 *  (⚠); unscored is hueless and reads `unrated`. */
export function SeverityPill({ severity }: { severity: DepSeverity }) {
  const glyph = severity === 'high' ? '⊘' : severity === 'medium' || severity === 'low' ? '⚠' : null;
  return (
    <span className={`deps-pill deps-sev deps-sev--${severity}`}>
      {glyph !== null && (
        <span className="deps-pill-glyph" aria-hidden="true">
          {glyph}
        </span>
      )}
      {SEVERITY_LABEL[severity]}
    </span>
  );
}

/** `major` carries the accent tint; minor/patch are hueless facts. */
export function UpdateKindPill({ kind }: { kind: DepUpdateKind }) {
  return (
    <span className={`deps-pill ${kind === 'major' ? 'deps-pill--major' : 'deps-pill--neutral'}`}>
      {UPDATE_LABEL[kind]}
    </span>
  );
}

/** A hueless tag/fact pill (`transitive`, `dev`, `+2`, `not verified`, `stale`). */
export function RowTag({ children, title }: { children: string; title?: string }) {
  return (
    <span className="deps-pill deps-pill--neutral" title={title}>
      {children}
    </span>
  );
}

/** §8.3: an unverified row never renders like a clean one. */
export function NotVerifiedPill({ title }: { title: string }) {
  return <RowTag title={title}>{NOT_VERIFIED}</RowTag>;
}

/** The spanned availability cell of an unknown / local / private row. */
export function AvailabilityCell({ row, reportWide }: { row: DepRow; reportWide: boolean }) {
  const words = availabilityWords(row, reportWide);
  if (words === null) return null;
  const hostTitle = words.reason?.host ?? null;
  const title = [words.title, hostTitle].filter((t): t is string => t !== null).join(' ');
  return (
    <span className="deps-avail" title={title === '' ? undefined : title}>
      <span className="deps-pill deps-pill--neutral">{words.pill}</span>
      {words.reason !== null && (
        <span className="deps-avail-reason">
          {words.reason.prefix}
          {words.reason.host !== null && (
            <>
              {' '}
              <span className="mono deps-avail-host">{words.reason.host}</span>
            </>
          )}
        </span>
      )}
    </span>
  );
}
