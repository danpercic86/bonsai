/** P111 §3 — the shared leaf-preserving ref/path label.
 *
 *  A ref is identified by its *tail* (`feature/api/retry-budget` vs
 *  `feature/api/retry-window`), so plain `text-overflow: ellipsis` clips exactly
 *  the part that names the thing. This renders the leading segments and the leaf
 *  as two spans: the head ellipsizes, the leaf never does.
 *
 *  **The truncation is CSS-only.** `value` is never sliced, never shortened with
 *  `…` in JS, never measured — the DOM always holds the whole string, which is
 *  what keeps the accessible name correct (P111 §5). An interactive host must
 *  still carry its own `aria-label`, because name computation over the two spans
 *  can insert a separating space.
 */
import { splitPath } from './StatusFileRow';

export interface RefLabelProps {
  /** The complete ref or path. Rendered whole into the DOM; CSS does the truncating. */
  value: string;
  /** Class on the wrapper, so each surface keeps its own font/colour. */
  className?: string;
  /** Omit `title` where an ancestor already owns the tooltip (the dock row does). */
  withTitle?: boolean;
}

export function RefLabel({ value, className, withTitle = false }: RefLabelProps) {
  const { dir, name } = splitPath(value);
  return (
    <span
      className={`ref-label${className !== undefined && className !== '' ? ` ${className}` : ''}`}
      title={withTitle ? value : undefined}
    >
      {dir !== null && <span className="ref-label-head">{dir}</span>}
      <span className="ref-label-leaf">{name}</span>
    </span>
  );
}
