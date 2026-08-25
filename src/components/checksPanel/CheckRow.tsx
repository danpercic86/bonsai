// P90: one StatusContext row (§2.3). state glyph · name+description · optional
// external link-out. The row is not itself a button — its only action is the ↗.
import type { StatusContext } from '../../ipc';
import { checkVisual } from './checkVisuals';

export interface CheckRowProps {
  context: StatusContext;
  /** Opens targetUrl via the backend (system browser); reused failure toast. */
  onOpen(url: string): void;
}

export function CheckRow({ context, onOpen }: CheckRowProps) {
  const v = checkVisual(context.state);
  const { name, description, targetUrl } = context;
  const accessible =
    description !== null ? `${v.word}: ${name}, ${description}` : `${v.word}: ${name}`;
  return (
    <li className="checks-row" role="listitem" aria-label={accessible}>
      <span className={`checks-glyph checks-glyph--${v.tone}`} aria-hidden="true">
        {v.glyph}
      </span>
      <span className="checks-row-main">
        <span className="checks-row-name" title={name}>
          {name}
        </span>
        {description !== null && (
          <span className="checks-row-desc" title={description}>
            {description}
          </span>
        )}
      </span>
      {targetUrl !== null && (
        <button
          type="button"
          className="btn-icon checks-row-link"
          aria-label={`Open ${name} in browser`}
          onClick={() => onOpen(targetUrl)}
        >
          ↗
        </button>
      )}
    </li>
  );
}
