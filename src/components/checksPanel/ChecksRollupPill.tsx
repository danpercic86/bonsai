// P90: the overall rollup verdict pill (§3.2). Count-summary label + glyph over a
// tinted pill; the glyph is aria-hidden and the meaning folds into aria-label.
import type { CommitStatus } from '../../ipc';
import { rollupPill } from './checkVisuals';

export function ChecksRollupPill({ status }: { status: CommitStatus }) {
  const pill = rollupPill(status);
  if (pill === null) return null;
  return (
    <span className={`checks-rollup-pill checks-rollup-pill--${pill.tone}`} aria-label={pill.aria}>
      <span className="checks-rollup-glyph" aria-hidden="true">
        {pill.glyph}
      </span>
      <span className="checks-rollup-label">{pill.label}</span>
    </span>
  );
}
