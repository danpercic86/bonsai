/** Left-click resolution for the graph canvas — extracted from GraphCanvas's
 *  handleClick (spec-004 size split). Resolves, in priority order: empty area,
 *  fold-pill row (whole-row toggle, §1), boundary "Collapse N" pill (§2), the
 *  P63 PR badge in the forge column, then the plain commit row (MODEL index). */

import type { FoldSpan, GraphLayout } from '../ipc';
import type { EffectiveMetrics } from './metrics';
import type { GraphDisplayOptions } from './rightColumns';
import { computeRightColumns } from './rightColumns';
import { layoutForgeCell, rowForgeSignal } from './forgeBadges';
import { forgeHitAt } from './hitTest';
import type { HitRow } from './hitTest';
import { collapsePillHit } from './drawFold';
import { displayToModel } from './foldModel';
import type { FoldModel } from './foldModel';

export type GraphClickAction =
  | { kind: 'deselect' }
  | { kind: 'toggleSpan'; start: number }
  | { kind: 'openPr'; number: number }
  | { kind: 'select'; modelRow: number }
  | { kind: 'none' };

export function resolveGraphClick(args: {
  hit: HitRow;
  x: number;
  layout: GraphLayout; // DISPLAY-space layout
  foldModel: FoldModel | null;
  foldRows: ReadonlyMap<number, FoldSpan> | null;
  boundaryRows: ReadonlyMap<number, FoldSpan> | null;
  ctx: CanvasRenderingContext2D | null;
  m: EffectiveMetrics;
  display: GraphDisplayOptions;
  effectiveWidth: number;
  prBadgesActive: boolean;
}): GraphClickAction {
  const { hit, x, layout, foldModel, foldRows, boundaryRows, ctx, m, display } = args;
  if (hit === null || hit === 'wip') return { kind: 'deselect' };
  // Spec-004: a fold-pill row's whole width toggles its span; a boundary row
  // toggles only on its collapse pill and selects otherwise.
  const span = foldRows?.get(hit);
  if (span !== undefined) return { kind: 'toggleSpan', start: span.start };
  const boundary = boundaryRows?.get(hit);
  if (boundary !== undefined && ctx !== null && collapsePillHit(ctx, boundary, m, x)) {
    return { kind: 'toggleSpan', start: boundary.start };
  }
  // PR-badge-placement §6: a PR pill in the FORGE column → open that PR (do
  // NOT select the row). Same pure helpers as the draw pass, so the pill rect
  // matches the pixels exactly.
  const node = layout.nodes[hit];
  if (args.prBadgesActive && node.refs !== undefined && node.refs.length > 0 && ctx !== null) {
    const cols = computeRightColumns(args.effectiveWidth, display, m);
    if (cols.forge !== null && x >= cols.forge.leftX && x <= cols.forge.rightX) {
      const signal = rowForgeSignal(node.refs, node, display);
      if (signal !== null) {
        const cell = layoutForgeCell(ctx, cols.forge.leftX, signal);
        const forgeHit = forgeHitAt(cell, x, m.ciBadgeSize);
        if (forgeHit !== null && forgeHit.kind === 'pr') {
          return { kind: 'openPr', number: forgeHit.pr.badge.number };
        }
      }
    }
  }
  // Selection is MODEL-keyed: map the display hit back (never a pill here).
  if (foldModel !== null) {
    const d = displayToModel(foldModel, hit);
    return d.kind === 'commit' ? { kind: 'select', modelRow: d.row } : { kind: 'none' };
  }
  return { kind: 'select', modelRow: hit };
}
