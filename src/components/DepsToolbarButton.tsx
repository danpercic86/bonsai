// P125-ui §3 — the header-toolbar Dependencies entry. Presentational: the
// view comes from App's single `useDepTracker` mount. The count chip is the
// ONLY out-of-dialog summary signal and shows vulnerabilities only (updates
// are routine and would turn it into noise — ui-reference §11 count chip).
import type { DepTrackerView } from '../ipc/types/depTracker';
import { DependenciesIcon } from './appIcons';
import { chipCount, toolbarNaming } from './deps/depsView';

/** U+2009 between the glyph and the count (built, not literal: lint). */
const THIN_SPACE = String.fromCharCode(0x2009);

export interface DepsToolbarButtonProps {
  view: DepTrackerView | null;
  onOpen(): void;
  /** Injectable clock for the `checked {age}` title (tests). */
  nowMs?: number;
}

export function DepsToolbarButton({ view, onOpen, nowMs }: DepsToolbarButtonProps) {
  const n = toolbarNaming(view, nowMs ?? Date.now());
  return (
    <button
      type="button"
      className={`btn-icon deps-toggle${n.chip !== null ? ' has-chip' : ''}`}
      onClick={onOpen}
      aria-label={n.label}
      title={n.title}
      aria-busy={n.busy ? true : undefined}
    >
      <DependenciesIcon />
      {n.chip !== null && (
        <span className="deps-count-chip" aria-hidden="true">
          <span className="deps-count-glyph">⊘</span>
          {THIN_SPACE}
          {chipCount(n.chip)}
        </span>
      )}
    </button>
  );
}
