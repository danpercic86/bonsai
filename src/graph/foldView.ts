/** Spec-004: GraphCanvas-facing fold view-model + small pure mapping helpers,
 *  kept out of GraphCanvas.tsx (file-size ratchet). All indices are explicit:
 *  "model" = wire layout rows, "display" = folded view rows. */

import type { FoldSpan, GraphNode } from '../ipc';
import { modelToDisplay, spanContaining } from './foldModel';
import type { FoldModel } from './foldModel';
import { foldCountLabel } from './drawFold';

/** The fold bundle RepoWorkspace threads into GraphCanvas (undefined = fold
 *  inactive; every internal path then short-circuits to identity). */
export interface GraphFoldView {
  model: FoldModel;
  /** Expanded spans — their `start` rows carry the boundary collapse pill. */
  expandedSpans: readonly FoldSpan[];
  /** Keyboard-active pill, keyed by span start (§3 land-don't-select). */
  activePillStart: number | null;
  /** Expand/collapse the span with this `start` (pill + boundary-pill clicks). */
  onToggleSpan(start: number): void;
}

/** Model selection → display paint state: the visible display row, or (when
 *  the selection is hidden inside a collapsed run, §6) the pill row that must
 *  carry the selection ring instead. */
export function displaySelection(
  model: FoldModel | null,
  selectedIndex: number | null,
): { row: number | null; pillRow: number | null } {
  if (selectedIndex === null) return { row: null, pillRow: null };
  if (model === null) return { row: selectedIndex, pillRow: null };
  const d = modelToDisplay(model, selectedIndex);
  if (spanContaining(model, selectedIndex) !== null) return { row: null, pillRow: d };
  return { row: d, pillRow: null };
}

/** Map model match rows (search / ask-history rings) to display rows; matches
 *  hidden inside collapsed runs are dropped (no ring on the pill — the pill
 *  already communicates elision). */
export function mapMatchRows(
  model: FoldModel | null,
  rows: ReadonlySet<number> | null,
): ReadonlySet<number> | null {
  if (rows === null || model === null) return rows;
  const out = new Set<number>();
  for (const r of rows) {
    if (spanContaining(model, r) === null) out.add(modelToDisplay(model, r));
  }
  return out.size > 0 ? out : null;
}

/** Map a single model row to its display row for paint (reveal flash); hidden
 *  rows flash their pill row (the reveal path auto-expands first, so this is a
 *  defensive fallback only). */
export function displayRowFor(model: FoldModel | null, row: number): number {
  return model === null ? row : modelToDisplay(model, row);
}

/** Accessible name + aria-expanded for the single sr-only active-descendant
 *  row element (ui-designer ruling on WCAG 4.1.2: `graph-row-{n}` must resolve
 *  to a real element). Pill rows get the §3 name; a boundary row appends the
 *  collapse hint; plain commit rows carry their row text. */
export function activeRowA11y(
  row: number,
  nodes: readonly GraphNode[],
  foldRows: ReadonlyMap<number, FoldSpan> | null,
  boundaryRows: ReadonlyMap<number, FoldSpan> | null,
): { label: string; expanded: boolean | undefined } {
  const span = foldRows?.get(row);
  if (span !== undefined) {
    return {
      label: `${foldCountLabel(span.count)} commits folded. Press Enter to expand.`,
      expanded: false,
    };
  }
  const node = nodes[row];
  const base = node !== undefined ? `${node.summary} — ${node.author}` : '';
  const boundary = boundaryRows?.get(row);
  if (boundary !== undefined) {
    return {
      label: `${base}, start of an expanded run of ${foldCountLabel(boundary.count)} commits. Press Left Arrow to collapse.`,
      expanded: true,
    };
  }
  return { label: base, expanded: undefined };
}

/** §1/§2 cursor affordance: 'pointer' over a fold-pill row or over a boundary
 *  row's collapse pill, else ''. */
export function foldCursorFor(
  row: number | null,
  x: number,
  foldRowSet: ReadonlySet<number> | null,
  boundaryRows: ReadonlyMap<number, FoldSpan> | null,
  hitBoundaryPill: (span: FoldSpan, x: number) => boolean,
): string {
  if (row === null || row < 0) return '';
  if (foldRowSet?.has(row) === true) return 'pointer';
  const boundary = boundaryRows?.get(row);
  if (boundary !== undefined && hitBoundaryPill(boundary, x)) return 'pointer';
  return '';
}

/** The MODEL rows of the visible display window's commit rows (fold rows
 *  skipped) — feeds the verify-badge request so a giant collapsed run never
 *  balloons into a giant verify range. */
export function visibleModelRows(
  model: FoldModel,
  firstDisplay: number,
  lastDisplay: number,
  foldRows: ReadonlyMap<number, FoldSpan>,
  displayNodes: number,
): number[] {
  const rows: number[] = [];
  const last = Math.min(lastDisplay, displayNodes - 1);
  for (let d = Math.max(0, firstDisplay); d <= last; d++) {
    if (foldRows.has(d)) continue;
    // Visible display commit rows map back exactly (never a pill here).
    rows.push(displayToModelRow(model, d));
  }
  return rows;
}

/** Inverse mapping for a KNOWN-commit display row (see foldModel.displayToModel;
 *  duplicated tiny wrapper to avoid re-importing the union at call sites). */
function displayToModelRow(model: FoldModel, d: number): number {
  // Binary search over pill rows: shrink applied by the last pill at/before d.
  let lo = 0;
  let hi = model.pillRows.length - 1;
  let i = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (model.pillRows[mid] <= d) {
      i = mid;
      lo = mid + 1;
    } else hi = mid - 1;
  }
  return i < 0 ? d : d + model.shrink[i];
}
