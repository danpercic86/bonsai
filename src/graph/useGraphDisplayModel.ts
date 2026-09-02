/** Spec-004 display-space derivation for `GraphCanvas` — the fold projection,
 *  the edge-culling index choice, the display-space selection and the search-
 *  match set. Moved verbatim out of `GraphCanvas.tsx` (file-size ratchet): this
 *  is render-phase memo work only, never touched by the per-frame paint path
 *  (which reads the results through `propsRef`).
 *
 *  Returns the derived values under their ORIGINAL identifier names so the
 *  container destructures them and every downstream line is unchanged. */
import { useMemo } from 'react';
import { buildEdgeIndex } from './edgeIndex';
import { projectLayout } from './foldProject';
import { pillRowOfStart } from './foldModel';
import { displaySelection, mapMatchRows } from './foldView';
import type { GraphCanvasProps } from './graphCanvasProps';

/** Exactly the `GraphCanvasProps` subset the derivation reads. */
type DisplayModelInput = Pick<
  GraphCanvasProps,
  'edgeIndex' | 'fold' | 'layout' | 'matchRows' | 'selectedIndex'
>;

export function useGraphDisplayModel({
  edgeIndex,
  fold,
  layout,
  matchRows,
  selectedIndex,
}: DisplayModelInput) {
  // Spec-004: display-space projection. Identity (the input layout object)
  // whenever fold is inactive or nothing is collapsed. Spans only exist after
  // the stream's `done`, so this never runs per streamed batch in anger. Keyed
  // on model/expandedSpans (NOT the whole `fold` bundle) so an active-pill
  // change never re-projects 20k rows.
  const foldModel = fold !== undefined ? fold.model : null;
  const foldExpanded = fold?.expandedSpans;
  const projected = useMemo(
    () =>
      foldModel !== null && foldExpanded !== undefined
        ? projectLayout(layout, foldModel, foldExpanded)
        : null,
    [layout, foldModel, foldExpanded],
  );
  const dLayout = projected !== null ? projected.layout : layout;
  const foldRows = projected !== null && !projected.identity ? projected.foldRows : null;
  const boundaryRows =
    projected !== null && projected.boundaryRows.size > 0 ? projected.boundaryRows : null;
  const foldRowSet = useMemo(
    () => (foldRows !== null ? new Set(foldRows.keys()) : null),
    [foldRows],
  );
  // Spec-004 §3: the keyboard-active pill row (display index), derived from the
  // stable span `start` so expansion remaps never leave a dangling row.
  const activeRow =
    fold !== undefined && fold.activePillStart !== null
      ? pillRowOfStart(fold.model, fold.activePillStart)
      : null;
  // Selection in display space; a HIDDEN selection moves its ring to the pill.
  const dSel = displaySelection(foldModel, selectedIndex);

  // Edge culling index, built once per layout object (§4.4). P65b: on the
  // streamed path the assembler supplies `edgeIndex` (its own incremental index),
  // so we skip the internal build entirely — otherwise it would be an O(n)
  // rebuild on every streamed batch (layout identity bumps per batch).
  // Spec-004: a non-identity projection owns its own display-space edge array,
  // so it always builds a one-shot index (streamed or not).
  const memoIndex = useMemo(() => {
    if (projected !== null && !projected.identity) return buildEdgeIndex(projected.layout);
    return edgeIndex !== undefined ? null : buildEdgeIndex(layout);
  }, [layout, edgeIndex, projected]);
  const incIndex = projected !== null && !projected.identity ? undefined : edgeIndex;

  // P50b: search-match set, rebuilt once per matchRows prop change (not per
  // frame). null when there are no matches so the draw pass skips the ring.
  // Spec-004: model rows mapped to display rows (hidden matches dropped).
  const matchSet = useMemo(() => {
    const base = matchRows !== undefined && matchRows.length > 0 ? new Set(matchRows) : null;
    return mapMatchRows(foldModel, base);
  }, [matchRows, foldModel]);

  return {
    activeRow,
    boundaryRows,
    dLayout,
    dSel,
    foldModel,
    foldRows,
    foldRowSet,
    incIndex,
    matchSet,
    memoIndex,
    projected,
  };
}
